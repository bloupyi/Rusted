//! Own process fixture, not a launcher or game. Accepts synthetic credentials only.
use std::{
    env, fs,
    io::Write,
    process::{Command, Stdio},
};
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--probe") {
        let pid: u32 = args[1].parse().unwrap();
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::*};
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            let mut status = 0;
            let alive =
                !handle.is_null() && GetExitCodeProcess(handle, &mut status) != 0 && status == 259;
            if !handle.is_null() {
                CloseHandle(handle);
            }
            println!("{}", if alive { "ALIVE" } else { "DEAD" });
        }
        #[cfg(target_os = "linux")]
        {
            let value = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
            let active = value
                .rsplit_once(") ")
                .is_some_and(|(_, s)| !s.starts_with('Z'));
            println!("{}", if active { "ALIVE" } else { "DEAD" });
        }
        return;
    }
    assert_eq!(args.len(), 2);
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&args[1]).unwrap()).unwrap();
    assert!(value["game_args"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s == "TEST_ONLY_NOT_A_REAL_SESSION_TOKEN"));
    value["parent_pid"] = std::process::id().into();
    let mut child = Command::new(&args[0])
        .arg("--bootstrap")
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&value).unwrap())
        .unwrap();
    std::process::exit(child.wait().unwrap().code().unwrap_or(1));
}
