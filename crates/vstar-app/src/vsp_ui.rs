//! The "VSP" popup: the AAVSO Variable Star Plotter chart for the loaded star.

use egui::{Context, TextureHandle};
use vstar_core::http::ReqwestClient;
use vstar_core::sources::vsp::{self, Chart};
use vstar_core::{Error, Result};

use crate::task::Task;

type Fetched = (Chart, egui::ColorImage);

#[derive(Default)]
pub struct VspState {
    open: bool,
    star: String,
    task: Option<Task<Fetched>>,
    chart: Option<Chart>,
    texture: Option<TextureHandle>,
    error: Option<String>,
}

impl VspState {
    /// Open the popup and start downloading the chart for `star`.
    pub fn open(&mut self, ctx: &Context, star: &str) {
        self.open = true;
        self.star = star.to_string();
        self.chart = None;
        self.texture = None;
        self.error = None;
        let name = star.to_string();
        self.task = Some(Task::spawn(ctx, "Downloading VSP chart", move |_| {
            let http = ReqwestClient::new()?;
            let (chart, bytes) = vsp::fetch_chart(&http, &name)?;
            let img = image::load_from_memory(&bytes)
                .map_err(|e| Error::Network(format!("couldn't read the chart image: {e}")))?
                .to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            Ok((
                chart,
                egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()),
            ))
        }));
    }

    fn poll(&mut self, ctx: &Context) {
        let Some(result) = self.task.as_ref().and_then(Task::poll) else {
            return;
        };
        self.task = None;
        let result: Result<Fetched> = result;
        match result {
            Ok((chart, image)) => {
                self.texture = Some(ctx.load_texture("vsp_chart", image, Default::default()));
                self.chart = Some(chart);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub fn show(&mut self, ctx: &Context) {
        if !self.open {
            return;
        }
        self.poll(ctx);
        let mut open = true;
        egui::Window::new(format!("VSP chart: {}", self.star))
            .open(&mut open)
            .default_size([620.0, 680.0])
            .resizable(true)
            .show(ctx, |ui| {
                if self.task.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Downloading the chart from AAVSO VSP…");
                    });
                }
                if let Some(e) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                    ui.label("The star name must be one that VSP knows.");
                    if ui.button("Try again").clicked() {
                        let star = self.star.clone();
                        self.open(ctx, &star);
                    }
                }
                if let Some(chart) = &self.chart {
                    ui.label(format!(
                        "Field of view {} arcmin, comparison stars to magnitude {}. Chart {}.",
                        vsp::DEFAULT_FOV,
                        vsp::DEFAULT_MAG_LIMIT,
                        chart.chart_id
                    ));
                    ui.hyperlink_to("Open in VSP on the web ↗", &chart.page_url);
                }
                if let Some(tex) = &self.texture {
                    egui::ScrollArea::both().show(ui, |ui| {
                        let avail = ui.available_width().max(200.0);
                        let size = tex.size_vec2();
                        let scale = (avail / size.x).min(1.0);
                        ui.image((tex.id(), size * scale));
                    });
                }
            });
        self.open = open;
    }
}
