//! Typed errors (`thiserror`). `anyhow` is not used because this is a library crate.

use thiserror::Error;

/// Error type used throughout the conversion pipeline.
///
/// Every variant carries a human-readable message that identifies the cause
/// without terminating the process (spec FR-009/FR-010, SC-004).
#[derive(Error, Debug)]
pub enum MarkItDownError {
    /// Unsupported URI scheme (anything other than http/https/file/data).
    #[error("unsupported URI scheme: '{0}' (supported: http, https, file, data)")]
    UnsupportedScheme(String),

    /// Failed to parse the URI.
    #[error("invalid URI: {0}")]
    InvalidUri(String),

    /// Failed to acquire content (permission denied, not found, network error, 404, timeout, etc.).
    #[error("failed to acquire content from '{uri}': {message}")]
    Acquire { uri: String, message: String },

    /// Unrecognized or unsupported format.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// Corrupted data or parse failure.
    #[error("failed to decode {format}: {message}")]
    Decode { format: String, message: String },

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl MarkItDownError {
    /// Helper to concisely construct an acquire error.
    pub fn acquire(uri: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Acquire {
            uri: uri.into(),
            message: message.into(),
        }
    }

    /// Helper to concisely construct a decode error.
    pub fn decode(format: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Decode {
            format: format.into(),
            message: message.into(),
        }
    }
}
