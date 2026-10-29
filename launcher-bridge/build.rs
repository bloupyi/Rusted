fn main() {
    use std::{env, fs, path::PathBuf};
    println!("cargo:rerun-if-env-changed=RUSTED_INSTALL_PAYLOAD");
    println!("cargo:rerun-if-changed=../artwork/rusted-icon.ico");
    let contents = if let Some(path) = env::var_os("RUSTED_INSTALL_PAYLOAD") {
        let path = fs::canonicalize(path).expect("installer payload must exist");
        println!("cargo:rerun-if-changed={}", path.display());
        format!(
            "static INSTALL_PAYLOAD: &[u8] = include_bytes!({:?});",
            path
        )
    } else {
        "static INSTALL_PAYLOAD: &[u8] = &[];".into()
    };
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("installer_payload.rs"),
        contents,
    )
    .unwrap();
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../artwork/rusted-icon.ico");
        resource
            .set("ProductName", "Rusted")
            .set("FileDescription", "Rusted launcher integration");
        resource.compile().expect("embed Rusted icon");
    }
}
