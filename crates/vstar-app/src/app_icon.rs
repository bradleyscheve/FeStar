//! The program's own icon: a glowing blue star, drawn in code so no image
//! file has to ship with the program.

/// A deterministic pseudo-random value in 0..1 for a pixel cell.
fn noise(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

/// Smoothly varying noise at a coarse scale, for the star's mottled surface.
fn blotch(x: f32, y: f32, scale: f32) -> f32 {
    let (fx, fy) = (x / scale, y / scale);
    let (ix, iy) = (fx.floor() as i32, fy.floor() as i32);
    let (tx, ty) = (fx - ix as f32, fy - iy as f32);
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * (t * t * (3.0 - 2.0 * t));
    let top = mix(noise(ix, iy), noise(ix + 1, iy), tx);
    let bottom = mix(noise(ix, iy + 1), noise(ix + 1, iy + 1), tx);
    mix(top, bottom, ty)
}

pub fn icon(size: u32) -> egui::IconData {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let s = size as f32;
    let centre = s / 2.0;
    let radius = s * 0.30;
    // A diagonal stream of gas passing behind the star.
    let dir = (0.32f32, 1.0f32);
    let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
    let dir = (dir.0 / len, dir.1 / len);

    for py in 0..size {
        for px in 0..size {
            let (x, y) = (px as f32 + 0.5 - centre, py as f32 + 0.5 - centre);
            let d = (x * x + y * y).sqrt();

            // Colour and opacity, built up from the back: stream, halo, star.
            let across = (x * dir.1 - y * dir.0).abs();
            let stream = ((1.0 - across / (s * 0.16)).clamp(0.0, 1.0)
                * (1.0 - (x * dir.0 + y * dir.1).abs() / (s * 0.5)).clamp(0.0, 1.0))
            .powf(1.2)
                * 0.9;
            let halo = (1.0 - ((d - radius) / (s * 0.17)).clamp(0.0, 1.0)).powf(2.0) * 0.8;
            let behind = stream.max(halo);
            let (color, alpha) = if d > radius {
                let h = halo / behind.max(1e-3);
                (
                    [
                        150.0 + (90.0 - 150.0) * h,
                        210.0 + (215.0 - 210.0) * h,
                        240.0 + (255.0 - 240.0) * h,
                    ],
                    behind,
                )
            } else {
                // The star itself: cyan, mottled, brighter toward the edge.
                let n = blotch(x, y, s * 0.045) * 0.6 + blotch(x, y, s * 0.015) * 0.4;
                let limb = (d / radius).powf(3.0);
                let t = (n * 0.95 + limb * 0.2).clamp(0.0, 1.0);
                (
                    [
                        20.0 + 215.0 * t.powf(2.5),
                        120.0 + 130.0 * t,
                        190.0 + 65.0 * t,
                    ],
                    // Soft edge.
                    (1.0 - ((d - radius + 1.5) / 1.5).clamp(0.0, 1.0)).max(behind),
                )
            };
            let i = ((py * size + px) * 4) as usize;
            rgba[i] = color[0].clamp(0.0, 255.0) as u8;
            rgba[i + 1] = color[1].clamp(0.0, 255.0) as u8;
            rgba[i + 2] = color[2].clamp(0.0, 255.0) as u8;
            rgba[i + 3] = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
    egui::IconData {
        rgba,
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
