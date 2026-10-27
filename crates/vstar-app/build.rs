//! Puts the FeStar icon into the Windows program file, so Explorer and the
//! taskbar show it.

#[path = "src/app_icon_render.rs"]
mod render;

fn main() {
    println!("cargo:rerun-if-changed=src/app_icon_render.rs");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    embed_icon();
}

fn embed_icon() {
    use image::ExtendedColorType;
    use image::codecs::ico::{IcoEncoder, IcoFrame};

    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("festar.ico");
    let frames: Vec<IcoFrame> = [16u32, 24, 32, 48, 64, 128, 256]
        .iter()
        .map(|&size| {
            let rgba = render::render(size);
            IcoFrame::as_png(&rgba, size, size, ExtendedColorType::Rgba8)
                .expect("encode an icon size")
        })
        .collect();
    let file = std::fs::File::create(&out).expect("create the icon file");
    IcoEncoder::new(file)
        .encode_images(&frames)
        .expect("write the icon file");

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(out.to_str().expect("icon path"));
        res.set("ProductName", "FeStar");
        res.set("FileDescription", "FeStar light curve analysis");
        res.compile().expect("embed the Windows icon");
    }
}
