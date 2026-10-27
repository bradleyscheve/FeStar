//! Talking to an AI provider.
//!
//! The app uses AI to choose settings (which periods to search, which period
//! to fold at); it never lets the AI do the arithmetic. All numbers the AI
//! proposes are checked against the data before they're used.
//!
//! What is sent to the provider is a short statistical summary of the data
//! (number of observations, time span, bands, a few candidate periods), not
//! the observations themselves.

pub mod advice;
#[cfg(test)]
pub(crate) mod testing;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::http::HttpClient;
use crate::secrets::SecretStore;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Provider {
    #[default]
    Anthropic,
    OpenAi,
    /// Any service that speaks the OpenAI chat API, such as OpenRouter or a
    /// model running on your own computer.
    Custom,
}

impl Provider {
    pub const ALL: [Self; 3] = [Self::Anthropic, Self::OpenAi, Self::Custom];

    pub fn label(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic (Claude)",
            Self::OpenAi => "OpenAI",
            Self::Custom => "Other (OpenAI-compatible)",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => "claude-sonnet-5-5",
            Self::OpenAi => "gpt-5",
            Self::Custom => "",
        }
    }

    /// Models worth offering in a drop-down; the box also accepts any name.
    pub fn suggested_models(self) -> &'static [&'static str] {
        match self {
            Self::Anthropic => &[
                "claude-sonnet-5-5",
                "claude-opus-5-5",
                "claude-haiku-5-5",
                "claude-fable-5-1",
            ],
            Self::OpenAi => &["gpt-5", "gpt-5-mini"],
            Self::Custom => &[],
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com",
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Custom => "",
        }
    }

    /// Where to create an API key.
    pub fn key_page(self) -> Option<&'static str> {
        match self {
            Self::Anthropic => Some("https://console.anthropic.com/settings/keys"),
            Self::OpenAi => Some("https://platform.openai.com/api-keys"),
            Self::Custom => None,
        }
    }

    /// The name this provider's key is stored under in the credential store.
    pub fn account(self) -> &'static str {
        match self {
            Self::Anthropic => "ai-anthropic",
            Self::OpenAi => "ai-openai",
            Self::Custom => "ai-custom",
        }
    }

    /// Local servers such as Ollama need no key.
    pub fn key_is_optional(self) -> bool {
        self == Self::Custom
    }
}

/// Which provider and model to use. The API key is kept separately in the
/// credential store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub provider: Provider,
    pub anthropic_model: String,
    pub openai_model: String,
    pub custom_model: String,
    pub custom_base_url: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: Provider::Anthropic,
            anthropic_model: Provider::Anthropic.default_model().into(),
            openai_model: Provider::OpenAi.default_model().into(),
            custom_model: String::new(),
            custom_base_url: "http://localhost:11434/v1".into(),
        }
    }
}

impl AiConfig {
    pub fn model(&self) -> &str {
        match self.provider {
            Provider::Anthropic => &self.anthropic_model,
            Provider::OpenAi => &self.openai_model,
            Provider::Custom => &self.custom_model,
        }
    }

    pub fn model_mut(&mut self) -> &mut String {
        match self.provider {
            Provider::Anthropic => &mut self.anthropic_model,
            Provider::OpenAi => &mut self.openai_model,
            Provider::Custom => &mut self.custom_model,
        }
    }

    pub fn base_url(&self) -> &str {
        match self.provider {
            Provider::Custom => self.custom_base_url.trim_end_matches('/'),
            p => p.default_base_url(),
        }
    }

    /// Has the user chosen everything needed besides the key?
    pub fn is_complete(&self) -> bool {
        !self.model().trim().is_empty()
            && (self.provider != Provider::Custom || !self.base_url().is_empty())
    }
}

/// Look up the saved key for a provider.
pub fn load_key(store: &dyn SecretStore, provider: Provider) -> Result<Option<String>> {
    Ok(store
        .get(provider.account())?
        .filter(|k| !k.trim().is_empty()))
}

/// A configured connection ready to make requests.
pub struct AiClient<'a> {
    pub http: &'a dyn HttpClient,
    pub config: &'a AiConfig,
    pub api_key: &'a str,
}

impl AiClient<'_> {
    /// Send one question and return the model's text answer.
    pub fn complete(&self, system: &str, user: &str, max_tokens: u32) -> Result<String> {
        if !self.config.is_complete() {
            return Err(Error::Ai("choose a model in Preferences first".into()));
        }
        if self.api_key.trim().is_empty() && !self.config.provider.key_is_optional() {
            return Err(Error::Ai(
                "no API key is saved; add one in File > Preferences".into(),
            ));
        }
        let model = self.config.model().trim();
        let (url, headers, body): (String, Vec<(&str, String)>, Value) = match self.config.provider
        {
            Provider::Anthropic => (
                format!("{}/v1/messages", self.config.base_url()),
                vec![
                    ("x-api-key", self.api_key.trim().to_string()),
                    ("anthropic-version", "2023-06-01".into()),
                ],
                json!({
                    "model": model,
                    "max_tokens": max_tokens,
                    "system": system,
                    "messages": [{"role": "user", "content": user}],
                }),
            ),
            Provider::OpenAi | Provider::Custom => {
                let mut headers = Vec::new();
                if !self.api_key.trim().is_empty() {
                    headers.push(("authorization", format!("Bearer {}", self.api_key.trim())));
                }
                (
                    format!("{}/chat/completions", self.config.base_url()),
                    headers,
                    json!({
                        "model": model,
                        "messages": [
                            {"role": "system", "content": system},
                            {"role": "user", "content": user},
                        ],
                    }),
                )
            }
        };
        let header_refs: Vec<(&str, &str)> =
            headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let reply = self
            .http
            .post_json(&url, &header_refs, &body.to_string())
            .map_err(|e| Error::Ai(friendly_error(&e.to_string())))?;
        extract_text(self.config.provider, &reply)
    }

    /// Make a trivial request to check the key and model work.
    pub fn test(&self) -> Result<()> {
        let answer = self.complete("Reply with the single word OK.", "Connection test.", 16)?;
        if answer.trim().is_empty() {
            return Err(Error::Ai("the provider answered with nothing".into()));
        }
        Ok(())
    }
}

/// Turn a provider's raw HTTP failure into something a person can act on.
fn friendly_error(raw: &str) -> String {
    let hint = if raw.contains("401") || raw.contains("403") {
        Some("The provider rejected the API key. Check it in File > Preferences.")
    } else if raw.contains("404") {
        Some(
            "The provider doesn't know that model or address. Check the model name in File > Preferences.",
        )
    } else if raw.contains("429") {
        Some(
            "The provider says you're sending too many requests, or your account is out of credit.",
        )
    } else {
        None
    };
    match hint {
        Some(h) => format!("{h} ({raw})"),
        None => raw.to_string(),
    }
}

fn extract_text(provider: Provider, reply: &str) -> Result<String> {
    let v: Value = serde_json::from_str(reply)
        .map_err(|e| Error::Ai(format!("unreadable answer from provider: {e}")))?;
    let text = match provider {
        Provider::Anthropic => v["content"].as_array().map(|blocks| {
            blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        }),
        Provider::OpenAi | Provider::Custom => v["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string),
    };
    text.ok_or_else(|| Error::Ai("the provider's answer had no text in it".into()))
}

#[cfg(test)]
mod tests {
    use super::testing::FakeAi;
    use super::*;

    #[test]
    fn anthropic_request_and_reply() {
        let http = FakeAi::new(
            r#"{"content":[{"type":"text","text":"Hel"},{"type":"text","text":"lo"}]}"#,
        );
        let cfg = AiConfig::default();
        let client = AiClient {
            http: &http,
            config: &cfg,
            api_key: " sk-test ",
        };
        assert_eq!(client.complete("sys", "hi", 100).unwrap(), "Hello");

        let sent = http.sent.lock().unwrap();
        let (url, headers, body) = &sent[0];
        assert_eq!(url, "https://api.anthropic.com/v1/messages");
        assert!(headers.contains(&("x-api-key".into(), "sk-test".into())));
        assert!(headers.contains(&("anthropic-version".into(), "2023-06-01".into())));
        let b: Value = serde_json::from_str(body).unwrap();
        assert_eq!(b["model"], "claude-sonnet-5-5");
        assert_eq!(b["system"], "sys");
        assert_eq!(b["messages"][0]["content"], "hi");
        assert_eq!(b["max_tokens"], 100);
    }

    #[test]
    fn openai_style_request_and_reply() {
        let http = FakeAi::new(r#"{"choices":[{"message":{"content":"Hi there"}}]}"#);
        let cfg = AiConfig {
            provider: Provider::OpenAi,
            ..Default::default()
        };
        let client = AiClient {
            http: &http,
            config: &cfg,
            api_key: "sk-o",
        };
        assert_eq!(client.complete("sys", "hi", 50).unwrap(), "Hi there");
        let sent = http.sent.lock().unwrap();
        assert_eq!(sent[0].0, "https://api.openai.com/v1/chat/completions");
        assert!(
            sent[0]
                .1
                .contains(&("authorization".into(), "Bearer sk-o".into()))
        );
        let b: Value = serde_json::from_str(&sent[0].2).unwrap();
        assert_eq!(b["messages"][0]["role"], "system");
        assert_eq!(b["model"], "gpt-5");
    }

    #[test]
    fn custom_provider_needs_no_key_and_uses_its_address() {
        let http = FakeAi::new(r#"{"choices":[{"message":{"content":"ok"}}]}"#);
        let cfg = AiConfig {
            provider: Provider::Custom,
            custom_model: "llama3".into(),
            custom_base_url: "http://localhost:11434/v1/".into(),
            ..Default::default()
        };
        let client = AiClient {
            http: &http,
            config: &cfg,
            api_key: "",
        };
        client.test().unwrap();
        let sent = http.sent.lock().unwrap();
        assert_eq!(sent[0].0, "http://localhost:11434/v1/chat/completions");
        assert!(sent[0].1.is_empty());
    }

    #[test]
    fn missing_key_or_model_is_explained() {
        let http = FakeAi::new("{}");
        let cfg = AiConfig::default();
        let no_key = AiClient {
            http: &http,
            config: &cfg,
            api_key: "  ",
        };
        assert!(
            no_key
                .complete("s", "u", 1)
                .unwrap_err()
                .to_string()
                .contains("API key")
        );
        let cfg = AiConfig {
            provider: Provider::Custom,
            ..Default::default()
        };
        let no_model = AiClient {
            http: &http,
            config: &cfg,
            api_key: "",
        };
        assert!(
            no_model
                .complete("s", "u", 1)
                .unwrap_err()
                .to_string()
                .contains("model")
        );
        assert!(http.sent.lock().unwrap().is_empty());
    }

    #[test]
    fn provider_errors_get_hints() {
        assert!(
            friendly_error("the server answered 401 Unauthorized: bad key")
                .contains("rejected the API key")
        );
        assert!(
            friendly_error("the server answered 429 Too Many Requests")
                .contains("too many requests")
        );
        assert_eq!(friendly_error("timed out"), "timed out");
    }

    #[test]
    fn unreadable_replies_are_errors() {
        assert!(extract_text(Provider::Anthropic, "not json").is_err());
        assert!(extract_text(Provider::OpenAi, r#"{"choices":[]}"#).is_err());
    }
}
