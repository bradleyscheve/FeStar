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

/// A small star picture for the info card: coloured by spectral type, drawn
/// as a pair for binaries and as a star inside a disc for novae.
pub fn star_icon(
    ui: &mut egui::Ui,
    look: vstar_core::vartypes::StarLook,
    rgb: Option<[u8; 3]>,
) -> egui::Response {
    use vstar_core::vartypes::StarLook;
    let size = Vec2::new(26.0, 20.0);
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
        StarLook::Nova => {
            // Accretion disc: a tilted, flattened ring around the star, with
            // the half in front of the star drawn over it.
            let tilt = -0.35_f32;
            let (sin_t, cos_t) = tilt.sin_cos();
            let ring = |rx: f32, ry: f32, from: f32, to: f32| -> Vec<egui::Pos2> {
                (0..=24)
                    .map(|i| {
                        let a = from + (to - from) * i as f32 / 24.0;
                        let (x, y) = (a.cos() * rx, a.sin() * ry);
                        c + Vec2::new(x * cos_t - y * sin_t, x * sin_t + y * cos_t)
                    })
                    .collect()
            };
            let disc = Color32::from_rgb(255, 150, 50);
            let tau = std::f32::consts::TAU;
            // Back half of the disc (behind the star).
            painter.add(egui::Shape::line(
                ring(12.0, 4.0, std::f32::consts::PI, tau),
                Stroke::new(3.0, disc.gamma_multiply(0.8)),
            ));
            star(c, 6.0, base);
            // Front half of the disc (in front of the star).
            painter.add(egui::Shape::line(
                ring(12.0, 4.0, 0.0, std::f32::consts::PI),
                Stroke::new(3.0, disc),
            ));
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
