//! A calmer, more modern look than egui's defaults: roomier spacing,
//! rounder corners and a neutral grey palette.

use egui::{Color32, CornerRadius, Margin, Stroke, Theme, Vec2};

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
