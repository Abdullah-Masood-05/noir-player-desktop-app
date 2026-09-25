fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Noir_Player_Logo.png");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let png = std::path::Path::new("Noir_Player_Logo.png");
        let ico_path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("noir.ico");
        let ico = generate_ico(png).expect("Failed to generate ICO from PNG");
        std::fs::write(&ico_path, ico).expect("Failed to write ICO");
        winresource::WindowsResource::new()
            .set_icon_with_id(ico_path.to_str().unwrap(), "1")
            .set("ProductName", "Noir Player")
            .set("FileDescription", "Noir Player")
            .set("InternalName", "Noir Player")
            .set("OriginalFilename", "Noir Player.exe")
            .compile()
            .expect("Failed to compile Noir Player Windows icon resource");
    }
}

fn generate_ico(png: &std::path::Path) -> image::ImageResult<Vec<u8>> {
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    use image::imageops::FilterType;
    use image::ExtendedColorType;

    let source = image::open(png)?.to_rgba8();
    let frames = [16u32, 24, 32, 48, 64, 128, 256]
        .into_iter()
        .map(|n| {
            let resized = image::imageops::resize(&source, n, n, FilterType::Lanczos3);
            IcoFrame::as_png(resized.as_raw(), n, n, ExtendedColorType::Rgba8)
        })
        .collect::<image::ImageResult<Vec<_>>>()?;
    let mut ico = Vec::new();
    IcoEncoder::new(&mut ico).encode_images(&frames)?;
    Ok(ico)
}
