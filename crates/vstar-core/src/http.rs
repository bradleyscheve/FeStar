//! A tiny HTTP abstraction so data sources can be tested with fixture files
//! instead of live web services.

use std::time::Duration;

use crate::{Error, Result};

pub trait HttpClient: Send + Sync {
    /// Fetch `url` and return the response body as text.
    fn get_text(&self, url: &str) -> Result<String>;
}

/// The real client, backed by `reqwest`. It uses the operating system's TLS
/// and certificate store (SChannel on Windows) and its proxy settings.
pub struct ReqwestClient {
    inner: reqwest::blocking::Client,
}

impl ReqwestClient {
    pub fn new() -> Result<Self> {
        let inner = reqwest::blocking::Client::builder()
            .user_agent(concat!("VStar-RS/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(Self { inner })
    }
}

impl HttpClient for ReqwestClient {
    fn get_text(&self, url: &str) -> Result<String> {
        let resp = self
            .inner
            .get(url)
            .send()
            .map_err(|e| Error::Network(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Error::Network(format!(
                "server answered {status} for {url}"
            )));
        }
        resp.text().map_err(|e| Error::Network(e.to_string()))
    }
}
