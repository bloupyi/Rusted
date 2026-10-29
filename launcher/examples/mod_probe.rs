//! Public Modrinth API/binaries only; never reads mod sources or starts Minecraft.
use rusted_launcher::{
    model::{Engine, Store},
    mods, Task,
};
use std::sync::{atomic::AtomicBool, mpsc, Arc};
fn main() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(root.path().join("owned")).unwrap();
    let mut instance = store
        .create("Fabric download probe", Engine::Fabric, "26.3")
        .unwrap();
    let (tx, _rx) = mpsc::channel();
    let task = Task {
        events: tx,
        cancel: Arc::new(AtomicBool::new(false)),
    };
    for (query, project) in [
        ("Sodium", "AANobbMI"),
        ("Iris", "YL57xq9U"),
        ("Distant Horizons", "uCdwusMi"),
    ] {
        let hits = mods::search(query, "26.3").unwrap();
        assert!(
            hits.iter().any(|h| h.id == project),
            "Expected project absent: {query}"
        );
        mods::install(&store, &mut instance, project, &task).unwrap();
        println!("VERIFIED_MOD_AND_REQUIRED_DEPENDENCIES: {query}");
    }
    assert_eq!(instance.engine, Engine::Fabric);
    println!(
        "FABRIC_MOD_INSTALLATION_OK: {} jar files; gameplay not tested",
        mods::files(&store, &instance).unwrap().len()
    );
}
