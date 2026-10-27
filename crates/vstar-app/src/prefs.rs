//! File > Preferences: connecting an AI provider.

use std::sync::atomic::{AtomicBool, Ordering};

use egui::RichText;
use vstar_core::ai::{AiClient, AiConfig, Provider, load_key};
use vstar_core::http::ReqwestClient;
use vstar_core::secrets::{KeyringStore, MemoryStore, SecretStore};

use crate::task::Task;
use crate::theme;

const KEYRING_SERVICE: &str = "VStar-RS";

/// API keys live in the operating system's credential store. If it can't be
/// used (some Linux setups), keys are kept in memory for this session only.
pub struct Secrets {
    keyring: KeyringStore,
    memory: MemoryStore,
    session_only: AtomicBool,
}

impl Secrets {
    pub fn new() -> Self {
        Self {
            keyring: KeyringStore::new(KEYRING_SERVICE),
            memory: MemoryStore::default(),
            session_only: AtomicBool::new(false),
        }
    }

    pub fn key(&self, provider: Provider) -> Option<String> {
        match load_key(&self.keyring, provider) {
            Ok(Some(k)) => Some(k),
            Ok(None) | Err(_) => load_key(&self.memory, provider).ok().flatten(),
        }
    }

    /// Save a key; returns where it went, for the confirmation message.
    pub fn save(&self, provider: Provider, key: &str) -> &'static str {
        match self.keyring.set(provider.account(), key) {
            Ok(()) => {
                let _ = self.memory.delete(provider.account());
                "saved in your computer's credential store"
            }
            Err(_) => {
                self.session_only.store(true, Ordering::Relaxed);
                let _ = self.memory.set(provider.account(), key);
                "kept for this session only (the credential store isn't available)"
            }
        }
    }

    pub fn remove(&self, provider: Provider) {
        let _ = self.keyring.delete(provider.account());
        let _ = self.memory.delete(provider.account());
    }

    pub fn store_name(&self) -> &'static str {
        if cfg!(target_os = "windows") {
            "Windows Credential Manager"
        } else if cfg!(target_os = "macos") {
            "the macOS Keychain"
        } else {
            "the system keyring"
        }
    }
}

#[derive(Default)]
pub struct PrefsState {
    pub open: bool,
    key_input: String,
    /// Result of the last action, shown under the key box: (ok, message).
    message: Option<(bool, String)>,
    test: Option<Task<()>>,
}

impl PrefsState {
    pub fn show(&mut self, ctx: &egui::Context, config: &mut AiConfig, secrets: &Secrets) {
        if !self.open {
            return;
        }
        self.finish_test();

        let mut open = true;
        egui::Window::new("Preferences")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .min_width(460.0)
            .show(ctx, |ui| self.contents(ui, config, secrets));
        self.open = open;
    }

    fn finish_test(&mut self) {
        let Some(result) = self.test.as_ref().and_then(Task::poll) else {
            return;
        };
        self.test = None;
        self.message = Some(match result {
            Ok(()) => (
                true,
                "Connected. The provider accepted the key and model.".into(),
            ),
            Err(e) => (false, e.to_string()),
        });
    }

    fn contents(&mut self, ui: &mut egui::Ui, config: &mut AiConfig, secrets: &Secrets) {
        ui.label(RichText::new("AI provider").strong().size(16.0));
        ui.label(
            "VStar-RS can ask an AI model to choose period-search and phase-plot settings. \
             Connect your own account by pasting an API key.",
        );
        ui.add_space(6.0);

        egui::Grid::new("ai_prefs")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Provider");
                let before = config.provider;
                egui::ComboBox::from_id_salt("ai_provider")
                    .selected_text(config.provider.label())
                    .width(280.0)
                    .show_ui(ui, |ui| {
                        for p in Provider::ALL {
                            ui.selectable_value(&mut config.provider, p, p.label());
                        }
                    });
                if config.provider != before {
                    self.key_input.clear();
                    self.message = None;
                }
                ui.end_row();

                if config.provider == Provider::Custom {
                    ui.label("Address");
                    ui.add(
                        egui::TextEdit::singleline(&mut config.custom_base_url)
                            .hint_text("http://localhost:11434/v1")
                            .desired_width(280.0),
                    );
                    ui.end_row();
                }

                ui.label("Model");
                let provider = config.provider;
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(config.model_mut()).desired_width(200.0));
                    if !provider.suggested_models().is_empty() {
                        egui::ComboBox::from_id_salt("ai_model_suggestions")
                            .selected_text("Suggestions")
                            .show_ui(ui, |ui| {
                                for m in provider.suggested_models() {
                                    if ui.selectable_label(config.model() == *m, *m).clicked() {
                                        *config.model_mut() = (*m).to_string();
                                    }
                                }
                            });
                    }
                });
                ui.end_row();

                ui.label("API key");
                let saved = secrets.key(config.provider).is_some();
                ui.vertical(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.key_input)
                            .password(true)
                            .hint_text(if saved {
                                "A key is saved. Paste a new one to replace it."
                            } else {
                                "Paste your API key"
                            })
                            .desired_width(280.0),
                    );
                    ui.horizontal(|ui| {
                        let can_save = !self.key_input.trim().is_empty();
                        if ui
                            .add_enabled(can_save, egui::Button::new("Save key"))
                            .clicked()
                        {
                            let place = secrets.save(config.provider, self.key_input.trim());
                            self.key_input.clear();
                            self.message = Some((true, format!("Key {place}.")));
                        }
                        if ui
                            .add_enabled(saved, egui::Button::new("Remove key"))
                            .clicked()
                        {
                            secrets.remove(config.provider);
                            self.message = Some((true, "Key removed.".into()));
                        }
                    });
                });
                ui.end_row();
            });

        ui.add_space(4.0);
        let provider = config.provider;
        match secrets.key(provider) {
            Some(_) => ui
                .label(RichText::new(format!("✔ A key for {} is saved.", provider.label())).weak()),
            None if provider.key_is_optional() => {
                ui.label(RichText::new("This provider may not need a key.").weak())
            }
            None => ui.label(RichText::new("No key saved yet.").weak()),
        };
        if let Some(url) = provider.key_page() {
            ui.hyperlink_to("Get an API key ↗", url);
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let testing = self.test.is_some();
            let ready = config.is_complete()
                && (secrets.key(provider).is_some() || provider.key_is_optional());
            if ui
                .add_enabled(ready && !testing, egui::Button::new("Test connection"))
                .clicked()
            {
                self.message = None;
                let (cfg, key) = (config.clone(), secrets.key(provider).unwrap_or_default());
                self.test = Some(Task::spawn(
                    ui.ctx(),
                    "Testing the AI connection",
                    move |_| {
                        let http = ReqwestClient::new()?;
                        AiClient {
                            http: &http,
                            config: &cfg,
                            api_key: &key,
                        }
                        .test()
                    },
                ));
            }
            if testing {
                ui.spinner();
                ui.label("Testing…");
            }
        });
        if let Some((ok, msg)) = &self.message {
            let color = if *ok {
                ui.visuals().text_color()
            } else {
                ui.visuals().error_fg_color
            };
            ui.colored_label(color, msg);
        }

        ui.add_space(8.0);
        theme::card(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("About your key and your data").strong());
            ui.label(
                RichText::new(format!(
                    "Your key is stored in {}, not in a file. Only a statistical summary is sent \
                     to the provider (the star's name and type, how many observations, the time span, \
                     and a few candidate periods), never the observations themselves. \
                     API usage is billed by the provider and is separate from any chat subscription.",
                    secrets.store_name()
                ))
                .weak(),
            );
        });
    }
}
