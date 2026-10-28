//! Draws the FeStar icon: a glowing blue star with magnetic field lines and
//! a beam along its magnetic axis. This file has no dependencies so the
//! build script can use it too, to make the Windows `.exe` icon.

/// Colour `src` (straight alpha) laid over `dst`.
fn over(dst: [f32; 4], src: [f32; 3], a: f32) -> [f32; 4] {
    let out_a = a + dst[3] * (1.0 - a);
    if out_a <= 0.0 {
        return [0.0; 4];
    }
    let mut out = [0.0, 0.0, 0.0, out_a];
    for i in 0..3 {
        out[i] = (src[i] * a + dst[i] * dst[3] * (1.0 - a)) / out_a;
    }
    out
}

/// Squared distance from point `p` to the segment `a`-`b`.
fn segment_distance_sq(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let len_sq = abx * abx + aby * aby;
    let t = if len_sq > 0.0 {
        (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (dx, dy) = (p.0 - (a.0 + t * abx), p.1 - (a.1 + t * aby));
    dx * dx + dy * dy
}

fn noise(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

/// Smooth noise at a coarse scale, for the star's mottled surface.
fn blotch(x: f32, y: f32, scale: f32) -> f32 {
    let (fx, fy) = (x / scale, y / scale);
    let (ix, iy) = (fx.floor() as i32, fy.floor() as i32);
    let (tx, ty) = (fx - ix as f32, fy - iy as f32);
    let mix = |a: f32, b: f32, t: f32| a + (b - a) * (t * t * (3.0 - 2.0 * t));
    let top = mix(noise(ix, iy), noise(ix + 1, iy), tx);
    let bottom = mix(noise(ix, iy + 1), noise(ix + 1, iy + 1), tx);
    mix(top, bottom, ty)
}

/// RGBA pixels of a square icon, `size` pixels on a side.
pub fn render(size: u32) -> Vec<u8> {
    let s = size as f32;
    let centre = s / 2.0;
    let radius = s * 0.21;
    // The magnetic axis, tilted a little from vertical.
    let len = (0.32f32 * 0.32 + 1.0).sqrt();
    let dir = (0.32 / len, 1.0 / len);
    let side = (dir.1, -dir.0);

    // Dipole field lines r = L sin^2(theta), on both sides of the axis.
    let mut lines: Vec<Vec<(f32, f32)>> = Vec::new();
    for l in [1.3f32, 1.6, 1.9] {
        let l = l * radius;
        let start = (radius / l).sqrt().asin();
        let steps = 90;
        for sign in [-1.0f32, 1.0] {
            let mut points = Vec::with_capacity(steps + 1);
            for i in 0..=steps {
                let theta = start + (std::f32::consts::PI - 2.0 * start) * i as f32 / steps as f32;
                let r = l * theta.sin().powi(2);
                let (along, across) = (r * theta.cos(), r * theta.sin() * sign);
                points.push((
                    centre + dir.0 * along + side.0 * across,
                    centre + dir.1 * along + side.1 * across,
                ));
            }
            lines.push(points);
        }
    }
    let line_half_width = (s * 0.0055).max(0.5);

    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for py in 0..size {
        for px in 0..size {
            let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
            let (x, y) = (fx - centre, fy - centre);
            let d = (x * x + y * y).sqrt();
            let mut px_color = [0.0f32; 4];

            // The beam along the magnetic axis, fading toward its ends.
            let across = (x * dir.1 - y * dir.0).abs();
            let along = (x * dir.0 + y * dir.1).abs();
            let beam = (1.0 - across / (s * 0.10)).clamp(0.0, 1.0).powf(1.3)
                * (1.0 - along / (s * 0.66)).clamp(0.0, 1.0).powf(0.8)
                * 0.6;
            px_color = over(px_color, [140.0, 205.0, 235.0], beam);

            // The glow around the star.
            let halo = (1.0 - ((d - radius) / (s * 0.14)).clamp(0.0, 1.0)).powf(2.0) * 0.8;
            px_color = over(px_color, [90.0, 215.0, 255.0], halo);

            // Field lines: distance to the nearest line segment.
            let mut nearest = f32::MAX;
            for line in &lines {
                for pair in line.windows(2) {
                    nearest = nearest.min(segment_distance_sq((fx, fy), pair[0], pair[1]));
                }
            }
            let dist = nearest.sqrt();
            let line = (1.0 - (dist - line_half_width).clamp(0.0, 1.0)) * 0.75;
            px_color = over(px_color, [170.0, 230.0, 255.0], line);

            // The star: cyan, mottled, brighter toward the edge.
            if d <= radius + 1.5 {
                let n = blotch(x, y, s * 0.04) * 0.6 + blotch(x, y, s * 0.014) * 0.4;
                let limb = (d / radius).powf(3.0);
                let t = (n * 0.95 + limb * 0.2).clamp(0.0, 1.0);
                let color = [
                    20.0 + 215.0 * t.powf(2.5),
                    120.0 + 130.0 * t,
                    190.0 + 65.0 * t,
                ];
                let cover = 1.0 - ((d - radius + 1.0) / 1.5).clamp(0.0, 1.0);
                px_color = over(px_color, color, cover);
            }

            let i = ((py * size + px) * 4) as usize;
            for c in 0..3 {
                rgba[i + c] = px_color[c].clamp(0.0, 255.0) as u8;
            }
            rgba[i + 3] = (px_color[3].clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
    rgba
}
