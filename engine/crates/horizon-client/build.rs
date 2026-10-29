//! Rusted Windows icon and executable identity, including cross builds.
fn main() {
    println!("cargo:rerun-if-changed=assets/rusted-icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/rusted-icon.ico");
    resource
        .set("ProductName", "Rusted")
        .set("FileDescription", "Rusted native Minecraft client");
    resource.compile().expect("embed Rusted identity");
}
