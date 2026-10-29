//! Test peer only: exercises process/session handover without Minecraft or a real account.
use std::{env, fs, path::PathBuf};

fn main() {
    if let Some(path) = env::var_os("HORIZON_SESSION_FILE") {
        let path = PathBuf::from(path);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
        }
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        fs::remove_file(path).unwrap();
        // Only the fixture marker is accepted; never call any authentication service.
        assert_eq!(value["access_token"], "TEST_ONLY_NOT_A_REAL_SESSION_TOKEN");
        assert_eq!(value["user_type"], "msa");
        assert_eq!(value["name"], "BridgeFixture");
        assert!(env::var_os("INST_JAVA_ARGS").is_none());
        assert!(!env::args().any(|arg| arg.contains("TEST_ONLY") || arg == "--accessToken"));
        assert_eq!(
            fs::canonicalize(env::current_dir().unwrap()).unwrap(),
            fs::canonicalize(env::var_os("HORIZON_HOME").unwrap()).unwrap()
        );
        println!("TEST_PEER_NATIVE_OK");
        if env::var("RUSTED_TEST_NATIVE_HOLD").as_deref() == Ok("FIXTURE_ONLY") {
            println!("TEST_PEER_NATIVE_PID={}", std::process::id());
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            loop {
                std::thread::park();
            }
        }
    } else if let Ok(loader) = env::var("RUSTED_TEST_JAVA_LOADER") {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text).unwrap();
        let main = "net.fabricmc.loader.impl.launch.knot.KnotClient";
        if loader == "PRISM_FIXTURE_ONLY" {
            assert_eq!(
                env::args().skip(1).collect::<Vec<_>>(),
                ["org.prismlauncher.EntryPoint"]
            );
            assert_eq!(
                text,
                format!("mainClass {main}\nlauncher standard\nlaunch\n")
            );
        } else {
            assert_eq!(loader, "FABRIC_FIXTURE_ONLY");
            assert_eq!(env::args().skip(1).collect::<Vec<_>>(), [main]);
            assert_eq!(text, "JAVA_STDIN_UNCHANGED\n");
        }
        assert!(env::var_os("HORIZON_SESSION_FILE").is_none());
        println!("TEST_PEER_JAVA_LOADER_OK");
    } else {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text).unwrap();
        assert_eq!(text, "JAVA_STDIN_UNCHANGED\n");
        assert_eq!(
            env::args().skip(1).collect::<Vec<_>>(),
            ["-Xmx2G", "argument with spaces"]
        );
        println!("TEST_PEER_JAVA_OK");
    }
    std::process::exit(37);
}
