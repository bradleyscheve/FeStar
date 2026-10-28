//! The program's own icon: a glowing blue star, drawn in code so no image
//! file has to ship with the program.

#[path = "app_icon_render.rs"]
mod render;

pub fn icon(size: u32) -> egui::IconData {
    egui::IconData {
        rgba: render::render(size),
        width: size,
        height: size,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn icon_is_a_square_with_a_visible_star() {
        let icon = super::icon(64);
        assert_eq!(icon.rgba.len(), 64 * 64 * 4);
        let centre = (32 * 64 + 32) * 4;
        assert!(icon.rgba[centre + 3] > 200, "star centre is opaque");
        assert_eq!(icon.rgba[3], 0, "corners are transparent");
        if let Some(path) = std::env::var_os("ICON_PREVIEW") {
            let big = super::icon(256);
            image::save_buffer(path, &big.rgba, 256, 256, image::ColorType::Rgba8).unwrap();
        }
    }
}
