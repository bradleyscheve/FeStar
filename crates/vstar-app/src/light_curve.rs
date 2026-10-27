//! The light curve plot: magnitude against Julian Date, brighter at the top.
//! The same plot also draws a phase plot, where the x axis is phase.

use std::collections::HashSet;

use egui_plot::{HoverPosition, Legend, Line, MarkerShape, Plot, PlotPoints, Points};
use vstar_core::analysis::phase;
use vstar_core::{Dataset, Validation, bands, time};

use crate::theme;

/// Error bars are drawn one line each, so skip them for huge datasets.
const MAX_ERROR_BARS: usize = 5_000;

/// What the x axis shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Time,
    /// Observations folded at `period` about `epoch`. With `two_cycles`
    /// the previous cycle is drawn too (phase - 1), as in VStar.
    Phase {
        epoch: f64,
        period: f64,
        two_cycles: bool,
    },
}

pub struct Options<'a> {
    pub mode: Mode,
    pub hidden_bands: &'a HashSet<String>,
    pub show_discrepant: bool,
    pub show_fainter_than: bool,
    pub show_errors: bool,
    pub reset_view: bool,
}

/// One plotted series and which observation each point came from, so the
/// hover label can describe the observation under the cursor.
struct Series {
    name: String,
    obs: Vec<usize>,
    /// The x value of each point (JD or phase).
    x: Vec<f64>,
}

/// The x values at which observation `o` is drawn.
fn x_positions(mode: Mode, jd: f64) -> impl Iterator<Item = f64> {
    let (first, second) = match mode {
        Mode::Time => (jd, None),
        Mode::Phase {
            epoch,
            period,
            two_cycles,
        } => {
            let ph = phase::phase(jd, epoch, period);
            (ph, two_cycles.then_some(ph - 1.0))
        }
    };
    std::iter::once(first).chain(second)
}

pub fn show(ui: &mut egui::Ui, ds: &Dataset, opts: &Options) {
    let mut series: Vec<(Series, Points<'static>)> = Vec::new();
    let mut error_bars: Vec<[[f64; 2]; 2]> = Vec::new();

    for (band, _) in ds.band_counts() {
        if opts.hidden_bands.contains(&band) {
            continue;
        }
        let color = theme::band_color(ui, &band);
        let desc = bands::description(&band);

        // Normal, fainter-than and discrepant observations get distinct markers.
        let kinds = [
            (
                desc.clone(),
                MarkerShape::Circle,
                true,
                false,
                Validation::Good,
            ),
            (
                format!("{desc} (fainter than)"),
                MarkerShape::Down,
                false,
                true,
                Validation::Good,
            ),
            (
                format!("{desc} (discrepant)"),
                MarkerShape::Cross,
                false,
                false,
                Validation::Discrepant,
            ),
        ];
        for (name, shape, filled, fainter, validation) in kinds {
            if (fainter && !opts.show_fainter_than)
                || (validation == Validation::Discrepant && !opts.show_discrepant)
            {
                continue;
            }
            let idx: Vec<usize> = ds
                .observations
                .iter()
                .enumerate()
                .filter(|(_, o)| {
                    o.band == band
                        && o.validation == validation
                        && (validation == Validation::Discrepant || o.fainter_than == fainter)
                })
                .map(|(i, _)| i)
                .collect();
            if idx.is_empty() {
                continue;
            }
            // One entry per drawn point; a phase plot of two cycles draws each
            // observation twice.
            let (mut obs_of_point, mut xs, mut pts) = (Vec::new(), Vec::new(), Vec::new());
            for &i in &idx {
                let o = &ds.observations[i];
                for x in x_positions(opts.mode, o.jd) {
                    obs_of_point.push(i);
                    xs.push(x);
                    pts.push([x, o.mag]);
                    if opts.show_errors && o.error > 0.0 && error_bars.len() < MAX_ERROR_BARS {
                        error_bars.push([[x, o.mag - o.error], [x, o.mag + o.error]]);
                    }
                }
            }
            let points = Points::new(name.clone(), PlotPoints::new(pts))
                .color(color)
                .shape(shape)
                .filled(filled)
                .radius(if shape == MarkerShape::Circle {
                    2.2
                } else {
                    3.5
                });
            series.push((
                Series {
                    name,
                    obs: obs_of_point,
                    x: xs,
                },
                points,
            ));
        }
    }

    let (meta, points): (Vec<Series>, Vec<Points>) = series.into_iter().unzip();
    let observations = &ds.observations;

    let phase_mode = matches!(opts.mode, Mode::Phase { .. });
    let mut plot = Plot::new(if phase_mode {
        "phase_plot"
    } else {
        "light_curve"
    })
    .invert_y(true)
    .x_axis_label(if phase_mode { "Phase" } else { "Julian Date" })
    .y_axis_label("Magnitude")
    .legend(Legend::default().follow_insertion_order(true))
    .label_formatter(move |pos| match pos {
        HoverPosition::NearDataPoint {
            plot_name, index, ..
        } => {
            let s = meta.iter().find(|s| s.name == *plot_name)?;
            let o = &observations[*s.obs.get(*index)?];
            let error = if o.error > 0.0 {
                format!("± {:.3}", o.error)
            } else {
                "not given".to_string()
            };
            let x = s.x[*index];
            let when = if phase_mode {
                format!("Phase {x:.4}\nJD {:.5}", o.jd)
            } else {
                format!("JD {:.5}", o.jd)
            };
            let mut text = format!(
                "{plot_name}\n{when}\n{}\nMag {}{:.3}\nError {error}\nObscode {}",
                time::jd_to_ut_string(o.jd),
                if o.fainter_than { "<" } else { "" },
                o.mag,
                o.observer.as_deref().unwrap_or("not given"),
            );
            if let Some(c) = &o.comment {
                text.push_str(&format!("\n{c}"));
            }
            Some(text)
        }
        HoverPosition::Elsewhere { position } => {
            let axis = if phase_mode { "Phase" } else { "JD" };
            Some(format!("{axis} {:.4}\nMag {:.3}", position.x, position.y))
        }
    });
    if opts.reset_view {
        plot = plot.reset();
    }

    if let Mode::Phase { two_cycles, .. } = opts.mode {
        plot = plot
            .include_x(if two_cycles { -1.0 } else { 0.0 })
            .include_x(1.0);
    }

    let bar_color = ui.visuals().weak_text_color().gamma_multiply(0.6);
    plot.show(ui, |plot_ui| {
        for bar in error_bars {
            plot_ui.line(
                Line::new("Error bars", PlotPoints::new(bar.to_vec()))
                    .color(bar_color)
                    .width(1.0)
                    .allow_hover(false),
            );
        }
        for p in points {
            plot_ui.points(p);
        }
    });
}
