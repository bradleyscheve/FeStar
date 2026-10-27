//! The rest of VStar's Analysis menu: AoV, DFT and spectral window, WWZ, and
//! the model tools (polynomial, Fourier, Loess, piecewise linear).

use egui::{Color32, ColorImage, RichText, TextureHandle};
use egui_plot::{Line, MarkerShape, Plot, PlotImage, PlotPoint, PlotPoints, Points, VLine};
use vstar_core::analysis::models::{self, ModelFit, PeriodTerm};
use vstar_core::analysis::{aov, dcdft, dft, wwz};

use crate::app::{Tab, VStarApp};
use crate::task::Task;
use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Aov,
    Dft,
    WwzFrequency,
    WwzPeriod,
    Polynomial,
    Loess,
    Fourier,
    Piecewise,
}

impl Tool {
    fn title(self) -> &'static str {
        match self {
            Self::Aov => "AoV with period range",
            Self::Dft => "DFT and spectral window",
            Self::WwzFrequency => "WWZ with frequency range",
            Self::WwzPeriod => "WWZ with period range",
            Self::Polynomial => "Polynomial fit",
            Self::Loess => "Loess fit",
            Self::Fourier => "Fourier model",
            Self::Piecewise => "Piecewise linear model from means",
        }
    }
}

/// One row of strongest results; clicking "Phase plot" folds at `period`.
pub struct Hit {
    period: f64,
    cells: Vec<String>,
}

/// A one-dimensional period search result (AoV, DFT and friends).
pub struct PeriodView {
    tab_title: String,
    title: String,
    summary: String,
    periods: Vec<f64>,
    frequencies: Vec<f64>,
    ys: Vec<f64>,
    y_label: String,
    columns: Vec<String>,
    hits: Vec<Hit>,
}

pub struct WwzView {
    summary: String,
    transform: wwz::Transform,
    by_period: bool,
    image: ColorImage,
    texture: Option<TextureHandle>,
}

enum Output {
    Spectrum(Box<PeriodView>),
    Wwz(Box<WwzView>),
    Model(Box<ModelFit>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    Spectrum,
    Wwz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WwzPane {
    Map,
    PeakPeriod,
    Table,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DftKind {
    Dft,
    SpectralWindow,
    DcDft,
    MultiHarmonic,
}

struct Fields {
    // AoV
    aov_min: String,
    aov_max: String,
    aov_step: String,
    aov_bins: String,
    // DFT
    dft_kind: DftKind,
    dft_harmonics: usize,
    dft_min: String,
    dft_max: String,
    dft_step: String,
    // WWZ
    wwz_min: String,
    wwz_max: String,
    wwz_step: String,
    wwz_fmin: String,
    wwz_fmax: String,
    wwz_fstep: String,
    wwz_decay: String,
    wwz_divisions: String,
    // Models
    poly_degree: String,
    loess_bandwidth: String,
    loess_iterations: String,
    fourier_periods: String,
    fourier_harmonics: String,
    piecewise_bin: String,
    initialised: bool,
}

impl Default for Fields {
    fn default() -> Self {
        Self {
            aov_min: String::new(),
            aov_max: String::new(),
            aov_step: String::new(),
            aov_bins: "10".into(),
            dft_kind: DftKind::Dft,
            dft_harmonics: 2,
            dft_min: String::new(),
            dft_max: String::new(),
            dft_step: String::new(),
            wwz_min: String::new(),
            wwz_max: String::new(),
            wwz_step: String::new(),
            wwz_fmin: String::new(),
            wwz_fmax: String::new(),
            wwz_fstep: String::new(),
            wwz_decay: wwz::DEFAULT_DECAY.to_string(),
            wwz_divisions: wwz::DEFAULT_TIME_DIVISIONS.to_string(),
            poly_degree: "3".into(),
            loess_bandwidth: models::LOESS_BANDWIDTH.to_string(),
            loess_iterations: models::LOESS_ROBUSTNESS_ITERATIONS.to_string(),
            fourier_periods: String::new(),
            fourier_harmonics: "1".into(),
            piecewise_bin: models::DEFAULT_BIN_DAYS.to_string(),
            initialised: false,
        }
    }
}

pub struct ToolsState {
    pub dialog: Option<Tool>,
    fields: Fields,
    task: Option<Task<Output>>,
    shown: Shown,
    spectrum: Option<PeriodView>,
    wwz: Option<WwzView>,
    wwz_pane: WwzPane,
    by_period: bool,
    selected: Option<usize>,
    /// Every model fitted for this star, newest last.
    pub models: Vec<ModelFit>,
    pub active_model: Option<usize>,
    pub models_dialog: bool,
    pub phase_history_dialog: bool,
    error: Option<String>,
}

impl Default for ToolsState {
    fn default() -> Self {
        Self {
            dialog: None,
            fields: Fields::default(),
            task: None,
            shown: Shown::Spectrum,
            spectrum: None,
            wwz: None,
            wwz_pane: WwzPane::Map,
            by_period: true,
            selected: None,
            models: Vec::new(),
            active_model: None,
            models_dialog: false,
            phase_history_dialog: false,
            error: None,
        }
    }
}

impl ToolsState {
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

    pub fn has_result(&self) -> bool {
        self.spectrum.is_some() || self.wwz.is_some()
    }

    pub fn tab_title(&self) -> &str {
        match self.shown {
            Shown::Spectrum => self.spectrum.as_ref().map_or("Result", |s| &s.tab_title),
            Shown::Wwz => "WWZ",
        }
    }

    pub fn active_model(&self) -> Option<&ModelFit> {
        self.active_model.and_then(|i| self.models.get(i))
    }
}

// ------------------------------------------------------------------ helpers

fn num(text: &str, what: &str) -> Result<f64, String> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("{what} must be a number."))
}

fn positive(text: &str, what: &str) -> Result<f64, String> {
    num(text, what).and_then(|v| {
        if v > 0.0 {
            Ok(v)
        } else {
            Err(format!("{what} must be greater than zero."))
        }
    })
}

fn whole(text: &str, what: &str) -> Result<usize, String> {
    text.trim()
        .parse::<usize>()
        .map_err(|_| format!("{what} must be a whole number."))
}

/// A short number: `x` with about four significant digits.
fn short(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    let digits = (3 - x.abs().log10().floor() as i32).clamp(0, 8) as usize;
    format!("{x:.digits$}")
}

fn parse_terms(periods: &str, default_harmonics: usize) -> Result<Vec<PeriodTerm>, String> {
    let mut terms = Vec::new();
    for part in periods
        .split([',', ';', ' ', '\n'])
        .filter(|p| !p.trim().is_empty())
    {
        let (p, h) = match part.split_once(':') {
            Some((p, h)) => (p, Some(h)),
            None => (part, None),
        };
        let period = positive(p, &format!("The period \"{p}\""))?;
        let harmonics = match h {
            Some(h) => whole(h, &format!("The harmonic count \"{h}\""))?,
            None => default_harmonics,
        };
        terms.push(PeriodTerm { period, harmonics });
    }
    if terms.is_empty() {
        return Err("Enter at least one period.".into());
    }
    Ok(terms)
}

fn viridis_like(t: f64) -> Color32 {
    const STOPS: [(f64, [f64; 3]); 5] = [
        (0.0, [4.0, 4.0, 20.0]),
        (0.25, [80.0, 18.0, 123.0]),
        (0.5, [182.0, 54.0, 121.0]),
        (0.75, [251.0, 136.0, 97.0]),
        (1.0, [252.0, 253.0, 191.0]),
    ];
    let t = t.clamp(0.0, 1.0);
    for w in STOPS.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if t <= b {
            let f = (t - a) / (b - a);
            let c = |i: usize| (ca[i] + (cb[i] - ca[i]) * f) as u8;
            return Color32::from_rgb(c(0), c(1), c(2));
        }
    }
    Color32::WHITE
}

fn wwz_image(t: &wwz::Transform) -> ColorImage {
    let (w, h) = (t.taus.len(), t.frequencies.len());
    let max = t.stats.iter().map(|s| s.wwz).fold(0.0, f64::max).max(1e-12);
    let mut pixels = vec![Color32::BLACK; w * h];
    // The first row of the picture is the top, so the lowest value on the
    // vertical axis (the first frequency step, or the shortest period)
    // goes in the last row.
    for ti in 0..w {
        for (fi, s) in t.row(ti).iter().enumerate() {
            pixels[(h - 1 - fi) * w + ti] = viridis_like((s.wwz / max).powf(0.7));
        }
    }
    ColorImage {
        size: [w, h],
        source_size: egui::vec2(w as f32, h as f32),
        pixels,
    }
}

// ------------------------------------------------------------------- running

impl VStarApp {
    pub(crate) fn open_tool(&mut self, tool: Tool) {
        let ts = self.analysis_series();
        let t = &mut self.tools;
        if !t.fields.initialised && ts.len() >= 4 {
            let f = &mut t.fields;
            let span = ts.span().max(1.0);
            let cadence = ts.median_cadence().unwrap_or(0.1).max(0.01);
            let min_p = (2.0 * cadence).max(span / 5000.0);
            let max_p = (span / 2.0).max(min_p * 2.0);
            f.aov_min = short(min_p);
            f.aov_max = short(max_p);
            f.aov_step = short(((max_p - min_p) / 2000.0).max(1e-6));
            let std = dcdft::Params::standard(&ts);
            f.dft_min = format!("{:.6}", 1.0 / span);
            f.dft_max = format!("{:.6}", std.hi_freq.min(0.5 / cadence));
            f.dft_step = format!("{:.7}", 0.05 / span);
            f.wwz_min = short(min_p);
            f.wwz_max = short((span / 3.0).max(min_p * 2.0));
            f.wwz_step = short(((span / 3.0 - min_p) / 100.0).max(1e-6));
            let max_wwz_p = (span / 3.0).max(min_p * 2.0);
            f.wwz_fmin = format!("{:.6}", 1.0 / max_wwz_p);
            f.wwz_fmax = format!("{:.6}", 1.0 / min_p);
            f.wwz_fstep = format!("{:.6}", (1.0 / min_p - 1.0 / max_wwz_p) / 100.0);
            f.initialised = true;
        }
        if tool == Tool::Fourier && t.fields.fourier_periods.is_empty() {
            let guess = self.search.result.as_ref().map(|r| {
                r.hits
                    .iter()
                    .take(1)
                    .map(|h| short(h.period))
                    .collect::<Vec<_>>()
                    .join(", ")
            });
            let catalogue = self.dataset.as_ref().and_then(|d| d.star.period_days);
            self.tools.fields.fourier_periods = guess
                .filter(|g| !g.is_empty())
                .or(catalogue.map(short))
                .unwrap_or_default();
        }
        self.tools.error = None;
        self.tools.dialog = Some(tool);
    }

    pub(crate) fn poll_tools(&mut self, ctx: &egui::Context) {
        let Some(result) = self.tools.task.as_ref().and_then(Task::poll) else {
            return;
        };
        self.tools.task = None;
        match result {
            Ok(Output::Spectrum(view)) => {
                self.tools.selected = view.hits.first().map(|_| 0);
                self.tools.spectrum = Some(*view);
                self.tools.shown = Shown::Spectrum;
                self.tools.error = None;
                self.tools.dialog = None;
                self.tab = Tab::Tool;
            }
            Ok(Output::Wwz(mut view)) => {
                view.texture = Some(ctx.load_texture(
                    "wwz_map",
                    view.image.clone(),
                    egui::TextureOptions::NEAREST,
                ));
                self.tools.selected = Some(0);
                self.tools.wwz = Some(*view);
                self.tools.shown = Shown::Wwz;
                self.tools.wwz_pane = WwzPane::Map;
                self.tools.error = None;
                self.tools.dialog = None;
                self.tab = Tab::Tool;
            }
            Ok(Output::Model(model)) => {
                self.tools.models.push(*model);
                self.tools.active_model = Some(self.tools.models.len() - 1);
                self.tools.error = None;
                self.tools.dialog = None;
                self.tab = Tab::Model;
                self.reset_view = true;
            }
            Err(vstar_core::Error::Cancelled) => {}
            Err(e) => self.tools.error = Some(e.to_string()),
        }
    }

    fn run_tool(&mut self, ctx: &egui::Context, tool: Tool) {
        self.tools.error = None;
        let ts = self.analysis_series();
        if ts.len() < 4 {
            self.tools.error = Some(
                "At least 4 usable observations are needed. Check which bands are shown.".into(),
            );
            return;
        }
        let f = &self.tools.fields;
        let n_used = ts.len();
        let started: Result<Task<Output>, String> = (|| match tool {
            Tool::Aov => {
                let params = aov::Params {
                    min_period: positive(&f.aov_min, "The minimum period")?,
                    max_period: positive(&f.aov_max, "The maximum period")?,
                    resolution: positive(&f.aov_step, "The resolution")?,
                    bins: whole(&f.aov_bins, "The number of bins")?,
                }
                .validated()
                .map_err(|e| e.to_string())?;
                Ok(Task::spawn(ctx, "AoV period search", move |h| {
                    let pg = aov::scan(&ts, params, &|x| h.report(x))?;
                    Ok(Output::Spectrum(Box::new(aov_view(pg, params, n_used))))
                }))
            }
            Tool::Dft => {
                let kind = match f.dft_kind {
                    DftKind::Dft => dft::Kind::Dft,
                    DftKind::SpectralWindow => dft::Kind::SpectralWindow,
                    DftKind::DcDft => dft::Kind::DcDft,
                    DftKind::MultiHarmonic => dft::Kind::MultiHarmonic(f.dft_harmonics),
                };
                let params = dft::Params {
                    min_frequency: num(&f.dft_min, "The minimum frequency")?,
                    max_frequency: num(&f.dft_max, "The maximum frequency")?,
                    resolution: positive(&f.dft_step, "The resolution")?,
                    kind,
                }
                .validated()
                .map_err(|e| e.to_string())?;
                Ok(Task::spawn(ctx, kind.label(), move |h| {
                    let s = dft::scan(&ts, params, &|x| h.report(x))?;
                    Ok(Output::Spectrum(Box::new(dft_view(s, params, n_used))))
                }))
            }
            Tool::WwzFrequency | Tool::WwzPeriod => {
                let by_period = tool == Tool::WwzPeriod;
                let (a, b, c) = if by_period {
                    (&f.wwz_min, &f.wwz_max, &f.wwz_step)
                } else {
                    (&f.wwz_fmin, &f.wwz_fmax, &f.wwz_fstep)
                };
                let (min, max, step) = (
                    positive(a, "The minimum")?,
                    positive(b, "The maximum")?,
                    positive(c, "The step")?,
                );
                let (min, max) = (min.min(max), min.max(max));
                let params = wwz::Params {
                    range: if by_period {
                        wwz::Range::Period { min, max, step }
                    } else {
                        wwz::Range::Frequency { min, max, step }
                    },
                    decay: num(&f.wwz_decay, "The decay constant")?,
                    time_divisions: num(&f.wwz_divisions, "The time divisions")?,
                };
                Ok(Task::spawn(ctx, "WWZ", move |h| {
                    let t = wwz::transform(&ts, params, &|x| h.report(x))?;
                    let image = wwz_image(&t);
                    Ok(Output::Wwz(Box::new(WwzView {
                        summary: format!(
                            "{} times by {} {}s, decay {}, {n_used} observations.",
                            t.taus.len(),
                            t.frequencies.len(),
                            if by_period { "period" } else { "frequency" },
                            params.decay
                        ),
                        transform: t,
                        by_period,
                        image,
                        texture: None,
                    })))
                }))
            }
            Tool::Polynomial => {
                let degree = whole(&f.poly_degree, "The degree")?;
                Ok(Task::spawn(ctx, "Polynomial fit", move |_| {
                    Ok(Output::Model(Box::new(models::polynomial_fit(
                        &ts, degree,
                    )?)))
                }))
            }
            Tool::Loess => {
                let bandwidth = positive(&f.loess_bandwidth, "The bandwidth")?;
                let iterations = whole(&f.loess_iterations, "The robustness iterations")?;
                Ok(Task::spawn(ctx, "Loess fit", move |_| {
                    Ok(Output::Model(Box::new(models::loess_fit(
                        &ts, bandwidth, iterations,
                    )?)))
                }))
            }
            Tool::Fourier => {
                let default_h = whole(&f.fourier_harmonics, "The harmonic count")?;
                let terms = parse_terms(&f.fourier_periods, default_h)?;
                Ok(Task::spawn(ctx, "Fourier model", move |_| {
                    Ok(Output::Model(Box::new(models::fourier_fit(&ts, &terms)?)))
                }))
            }
            Tool::Piecewise => {
                let bin = positive(&f.piecewise_bin, "The bin size")?;
                Ok(Task::spawn(ctx, "Piecewise linear model", move |_| {
                    Ok(Output::Model(Box::new(models::piecewise_linear_fit(
                        &ts, bin,
                    )?)))
                }))
            }
        })();
        match started {
            Ok(task) => self.tools.task = Some(task),
            Err(msg) => self.tools.error = Some(msg),
        }
    }

    /// Fill the Fourier period box from the strongest DC DFT peaks.
    fn fourier_from_peaks(&mut self) {
        let ts = self.analysis_series();
        if ts.len() < 4 {
            return;
        }
        let hits = match &self.search.result {
            Some(r) if !r.hits.is_empty() => r.hits.iter().map(|h| h.period).collect::<Vec<_>>(),
            _ => {
                let quick = ts.thinned(5000);
                match dcdft::scan(&quick, dcdft::Params::standard(&quick), &|_| true) {
                    Ok(s) => s.top_hits(10).iter().map(|h| h.period).collect(),
                    Err(e) => {
                        self.tools.error = Some(e.to_string());
                        return;
                    }
                }
            }
        };
        // Strongest peaks that aren't just aliases of one another.
        let mut chosen: Vec<f64> = Vec::new();
        for p in hits {
            if chosen.iter().all(|c| (c - p).abs() / c.max(p) > 0.02) {
                chosen.push(p);
            }
            if chosen.len() == 3 {
                break;
            }
        }
        self.tools.fields.fourier_periods = chosen
            .iter()
            .map(|p| short(*p))
            .collect::<Vec<_>>()
            .join(", ");
    }

    // ------------------------------------------------------------- dialogs

    pub(crate) fn show_tool_dialog(&mut self, ctx: &egui::Context) {
        let Some(tool) = self.tools.dialog else {
            return;
        };
        let used = self.analysis_series().len();
        let total = self.dataset.as_ref().map_or(0, |d| d.observations.len());
        let span = self.analysis_series().span();
        let busy = self.tools.task.is_some();
        let mut open = true;
        let (mut run, mut from_peaks) = (false, false);

        egui::Window::new(tool.title())
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .min_width(420.0)
            .show(ctx, |ui| {
                let t = &mut self.tools;
                ui.label(
                    RichText::new(format!(
                        "Using {used} of {total} observations (shown bands, without fainter-than estimates)."
                    ))
                    .weak(),
                );
                ui.add_space(4.0);
                let f = &mut t.fields;
                let field = |ui: &mut egui::Ui, label: &str, text: &mut String, hint: &str| {
                    ui.label(label);
                    ui.add(egui::TextEdit::singleline(text).desired_width(120.0))
                        .on_hover_text(hint);
                    ui.end_row();
                };
                egui::Grid::new("tool_fields")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| match tool {
                        Tool::Aov => {
                            field(ui, "Minimum period (days)", &mut f.aov_min, "");
                            field(ui, "Maximum period (days)", &mut f.aov_max, "");
                            field(ui, "Resolution (days)", &mut f.aov_step, "The step between trial periods.");
                            field(ui, "Bins", &mut f.aov_bins, "Phase bins, 1 to 50.");
                        }
                        Tool::Dft => {
                            ui.label("Analysis type");
                            ui.vertical(|ui| {
                                ui.radio_value(&mut f.dft_kind, DftKind::Dft, "DFT (Deeming 1975)");
                                ui.radio_value(&mut f.dft_kind, DftKind::SpectralWindow, "Spectral window (Deeming 1975)");
                                ui.radio_value(&mut f.dft_kind, DftKind::DcDft, "DC DFT (Ferraz-Mello 1981)");
                                ui.horizontal(|ui| {
                                    ui.radio_value(&mut f.dft_kind, DftKind::MultiHarmonic, "Multi-harmonic DFT (Andronov 1994)");
                                });
                                if f.dft_kind == DftKind::MultiHarmonic {
                                    ui.horizontal(|ui| {
                                        ui.label("Harmonics");
                                        egui::ComboBox::from_id_salt("harmonics")
                                            .selected_text(f.dft_harmonics.to_string())
                                            .show_ui(ui, |ui| {
                                                for h in 1..=dft::MAX_HARMONICS {
                                                    ui.selectable_value(&mut f.dft_harmonics, h, h.to_string());
                                                }
                                            });
                                    });
                                }
                            });
                            ui.end_row();
                            field(ui, "Minimum frequency", &mut f.dft_min, "Cycles per day.");
                            field(ui, "Maximum frequency", &mut f.dft_max, "Cycles per day.");
                            field(ui, "Resolution", &mut f.dft_step, "Frequency step, cycles per day.");
                            ui.label("");
                            if ui.button("Recommended resolution").clicked() && span > 0.0 {
                                let h = if f.dft_kind == DftKind::MultiHarmonic { f.dft_harmonics } else { 1 };
                                if let Some(r) = dft::Params::recommended_resolution(span, h) {
                                    f.dft_step = format!("{r:.7}");
                                }
                            }
                            ui.end_row();
                        }
                        Tool::WwzFrequency | Tool::WwzPeriod => {
                            let (unit, name) = if tool == Tool::WwzPeriod { ("days", "period") } else { ("cycles/day", "frequency") };
                            let (a, b, c) = if tool == Tool::WwzPeriod {
                                (&mut f.wwz_min, &mut f.wwz_max, &mut f.wwz_step)
                            } else {
                                (&mut f.wwz_fmin, &mut f.wwz_fmax, &mut f.wwz_fstep)
                            };
                            field(ui, &format!("Minimum {name} ({unit})"), a, "");
                            field(ui, &format!("Maximum {name} ({unit})"), b, "");
                            field(ui, &format!("{} step", name[..1].to_uppercase() + &name[1..]), c, "");
                            field(ui, "Decay constant", &mut f.wwz_decay, "Width of the sliding window: larger values give a narrower window.");
                            field(ui, "Time divisions", &mut f.wwz_divisions, "About how many time steps to compute across the data.");
                        }
                        Tool::Polynomial => {
                            field(ui, "Degree (0 to 30)", &mut f.poly_degree, "");
                        }
                        Tool::Loess => {
                            field(ui, "Bandwidth", &mut f.loess_bandwidth, "Fraction of the data used for each local fit (VStar uses 0.3).");
                            field(ui, "Robustness iterations", &mut f.loess_iterations, "VStar uses 2.");
                        }
                        Tool::Fourier => {
                            field(ui, "Periods (days)", &mut f.fourier_periods, "Separate with commas. Write 12.5:3 to use 3 harmonics for that period.");
                            field(ui, "Harmonics per period", &mut f.fourier_harmonics, "Used for periods without their own count.");
                            ui.label("");
                            if ui.button("Use strongest DC DFT peaks").clicked() {
                                from_peaks = true;
                            }
                            ui.end_row();
                        }
                        Tool::Piecewise => {
                            field(ui, "Bin size (days)", &mut f.piecewise_bin, "The means are taken over bins this wide (VStar uses 20).");
                        }
                    });
                if let Some(err) = &t.error {
                    ui.add_space(4.0);
                    ui.colored_label(ui.visuals().error_fg_color, err);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!busy, egui::Button::new("Run").min_size([100.0, 28.0].into()))
                        .clicked()
                    {
                        run = true;
                    }
                    if busy {
                        ui.spinner();
                        ui.label("Working…");
                    }
                });
            });
        if from_peaks {
            self.fourier_from_peaks();
        }
        if run {
            self.run_tool(ctx, tool);
        }
        if !open {
            self.tools.dialog = None;
        }
    }

    pub(crate) fn show_history_dialogs(&mut self, ctx: &egui::Context) {
        if self.tools.models_dialog {
            let mut open = true;
            let mut show: Option<usize> = None;
            let mut delete: Option<usize> = None;
            egui::Window::new("Models")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .min_width(420.0)
                .show(ctx, |ui| {
                    if self.tools.models.is_empty() {
                        ui.label("No models yet. Create one from the Analysis menu (polynomial, Loess, Fourier or piecewise linear).");
                    }
                    for (i, m) in self.tools.models.iter().enumerate().rev() {
                        ui.horizontal(|ui| {
                            let active = self.tools.active_model == Some(i);
                            if ui.selectable_label(active, &m.description).clicked() {
                                show = Some(i);
                            }
                            ui.label(RichText::new(format!("RMS {}", short(m.rms))).weak());
                            if ui.small_button("Delete").clicked() {
                                delete = Some(i);
                            }
                        });
                    }
                });
            if let Some(i) = show {
                self.tools.active_model = Some(i);
                self.tab = Tab::Model;
                self.reset_view = true;
            }
            if let Some(i) = delete {
                self.tools.models.remove(i);
                self.tools.active_model = match self.tools.active_model {
                    Some(a) if a == i => None,
                    Some(a) if a > i => Some(a - 1),
                    other => other,
                };
                if self.tab == Tab::Model && self.tools.active_model.is_none() {
                    self.tab = Tab::LightCurve;
                }
            }
            self.tools.models_dialog = open;
        }

        if self.tools.phase_history_dialog {
            let mut open = true;
            let mut plot: Option<(f64, f64)> = None;
            egui::Window::new("Previous phase plots")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .min_width(360.0)
                .show(ctx, |ui| {
                    if self.phase.history.is_empty() {
                        ui.label("No phase plots yet for this star.");
                    }
                    for (period, epoch) in self.phase.history.iter().rev() {
                        ui.horizontal(|ui| {
                            ui.label(format!("Period {period} d, epoch JD {epoch:.4}"));
                            if ui.small_button("Plot").clicked() {
                                plot = Some((*period, *epoch));
                            }
                        });
                    }
                });
            if let Some((period, epoch)) = plot {
                self.phase.period_text = period.to_string();
                self.phase.epoch_text = format!("{epoch:.4}");
                self.phase.active = Some((period, epoch));
                self.tab = Tab::PhasePlot;
                self.reset_view = true;
            }
            self.tools.phase_history_dialog = open;
        }
    }

    // ------------------------------------------------------------- results

    /// The tab for the latest AoV, DFT or WWZ result.
    pub(crate) fn show_tool_results(&mut self, ui: &mut egui::Ui) {
        let mut phase_at: Option<f64> = None;
        match self.tools.shown {
            Shown::Spectrum => {
                let ToolsState {
                    spectrum,
                    by_period,
                    selected,
                    ..
                } = &mut self.tools;
                if let Some(view) = spectrum {
                    show_spectrum(ui, view, by_period, selected, &mut phase_at);
                }
            }
            Shown::Wwz => {
                let ToolsState {
                    wwz,
                    wwz_pane,
                    selected,
                    ..
                } = &mut self.tools;
                if let Some(view) = wwz {
                    show_wwz(ui, view, wwz_pane, selected, &mut phase_at);
                }
            }
        }
        if let Some(period) = phase_at {
            self.plot_phase_at(period);
        }
    }

    /// The model tab: the light curve with the model drawn over it, and the
    /// residuals below.
    pub(crate) fn show_model_tab(&mut self, ui: &mut egui::Ui) {
        let dot_color = ui.visuals().strong_text_color();
        let Some(model) = self.tools.active_model().cloned() else {
            self.tab = Tab::LightCurve;
            return;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(&model.description).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Hide model").clicked() {
                    self.tools.active_model = None;
                    self.tab = Tab::LightCurve;
                }
            });
        });
        let mut line = format!("RMS {}", short(model.rms));
        if model.aic.is_finite() && model.bic.is_finite() {
            line.push_str(&format!(
                "   AIC {}   BIC {}",
                short(model.aic),
                short(model.bic)
            ));
        }
        ui.label(RichText::new(line).weak());
        for note in &model.notes {
            ui.label(RichText::new(note).weak());
        }
        ui.add_space(4.0);

        let total = ui.available_height();
        let width = ui.available_width();
        ui.allocate_ui(egui::vec2(width, (total * 0.66).max(200.0)), |ui| {
            self.show_plot(ui, crate::light_curve::Mode::Time);
        });
        ui.add_space(4.0);
        ui.label(RichText::new("Residuals (observation minus model)").weak());
        let pts: Vec<[f64; 2]> = model
            .times
            .iter()
            .zip(&model.residuals)
            .map(|(&t, &r)| [t, r])
            .collect();
        Plot::new("residuals")
            .invert_y(true)
            .x_axis_label("Julian Date")
            .y_axis_label("Residual (mag)")
            .link_axis(egui::Id::unique("model_x"), [true, false])
            .link_cursor(egui::Id::unique("model_x"), [true, false])
            .show(ui, |plot_ui| {
                plot_ui.points(
                    Points::new("Residuals", PlotPoints::new(pts))
                        .shape(MarkerShape::Circle)
                        .radius(2.0)
                        .color(dot_color),
                );
                plot_ui.hline(egui_plot::HLine::new("Zero", 0.0).color(theme::ACCENT));
            });
    }
}

fn p_text(p: f64) -> String {
    if p == 0.0 {
        "0".into()
    } else if p.is_nan() {
        "n/a".into()
    } else {
        format!("{p:.3e}")
    }
}

fn aov_view(pg: aov::Periodogram, params: aov::Params, n_used: usize) -> PeriodView {
    let hits = pg
        .top_hits(aov::MAX_TOP_HITS)
        .into_iter()
        .map(|p| Hit {
            period: p.period,
            cells: vec![
                format!("{:.6}", p.period),
                format!("{:.6}", p.frequency),
                format!("{:.4}", p.f_statistic),
                p_text(p.p_value),
            ],
        })
        .collect();
    PeriodView {
        tab_title: "AoV".into(),
        title: "AoV periodogram".into(),
        summary: format!(
            "{} periods from {} to {} days in steps of {}, {} bins. Used {n_used} observations.",
            params.count(),
            params.min_period,
            params.max_period,
            params.resolution,
            params.bins
        ),
        periods: pg.points.iter().map(|p| p.period).collect(),
        frequencies: pg.points.iter().map(|p| p.frequency).collect(),
        ys: pg.points.iter().map(|p| p.f_statistic).collect(),
        y_label: "F statistic".into(),
        columns: ["Period (days)", "Frequency", "F statistic", "p-value"]
            .map(String::from)
            .to_vec(),
        hits,
    }
}

fn dft_view(s: dft::Spectrum, params: dft::Params, n_used: usize) -> PeriodView {
    let label = params.kind.label();
    let window = params.kind == dft::Kind::SpectralWindow;
    let hits = s
        .top_hits(100)
        .into_iter()
        .map(|p| Hit {
            period: p.period,
            cells: vec![
                format!("{:.6}", p.period),
                format!("{:.6}", p.frequency),
                format!("{:.4}", p.power),
                format!("{:.4}", p.semi_amplitude),
            ],
        })
        .collect();
    PeriodView {
        tab_title: if window { "Spectral window" } else { label }.into(),
        title: format!("{label} power spectrum"),
        summary: format!(
            "{} frequencies from {} to {} cycles/day in steps of {}. Used {n_used} observations.",
            params.count(),
            params.min_frequency,
            params.max_frequency,
            params.resolution
        ),
        periods: s.points.iter().map(|p| p.period).collect(),
        frequencies: s.points.iter().map(|p| p.frequency).collect(),
        ys: s.points.iter().map(|p| p.power).collect(),
        y_label: "Power".into(),
        columns: ["Period (days)", "Frequency", "Power", "Semi-amplitude"]
            .map(String::from)
            .to_vec(),
        hits,
    }
}

fn show_spectrum(
    ui: &mut egui::Ui,
    view: &PeriodView,
    by_period: &mut bool,
    selected: &mut Option<usize>,
    phase_at: &mut Option<f64>,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(&view.title).strong().size(16.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.selectable_value(by_period, false, "Frequency");
            ui.selectable_value(by_period, true, "Period");
            ui.label("Axis:");
        });
    });
    ui.label(RichText::new(&view.summary).weak());
    ui.add_space(4.0);

    let by = *by_period;
    let xs = if by { &view.periods } else { &view.frequencies };
    let line: Vec<[f64; 2]> = xs
        .iter()
        .zip(&view.ys)
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .map(|(&x, &y)| [x, y])
        .collect();
    let hit_x = |h: &Hit| if by { h.period } else { 1.0 / h.period };
    let y_at = |h: &Hit| {
        view.periods
            .iter()
            .position(|p| (p - h.period).abs() < f64::EPSILON * h.period.abs() * 4.0)
            .and_then(|i| view.ys.get(i).copied())
            .unwrap_or(0.0)
    };
    let hit_points: Vec<[f64; 2]> = view
        .hits
        .iter()
        .take(10)
        .map(|h| [hit_x(h), y_at(h)])
        .collect();
    let line_color = ui.visuals().strong_text_color();
    let height = (ui.available_height() * 0.5).max(180.0);
    Plot::new("tool_spectrum")
        .height(height)
        .x_axis_label(if by {
            "Period (days)"
        } else {
            "Frequency (cycles/day)"
        })
        .y_axis_label(view.y_label.clone())
        .allow_zoom([true, false])
        .allow_scroll([true, false])
        .show(ui, |plot_ui| {
            plot_ui.line(
                Line::new(view.y_label.clone(), PlotPoints::new(line))
                    .color(line_color)
                    .width(1.2),
            );
            plot_ui.points(
                Points::new("Strongest", PlotPoints::new(hit_points))
                    .shape(MarkerShape::Circle)
                    .radius(3.5)
                    .color(theme::ACCENT),
            );
            if let Some(h) = selected.and_then(|i| view.hits.get(i)) {
                plot_ui.vline(VLine::new("Selected", hit_x(h)).color(theme::ACCENT));
            }
        });

    ui.add_space(6.0);
    ui.label(RichText::new("Strongest results").strong());
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("tool_hits")
            .num_columns(view.columns.len() + 2)
            .striped(true)
            .spacing([18.0, 4.0])
            .show(ui, |ui| {
                ui.label(RichText::new("#").weak());
                for c in &view.columns {
                    ui.label(RichText::new(c).weak());
                }
                ui.label("");
                ui.end_row();
                for (i, hit) in view.hits.iter().take(50).enumerate() {
                    if ui
                        .selectable_label(*selected == Some(i), (i + 1).to_string())
                        .clicked()
                    {
                        *selected = Some(i);
                    }
                    for c in &hit.cells {
                        ui.label(c);
                    }
                    if ui.small_button("Phase plot").clicked() {
                        *phase_at = Some(hit.period);
                    }
                    ui.end_row();
                }
            });
    });
}

fn show_wwz(
    ui: &mut egui::Ui,
    view: &WwzView,
    pane: &mut WwzPane,
    selected: &mut Option<usize>,
    phase_at: &mut Option<f64>,
) {
    let t = &view.transform;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Weighted wavelet Z-transform")
                .strong()
                .size(16.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.selectable_value(pane, WwzPane::Table, "Maxima table");
            ui.selectable_value(pane, WwzPane::PeakPeriod, "Peak over time");
            ui.selectable_value(pane, WwzPane::Map, "Map");
        });
    });
    ui.label(RichText::new(&view.summary).weak());
    ui.add_space(4.0);
    let y_name = if view.by_period {
        "Period (days)"
    } else {
        "Frequency (cycles/day)"
    };

    match *pane {
        WwzPane::Map => {
            if let Some(tex) = &view.texture {
                let (w, h) = (t.taus.len(), t.frequencies.len());
                let tau_step = if w > 1 { t.taus[1] - t.taus[0] } else { 1.0 };
                let ys: Vec<f64> = t
                    .frequencies
                    .iter()
                    .map(|f| if view.by_period { 1.0 / f } else { *f })
                    .collect();
                let (y_lo, y_hi) = (
                    ys.iter().copied().fold(f64::INFINITY, f64::min),
                    ys.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                );
                let y_step = if h > 1 {
                    (y_hi - y_lo) / (h - 1) as f64
                } else {
                    1.0
                };
                let (x0, x1) = (t.taus[0] - tau_step / 2.0, t.taus[w - 1] + tau_step / 2.0);
                let (y0, y1) = (y_lo - y_step / 2.0, y_hi + y_step / 2.0);
                let center = PlotPoint::new((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                let size = egui::vec2((x1 - x0) as f32, (y1 - y0) as f32);
                let image = PlotImage::new("WWZ", tex.id(), center, size);
                let maxima: Vec<[f64; 2]> = t
                    .maxima
                    .iter()
                    .map(|s| {
                        [
                            s.tau,
                            if view.by_period {
                                s.period()
                            } else {
                                s.frequency
                            },
                        ]
                    })
                    .collect();
                Plot::new("wwz_map")
                    .x_axis_label("Julian Date (time centre of the window)")
                    .y_axis_label(y_name)
                    .show(ui, |plot_ui| {
                        plot_ui.image(image);
                        plot_ui.points(
                            Points::new("Strongest at each time", PlotPoints::new(maxima))
                                .shape(MarkerShape::Circle)
                                .radius(2.5)
                                .color(Color32::WHITE),
                        );
                    });
                ui.label(RichText::new("Brighter means a stronger signal. White dots mark the strongest period at each time.").weak());
            }
        }
        WwzPane::PeakPeriod => {
            let pts: Vec<[f64; 2]> = t
                .maxima
                .iter()
                .map(|s| {
                    [
                        s.tau,
                        if view.by_period {
                            s.period()
                        } else {
                            s.frequency
                        },
                    ]
                })
                .collect();
            let line_color = ui.visuals().strong_text_color();
            Plot::new("wwz_peak")
                .x_axis_label("Julian Date")
                .y_axis_label(y_name)
                .show(ui, |plot_ui| {
                    plot_ui.line(
                        Line::new("Strongest", PlotPoints::new(pts.clone())).color(line_color),
                    );
                    plot_ui.points(
                        Points::new("Strongest", PlotPoints::new(pts))
                            .radius(3.0)
                            .color(theme::ACCENT),
                    );
                });
        }
        WwzPane::Table => {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("wwz_table")
                    .num_columns(8)
                    .striped(true)
                    .spacing([18.0, 4.0])
                    .show(ui, |ui| {
                        for h in [
                            "#",
                            "Time (JD)",
                            "Period (days)",
                            "Frequency",
                            "WWZ",
                            "Semi-amplitude",
                            "Mean mag",
                            "N eff",
                            "",
                        ] {
                            ui.label(RichText::new(h).weak());
                        }
                        ui.end_row();
                        for (i, s) in t.maxima.iter().enumerate() {
                            if ui
                                .selectable_label(*selected == Some(i), (i + 1).to_string())
                                .clicked()
                            {
                                *selected = Some(i);
                            }
                            ui.label(format!("{:.3}", s.tau));
                            ui.label(format!("{:.5}", s.period()));
                            ui.label(format!("{:.6}", s.frequency));
                            ui.label(format!("{:.4}", s.wwz));
                            ui.label(format!("{:.4}", s.semi_amplitude));
                            ui.label(format!("{:.4}", s.mean_mag));
                            ui.label(format!("{:.2}", s.neff));
                            if ui.small_button("Phase plot").clicked() {
                                *phase_at = Some(s.period());
                            }
                            ui.end_row();
                        }
                    });
            });
        }
    }
}
