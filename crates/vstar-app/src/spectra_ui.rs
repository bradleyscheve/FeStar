//! The Spectra tab: the most recent spectra of the star from the AAVSO
//! spectroscopy database (AVSpec).

use egui::RichText;
use egui_plot::{Line, Plot, PlotPoints};
use vstar_core::http::ReqwestClient;
use vstar_core::sources::avspec::{self, Record};
use vstar_core::spectra;

use crate::app::VStarApp;
use crate::task::Task;
use crate::theme;

/// How many spectra the button loads.
const COUNT: usize = 4;

#[derive(Default)]
pub struct SpectraState {
    task: Option<Task<Vec<Record>>>,
    records: Vec<Record>,
    error: Option<String>,
    /// The star the spectra were asked for.
    star: String,
    /// Whether the tab has been opened for this star.
    pub opened: bool,
}

impl SpectraState {
    pub fn request(&mut self, ctx: &egui::Context, star: &str) {
        let name = star.to_string();
        self.star = name.clone();
        self.error = None;
        self.opened = true;
        self.task = Some(Task::spawn(
            ctx,
            format!("Loading spectra of {name} from AVSpec"),
            move |_| {
                let http = ReqwestClient::new()?;
                avspec::latest(&http, &name, COUNT)
            },
        ));
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

fn describe(record: &Record) -> String {
    let row = &record.row;
    let mut text = format!("{} · {}", row.observer, row.date);
    if let Some(r) = row.resolution {
        text.push_str(&format!(" · R {r:.0}"));
    }
    if let Some(e) = row.exposure_s.filter(|e| *e > 0.0) {
        text.push_str(&format!(" · {e:.0} s"));
    }
    text
}

impl VStarApp {
    pub(crate) fn poll_spectra(&mut self) {
        let Some(result) = self.spectra.task.as_ref().and_then(Task::poll) else {
            return;
        };
        self.spectra.task = None;
        match result {
            Ok(records) => self.spectra.records = records,
            Err(e) => {
                self.spectra.records.clear();
                self.spectra.error = Some(e.to_string());
            }
        }
    }

    pub(crate) fn show_spectra_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "Most recent spectra of {} from the AAVSO spectroscopy database",
                    self.spectra.star
                ))
                .strong(),
            );
            if self.spectra.task.is_some() {
                ui.spinner();
            } else if ui.small_button("Reload").clicked() {
                let star = self.spectra.star.clone();
                self.spectra.request(ui.ctx(), &star);
            }
            ui.hyperlink_to("Open AVSpec ↗", avspec::SEARCH_URL);
        });
        if let Some(error) = &self.spectra.error {
            ui.add_space(6.0);
            ui.colored_label(ui.visuals().error_fg_color, error);
            return;
        }
        if self.spectra.task.is_some() {
            ui.add_space(6.0);
            ui.label(RichText::new("Loading…").weak());
            return;
        }
        let records = &self.spectra.records;
        let line_color = ui.visuals().strong_text_color();
        let height = ((ui.available_height() - 90.0) / 2.0).max(140.0);
        ui.add_space(4.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for pair in records.chunks(2) {
                ui.columns(2, |cols| {
                    for (col, record) in cols.iter_mut().zip(pair) {
                        spectrum_plot(col, record, height, line_color);
                    }
                });
                ui.add_space(8.0);
            }
        });
    }
}

fn spectrum_plot(ui: &mut egui::Ui, record: &Record, height: f32, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(describe(record)).strong());
        ui.hyperlink_to("AVSpec ↗", spectra::page_url(record.row.id));
    });
    let s = &record.spectrum;
    let points: PlotPoints = s
        .wavelengths
        .iter()
        .zip(&s.flux)
        .map(|(w, f)| [*w, *f])
        .collect();
    Plot::new(("spectrum", record.row.id))
        .height(height)
        .x_axis_label("Wavelength (Å)")
        .y_axis_label("Flux")
        .allow_scroll(false)
        .show(ui, |plot| {
            plot.line(Line::new("spectrum", points).color(theme::ACCENT.lerp_to_gamma(color, 0.5)));
        });
}
