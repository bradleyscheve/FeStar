//! Analysis > DC DFT: choose which periods to search (or let the AI), run
//! the search, and show the power spectrum and strongest peaks.

use egui::RichText;
use egui_plot::{Line, MarkerShape, Plot, PlotPoints, Points, VLine};
use vstar_core::ai::AiClient;
use vstar_core::ai::advice::{self, DataSummary};
use vstar_core::analysis::dcdft::{self, Params, Peak, Spectrum};
use vstar_core::http::ReqwestClient;

use crate::app::{Tab, VStarApp};
use crate::phase_ui::band_counts;
use crate::task::Task;
use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMode {
    /// VStar's default scan.
    #[default]
    Standard,
    /// A range of periods typed in by the user.
    PeriodRange,
}

pub struct SearchResult {
    pub params: Params,
    pub spectrum: Spectrum,
    pub hits: Vec<Peak>,
    /// How many observations the search used.
    pub n_used: usize,
    /// A description of the settings, with the AI's reasoning if it chose them.
    pub summary: String,
}

pub struct SearchState {
    pub dialog_open: bool,
    mode: SearchMode,
    min_text: String,
    max_text: String,
    oversample_text: String,
    task: Option<Task<(SearchResult, Option<RangeText>)>>,
    pub result: Option<SearchResult>,
    show_period_axis: bool,
    selected: Option<usize>,
    error: Option<String>,
}

/// Range settings to write back into the text boxes after an AI choice.
struct RangeText {
    min: f64,
    max: f64,
    oversample: f64,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            dialog_open: false,
            mode: SearchMode::Standard,
            min_text: String::new(),
            max_text: String::new(),
            oversample_text: "10".into(),
            task: None,
            result: None,
            show_period_axis: true,
            selected: None,
            error: None,
        }
    }
}

impl SearchState {
    pub fn running(&self) -> Option<(&str, f32)> {
        self.task
            .as_ref()
            .map(|t| (t.description.as_str(), t.progress()))
    }

    pub fn cancel(&self) {
        if let Some(t) = &self.task {
            t.cancel();
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

fn describe(params: &Params) -> String {
    format!(
        "{} frequencies from {:.5} to {:.5} cycles/day (periods {:.3} to {:.3} days)",
        params.count(),
        params.lo_freq,
        params.hi_freq,
        1.0 / params.hi_freq,
        1.0 / params.lo_freq
    )
}

fn run_scan(
    ts: &vstar_core::analysis::TimeSeries,
    params: Params,
    summary_prefix: String,
    h: &crate::task::Handle,
    progress_from: f64,
) -> vstar_core::Result<SearchResult> {
    let spectrum = dcdft::scan(ts, params, &|f| {
        h.report(progress_from + (1.0 - progress_from) * f)
    })?;
    let hits = spectrum.top_hits(dcdft::MAX_TOP_HITS);
    Ok(SearchResult {
        params,
        hits,
        spectrum,
        n_used: ts.len(),
        summary: format!("{summary_prefix}{}", describe(&params)),
    })
}

impl VStarApp {
    pub(crate) fn open_search_dialog(&mut self) {
        let ts = self.analysis_series();
        let s = &mut self.search;
        if s.min_text.is_empty() && ts.len() >= 4 {
            let std = Params::standard(&ts);
            s.min_text = format!("{:.4}", (1.0 / std.hi_freq).max(0.02));
            s.max_text = format!("{:.2}", ts.span().max(1.0));
        }
        s.error = None;
        s.dialog_open = true;
    }

    pub(crate) fn poll_search(&mut self) {
        let Some(result) = self.search.task.as_ref().and_then(Task::poll) else {
            return;
        };
        self.search.task = None;
        match result {
            Ok((result, ranges)) => {
                if let Some(r) = ranges {
                    self.search.mode = SearchMode::PeriodRange;
                    self.search.min_text = format!("{:.4}", r.min);
                    self.search.max_text = format!("{:.2}", r.max);
                    self.search.oversample_text = format!("{:.0}", r.oversample);
                }
                self.search.selected = result.hits.first().map(|_| 0);
                self.search.result = Some(result);
                self.search.error = None;
                self.search.dialog_open = false;
                self.tab = Tab::PeriodSearch;
            }
            Err(vstar_core::Error::Cancelled) => {}
            Err(e) => self.search.error = Some(e.to_string()),
        }
    }

    fn start_search(&mut self, ctx: &egui::Context, use_ai: bool) {
        self.search.error = None;
        let Some(ds) = &self.dataset else { return };
        let ts = self.analysis_series();
        if ts.len() < 4 {
            self.search.error = Some(
                "At least 4 usable observations are needed. Check which bands are shown.".into(),
            );
            return;
        }

        if use_ai {
            let observations = self.analysis_observations();
            let Some(summary) = DataSummary::new(&ds.star, &ts, band_counts(&observations)) else {
                return;
            };
            let (config, key) = match self.require_ai() {
                Ok(v) => v,
                Err(msg) => {
                    self.search.error = Some(msg);
                    return;
                }
            };
            self.search.task = Some(Task::spawn(
                ctx,
                "Asking the AI for a period range, then searching",
                move |h| {
                    let http = ReqwestClient::new()?;
                    let client = AiClient {
                        http: &http,
                        config: &config,
                        api_key: &key,
                    };
                    let advice = advice::suggest_search_settings(&client, &summary, &ts)?;
                    let params = advice.params(&ts)?;
                    h.report(0.1);
                    let prefix = format!("AI chose this range. {} ", advice.rationale);
                    let ranges = RangeText {
                        min: advice.min_period,
                        max: advice.max_period,
                        oversample: advice.oversample,
                    };
                    Ok((run_scan(&ts, params, prefix, h, 0.1)?, Some(ranges)))
                },
            ));
            return;
        }

        let params = match self.search.mode {
            SearchMode::Standard => Ok(Params::standard(&ts)),
            SearchMode::PeriodRange => {
                let num = |t: &str, what: &str| {
                    t.trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite() && *v > 0.0)
                        .ok_or_else(|| format!("{what} must be a positive number."))
                };
                num(&self.search.min_text, "The shortest period").and_then(|min| {
                    let max = num(&self.search.max_text, "The longest period")?;
                    let over = num(&self.search.oversample_text, "Oversampling")?;
                    Params::from_period_range(&ts, min, max, over).map_err(|e| e.to_string())
                })
            }
        };
        match params {
            Ok(params) => {
                let label = match self.search.mode {
                    SearchMode::Standard => "Standard scan: ",
                    SearchMode::PeriodRange => "Period range: ",
                };
                self.search.task = Some(Task::spawn(ctx, "Searching for periods", move |h| {
                    Ok((run_scan(&ts, params, label.to_string(), h, 0.0)?, None))
                }));
            }
            Err(msg) => self.search.error = Some(msg),
        }
    }

    pub(crate) fn show_search_dialog(&mut self, ctx: &egui::Context) {
        if !self.search.dialog_open {
            return;
        }
        let used = self.analysis_series().len();
        let total = self.dataset.as_ref().map_or(0, |d| d.observations.len());
        let ai_ready = self.ai_configured();
        let mut open = true;
        let (mut run, mut run_ai, mut open_prefs) = (false, false, false);

        egui::Window::new("DC DFT period search")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .min_width(440.0)
            .show(ctx, |ui| {
                let s = &mut self.search;
                ui.label(
                    "The date-compensated discrete Fourier transform finds periodic signals in unevenly \
                     sampled data. Choose which periods to search.",
                );
                ui.label(RichText::new(format!("Using {used} of {total} observations (shown bands, without fainter-than estimates).")).weak());
                ui.add_space(6.0);

                ui.radio_value(&mut s.mode, SearchMode::Standard, "Standard scan (VStar's default)");
                ui.radio_value(&mut s.mode, SearchMode::PeriodRange, "Period range");
                if s.mode == SearchMode::PeriodRange {
                    egui::Grid::new("range").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                        ui.label("Shortest period (days)");
                        ui.add(egui::TextEdit::singleline(&mut s.min_text).desired_width(120.0));
                        ui.end_row();
                        ui.label("Longest period (days)");
                        ui.add(egui::TextEdit::singleline(&mut s.max_text).desired_width(120.0));
                        ui.end_row();
                        ui.label("Oversampling");
                        ui.add(egui::TextEdit::singleline(&mut s.oversample_text).desired_width(120.0))
                            .on_hover_text("Frequency steps per natural peak width. Higher traces peaks more smoothly but takes longer.");
                        ui.end_row();
                    });
                }

                ui.add_space(6.0);
                ui.separator();
                ui.label(RichText::new("AI").strong());
                ui.label(RichText::new("Let the AI pick the period range (and oversampling) that suits this star and data, then run the search.").weak());
                ui.horizontal(|ui| {
                    let busy = s.task.is_some();
                    if ui.add_enabled(!busy, egui::Button::new("Let AI choose periods")).clicked() {
                        run_ai = true;
                    }
                    if !ai_ready && ui.link("Connect an AI provider").clicked() {
                        open_prefs = true;
                    }
                });

                if let Some(err) = &s.error {
                    ui.colored_label(ui.visuals().error_fg_color, err);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let busy = s.task.is_some();
                    if ui.add_enabled(!busy, egui::Button::new("Run search").min_size([100.0, 28.0].into())).clicked() {
                        run = true;
                    }
                    if busy {
                        ui.spinner();
                        ui.label("Working…");
                    }
                });
            });

        if open_prefs {
            self.prefs.open = true;
        }
        if run {
            self.start_search(ctx, false);
        }
        if run_ai {
            self.start_search(ctx, true);
        }
        self.search.dialog_open = open;
    }

    /// The period-search tab: spectrum plot and the strongest peaks.
    pub(crate) fn show_search_results(&mut self, ui: &mut egui::Ui) {
        let SearchState {
            result,
            show_period_axis,
            selected,
            ..
        } = &mut self.search;
        let Some(result) = result.as_ref() else {
            return;
        };
        let mut phase_at: Option<f64> = None;

        ui.horizontal(|ui| {
            ui.label(RichText::new("DC DFT power spectrum").strong().size(16.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.selectable_value(&mut *show_period_axis, false, "Frequency");
                ui.selectable_value(&mut *show_period_axis, true, "Period");
                ui.label("Axis:");
            });
        });
        ui.label(
            RichText::new(format!(
                "{} Used {} observations.",
                result.summary, result.n_used
            ))
            .weak(),
        );
        ui.add_space(4.0);

        let by_period = *show_period_axis;
        let x_of = |f: f64| if by_period { 1.0 / f } else { f };
        let line: Vec<[f64; 2]> = result
            .spectrum
            .frequency
            .iter()
            .zip(&result.spectrum.power)
            .map(|(&f, &p)| [x_of(f), p])
            .collect();
        let hit_points: Vec<[f64; 2]> = result
            .hits
            .iter()
            .take(10)
            .map(|h| [x_of(h.frequency), h.power])
            .collect();
        let selected_peak = selected.and_then(|i| result.hits.get(i)).copied();
        let line_color = ui.visuals().strong_text_color();

        let plot_height = (ui.available_height() * 0.5).max(180.0);
        Plot::new("spectrum")
            .height(plot_height)
            .x_axis_label(if by_period {
                "Period (days)"
            } else {
                "Frequency (cycles/day)"
            })
            .y_axis_label("Power")
            // Zooming stretches the frequency axis only; power stays in view.
            .allow_zoom([true, false])
            .allow_scroll([true, false])
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new("Power", PlotPoints::new(line))
                        .color(line_color)
                        .width(1.2),
                );
                plot_ui.points(
                    Points::new("Strongest peaks", PlotPoints::new(hit_points))
                        .shape(MarkerShape::Circle)
                        .radius(3.5)
                        .color(theme::ACCENT),
                );
                if let Some(h) = selected_peak {
                    plot_ui.vline(VLine::new("Selected", x_of(h.frequency)).color(theme::ACCENT));
                }
            });

        ui.add_space(6.0);
        ui.label(RichText::new("Strongest peaks").strong());
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("hits")
                .num_columns(6)
                .striped(true)
                .spacing([18.0, 4.0])
                .show(ui, |ui| {
                    for h in [
                        "#",
                        "Period (days)",
                        "Frequency",
                        "Power",
                        "Semi-amplitude",
                        "",
                    ] {
                        ui.label(RichText::new(h).weak());
                    }
                    ui.end_row();
                    for (i, hit) in result.hits.iter().take(50).enumerate() {
                        let is_selected = *selected == Some(i);
                        if ui
                            .selectable_label(is_selected, (i + 1).to_string())
                            .clicked()
                        {
                            *selected = Some(i);
                        }
                        ui.label(format!("{:.6}", hit.period));
                        ui.label(format!("{:.6}", hit.frequency));
                        ui.label(format!("{:.2}", hit.power));
                        ui.label(format!("{:.4}", hit.amplitude));
                        if ui.small_button("Phase plot").clicked() {
                            phase_at = Some(hit.period);
                        }
                        ui.end_row();
                    }
                });
        });
        if let Some(period) = phase_at {
            self.plot_phase_at(period);
        }
    }
}
