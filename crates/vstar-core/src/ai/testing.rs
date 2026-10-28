//! A stand-in AI provider for tests.

use std::sync::Mutex;

use crate::http::HttpClient;
use crate::{Error, Result};

/// One POST: `(url, headers, body)`.
pub type Sent = (String, Vec<(String, String)>, String);

/// Records POSTs and replies with a canned body.
pub struct FakeAi {
    pub reply: String,
    pub sent: Mutex<Vec<Sent>>,
}

impl FakeAi {
    pub fn new(reply: &str) -> Self {
        Self {
            reply: reply.into(),
            sent: Mutex::new(Vec::new()),
        }
    }
}

impl HttpClient for FakeAi {
    fn get_text(&self, _: &str) -> Result<String> {
        Err(Error::Network("no GET".into()))
    }

    fn post_json(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<String> {
        let h = headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.sent.lock().unwrap().push((url.into(), h, body.into()));
        Ok(self.reply.clone())
    }
}
