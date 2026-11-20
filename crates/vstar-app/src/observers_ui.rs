//! The "Observers" popup: a searchable list of the observer codes in the
//! observations loaded for the active star.

use std::collections::BTreeMap;

use egui::{Context, RichText};
use vstar_core::Dataset;

#[derive(Default)]
pub struct ObserversState {
    pub open: bool,
    search: String,
}

impl ObserversState {
    pub fn show(&mut self, ctx: &Context, dataset: Option<&Dataset>) {
        if !self.open {
            return;
        }
        let mut open = true;
        egui::Window::new("Observers")
            .open(&mut open)
            .default_size([300.0, 420.0])
            .show(ctx, |ui| {
                let Some(ds) = dataset else {
                    ui.label("No star is loaded.");
                    return;
                };
                let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
                for o in &ds.observations {
                    if let Some(code) = o.observer.as_deref().filter(|c| !c.trim().is_empty()) {
                        *counts.entry(code.trim()).or_default() += 1;
                    }
                }
                ui.label(
                    RichText::new(format!(
                        "{} observer codes in the {} observations of {}",
                        counts.len(),
                        ds.observations.len(),
                        ds.star.name
                    ))
                    .weak(),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Search observer codes")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(4.0);
                let query = self.search.trim().to_lowercase();
                let shown: Vec<(&str, usize)> = counts
                    .into_iter()
                    .filter(|(code, _)| query.is_empty() || code.to_lowercase().contains(&query))
                    .collect();
                if shown.is_empty() {
                    ui.label(RichText::new("No matching observer codes.").weak());
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("observer_codes")
                        .num_columns(2)
                        .spacing([24.0, 4.0])
                        .show(ui, |ui| {
                            for (code, count) in shown {
                                ui.label(code);
                                ui.label(RichText::new(count.to_string()).weak());
                                ui.end_row();
                            }
                        });
                });
            });
        self.open = open;
    }
}
