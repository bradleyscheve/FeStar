//! Analysis > Phase plot: the parameters pop-up, with an option to let the
//! AI choose the period and epoch.

use egui::RichText;
use vstar_core::Observation;
use vstar_core::ai::AiClient;
use vstar_core::ai::advice::{self, DataSummary, PeriodAdvice};
use vstar_core::analysis::dcdft::{self, Params};
use vstar_core::analysis::phase::EpochStrategy;
use vstar_core::http::ReqwestClient;

use crate::app::{Tab, VStarApp};
use crate::task::Task;

/// Very large data sets are thinned to this many points for the quick scan
/// that finds candidate periods.
const QUICK_SCAN_POINTS: usize = 8_000;

pub struct PhaseState {
    pub dialog_open: bool,
    pub period_text: String,
    pub epoch_text: String,
    pub two_cycles: bool,
    /// The period and epoch currently plotted.
    pub active: Option<(f64, f64)>,
    /// Every (period, epoch) plotted for this star, oldest first.
    pub history: Vec<(f64, f64)>,
    ai: Option<Task<AiPhaseResult>>,
    ai_note: Option<String>,
    error: Option<String>,
}

impl Default for PhaseState {
    fn default() -> Self {
        Self {
            dialog_open: false,
            period_text: String::new(),
            epoch_text: String::new(),
            two_cycles: true,
            active: None,
            history: Vec::new(),
            ai: None,
            ai_note: None,
            error: None,
        }
    }
}

struct AiPhaseResult {
    advice: PeriodAdvice,
    epoch_jd: Option<f64>,
    candidates: usize,
}

impl PhaseState {
    /// Add the active plot to the history unless it is already the latest.
    pub fn remember(&mut self) {
        if let Some(a) = self.active
            && self.history.last() != Some(&a)
        {
            self.history.push(a);
        }
    }

    pub fn task(&self) -> Option<(&str, f32)> {
        self.ai
            .as_ref()
            .map(|t| (t.description.as_str(), t.progress()))
    }

    pub fn cancel(&self) {
        if let Some(t) = &self.ai {
            t.cancel();
        }
    }

    /// Forget everything when a different star is loaded.
    pub fn reset(&mut self) {
        let two_cycles = self.two_cycles;
        *self = Self {
            two_cycles,
            ..Self::default()
        };
    }
}

fn parse_positive(text: &str, what: &str) -> Result<f64, String> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or_else(|| format!("{what} must be a positive number."))
}

impl VStarApp {
    pub(crate) fn open_phase_dialog(&mut self) {
        let ts = self.analysis_series();
        let catalogue = self.dataset.as_ref().and_then(|d| d.star.period_days);
        let p = &mut self.phase;
        if p.period_text.trim().is_empty()
            && let Some(period) = catalogue
        {
            p.period_text = period.to_string();
        }
        if p.epoch_text.trim().is_empty()
            && let Some(epoch) = EpochStrategy::MeanOfEnds.epoch(&ts)
        {
            p.epoch_text = format!("{epoch:.4}");
        }
        p.error = None;
        p.dialog_open = true;
    }

    /// Plot at a given period (from a period-search result, say).
    pub(crate) fn plot_phase_at(&mut self, period: f64) {
        let ts = self.analysis_series();
        if let Some(epoch) = EpochStrategy::MeanOfEnds.epoch(&ts) {
            self.phase.period_text = period.to_string();
            self.phase.epoch_text = format!("{epoch:.4}");
            self.phase.active = Some((period, epoch));
            self.phase.remember();
            self.tab = Tab::PhasePlot;
            self.reset_view = true;
        }
    }

    pub(crate) fn poll_phase_ai(&mut self) {
        let Some(result) = self.phase.ai.as_ref().and_then(Task::poll) else {
            return;
        };
        self.phase.ai = None;
        match result {
            Ok(r) => {
                self.phase.period_text = r.advice.period.to_string();
                if let Some(epoch) = r.epoch_jd {
                    self.phase.epoch_text = format!("{epoch:.4}");
                }
                self.phase.ai_note = Some(format!(
                    "AI chose a period of {:.6} days and phase 0 at the {} (from {} candidate periods). {}",
                    r.advice.period,
                    match r.advice.epoch {
                        EpochStrategy::Maximum => "brightest point",
                        EpochStrategy::Minimum => "faintest point",
                        EpochStrategy::MeanOfEnds => "middle of the observations",
                    },
                    r.candidates,
                    r.advice.rationale
                ));
                self.phase.error = None;
            }
            Err(vstar_core::Error::Cancelled) => {}
            Err(e) => self.phase.error = Some(e.to_string()),
        }
    }

    fn start_phase_ai(&mut self, ctx: &egui::Context) {
        self.phase.error = None;
        let Some(ds) = &self.dataset else { return };
        let ts = self.analysis_series();
        let observations = self.analysis_observations();
        let Some(summary) = DataSummary::new(&ds.star, &ts, band_counts(&observations)) else {
            self.phase.error = Some("There are no observations to analyse.".into());
            return;
        };
        let (config, key) = match self.require_ai() {
            Ok(v) => v,
            Err(msg) => {
                self.phase.error = Some(msg);
                return;
            }
        };
        // Reuse a period search that was already run on exactly this data.
        let prior = self
            .search
            .result
            .as_ref()
            .filter(|r| r.n_used == ts.len())
            .map(|r| (r.spectrum.clone(), r.params.resolution));

        self.phase.ai_note = None;
        self.phase.ai = Some(Task::spawn(
            ctx,
            "Finding candidate periods and asking the AI",
            move |h| {
                let (spectrum, resolution) = match prior {
                    Some(p) => p,
                    None => {
                        let quick = ts.thinned(QUICK_SCAN_POINTS);
                        let params = Params::standard(&quick);
                        (
                            dcdft::scan(&quick, params, &|f| h.report(0.8 * f))?,
                            params.resolution,
                        )
                    }
                };
                let candidates = advice::candidates(&ts, &spectrum, resolution);
                h.report(0.85);
                if h.is_cancelled() {
                    return Err(vstar_core::Error::Cancelled);
                }
                let http = ReqwestClient::new()?;
                let client = AiClient {
                    http: &http,
                    config: &config,
                    api_key: &key,
                };
                let advice = advice::choose_period(&client, &summary, &candidates)?;
                let epoch_jd = advice.epoch.epoch(&ts);
                Ok(AiPhaseResult {
                    advice,
                    epoch_jd,
                    candidates: candidates.len(),
                })
            },
        ));
    }

    pub(crate) fn show_phase_dialog(&mut self, ctx: &egui::Context) {
        if !self.phase.dialog_open {
            return;
        }
        let ts = self.analysis_series();
        let catalogue = self.dataset.as_ref().and_then(|d| d.star.period_days);
        let ai_ready = self.ai_configured();
        let mut open = true;
        let (mut plot, mut ask_ai, mut open_prefs) = (false, false, false);

        egui::Window::new("Phase plot")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .min_width(420.0)
            .show(ctx, |ui| {
                let p = &mut self.phase;
                ui.label("Fold the light curve at a period to see its repeating shape.");
                ui.add_space(4.0);
                egui::Grid::new("phase_params").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label("Period (days)");
                    ui.vertical(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut p.period_text).desired_width(200.0));
                        if let Some(c) = catalogue
                            && ui.small_button(format!("Use catalogue period ({c} d)")).clicked()
                        {
                            p.period_text = c.to_string();
                        }
                    });
                    ui.end_row();

                    ui.label("Epoch (JD of phase 0)");
                    ui.vertical(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut p.epoch_text).desired_width(200.0));
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("Set from:").weak());
                            for (strategy, label) in [
                                (EpochStrategy::MeanOfEnds, "Middle"),
                                (EpochStrategy::Maximum, "Brightest"),
                                (EpochStrategy::Minimum, "Faintest"),
                            ] {
                                if ui.small_button(label).on_hover_text(strategy.label()).clicked()
                                    && let Some(epoch) = strategy.epoch(&ts)
                                {
                                    p.epoch_text = format!("{epoch:.4}");
                                }
                            }
                        });
                    });
                    ui.end_row();
                });
                ui.checkbox(&mut p.two_cycles, "Show two cycles");

                ui.add_space(6.0);
                ui.separator();
                ui.label(RichText::new("AI").strong());
                ui.label(
                    RichText::new(
                        "Searches the data for candidate periods, then asks the AI to choose the \
                         best period and where phase 0 falls. You can adjust the result before plotting.",
                    )
                    .weak(),
                );
                ui.horizontal(|ui| {
                    let busy = p.ai.is_some();
                    if ui.add_enabled(!busy, egui::Button::new("Let AI choose settings")).clicked() {
                        ask_ai = true;
                    }
                    if busy {
                        ui.spinner();
                        ui.label("Working…");
                    }
                    if !ai_ready && ui.link("Connect an AI provider").clicked() {
                        open_prefs = true;
                    }
                });
                if let Some(note) = &p.ai_note {
                    ui.label(note);
                }
                if let Some(err) = &p.error {
                    ui.colored_label(ui.visuals().error_fg_color, err);
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new("Plot").min_size([100.0, 28.0].into()))
                        .clicked()
                    {
                        plot = true;
                    }
                });
            });

        if open_prefs {
            self.prefs.open = true;
        }
        if ask_ai {
            self.start_phase_ai(ctx);
        }
        if plot {
            let parsed = parse_positive(&self.phase.period_text, "The period").and_then(|period| {
                let epoch = if self.phase.epoch_text.trim().is_empty() {
                    EpochStrategy::MeanOfEnds
                        .epoch(&ts)
                        .ok_or_else(|| "There are no observations to plot.".to_string())?
                } else {
                    self.phase
                        .epoch_text
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|e| e.is_finite())
                        .ok_or_else(|| "The epoch must be a Julian Date.".to_string())?
                };
                Ok((period, epoch))
            });
            match parsed {
                Ok(pe) => {
                    self.phase.active = Some(pe);
                    self.phase.remember();
                    self.phase.error = None;
                    self.tab = Tab::PhasePlot;
                    self.reset_view = true;
                    open = false;
                }
                Err(msg) => self.phase.error = Some(msg),
            }
        }
        self.phase.dialog_open = open;
    }
}

/// Observation counts per band, most populous first.
pub fn band_counts(observations: &[&Observation]) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for o in observations {
        match counts.iter_mut().find(|(b, _)| *b == o.band) {
            Some((_, n)) => *n += 1,
            None => counts.push((o.band.clone(), 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_positive_numbers_only() {
        assert_eq!(parse_positive(" 12.5 ", "x"), Ok(12.5));
        assert!(
            parse_positive("0", "The period")
                .unwrap_err()
                .contains("The period")
        );
        assert!(parse_positive("-3", "x").is_err());
        assert!(parse_positive("abc", "x").is_err());
        assert!(parse_positive("inf", "x").is_err());
    }

    #[test]
    fn counts_bands() {
        let a = Observation::new(1.0, 1.0, "V");
        let b = Observation::new(2.0, 1.0, "B");
        let c = Observation::new(3.0, 1.0, "V");
        assert_eq!(
            band_counts(&[&a, &b, &c]),
            vec![("V".to_string(), 2), ("B".to_string(), 1)]
        );
    }
}
