//! Developer validation of import only; no game, Java process or account is started.
use rusted_launch::{
    launcher_install::{self, LauncherFiles},
    preparation::Preparation,
};
use std::path::PathBuf;
fn main() {
    let mut args = std::env::args_os().skip(1);
    let input = PathBuf::from(args.next().expect("input JSON required"));
    let root = PathBuf::from(args.next().expect("fresh preparation root required"));
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
    let files = LauncherFiles {
        jars: value["jars"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| PathBuf::from(p.as_str().unwrap()))
            .collect(),
        assets: PathBuf::from(value["assets"].as_str().unwrap()),
    };
    let preparation = Preparation {
        root,
        executable: PathBuf::from("unused-import-only"),
        server_jar: None,
        minecraft_directory: None,
        download_server: false,
        import_launcher: true,
    };
    match launcher_install::import(&preparation, &files) {
        Ok(root) => println!("Import verified: {}", root.display()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
