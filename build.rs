fn main() {
    println!("cargo:rerun-if-changed=assets/caps.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let icon_path = std::path::Path::new("assets/caps.ico")
        .canonicalize()
        .expect("failed to locate CAPS Windows icon");
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon(&icon_path.to_string_lossy())
        .set("CompanyName", "ref42")
        .set("FileDescription", "CAPS")
        .set("InternalName", "caps.exe")
        .set("LegalCopyright", "ref42")
        .set("OriginalFilename", "caps.exe")
        .set("ProductName", "CAPS");
    resource
        .compile()
        .expect("failed to embed CAPS Windows resources");
}
