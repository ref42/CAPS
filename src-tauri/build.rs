#[cfg(windows)]
fn main() {
    println!("cargo:rerun-if-changed=../assets/caps.png");

    embedinator::ResourceBuilder::from_env()
        .add_string("CompanyName", "ref42")
        .add_string("FileDescription", "CAPS")
        .add_string("LegalCopyright", "ref42")
        .add_string("OriginalFilename", "caps.exe")
        .add_string("ProductName", "CAPS")
        .add_icon(32512, embedinator::Icon::from_png_bytes(app_icon_png()))
        .finish();
}

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn app_icon_png() -> Vec<u8> {
    let image = image::open("../assets/caps.png")
        .expect("failed to open CAPS app icon")
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("failed to encode CAPS app icon");
    bytes.into_inner()
}
