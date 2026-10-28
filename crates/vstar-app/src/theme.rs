//! A calmer, more modern look than egui's defaults: roomier spacing,
//! rounder corners and a neutral grey palette.

use egui::{Color32, CornerRadius, Margin, Stroke, Theme, Vec2};

/// The colour of a fitted model drawn over the data.
pub const MODEL: Color32 = Color32::from_rgb(0xff, 0xb0, 0x3a);
pub const ACCENT: Color32 = Color32::from_rgb(0x9a, 0x9a, 0x9a);

pub fn install(ctx: &egui::Context) {
    for theme in [Theme::Dark, Theme::Light] {
        ctx.style_mut_of(theme, |style| {
            style.spacing.item_spacing = Vec2::new(8.0, 6.0);
            style.spacing.button_padding = Vec2::new(10.0, 5.0);
            style.spacing.interact_size.y = 26.0;
            style.spacing.window_margin = Margin::same(12);

            let v = &mut style.visuals;
            let radius = CornerRadius::same(6);
            v.window_corner_radius = CornerRadius::same(10);
            v.menu_corner_radius = radius;
            for w in [
                &mut v.widgets.noninteractive,
                &mut v.widgets.inactive,
                &mut v.widgets.hovered,
                &mut v.widgets.active,
                &mut v.widgets.open,
            ] {
                w.corner_radius = radius;
            }
            v.selection.bg_fill =
                ACCENT.gamma_multiply(if theme == Theme::Dark { 0.6 } else { 0.35 });
            v.selection.stroke = Stroke::new(1.0, ACCENT);
            v.hyperlink_color = if theme == Theme::Dark {
                Color32::from_rgb(0xd0, 0xd0, 0xd0)
            } else {
                Color32::from_rgb(0x40, 0x40, 0x40)
            };

            if theme == Theme::Dark {
                v.panel_fill = Color32::from_rgb(0x1e, 0x1e, 0x1e);
                v.window_fill = Color32::from_rgb(0x26, 0x26, 0x26);
                v.extreme_bg_color = Color32::from_rgb(0x16, 0x16, 0x16);
                v.faint_bg_color = Color32::from_rgb(0x2a, 0x2a, 0x2a);
            } else {
                v.panel_fill = Color32::from_rgb(0xf0, 0xf0, 0xf0);
                v.window_fill = Color32::WHITE;
                v.extreme_bg_color = Color32::WHITE;
                v.faint_bg_color = Color32::from_rgb(0xe4, 0xe4, 0xe4);
            }
        });
    }
}

/// A rounded "card" frame for grouping controls in the side panel.
pub fn card(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(12))
}

/// Plot colour for a band, adjusted so dark VStar colours stay visible on
/// a dark background.
pub fn band_color(ui: &egui::Ui, band: &str) -> Color32 {
    let Some([r, g, b]) = vstar_core::bands::color(band) else {
        return ui.visuals().strong_text_color();
    };
    let c = Color32::from_rgb(r, g, b);
    let luma = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    if ui.visuals().dark_mode && luma < 90.0 {
        c.lerp_to_gamma(Color32::WHITE, 0.45)
    } else if !ui.visuals().dark_mode && luma > 200.0 {
        c.lerp_to_gamma(Color32::BLACK, 0.3)
    } else {
        c
    }
}

/// Small hand-drawn icons, so they look the same on every system (fonts
/// differ in which symbols they include).
#[derive(Clone, Copy)]
pub enum Icon {
    StarChart,
    Book,
    Moon,
    Telescope,
}

pub fn icon(ui: &mut egui::Ui, icon: Icon) {
    let size = 16.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let color = ui.visuals().text_color();
    let stroke = Stroke::new(1.2, color);
    let c = rect.center();
    match icon {
        Icon::StarChart => {
            painter.circle_stroke(c, 7.0, stroke);
            for (dx, dy, r) in [
                (-3.0, -2.5, 1.3),
                (2.5, -3.5, 1.0),
                (3.0, 2.0, 1.5),
                (-2.0, 3.5, 1.0),
            ] {
                painter.circle_filled(c + Vec2::new(dx, dy), r, color);
            }
            painter.line_segment(
                [c + Vec2::new(-1.0, -1.0), c + Vec2::new(2.5, 2.0)],
                Stroke::new(0.8, color),
            );
        }
        Icon::Moon => {
            // A crescent: a disc with a second disc cut out of it.
            painter.circle_filled(c, 6.5, color);
            painter.circle_filled(c + Vec2::new(3.0, -2.0), 5.5, ui.visuals().window_fill);
            painter.circle_filled(c + Vec2::new(4.5, 4.0), 1.0, color);
        }
        Icon::Telescope => {
            // A tube on a tripod.
            let tube = [
                egui::pos2(c.x - 7.0, c.y - 1.0),
                egui::pos2(c.x - 5.5, c.y - 4.5),
                egui::pos2(c.x + 6.0, c.y - 0.5),
                egui::pos2(c.x + 5.0, c.y + 2.5),
            ];
            painter.add(egui::Shape::convex_polygon(
                tube.to_vec(),
                color.gamma_multiply(0.35),
                stroke,
            ));
            let top = egui::pos2(c.x - 1.0, c.y + 1.5);
            for foot in [
                egui::pos2(c.x - 5.0, c.y + 7.5),
                egui::pos2(c.x + 3.5, c.y + 7.5),
                egui::pos2(c.x - 0.5, c.y + 7.5),
            ] {
                painter.line_segment([top, foot], Stroke::new(1.0, color));
            }
        }
        Icon::Book => {
            let (l, r, t, b) = (c.x - 7.0, c.x + 7.0, c.y - 5.5, c.y + 5.5);
            let left = [
                egui::pos2(c.x, t + 1.0),
                egui::pos2(l, t),
                egui::pos2(l, b),
                egui::pos2(c.x, b + 1.0),
            ];
            let right = [
                egui::pos2(c.x, t + 1.0),
                egui::pos2(r, t),
                egui::pos2(r, b),
                egui::pos2(c.x, b + 1.0),
            ];
            painter.add(egui::Shape::closed_line(left.to_vec(), stroke));
            painter.add(egui::Shape::closed_line(right.to_vec(), stroke));
            for dy in [-1.5, 1.0] {
                painter.line_segment(
                    [
                        egui::pos2(l + 2.5, c.y + dy),
                        egui::pos2(c.x - 2.0, c.y + dy + 0.8),
                    ],
                    Stroke::new(0.8, color),
                );
            }
        }
    }
}

/// The nova picture is drawn at this fraction of its full size.
const NOVA_SCALE: f32 = 0.375;

/// A small star picture for the info card: coloured by spectral type, drawn
/// as a pair for binaries and as a star inside a disc for novae.
pub fn star_icon(
    ui: &mut egui::Ui,
    look: vstar_core::vartypes::StarLook,
    rgb: Option<[u8; 3]>,
) -> egui::Response {
    use vstar_core::vartypes::StarLook;
    let size = if look == StarLook::Nova {
        Vec2::new(72.0 * NOVA_SCALE, 36.0 * NOVA_SCALE)
    } else if look == StarLook::Pulsator {
        Vec2::new(30.0, 28.0)
    } else {
        Vec2::new(40.0, 22.0)
    };
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let base = rgb.map_or(Color32::from_gray(170), |[r, g, b]| {
        Color32::from_rgb(r, g, b)
    });
    let rim = if ui.visuals().dark_mode {
        Color32::from_black_alpha(160)
    } else {
        Color32::from_gray(90)
    };
    let outline = Stroke::new(1.0, rim);
    let c = rect.center();
    let star = |center: egui::Pos2, radius: f32, color: Color32| {
        painter.circle_filled(center, radius, color);
        painter.circle_stroke(center, radius, outline);
        // A small highlight so the disc reads as a glowing star.
        painter.circle_filled(
            center + Vec2::new(-radius * 0.3, -radius * 0.3),
            radius * 0.3,
            color.lerp_to_gamma(Color32::WHITE, 0.6),
        );
    };
    match look {
        StarLook::Single => star(c, 8.0, base),
        StarLook::Binary => {
            // A faint orbit with a big and a small star on it.
            let orbit: Vec<egui::Pos2> = (0..32)
                .map(|i| {
                    let a = i as f32 / 32.0 * std::f32::consts::TAU;
                    c + Vec2::new(a.cos() * 11.0, a.sin() * 5.0)
                })
                .collect();
            painter.add(egui::Shape::closed_line(
                orbit,
                Stroke::new(0.8, ui.visuals().text_color().gamma_multiply(0.4)),
            ));
            let companion = base.lerp_to_gamma(Color32::from_rgb(255, 140, 60), 0.6);
            star(c + Vec2::new(5.5, 0.0), 4.0, companion);
            star(c + Vec2::new(-5.0, 0.0), 6.5, base);
        }
        StarLook::Contact => {
            // Two stars stretched toward each other, overlapping so they
            // touch at a narrow waist. The dark rim is drawn first, slightly
            // larger, so it outlines the joined shape rather than each star.
            let parts = [(-6.5_f32, 8.5_f32, 7.0_f32), (6.5, 8.5, 7.0)];
            let ellipse = |cx: f32, rx: f32, ry: f32| -> Vec<egui::Pos2> {
                (0..40)
                    .map(|i| {
                        let a = i as f32 / 40.0 * std::f32::consts::TAU;
                        c + Vec2::new(cx + a.cos() * rx, a.sin() * ry)
                    })
                    .collect()
            };
            for (cx, rx, ry) in parts {
                painter.add(egui::Shape::convex_polygon(
                    ellipse(cx, rx + 0.9, ry + 0.9),
                    rim,
                    Stroke::NONE,
                ));
            }
            for (cx, rx, ry) in parts {
                painter.add(egui::Shape::convex_polygon(
                    ellipse(cx, rx, ry),
                    base,
                    Stroke::NONE,
                ));
            }
            for cx in [-8.0_f32, 8.0] {
                painter.circle_filled(
                    c + Vec2::new(cx, -2.5),
                    2.2,
                    base.lerp_to_gamma(Color32::WHITE, 0.6),
                );
            }
        }
        StarLook::Pulsator => {
            // The star with a faint halo in the star's own colour, and four arrows pointing outward
            // between the two to show it swelling and shrinking.
            let ring = base;
            painter.circle_filled(c, 12.5, ring.gamma_multiply(0.12));
            painter.circle_stroke(c, 12.5, Stroke::new(1.2, ring.gamma_multiply(0.75)));
            star(c, 5.0, base);
            let arrow = Stroke::new(1.5, ui.visuals().strong_text_color());
            for k in 0..4 {
                let angle = std::f32::consts::FRAC_PI_4 + k as f32 * std::f32::consts::FRAC_PI_2;
                let dir = Vec2::new(angle.cos(), -angle.sin());
                let (from, tip) = (c + dir * 6.4, c + dir * 11.2);
                painter.line_segment([from, tip], arrow);
                for side in [-0.55_f32, 0.55] {
                    let (s, co) = side.sin_cos();
                    let back = Vec2::new(-dir.x * co + dir.y * s, -dir.y * co - dir.x * s);
                    painter.line_segment([tip, tip + back * 3.4], arrow);
                }
            }
        }
        StarLook::Nova => {
            const K: f32 = NOVA_SCALE;
            // After the artist's sketch: a big dark red donor star on the
            // left, a stream that runs orange, yellow, then white as it
            // narrows toward a tilted pale blue disc with a white dwarf at
            // its centre. Positions are in sketch pixels (donor centre
            // 160,160), scaled to fit the same box as before.
            let u = 0.126 * K;
            let at = |x: f32, y: f32| {
                egui::pos2(
                    rect.left() + (x - 10.0) * u,
                    rect.center().y + (y - 160.0) * u,
                )
            };
            let donor_col = Color32::from_rgb(140, 0, 20);
            let mut mesh = egui::Mesh::default();
            // (top, bottom) edge of the stream at each stage, with its colour.
            let stages = [
                (
                    at(272.0, 172.0),
                    at(268.0, 222.0),
                    Color32::from_rgb(255, 125, 40),
                ),
                (
                    at(305.0, 172.0),
                    at(305.0, 205.0),
                    Color32::from_rgb(255, 235, 0),
                ),
                (
                    at(360.0, 168.0),
                    at(360.0, 192.0),
                    Color32::from_rgb(255, 255, 40),
                ),
                (at(364.0, 166.0), at(364.0, 192.0), Color32::WHITE),
            ];
            for (top, bottom, color) in stages {
                mesh.colored_vertex(top, color);
                mesh.colored_vertex(bottom, color);
            }
            let tip = at(468.0, 147.0);
            mesh.colored_vertex(tip, Color32::WHITE);
            for i in 0..3u32 {
                let k = i * 2;
                mesh.add_triangle(k, k + 1, k + 2);
                mesh.add_triangle(k + 1, k + 3, k + 2);
            }
            mesh.add_triangle(6, 7, 8);
            painter.add(egui::Shape::mesh(mesh));
            // The disc, an ellipse tilted up to the right.
            let disc_c = at(478.0, 181.0);
            let (sin, cos) = (-0.42_f32).sin_cos();
            let ellipse: Vec<egui::Pos2> = (0..32)
                .map(|i| {
                    let a = i as f32 / 32.0 * std::f32::consts::TAU;
                    let (x, y) = (a.cos() * 85.0 * u, a.sin() * 36.0 * u);
                    disc_c + Vec2::new(x * cos - y * sin, x * sin + y * cos)
                })
                .collect();
            painter.add(egui::Shape::convex_polygon(
                ellipse,
                Color32::from_rgb(165, 205, 255),
                Stroke::new(0.6, rim.gamma_multiply(0.6)),
            ));
            painter.circle_filled(disc_c, 9.0 * u * 1.4, Color32::WHITE);
            // The donor star, over the start of the stream.
            star(at(160.0, 160.0), 125.0 * u, donor_col);
        }
    }
    response
}

/// A sliding on/off switch with a label on its left (for right-to-left
/// layouts the label reads first).
pub fn switch(ui: &mut egui::Ui, on: &mut bool, label: &str) -> egui::Response {
    let size = Vec2::new(34.0, 18.0);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(response.id, *on);
    let off = ui.visuals().widgets.inactive.bg_fill;
    let track = off.lerp_to_gamma(ACCENT, t);
    let r = rect.height() / 2.0;
    ui.painter().rect_filled(rect, r, track);
    let x = egui::lerp((rect.left() + r)..=(rect.right() - r), t);
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), r - 2.5, Color32::WHITE);
    ui.label(label);
    response.on_hover_text("Switch between dark and light mode")
}
