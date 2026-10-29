//! Verify all catalogue metadata and optionally preload selected versions; no account/game.
use rusted_launcher::{
    catalog, install,
    model::{Engine, Store},
    Event, Task,
};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, mpsc, Arc},
};
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    assert!(
        args.len() >= 2,
        "DATA_DIR JAVA_PATH [VERSIONS_TO_PRELOAD...]"
    );
    let store = Store::open(PathBuf::from(&args[0])).unwrap();
    let catalog = catalog::refresh(&store).unwrap();
    for entry in &catalog.versions {
        let m = install::metadata(&store, &entry.id).unwrap();
        catalog::validate_metadata(&entry.id, &m).unwrap();
        assert!(m["javaVersion"]["majorVersion"].as_u64().unwrap() >= 25);
    }
    println!(
        "OFFICIAL_METADATA_OK: {} versions, {} releases",
        catalog.versions.len(),
        catalog.ids(false).len()
    );
    let (tx, rx) = mpsc::channel();
    let task = Task {
        events: tx,
        cancel: Arc::new(AtomicBool::new(false)),
    };
    let reader = std::thread::spawn(move || {
        while let Ok(e) = rx.recv() {
            if let Event::Status(ref s) = e {
                println!("STAGE {s}");
            }
            if matches!(e, Event::GameStarted) {
                panic!("Preload unexpectedly started a game");
            }
        }
    });
    for version in &args[2..] {
        let mut instance = store
            .create(&format!("Preload {version}"), Engine::Java, version)
            .unwrap();
        instance.java_path = args[1].clone();
        store.save(&instance).unwrap();
        let ready = install::prepare(
            &store,
            &instance,
            &PathBuf::from("UNUSED_JAVA_PRELOAD"),
            &task,
        )
        .unwrap();
        println!(
            "PRELOAD_OK: {version}, {} classpath entries",
            ready.classpath.len()
        );
    }
    drop(task);
    reader.join().unwrap();
}
