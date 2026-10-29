//! Developer round trip with full Rusted packs; no game, account or player's folder.
use rusted_launcher::{
    model::{Engine, Store},
    packs, Task,
};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, mpsc, Arc},
};
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    assert_eq!(args.len(), 3, "PRISM_ZIP MRPACK RUNTIME");
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(root.path().join("owned")).unwrap();
    let runtime = PathBuf::from(&args[2]);
    let (tx, _rx) = mpsc::channel();
    let task = Task {
        events: tx,
        cancel: Arc::new(AtomicBool::new(false)),
    };
    for pack in &args[..2] {
        let instance = packs::import(&store, &PathBuf::from(pack), &runtime, &task).unwrap();
        assert_eq!(instance.engine, Engine::Native);
        assert!(!store
            .game(&instance)
            .unwrap()
            .join("mods/rusted-bootstrap.jar")
            .exists());
        let export = root.path().join(format!("export-{}.zip", instance.id));
        packs::export_prism(&store, &instance, &runtime, &export, &task).unwrap();
        let reimport = packs::import(&store, &export, &runtime, &task).unwrap();
        assert_eq!(reimport.engine, Engine::Native);
        assert!(store
            .game(&reimport)
            .unwrap()
            .join("rusted/native-data/shaderpacks/Rusted-Soft-Light.hshader")
            .is_file());
        assert!(!store
            .game(&reimport)
            .unwrap()
            .join("mods/rusted-bootstrap.jar")
            .exists());
    }
    println!("FULL_PRISM_MRPACK_ROUND_TRIP_OK");
}
