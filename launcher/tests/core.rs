use rusted_launcher::{
    download::{self, Algorithm, Artifact},
    launch,
    model::{self, Engine, Store},
    packs, Task,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    sync::{atomic::AtomicBool, mpsc, Arc},
};
fn task() -> Task {
    let (tx, _) = mpsc::channel();
    Task {
        events: tx,
        cancel: Arc::new(AtomicBool::new(false)),
    }
}

#[test]
fn microsoft_identity_is_publisher_configuration_not_player_settings() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("launcher-config.json");
    assert_eq!(
        rusted_launcher::auth::configured_client_id(directory.path()).unwrap(),
        rusted_launcher::auth::DEFAULT_CLIENT_ID
    );
    for client in [
        "",
        rusted_launcher::auth::DEFAULT_CLIENT_ID,
        "11111111-2222-4333-8444-555555555555",
    ] {
        model::atomic_json(
            &config,
            &serde_json::json!({"schema":1,"microsoft_client_id":client}),
        )
        .unwrap();
        assert_eq!(
            rusted_launcher::auth::configured_client_id(directory.path()).unwrap(),
            if client.is_empty() {
                rusted_launcher::auth::DEFAULT_CLIENT_ID
            } else {
                client
            }
        );
    }
    for client in [
        "00000000-0000-0000-0000-000000000000",
        "------------------------------------",
        "123",
        "111111112222-4333-8444-555555555555",
        "a-client-secret",
    ] {
        model::atomic_json(
            &config,
            &serde_json::json!({"schema":1,"microsoft_client_id":client}),
        )
        .unwrap();
        assert!(rusted_launcher::auth::configured_client_id(directory.path()).is_err());
    }
    let settings: model::Settings = serde_json::from_value(
        serde_json::json!({"microsoft_client_id":"legacy-player-override","selected":"fixture"}),
    )
    .unwrap();
    assert_eq!(settings.selected, "fixture");
    assert!(serde_json::to_value(settings)
        .unwrap()
        .get("microsoft_client_id")
        .is_none());
    fs::write(&config, vec![b' '; 65537]).unwrap();
    assert!(rusted_launcher::auth::configured_client_id(directory.path()).is_err());
}
#[test]
fn instance_copy_trash_and_settings_preserve_source_worlds() {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(d.path().join("owned")).unwrap();
    let mut i = s.create("Mon cuivre", Engine::Native, "26.3").unwrap();
    i.memory_mb = 6144;
    s.save(&i).unwrap();
    let world = s.game(&i).unwrap().join("saves/World");
    fs::create_dir_all(&world).unwrap();
    fs::write(world.join("marker"), b"SOURCE_WORLD").unwrap();
    let copy = s.duplicate(&i).unwrap();
    assert_eq!(
        fs::read(s.game(&copy).unwrap().join("saves/World/marker")).unwrap(),
        b"SOURCE_WORLD"
    );
    s.trash(&copy).unwrap();
    assert_eq!(s.list().unwrap().len(), 1);
    assert_eq!(fs::read(world.join("marker")).unwrap(), b"SOURCE_WORLD");
    assert_eq!(s.list().unwrap()[0].memory_mb, 6144);
    assert!(s.path("../outside").is_err());
}
#[test]
fn concurrent_play_lock_is_rejected_and_released() {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(d.path().into()).unwrap();
    let i = s.create("Owned", Engine::Java, "26.3").unwrap();
    let lock = launch::profile_lock(&s, &i).unwrap();
    assert!(launch::profile_lock(&s, &i).is_err());
    assert!(s.trash(&i).is_err());
    assert!(s.game(&i).unwrap().is_dir());
    drop(lock);
    assert!(launch::profile_lock(&s, &i).is_ok());
    s.trash(&i).unwrap();
    assert!(launch::profile_lock(&s, &i).is_err());
}
#[test]
fn cached_downloads_are_verified_without_network_and_cancel_keeps_files() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("asset");
    fs::write(&path, b"OWN_ASSET").unwrap();
    let a = Artifact {
        url: "https://example.invalid/never-contacted".into(),
        path: path.clone(),
        size: 9,
        hash: format!("{:x}", Sha256::digest(b"OWN_ASSET")),
        algorithm: Algorithm::Sha256,
    };
    assert!(download::valid(&a));
    download::queue(vec![a.clone()], 4, &task()).unwrap();
    let t = task();
    t.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(download::queue(vec![a.clone()], 4, &t).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"OWN_ASSET");
    fs::write(&path, b"CORRUPTED").unwrap();
    assert!(!download::valid(&a));
}
#[test]
fn extraction_validates_all_paths_before_writing_any_member() {
    let d = tempfile::tempdir().unwrap();
    let zip = d.path().join("bad.zip");
    let f = fs::File::create(&zip).unwrap();
    let mut z = zip::ZipWriter::new(f);
    for name in ["first.txt", "../escape.txt"] {
        z.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(b"fixture").unwrap();
    }
    z.finish().unwrap();
    let out = d.path().join("out");
    assert!(download::extract_zip(&zip, &out, 1000).is_err());
    assert!(!out.join("first.txt").exists());
    assert!(!d.path().join("escape.txt").exists());
    for name in [
        "/absolute",
        "C:/drive",
        "a/../b",
        "a/./b",
        "dir\\x",
        "CON.txt",
        "NUL",
        "COM1.jar",
        "dir/evil\nname",
    ] {
        assert!(download::path(d.path(), name).is_err(), "{name}");
    }
}
#[test]
fn java_classpath_proxy_handles_long_unicode_paths_without_credentials() {
    let d = tempfile::tempdir().unwrap();
    let paths = (0..200)
        .map(|i| {
            d.path().join(format!(
                "Jean Dupont Cuivre é #{i}/very-long-library-name-{i}.jar"
            ))
        })
        .collect::<Vec<_>>();
    let jar = launch::classpath_jar(&d.path().join("classpath.jar"), &paths).unwrap();
    let mut z = zip::ZipArchive::new(fs::File::open(jar).unwrap()).unwrap();
    let mut text = String::new();
    use std::io::Read;
    z.by_name("META-INF/MANIFEST.MF")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert!(text.contains("%20") && text.contains("%C3%A9"));
    assert!(text.lines().all(|l| l.len() <= 72));
    assert_eq!(text.replace("\r\n ", "").matches("file:").count(), 200);
}
#[test]
fn prism_import_creates_new_instance_and_removes_only_verified_own_bootstrap() {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(d.path().join("data")).unwrap();
    let runtime = d.path().join("runtime");
    fs::create_dir(&runtime).unwrap();
    model::atomic_json(
        &runtime.join("bootstrap.json"),
        &serde_json::json!({"jar_sha256":format!("{:x}",Sha256::digest(b"OWN_BOOTSTRAP"))}),
    )
    .unwrap();
    let file = d.path().join("pack.zip");
    let mut z = zip::ZipWriter::new(fs::File::create(&file).unwrap());
    for(name,body)in [("mmc-pack.json",br#"{"components":[{"uid":"net.minecraft","version":"26.3"},{"uid":"net.fabricmc.fabric-loader","version":"0.20.1"}]}"#.as_slice()),("minecraft/mods/rusted-bootstrap.jar",b"OWN_BOOTSTRAP".as_slice()),("minecraft/options.txt",b"owned:true".as_slice())]{z.start_file(name,zip::write::SimpleFileOptions::default()).unwrap();z.write_all(body).unwrap();}
    z.finish().unwrap();
    let i = packs::import(&s, &file, &runtime, &task()).unwrap();
    assert_eq!(i.engine, Engine::Fabric);
    assert_eq!(
        fs::read(s.game(&i).unwrap().join("options.txt")).unwrap(),
        b"owned:true"
    );
    assert!(!s
        .game(&i)
        .unwrap()
        .join("mods/rusted-bootstrap.jar")
        .exists());
}
