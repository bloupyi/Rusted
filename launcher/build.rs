fn main() {
    println!("cargo:rerun-if-changed=../artwork/rusted-icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../artwork/rusted-icon.ico");
        resource
            .set("ProductName", "Rusted Launcher")
            .set("FileDescription", "Rusted Launcher");
        resource.compile().expect("embed Rusted Launcher icon");
    }
}
