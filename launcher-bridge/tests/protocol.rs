use rusted_launch::{game_args, read_prism, Config, Launch};
use std::{fs, io::Cursor};

fn params(dir: &std::path::Path) -> Vec<String> {
    [
        "--username",
        "BridgeFixture",
        "--uuid",
        "123456781234123412341234567890ab",
        "--accessToken",
        "TEST_ONLY_NOT_A_REAL_SESSION_TOKEN",
        "--userType",
        "msa",
        "--version",
        "26.3",
        "--gameDir",
        dir.to_str().unwrap(),
    ]
    .map(str::to_owned)
    .to_vec()
}

#[test]
fn modrinth_extracts_game_args_without_jvm_flags() {
    let dir = tempfile::tempdir().unwrap();
    let mut raw = vec![
        "C:/Program Files/Java/java.exe".into(),
        "-cp".into(),
        "library paths".into(),
        "net.minecraft.client.main.Main".into(),
    ];
    raw.extend(params(dir.path()));
    let launch = Launch::from_game_args(game_args(&raw).unwrap(), dir.path()).unwrap();
    assert_eq!(launch.game_directory, fs::canonicalize(dir.path()).unwrap());
    assert!(launch.native_args.is_empty());
    assert!(game_args(&["net.fabricmc.loader.impl.launch.knot.KnotClient".into()]).is_err());
}

#[test]
fn prism_preserves_spaces_waits_for_launch_and_honors_abort() {
    let dir = tempfile::Builder::new()
        .prefix("path with spaces ")
        .tempdir()
        .unwrap();
    let mut text = "mainClass net.minecraft.client.main.Main\r\nlauncher modern\r\nwindowParams 1600x900\r\nserverAddress localhost\r\nserverPort 25565\r\n".to_owned();
    for param in params(dir.path()) {
        text.push_str(&format!("param {param}\r\n"));
    }
    let incomplete = text.clone();
    text.push_str("launch\r\nignored input after launch\n");
    let args = read_prism(&mut Cursor::new(text)).unwrap().unwrap();
    let launch = Launch::from_game_args(&args, dir.path()).unwrap();
    assert_eq!(
        launch.native_args,
        [
            "--width",
            "1600",
            "--height",
            "900",
            "--server",
            "localhost:25565"
        ]
    );
    assert!(read_prism(&mut Cursor::new(incomplete)).is_err());
    assert!(read_prism(&mut Cursor::new("abort\n")).unwrap().is_none());
    assert!(read_prism(&mut Cursor::new("launcher legacy\nlaunch\n")).is_err());
    assert!(read_prism(&mut Cursor::new(format!("param {}\n", "a".repeat(65537)))).is_err());
}

#[test]
fn prism_standard_26_3_without_user_type_preserves_account_checks() {
    let dir = tempfile::tempdir().unwrap();
    let mut parameters = params(dir.path());
    let position = parameters.iter().position(|s| s == "--userType").unwrap();
    parameters.drain(position..position + 2);
    let script = format!(
        "mainClass net.minecraft.client.main.Main\nlauncher standard\n{}launch\n",
        parameters
            .iter()
            .map(|p| format!("param {p}\n"))
            .collect::<String>()
    );
    let parsed = read_prism(&mut Cursor::new(script)).unwrap().unwrap();
    let launch = Launch::from_game_args(&parsed, dir.path()).unwrap();
    assert_eq!(launch.name, "BridgeFixture");
    assert_eq!(launch.access_token, "TEST_ONLY_NOT_A_REAL_SESSION_TOKEN");
    for (flag, value) in [
        ("--accessToken", "0"),
        ("--uuid", "00000000000000000000000000000000"),
        ("--version", "1.20.4"),
    ] {
        let mut invalid = parameters.clone();
        let index = invalid.iter().position(|s| s == flag).unwrap();
        invalid[index + 1] = value.into();
        assert!(Launch::from_game_args(&invalid, dir.path()).is_err());
    }
    assert!(read_prism(&mut Cursor::new("launcher legacy\nlaunch\n")).is_err());
}

#[test]
fn wrapper_commands_use_forward_slashes_and_handle_extended_windows_paths() {
    for (input, output) in [
        (
            r"C:\Users\Jean Dupont\rusted-launch.exe",
            "C:/Users/Jean Dupont/rusted-launch.exe",
        ),
        (
            r"\\?\C:\Users\O'Neil\rusted-launch.exe",
            "C:/Users/O'Neil/rusted-launch.exe",
        ),
        (
            r"\\?\UNC\server\share\Rusted Native\rusted-launch.exe",
            "//server/share/Rusted Native/rusted-launch.exe",
        ),
    ] {
        assert_eq!(
            rusted_launch::wrapper_command(std::path::Path::new(input)).unwrap(),
            format!("\"{output}\" --")
        );
    }
    assert!(rusted_launch::wrapper_command(std::path::Path::new("bad\"path")).is_err());
}

#[test]
fn rejects_offline_demo_duplicate_credentials_and_java_world_quick_play() {
    let dir = tempfile::tempdir().unwrap();
    for (key, value) in [
        ("--userType", "legacy"),
        ("--uuid", "00000000000000000000000000000000"),
        ("--accessToken", "0"),
        ("--username", "invalid name"),
    ] {
        let mut args = params(dir.path());
        let index = args.iter().position(|a| a == key).unwrap();
        args[index + 1] = value.into();
        assert!(Launch::from_game_args(&args, dir.path()).is_err());
    }
    for extra in [
        vec!["--demo"],
        vec!["--accessToken", "SECOND_TEST_CREDENTIAL"],
        vec!["--quickPlaySingleplayer", "My personal world"],
        vec!["--quickPlayRealms", "123"],
        vec!["--width", "0"],
    ] {
        let mut args = params(dir.path());
        args.extend(extra.into_iter().map(str::to_owned));
        assert!(Launch::from_game_args(&args, dir.path()).is_err());
    }
}

#[test]
fn never_silently_ignores_java_mods_or_a_different_game_version() {
    let dir = tempfile::tempdir().unwrap();
    let args = params(dir.path());
    let mut launch = Launch::from_game_args(&args, dir.path()).unwrap();
    launch.validate_native().unwrap();
    fs::create_dir(dir.path().join("mods")).unwrap();
    fs::write(dir.path().join("mods/sodium.JAR"), []).unwrap();
    assert!(launch.validate_native().is_err());
    fs::remove_file(dir.path().join("mods/sodium.JAR")).unwrap();
    launch.version = "1.20.4".into();
    assert!(launch.validate_native().is_err());
}

#[test]
fn config_paths_are_relative_to_config_not_launcher_working_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let mut value = serde_json::json!({"schema":1,"mode":"native","horizon_revision":rusted_launch::HORIZON_REVISION,"minecraft_version":"26.3","executable":"native/client.exe","resources":"resources","data_directory":"data"});
    fs::write(&path, value.to_string()).unwrap();
    let config = Config::load(&path).unwrap();
    assert_eq!(
        config.executable,
        fs::canonicalize(dir.path())
            .unwrap()
            .join("native/client.exe")
    );
    assert!(config.check_native().is_err());
    fs::create_dir_all(dir.path().join("native")).unwrap();
    fs::write(&config.executable, []).unwrap();
    fs::create_dir_all(&config.resources).unwrap();
    fs::write(
        config.resources.join("horizon-resources.json"),
        r#"{"minecraft":"1.20.4"}"#,
    )
    .unwrap();
    assert!(config.check_native().is_err());
    fs::write(
        config.resources.join("horizon-resources.json"),
        r#"{"minecraft":"26.3"}"#,
    )
    .unwrap();
    config.check_native().unwrap();
    value["unknown"] = true.into();
    fs::write(&path, value.to_string()).unwrap();
    assert!(Config::load(&path).is_err());
}
