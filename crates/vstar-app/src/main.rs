// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod app_icon;
mod external;
mod light_curve;
mod loader;
mod phase_ui;
mod prefs;
mod search_ui;
mod task;
mod theme;
mod tools_ui;
mod vsp_ui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FeStar")
            .with_icon(std::sync::Arc::new(app_icon::icon(256)))
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        // OpenGL is the fallback for machines whose graphics drivers don't
        // work with the default (DirectX 12 / Vulkan) renderer.
        renderer: if std::env::var_os("VSTAR_OPENGL").is_some() {
            eframe::Renderer::Glow
        } else {
            eframe::Renderer::default()
        },
        ..Default::default()
    };
    // A file given on the command line (or via Windows "Open with") is loaded at startup.
    let open_file = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    eframe::run_native(
        // Kept as the old name so saved settings carry over after the rename.
        "VStar-RS",
        options,
        Box::new(|cc| Ok(Box::new(app::VStarApp::new(cc, open_file)))),
    )
}
