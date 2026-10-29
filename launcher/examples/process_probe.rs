//! Developer regression: cancellation kills descendants and log output redacts the fixture token.
use rusted_launcher::{launch, Event, Task};
use std::{
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
fn main() {
    let fixture = std::env::args().nth(1).unwrap();
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let task = Task {
        events: tx,
        cancel: cancel.clone(),
    };
    let (done, result) = mpsc::channel();
    std::thread::spawn(move || {
        let r = launch::run(
            Command::new(fixture),
            &task,
            &["TEST_ONLY_FAKE_SESSION_VALUE"],
        );
        done.send(r).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut leaf = false;
    let mut redacted = false;
    while Instant::now() < deadline {
        if let Ok(Event::Log(line)) = rx.recv_timeout(Duration::from_millis(500)) {
            assert!(!line.contains("TEST_ONLY_FAKE_SESSION_VALUE"));
            leaf |= line.starts_with("LEAF_PID=");
            redacted |= line == "SESSION=<session>";
            if leaf && redacted {
                break;
            }
        }
    }
    assert!(leaf && redacted);
    cancel.store(true, Ordering::Relaxed);
    assert!(result
        .recv_timeout(Duration::from_secs(10))
        .expect("process-tree cancellation hung")
        .is_ok());
    println!("OWN_PROCESS_TREE_AND_REDACTION_OK");
}
