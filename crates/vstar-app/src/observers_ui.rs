//! The "Observers" popup: a searchable list of the observer codes in the
//! observations loaded for the active star, each with a checkbox to show or
//! leave out that observer's observations.

use std::collections::{BTreeMap, HashSet};

use egui::{Context, RichText};
use vstar_core::{Dataset, Observation};

/// The observer code an observation is filed under; observations with no
/// observer share the empty code.
pub fn observer_key(o: &Observation) -> &str {
    o.observer.as_deref().map_or("", str::trim)
}

#[derive(Default)]
pub struct ObserversState {
    pub open: bool,
    search: String,
}

impl ObserversState {
    pub fn show(&mut self, ctx: &Context, dataset: Option<&Dataset>, hidden: &mut HashSet<String>) {
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
                    *counts.entry(observer_key(o)).or_default() += 1;
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
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("Search observer codes")
                            .desired_width(170.0),
                    );
                    if ui.small_button("All").clicked() {
                        hidden.clear();
                    }
                    if ui.small_button("None").clicked() {
                        *hidden = counts.keys().map(|c| c.to_string()).collect();
                    }
                });
                ui.label(
                    RichText::new(
                        "Unchecked observers are left out of the chart and the analysis.",
                    )
                    .weak()
                    .small(),
                );
                ui.add_space(4.0);
                let query = self.search.trim().to_lowercase();
                let shown: Vec<(&str, usize)> = counts
                    .into_iter()
                    .filter(|(code, _)| {
                        query.is_empty()
                            || code.to_lowercase().contains(&query)
                            || (code.is_empty() && "(no observer)".contains(&query))
                    })
                    .collect();
                if shown.is_empty() {
                    ui.label(RichText::new("No matching observer codes.").weak());
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (code, count) in shown {
                        let mut checked = !hidden.contains(code);
                        ui.horizontal(|ui| {
                            let label = if code.is_empty() {
                                "(no observer)"
                            } else {
                                code
                            };
                            if ui.checkbox(&mut checked, label).changed() {
                                if checked {
                                    hidden.remove(code);
                                } else {
                                    hidden.insert(code.to_string());
                                }
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(RichText::new(count.to_string()).weak());
                                },
                            );
                        });
                    }
                });
            });
        self.open = open;
    }
}
