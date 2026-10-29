//! Developer-only preload probe. Does not authenticate or start Minecraft.
use rusted_launcher::{
    install,
    model::{Engine, Store},
    Event, Task,
};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, mpsc, Arc},
};
fn main() {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    assert!(
        a.len() >= 2,
        "DATA_DIR JAVA_PATH [managed-java|fabric|native RUNTIME_DIR]"
    );
    let store = Store::open(PathBuf::from(&a[0])).unwrap();
    let mut instance = store.list().unwrap().into_iter().next().unwrap_or_else(|| {
        store
            .create("Preload fixture", Engine::Java, "26.3")
            .unwrap()
    });
    instance.java_path = a[1].clone();
    if a.get(2).is_some_and(|s| s == "fabric") {
        instance.engine = Engine::Fabric;
    }
    if a.get(2).is_some_and(|s| s == "native") {
        instance.engine = Engine::Native;
    }
    store.save(&instance).unwrap();
    let (tx, rx) = mpsc::channel();
    let task = Task {
        events: tx,
        cancel: Arc::new(AtomicBool::new(false)),
    };
    let reader = std::thread::spawn(move || {
        while let Ok(e) = rx.recv() {
            match e {
                Event::Status(s) => println!("STAGE {s}"),
                Event::Progress { done, total, .. } if done == total => {
                    println!("VERIFIED {total} bytes")
                }
                _ => {}
            }
        }
    });
    let result = if a.get(2).is_some_and(|s| s == "managed-java") {
        instance.java_path.clear();
        install::java(&store, &instance, &task).map(|p| p.to_string_lossy().into_owned())
    } else {
        install::prepare(
            &store,
            &instance,
            &a.get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("UNUSED_IN_JAVA_PRELOAD")),
            &task,
        )
        .map(|r| {
            format!(
                "{} classpath entries; assets={}",
                r.classpath.len(),
                r.assets.display()
            )
        })
    };
    drop(task);
    reader.join().unwrap();
    match result {
        Ok(s) => println!("PRELOAD_OK: {s}"),
        Err(e) => {
            eprintln!("PRELOAD_ERROR: {e}");
            std::process::exit(1)
        }
    }
}
