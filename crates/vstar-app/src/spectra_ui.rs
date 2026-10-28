//! The Spectra tab: the most recent spectra of the star from the AAVSO
//! spectroscopy database (AVSpec).

use egui::RichText;
use egui_plot::{HoverPosition, Line, Plot, PlotPoints};
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
    /// A "load more" request in progress, and whether the last one found
    /// nothing new.
    more_task: Option<Task<Vec<Record>>>,
    no_more: bool,
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
        self.no_more = false;
        self.more_task = None;
        self.opened = true;
        self.task = Some(Task::spawn(
            ctx,
            format!("Loading spectra of {name} from AVSpec"),
            move |_| {
                let http = ReqwestClient::new()?;
                avspec::latest(&http, &name, 0, COUNT)
            },
        ));
    }

    /// Fetch the next few spectra after those already shown.
    fn request_more(&mut self, ctx: &egui::Context) {
        let name = self.star.clone();
        let skip = self.records.len();
        self.error = None;
        self.more_task = Some(Task::spawn(
            ctx,
            format!("Loading more spectra of {name} from AVSpec"),
            move |_| {
                let http = ReqwestClient::new()?;
                avspec::latest(&http, &name, skip, COUNT)
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
        if let Some(result) = self.spectra.more_task.as_ref().and_then(Task::poll) {
            self.spectra.more_task = None;
            match result {
                Ok(mut more) => {
                    self.spectra.no_more = more.len() < COUNT;
                    self.spectra.records.append(&mut more);
                }
                Err(e) => self.spectra.error = Some(e.to_string()),
            }
        }
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
            if self.spectra.records.is_empty() {
                return;
            }
        }
        if self.spectra.task.is_some() {
            ui.add_space(6.0);
            ui.label(RichText::new("Loading…").weak());
            return;
        }
        let nm = self.settings.spectrum_nm;
        let line_color = ui.visuals().strong_text_color();
        let height = ((ui.available_height() - 130.0) / 2.0).max(140.0);
        ui.add_space(4.0);
        let mut more = false;
        egui::ScrollArea::vertical()
            .max_height((ui.available_height() - 36.0).max(100.0))
            .show(ui, |ui| {
                for pair in self.spectra.records.chunks(2) {
                    ui.columns(2, |cols| {
                        for (col, record) in cols.iter_mut().zip(pair) {
                            spectrum_plot(col, record, height, line_color, nm);
                        }
                    });
                    ui.add_space(8.0);
                }
            });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if self.spectra.more_task.is_some() {
                ui.spinner();
                ui.label(RichText::new("Loading more…").weak());
            } else if self.spectra.no_more {
                ui.label(RichText::new("No more spectra of this star in AVSpec.").weak());
            } else {
                more = ui.button("Load more").clicked();
            }
        });
        if more {
            self.spectra.request_more(ui.ctx());
        }
    }
}

fn spectrum_plot(ui: &mut egui::Ui, record: &Record, height: f32, color: egui::Color32, nm: bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(describe(record)).strong());
        ui.hyperlink_to("AVSpec ↗", spectra::page_url(record.row.id));
    });
    let s = &record.spectrum;
    // Wavelengths are stored in ångströms; one nanometre is ten of them.
    let (scale, unit) = if nm { (0.1, "nm") } else { (1.0, "Å") };
    let points: PlotPoints = s
        .wavelengths
        .iter()
        .zip(&s.flux)
        .map(|(w, f)| [*w * scale, *f])
        .collect();
    Plot::new(("spectrum", record.row.id))
        .height(height)
        .x_axis_label(format!("Wavelength ({unit})"))
        .label_formatter(move |pos| {
            let (HoverPosition::NearDataPoint { position, .. }
            | HoverPosition::Elsewhere { position }) = pos;
            let digits = if nm { 2 } else { 1 };
            Some(format!(
                "{:.digits$} {unit}\nflux {:.4}",
                position.x, position.y
            ))
        })
        .y_axis_label("Flux")
        .allow_scroll(false)
        .show(ui, |plot| {
            plot.line(Line::new("spectrum", points).color(theme::ACCENT.lerp_to_gamma(color, 0.5)));
        });
}
