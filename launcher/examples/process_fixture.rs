//! Only original test processes; never a game, real token or authentication endpoint.
use std::{env, io::Write, process::Command};
fn main() {
    if env::args().any(|s| s == "--leaf") {
        println!("LEAF_PID={}", std::process::id());
        std::io::stdout().flush().unwrap();
        loop {
            std::thread::park();
        }
    }
    let executable = env::current_exe().unwrap();
    // This fixture deliberately stays alive until its complete Windows job is killed.
    #[allow(clippy::zombie_processes)]
    let _child = Command::new(executable).arg("--leaf").spawn().unwrap();
    println!("SESSION=TEST_ONLY_FAKE_SESSION_VALUE");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}
