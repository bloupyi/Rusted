use rusted_launcher::{
    compat,
    model::{Engine, Store},
    Task,
};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    sync::{atomic::AtomicBool, mpsc, Arc},
};
fn task() -> Task {
    let (tx, _) = mpsc::channel();
    Task {
        events: tx,
        cancel: Arc::new(AtomicBool::new(false)),
    }
}
fn fixture(path: &Path, extra: Option<(&str, &[u8])>, mixin: bool) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("fabric.mod.json", options).unwrap();
    let meta = if mixin {
        json!({"schemaVersion":1,"id":"own_resource_fixture","mixins":["hook.json"]})
    } else {
        json!({"schemaVersion":1,"id":"own_resource_fixture","name":"Own fixture","depends":{"minecraft":"26.3"}})
    };
    zip.write_all(&serde_json::to_vec(&meta).unwrap()).unwrap();
    zip.start_file("assets/minecraft/lang/fr_fr.json", options)
        .unwrap();
    zip.write_all(br#"{"menu.singleplayer":"Solo Rusted test"}"#)
        .unwrap();
    if let Some((name, body)) = extra {
        zip.start_file(name, options).unwrap();
        zip.write_all(body).unwrap();
    }
    zip.finish().unwrap();
}
#[test]
fn resource_only_fabric_archive_becomes_native_pack_without_jvm_or_source_mutation() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(root.path().join("owned")).unwrap();
    let instance = store.create("Native", Engine::Native, "26.3").unwrap();
    let source = root.path().join("own.jar");
    fixture(&source, None, false);
    let original = fs::read(&source).unwrap();
    let output = compat::import_resources(&store, &instance, &source, &task()).unwrap();
    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(store.list().unwrap()[0].engine, Engine::Native);
    assert!(!store.game(&instance).unwrap().join("mods").exists());
    let mut zip = zip::ZipArchive::new(fs::File::open(&output).unwrap()).unwrap();
    assert_eq!(zip.len(), 2);
    let mut body = String::new();
    zip.by_name("assets/minecraft/lang/fr_fr.json")
        .unwrap()
        .read_to_string(&mut body)
        .unwrap();
    assert!(body.contains("Solo Rusted test"));
    assert!(zip.by_name("fabric.mod.json").is_err());
    assert!(output.with_extension("rusted.json").is_file());
    assert!(compat::import_resources(&store, &instance, &source, &task()).is_err());
}
#[test]
fn executable_gameplay_or_traversal_mods_are_rejected_before_any_native_install() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(root.path().join("owned")).unwrap();
    let instance = store.create("Native", Engine::Native, "26.3").unwrap();
    for (index, extra, mixin) in [
        (
            0,
            Some(("example/Mod.class", b"NOT_JAVA".as_slice())),
            false,
        ),
        (1, Some(("../escape", b"ESCAPE".as_slice())), false),
        (2, None, true),
        (
            3,
            Some(("data/example/recipe.json", b"{}".as_slice())),
            false,
        ),
    ] {
        let source = root.path().join(format!("{index}.jar"));
        fixture(&source, extra, mixin);
        assert!(compat::import_resources(&store, &instance, &source, &task()).is_err());
        assert!(!store
            .game(&instance)
            .unwrap()
            .join("rusted/native-data/resourcepacks")
            .exists());
    }
    assert!(!root.path().join("escape").exists());
}
