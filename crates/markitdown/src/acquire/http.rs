//! `http:` / `https:` scheme acquisition (US2). Uses `reqwest` blocking client + rustls.

use url::Url;

use super::{filename_from_url, Acquirer};
use crate::error::MarkItDownError;
use crate::source::{Origin, SourceContent};

/// Acquirer that fetches remote content.
pub struct HttpAcquirer {
    user_agent: String,
}

impl Default for HttpAcquirer {
    fn default() -> Self {
        Self {
            user_agent: concat!("markitdown-gemini/", env!("CARGO_PKG_VERSION")).to_string(),
        }
    }
}

/// Splits a content-type value into its MIME type and charset.
fn split_content_type(value: &str) -> (Option<String>, Option<String>) {
    let mut parts = value.split(';');
    let mime = parts
        .next()
        .map(|m| m.trim().to_lowercase())
        .filter(|m| !m.is_empty());
    let mut charset = None;
    for param in parts {
        let param = param.trim();
        if let Some(rest) = param.strip_prefix("charset=") {
            charset = Some(rest.trim_matches('"').to_string());
        }
    }
    (mime, charset)
}

impl Acquirer for HttpAcquirer {
    fn acquire(&self, url: &Url) -> Result<SourceContent, MarkItDownError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(&self.user_agent)
            .build()
            .map_err(|e| MarkItDownError::acquire(url.as_str(), e.to_string()))?;

        let response = client
            .get(url.clone())
            .send()
            .map_err(|e| MarkItDownError::acquire(url.as_str(), e.to_string()))?;

        let response = response
            .error_for_status()
            .map_err(|e| MarkItDownError::acquire(url.as_str(), e.to_string()))?;

        let final_url = response.url().clone();
        let (mime, charset) = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(split_content_type)
            .unwrap_or((None, None));

        let bytes = response
            .bytes()
            .map_err(|e| MarkItDownError::acquire(url.as_str(), e.to_string()))?
            .to_vec();

        Ok(SourceContent {
            bytes,
            mime,
            filename: filename_from_url(&final_url),
            charset,
            origin: Origin::Remote(final_url),
        })
    }
}
