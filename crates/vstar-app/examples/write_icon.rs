//! Writes the FeStar icon as a PNG, for the Linux download's desktop entry:
//! `cargo run -p vstar-app --example write_icon -- festar.png`

#[path = "../src/app_icon_render.rs"]
mod render;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "festar.png".into());
    let size = 256;
    image::save_buffer(
        &path,
        &render::render(size),
        size,
        size,
        image::ColorType::Rgba8,
    )
    .expect("write the icon");
}
