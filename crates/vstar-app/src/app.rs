use std::collections::HashSet;

use egui::{Align, Layout, RichText};
use serde::{Deserialize, Serialize};
use vstar_core::ai::AiConfig;
use vstar_core::analysis::TimeSeries;
use vstar_core::sources::aavso;
use vstar_core::{Dataset, Observation, StarQuery, Validation, bands, time};

use crate::loader::{Job, Request};
use crate::phase_ui::PhaseState;
use crate::prefs::{PrefsState, Secrets};
use crate::search_ui::SearchState;
use crate::vsp_ui::VspState;
use crate::{light_curve, theme};

const BAA_DATABASE_URL: &str = "https://britastro.org/vssdb/data.php";
const MAX_RECENT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    Aavso,
    BaaFile,
    OtherFile,
    Gaia,
}

impl SourceKind {
    fn label(self) -> &'static str {
        match self {
            SourceKind::Aavso => "AAVSO International Database",
            SourceKind::BaaFile => "BAA VSS database (downloaded file)",
            SourceKind::OtherFile => "Other file (CSV, text)",
            SourceKind::Gaia => "Gaia DR3 (coming in step 3)",
        }
    }
}

/// Which view fills the main area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    LightCurve,
    PhasePlot,
    PeriodSearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeChoice {
    System,
    Dark,
    Light,
}

/// Remembered between runs.
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    source: SourceKind,
    star_name: String,
    limit_dates: bool,
    /// The date ranges are not remembered: each run starts with the past
    /// year (see `default_ranges`).
    #[serde(skip)]
    min_jd: String,
    #[serde(skip)]
    max_jd: String,
    limit_calendar: bool,
    #[serde(skip)]
    min_date: String,
    #[serde(skip)]
    max_date: String,
    ai: AiConfig,
    show_errors: bool,
    show_discrepant: bool,
    show_fainter_than: bool,
    theme: ThemeChoice,
    recent_stars: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            source: SourceKind::Aavso,
            star_name: String::new(),
            limit_dates: false,
            min_jd: String::new(),
            max_jd: String::new(),
            limit_calendar: false,
            min_date: String::new(),
            max_date: String::new(),
            ai: AiConfig::default(),
            show_errors: true,
            show_discrepant: false,
            show_fainter_than: true,
            theme: ThemeChoice::System,
            recent_stars: Vec::new(),
        }
    }
}

enum Status {
    Ready,
    Info(String),
    Error(String),
}

/// The four default date-range strings: from-JD, to-JD, from-date, to-date.
/// "From" is exactly one year before now and "to" is now.
fn default_ranges(now_jd: f64) -> [String; 4] {
    let from = time::one_year_before(now_jd);
    [
        format!("{from:.4}"),
        format!("{now_jd:.4}"),
        time::jd_to_date_string(from),
        time::jd_to_date_string(now_jd),
    ]
}

pub struct VStarApp {
    settings: Settings,
    pub(crate) dataset: Option<Dataset>,
    pub(crate) hidden_bands: HashSet<String>,
    job: Option<Job>,
    status: Status,
    pub(crate) reset_view: bool,
    pub(crate) tab: Tab,
    pub(crate) phase: PhaseState,
    pub(crate) search: SearchState,
    pub(crate) prefs: PrefsState,
    vsp: VspState,
    secrets: Secrets,
}

impl VStarApp {
    pub fn new(cc: &eframe::CreationContext<'_>, open_file: Option<std::path::PathBuf>) -> Self {
        theme::install(&cc.egui_ctx);
        let settings: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();
        apply_theme(&cc.egui_ctx, settings.theme);
        let job = open_file.map(|path| {
            let request = Request::File {
                path,
                source_label: "File".into(),
            };
            Job::start(request, cc.egui_ctx.clone())
        });
        let mut settings = settings;
        [
            settings.min_jd,
            settings.max_jd,
            settings.min_date,
            settings.max_date,
        ] = default_ranges(time::now_jd());
        Self {
            settings,
            dataset: None,
            hidden_bands: HashSet::new(),
            job,
            status: Status::Ready,
            reset_view: false,
            tab: Tab::LightCurve,
            phase: PhaseState::default(),
            search: SearchState::default(),
            prefs: PrefsState::default(),
            vsp: VspState::default(),
            secrets: Secrets::new(),
        }
    }

    /// The observations that analysis tools use: those in shown bands that
    /// are good measurements (not bad, not "fainter than", and not
    /// discrepant unless those are shown).
    pub(crate) fn analysis_observations(&self) -> Vec<&Observation> {
        let Some(ds) = &self.dataset else {
            return Vec::new();
        };
        ds.observations
            .iter()
            .filter(|o| !self.hidden_bands.contains(&o.band))
            .filter(|o| !o.fainter_than)
            .filter(|o| match o.validation {
                Validation::Good => true,
                Validation::Discrepant => self.settings.show_discrepant,
                Validation::Bad => false,
            })
            .collect()
    }

    pub(crate) fn analysis_series(&self) -> TimeSeries {
        TimeSeries::from_observations(self.analysis_observations())
    }

    /// Is an AI provider set up with a key (or one that needs none)?
    pub(crate) fn ai_configured(&self) -> bool {
        let provider = self.settings.ai.provider;
        self.settings.ai.is_complete()
            && (provider.key_is_optional() || self.secrets.key(provider).is_some())
    }

    /// The saved provider settings and key, or a message saying what to set up.
    pub(crate) fn require_ai(&self) -> Result<(AiConfig, String), String> {
        let config = self.settings.ai.clone();
        let key = self.secrets.key(config.provider).unwrap_or_default();
        if !config.is_complete() || (key.is_empty() && !config.provider.key_is_optional()) {
            return Err("Connect an AI provider first: File > Preferences.".into());
        }
        Ok((config, key))
    }

    fn start_load(&mut self, ctx: &egui::Context) {
        let request = match self.settings.source {
            SourceKind::Aavso => {
                let name = self.settings.star_name.trim().to_string();
                if name.is_empty() {
                    self.status =
                        Status::Error("Type a star name first, for example SS Cyg.".into());
                    return;
                }
                let mut query = StarQuery::named(name);
                if self.settings.limit_dates {
                    query.min_jd = parse_jd(&self.settings.min_jd);
                    query.max_jd = parse_jd(&self.settings.max_jd);
                } else if self.settings.limit_calendar {
                    match calendar_range(&self.settings.min_date, &self.settings.max_date) {
                        Ok((min, max)) => (query.min_jd, query.max_jd) = (min, max),
                        Err(msg) => {
                            self.status = Status::Error(msg);
                            return;
                        }
                    }
                }
                Request::Aavso(query)
            }
            SourceKind::BaaFile | SourceKind::OtherFile => {
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("Observation files", &["csv", "txt", "tsv", "dat"])
                    .add_filter("All files", &["*"])
                    .pick_file()
                else {
                    return;
                };
                let source_label = if self.settings.source == SourceKind::BaaFile {
                    "BAA VSS database"
                } else {
                    "File"
                };
                Request::File {
                    path,
                    source_label: source_label.into(),
                }
            }
            SourceKind::Gaia => return,
        };
        self.job = Some(Job::start(request, ctx.clone()));
    }

    fn finish_load(&mut self) {
        let Some(result) = self.job.as_ref().and_then(Job::poll) else {
            return;
        };
        let was_aavso = self.job.take().is_some_and(|j| j.from_database);
        match result {
            Ok(ds) => {
                let n = ds.observations.len();
                self.status = Status::Info(format!(
                    "Loaded {n} observations of {} from {}.",
                    ds.star.name, ds.source
                ));
                // Only database stars can be re-fetched by name.
                if was_aavso && !ds.star.name.is_empty() {
                    remember_star(&mut self.settings.recent_stars, &ds.star.name);
                }
                self.hidden_bands.clear();
                self.dataset = Some(ds);
                self.reset_view = true;
                self.phase.reset();
                self.search.reset();
                self.tab = Tab::LightCurve;
            }
            Err(vstar_core::Error::Cancelled) => {
                self.status = Status::Info("Loading cancelled.".into())
            }
            Err(e) => self.status = Status::Error(format!("Couldn't load the data: {e}")),
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open observation file…").clicked() {
                    self.settings.source = SourceKind::OtherFile;
                    self.start_load(ui.ctx());
                    ui.close();
                }
                if ui.button("Preferences…").clicked() {
                    self.prefs.open = true;
                    ui.close();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Analysis", |ui| {
                let has_data = self.dataset.is_some();
                let tip = "Load a star first";
                if ui
                    .add_enabled(has_data, egui::Button::new("Phase plot…"))
                    .on_disabled_hover_text(tip)
                    .clicked()
                {
                    self.open_phase_dialog();
                    ui.close();
                }
                if ui
                    .add_enabled(has_data, egui::Button::new("DC DFT period search…"))
                    .on_disabled_hover_text(tip)
                    .clicked()
                {
                    self.open_search_dialog();
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Reset zoom").clicked() {
                    self.reset_view = true;
                    ui.close();
                }
                ui.separator();
                ui.label("Theme");
                for (choice, label) in
                    [(ThemeChoice::System, "Follow Windows"), (ThemeChoice::Dark, "Dark"), (ThemeChoice::Light, "Light")]
                {
                    if ui.radio_value(&mut self.settings.theme, choice, label).clicked() {
                        apply_theme(ui.ctx(), choice);
                    }
                }
            });
            ui.menu_button("Help", |ui| {
                ui.hyperlink_to("VStar user manual", "https://aavso.github.io/VStar/docs/vstar/release/user_manual/VStarUserManual.pdf");
                ui.hyperlink_to("AAVSO VSX catalogue", "https://vsx.aavso.org/");
                ui.hyperlink_to("BAA VSS database", BAA_DATABASE_URL);
                ui.separator();
                ui.label(format!("VStar-RS {}", env!("CARGO_PKG_VERSION")));
                ui.label("Based on AAVSO VStar. AGPL-3.0 licence.");
            });
        });
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        let busy = self.job.is_some();
        ui.add_space(8.0);
        ui.heading(RichText::new("VStar-RS").strong());
        ui.label(RichText::new("Variable star light curves").weak());
        ui.add_space(8.0);

        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Load a star").strong());
            egui::ComboBox::from_id_salt("source")
                .width(ui.available_width())
                .selected_text(self.settings.source.label())
                .show_ui(ui, |ui| {
                    for kind in [SourceKind::Aavso, SourceKind::BaaFile, SourceKind::OtherFile] {
                        ui.selectable_value(&mut self.settings.source, kind, kind.label());
                    }
                    ui.add_enabled(false, egui::Button::selectable(false, SourceKind::Gaia.label()));
                });

            match self.settings.source {
                SourceKind::Aavso => {
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.settings.star_name)
                            .hint_text("Star name, e.g. SS Cyg")
                            .desired_width(f32::INFINITY),
                    );
                    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    // The two ranges are alternatives: ticking one clears the other.
                    if ui.checkbox(&mut self.settings.limit_dates, "Limit to a JD range").changed()
                        && self.settings.limit_dates
                    {
                        self.settings.limit_calendar = false;
                    }
                    if self.settings.limit_dates {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut self.settings.min_jd).hint_text("from JD").desired_width(100.0));
                            ui.add(egui::TextEdit::singleline(&mut self.settings.max_jd).hint_text("to JD").desired_width(100.0));
                        });
                    }
                    if ui.checkbox(&mut self.settings.limit_calendar, "Limit to a calendar date range").changed()
                        && self.settings.limit_calendar
                    {
                        self.settings.limit_dates = false;
                    }
                    if self.settings.limit_calendar {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut self.settings.min_date).hint_text("from YYYY-MM-DD").desired_width(100.0));
                            ui.add(egui::TextEdit::singleline(&mut self.settings.max_date).hint_text("to YYYY-MM-DD").desired_width(100.0));
                        });
                        match calendar_range(&self.settings.min_date, &self.settings.max_date) {
                            Ok((None, None)) => {}
                            Ok((min, max)) => {
                                let fmt = |jd: Option<f64>| jd.map_or("…".to_string(), |j| format!("{j:.1}"));
                                ui.label(RichText::new(format!("JD {} – {}", fmt(min), fmt(max))).weak());
                            }
                            Err(msg) => {
                                ui.colored_label(ui.visuals().error_fg_color, msg);
                            }
                        }
                    }
                    let load = ui.add_enabled(!busy, egui::Button::new("Load observations").min_size([ui.available_width(), 30.0].into()));
                    if (load.clicked() || enter) && !busy {
                        self.start_load(ui.ctx());
                    }
                    if !self.settings.recent_stars.is_empty() {
                        ui.add_space(4.0);
                        ui.label(RichText::new("Recent").weak());
                        ui.horizontal_wrapped(|ui| {
                            let mut picked = None;
                            for name in &self.settings.recent_stars {
                                if ui.add_enabled(!busy, egui::Button::new(name).small()).clicked() {
                                    picked = Some(name.clone());
                                }
                            }
                            if let Some(name) = picked {
                                self.settings.star_name = name;
                                self.start_load(ui.ctx());
                            }
                        });
                    }
                }
                SourceKind::BaaFile => {
                    ui.label("Download the star's observations as CSV from the BAA VSS database, then open the file here.");
                    ui.hyperlink_to("Open the BAA VSS database ↗", BAA_DATABASE_URL);
                    if ui.add_enabled(!busy, egui::Button::new("Open downloaded file…").min_size([ui.available_width(), 30.0].into())).clicked() {
                        self.start_load(ui.ctx());
                    }
                }
                SourceKind::OtherFile => {
                    ui.label("Opens AAVSO download files, VStar simple-format files, and most CSV or tab-separated files with JD and magnitude columns.");
                    if ui.add_enabled(!busy, egui::Button::new("Open file…").min_size([ui.available_width(), 30.0].into())).clicked() {
                        self.start_load(ui.ctx());
                    }
                }
                SourceKind::Gaia => {}
            }
        });

        let Some(ds) = &self.dataset else { return };
        ui.add_space(8.0);
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(if ds.star.name.is_empty() {
                    "Unnamed star"
                } else {
                    &ds.star.name
                })
                .strong()
                .size(16.0),
            );
            egui::Grid::new("star_info")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    let mut row = |k: &str, v: Option<String>| {
                        if let Some(v) = v {
                            ui.label(RichText::new(k).weak());
                            ui.label(v);
                            ui.end_row();
                        }
                    };
                    row("Type", ds.star.var_type.clone());
                    row("Period", ds.star.period_days.map(|p| format!("{p} d")));
                    row("Epoch", ds.star.epoch_jd.map(|e| format!("JD {e}")));
                    row("Spectrum", ds.star.spectral_type.clone());
                    row("RA", ds.star.ra_deg.map(format_ra));
                    row("Dec", ds.star.dec_deg.map(format_dec));
                    row("AUID", ds.star.auid.clone());
                    row("Observations", Some(ds.observations.len().to_string()));
                    row("Source", Some(ds.source.clone()));
                    if let Some((a, b)) = ds.jd_range() {
                        row("JD range", Some(format!("{a:.1} – {b:.1}")));
                    }
                });
            if ui.button("VSP").clicked() {
                self.vsp.open(ui.ctx(), &ds.star.name);
            }
            if ds.star.auid.is_some() {
                ui.hyperlink_to("View on VSX ↗", aavso::vsx_page_url(&ds.star.name));
            }
        });

        ui.add_space(8.0);
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Bands").strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("None").clicked() {
                        self.hidden_bands = ds.band_counts().into_iter().map(|(b, _)| b).collect();
                    }
                    if ui.small_button("All").clicked() {
                        self.hidden_bands.clear();
                    }
                });
            });
            for (band, count) in ds.band_counts() {
                let mut shown = !self.hidden_bands.contains(&band);
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size([10.0, 10.0].into(), egui::Sense::hover());
                    ui.painter()
                        .circle_filled(rect.center(), 5.0, theme::band_color(ui, &band));
                    if ui.checkbox(&mut shown, bands::description(&band)).changed() {
                        if shown {
                            self.hidden_bands.remove(&band);
                        } else {
                            self.hidden_bands.insert(band.clone());
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(count.to_string()).weak());
                    });
                });
            }
            ui.separator();
            ui.checkbox(&mut self.settings.show_errors, "Error bars");
            ui.checkbox(
                &mut self.settings.show_fainter_than,
                "Fainter-than estimates",
            );
            ui.checkbox(
                &mut self.settings.show_discrepant,
                "Discrepant observations",
            );
        });
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::LightCurve, "Light curve");
            if self.phase.active.is_some() {
                ui.selectable_value(&mut self.tab, Tab::PhasePlot, "Phase plot");
            }
            if self.search.result.is_some() {
                ui.selectable_value(&mut self.tab, Tab::PeriodSearch, "Period search");
            }
            if let (Tab::PhasePlot, Some((period, epoch))) = (self.tab, self.phase.active) {
                ui.separator();
                ui.label(RichText::new(format!("Period {period} d, epoch JD {epoch:.4}")).weak());
                if ui.small_button("Change…").clicked() {
                    self.open_phase_dialog();
                }
            }
        });
    }

    fn show_plot(&mut self, ui: &mut egui::Ui, mode: light_curve::Mode) {
        let Some(ds) = &self.dataset else { return };
        let opts = light_curve::Options {
            mode,
            hidden_bands: &self.hidden_bands,
            show_discrepant: self.settings.show_discrepant,
            show_fainter_than: self.settings.show_fainter_than,
            show_errors: self.settings.show_errors,
            reset_view: std::mem::take(&mut self.reset_view),
        };
        light_curve::show(ui, ds, &opts);
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(job) = &self.job {
                ui.spinner();
                let n = job.loaded();
                ui.label(if n > 0 {
                    format!("{} {n} observations so far", job.description)
                } else {
                    job.description.clone()
                });
                if ui.small_button("Cancel").clicked() {
                    job.cancel();
                }
                return;
            }
            let analysis = self.search.running().or_else(|| self.phase.task());
            if let Some((description, progress)) = analysis {
                ui.spinner();
                ui.label(description);
                ui.add(egui::ProgressBar::new(progress).desired_width(160.0));
                if ui.small_button("Cancel").clicked() {
                    self.search.cancel();
                    self.phase.cancel();
                }
                return;
            }
            match &self.status {
                Status::Ready => ui.label(RichText::new("Ready").weak()),
                Status::Info(msg) => ui.label(msg),
                Status::Error(msg) => ui.colored_label(ui.visuals().error_fg_color, msg),
            };
        });
    }
}

impl eframe::App for VStarApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.finish_load();
        self.poll_search();
        self.poll_phase_ai();
        let ctx = ui.ctx().clone();
        self.vsp.show(&ctx);
        self.show_phase_dialog(&ctx);
        self.show_search_dialog(&ctx);
        self.prefs.show(&ctx, &mut self.settings.ai, &self.secrets);

        egui::Panel::top("menu").show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("controls")
            .resizable(true)
            .default_size(300.0)
            .min_size(240.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.side_panel(ui));
            });
        egui::CentralPanel::default().show(ui, |ui| {
            if self.dataset.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new(
                            "Load a star from the panel on the left to see its light curve.",
                        )
                        .weak()
                        .size(16.0),
                    );
                });
                return;
            }
            self.tab_bar(ui);
            ui.add_space(4.0);
            match self.tab {
                Tab::LightCurve => self.show_plot(ui, light_curve::Mode::Time),
                Tab::PhasePlot => match self.phase.active {
                    Some((period, epoch)) => {
                        let two_cycles = self.phase.two_cycles;
                        self.show_plot(
                            ui,
                            light_curve::Mode::Phase {
                                epoch,
                                period,
                                two_cycles,
                            },
                        );
                    }
                    None => self.tab = Tab::LightCurve,
                },
                Tab::PeriodSearch => self.show_search_results(ui),
            }
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }
}

fn apply_theme(ctx: &egui::Context, choice: ThemeChoice) {
    ctx.set_theme(match choice {
        ThemeChoice::System => egui::ThemePreference::System,
        ThemeChoice::Dark => egui::ThemePreference::Dark,
        ThemeChoice::Light => egui::ThemePreference::Light,
    });
}

fn parse_jd(s: &str) -> Option<f64> {
    s.trim().parse().ok()
}

/// Convert a calendar date range (UT) to a JD range. Either end may be
/// blank. A "to" date without a time includes that whole day.
fn calendar_range(from: &str, to: &str) -> Result<(Option<f64>, Option<f64>), String> {
    let parse = |s: &str, end_of_day: bool| -> Result<Option<f64>, String> {
        let s = s.trim();
        if s.is_empty() {
            return Ok(None);
        }
        let jd = time::parse_ut_to_jd(s)
            .ok_or_else(|| format!("\"{s}\" isn't a date. Use YYYY-MM-DD, e.g. 2024-03-15."))?;
        let has_time =
            s.contains([' ', 'T']) || s.split('-').nth(2).is_some_and(|d| d.contains('.'));
        Ok(Some(if end_of_day && !has_time {
            jd + 1.0
        } else {
            jd
        }))
    };
    let (min, max) = (parse(from, false)?, parse(to, true)?);
    if let (Some(a), Some(b)) = (min, max)
        && a >= b
    {
        return Err("The \"from\" date must be before the \"to\" date.".into());
    }
    Ok((min, max))
}

fn remember_star(recent: &mut Vec<String>, name: &str) {
    recent.retain(|n| !n.eq_ignore_ascii_case(name));
    recent.insert(0, name.to_string());
    recent.truncate(MAX_RECENT);
}

fn format_ra(deg: f64) -> String {
    let h = deg.rem_euclid(360.0) / 15.0;
    let (hh, rem) = (h.trunc(), h.fract() * 60.0);
    format!(
        "{:02}h {:02}m {:04.1}s",
        hh as u32,
        rem.trunc() as u32,
        rem.fract() * 60.0
    )
}

fn format_dec(deg: f64) -> String {
    let sign = if deg < 0.0 { '−' } else { '+' };
    let d = deg.abs();
    let (dd, rem) = (d.trunc(), d.fract() * 60.0);
    format!(
        "{sign}{:02}° {:02}′ {:02.0}″",
        dd as u32,
        rem.trunc() as u32,
        rem.fract() * 60.0
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_calendar_ranges() {
        assert_eq!(calendar_range("", ""), Ok((None, None)));
        assert_eq!(
            calendar_range("2000-01-01", "2000-01-01"),
            Ok((Some(2451544.5), Some(2451545.5)))
        );
        assert_eq!(
            calendar_range("2000-01-01 12:00", ""),
            Ok((Some(2451545.0), None))
        );
        assert!(calendar_range("2000-13-01", "").is_err());
        assert!(calendar_range("2001-01-01", "2000-01-01").is_err());
    }

    #[test]
    fn formats_coordinates() {
        assert_eq!(format_ra(325.67858), "21h 42m 42.9s");
        assert_eq!(format_dec(43.58611), "+43° 35′ 10″");
        assert_eq!(format_dec(-5.5), "−05° 30′ 00″");
    }

    #[test]
    fn recent_stars_are_deduplicated_and_capped() {
        let mut r = vec!["SS Cyg".to_string(), "R Leo".to_string()];
        remember_star(&mut r, "r leo");
        assert_eq!(r, vec!["r leo", "SS Cyg"]);
        for i in 0..20 {
            remember_star(&mut r, &format!("S{i}"));
        }
        assert_eq!(r.len(), MAX_RECENT);
    }
}
