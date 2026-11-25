fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        resource.set("ProductName", "ProtoGrid");
        resource.set("FileDescription", "ProtoGrid — native proxy client");
        resource.set("LegalCopyright", "MIT");
        if let Err(error) = resource.compile() {
            eprintln!("не удалось встроить иконку Windows: {error}");
        }
    }
}
