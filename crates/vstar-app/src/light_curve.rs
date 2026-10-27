//! The light curve plot: magnitude against Julian Date, brighter at the top.

use std::collections::HashSet;

use egui_plot::{HoverPosition, Legend, Line, MarkerShape, Plot, PlotPoints, Points};
use vstar_core::{Dataset, Validation, bands, time};

use crate::theme;

/// Error bars are drawn one line each, so skip them for huge datasets.
const MAX_ERROR_BARS: usize = 5_000;

pub struct Options<'a> {
    pub hidden_bands: &'a HashSet<String>,
    pub show_discrepant: bool,
    pub show_fainter_than: bool,
    pub show_errors: bool,
    pub reset_view: bool,
}

/// One plotted series and which observations its points came from, so the
/// hover label can describe the observation under the cursor.
struct Series {
    name: String,
    obs: Vec<usize>,
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
            let pts: Vec<[f64; 2]> = idx
                .iter()
                .map(|&i| [ds.observations[i].jd, ds.observations[i].mag])
                .collect();
            if opts.show_errors {
                for &i in &idx {
                    let o = &ds.observations[i];
                    if o.error > 0.0 && error_bars.len() < MAX_ERROR_BARS {
                        error_bars.push([[o.jd, o.mag - o.error], [o.jd, o.mag + o.error]]);
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
            series.push((Series { name, obs: idx }, points));
        }
    }

    let (meta, points): (Vec<Series>, Vec<Points>) = series.into_iter().unzip();
    let observations = &ds.observations;

    let mut plot = Plot::new("light_curve")
        .invert_y(true)
        .x_axis_label("Julian Date")
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
                let mut text = format!(
                    "{plot_name}\nJD {:.5}\n{}\nMag {}{:.3}\nError {error}\nObscode {}",
                    o.jd,
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
                Some(format!("JD {:.4}\nMag {:.3}", position.x, position.y))
            }
        });
    if opts.reset_view {
        plot = plot.reset();
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
