//! Original Fabric resource fixture for native integration tests; no external mod code.
use rusted_launcher::{
    compat,
    model::{self, Engine, Store},
    Task,
};
use serde_json::json;
use std::{
    io::Write,
    path::PathBuf,
    sync::{atomic::AtomicBool, mpsc, Arc},
};
fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("OWNED_TEST_DIRECTORY"));
    let store = Store::open(root).unwrap();
    let instance = store
        .create("Native resource fixture", Engine::Native, "26.3")
        .unwrap();
    let source = store.root.join("own-resource-fixture.jar");
    let mut archive = zip::ZipWriter::new(std::fs::File::create(&source).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in [
        (
            "fabric.mod.json",
            json!({"schemaVersion":1,"id":"rusted_fixture","name":"Rusted original resource fixture","depends":{"minecraft":"26.3"}}),
        ),
        (
            "assets/minecraft/lang/en_us.json",
            json!({"menu.singleplayer":"Rusted native resource proof"}),
        ),
    ] {
        archive.start_file(name, options).unwrap();
        archive
            .write_all(&serde_json::to_vec(&body).unwrap())
            .unwrap();
    }
    archive.finish().unwrap();
    let (tx, _rx) = mpsc::channel();
    let task = Task {
        events: tx,
        cancel: Arc::new(AtomicBool::new(false)),
    };
    let pack = compat::import_resources(&store, &instance, &source, &task).unwrap();
    let home = store.game(&instance).unwrap().join("rusted/native-data");
    std::fs::write(
        home.join("options.txt"),
        format!(
            "lang:en_us\nresourcePacks:[\"vanilla\",\"file/{}\"]\n",
            pack.file_name().unwrap().to_string_lossy()
        ),
    )
    .unwrap();
    let result = json!({"home":home,"pack":pack,"executes_java":false});
    model::atomic_json(&store.root.join("fixture.json"), &result).unwrap();
    println!("{result}");
}
