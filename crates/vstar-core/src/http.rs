//! A tiny HTTP abstraction so data sources can be tested with fixture files
//! instead of live web services.

use std::time::Duration;

use crate::{Error, Result};

pub trait HttpClient: Send + Sync {
    /// Fetch `url` and return the response body as text.
    fn get_text(&self, url: &str) -> Result<String>;

    /// Like `get_text`, with extra request `headers` (for authenticated APIs).
    fn get_text_with_headers(&self, _url: &str, _headers: &[(&str, &str)]) -> Result<String> {
        Err(Error::Network(
            "this HTTP client can't send custom headers".into(),
        ))
    }

    /// Fetch `url` and return the response body as raw bytes (images).
    fn get_bytes(&self, _url: &str) -> Result<Vec<u8>> {
        Err(Error::Network(
            "this HTTP client can't download binary files".into(),
        ))
    }

    /// POST a JSON `body` to `url` with extra `headers` and return the
    /// response body. On an error status the returned error includes the
    /// server's explanation, which AI providers use for "invalid API key"
    /// and "rate limited" messages.
    fn post_json(&self, _url: &str, _headers: &[(&str, &str)], _body: &str) -> Result<String> {
        Err(Error::Network(
            "this HTTP client can't send POST requests".into(),
        ))
    }
}

/// The real client, backed by `reqwest`. It uses the operating system's TLS
/// and certificate store (SChannel on Windows) and its proxy settings.
pub struct ReqwestClient {
    inner: reqwest::blocking::Client,
}

impl ReqwestClient {
    pub fn new() -> Result<Self> {
        let inner = reqwest::blocking::Client::builder()
            .user_agent(concat!("FeStar/", env!("CARGO_PKG_VERSION")))
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

    fn get_text_with_headers(&self, url: &str, headers: &[(&str, &str)]) -> Result<String> {
        let mut req = self.inner.get(url);
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        let resp = req.send().map_err(|e| Error::Network(e.to_string()))?;
        let status = resp.status();
        let text = resp.text().map_err(|e| Error::Network(e.to_string()))?;
        if !status.is_success() {
            let detail: String = text.chars().take(400).collect();
            return Err(Error::Network(format!(
                "the server answered {status}: {detail}"
            )));
        }
        Ok(text)
    }

    fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
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
        resp.bytes()
            .map(|b| b.to_vec())
            .map_err(|e| Error::Network(e.to_string()))
    }

    fn post_json(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<String> {
        let mut req = self
            .inner
            .post(url)
            .header("content-type", "application/json")
            .body(body.to_string());
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        let resp = req.send().map_err(|e| Error::Network(e.to_string()))?;
        let status = resp.status();
        let text = resp.text().map_err(|e| Error::Network(e.to_string()))?;
        if !status.is_success() {
            let detail: String = text.chars().take(400).collect();
            return Err(Error::Network(format!(
                "the server answered {status}: {detail}"
            )));
        }
        Ok(text)
    }
}
