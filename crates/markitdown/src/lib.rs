//! # markitdown
//!
//! Conversion engine that converts documents pointed to by URIs (`http`/`https`/`file`/`data`) into Markdown.
//! Supports the same formats as `microsoft/markitdown` (spec FR-006).
//!
//! ```no_run
//! let md = markitdown::convert_to_string("file:///path/to/doc.pdf")?;
//! println!("{md}");
//! # Ok::<(), markitdown::MarkItDownError>(())
//! ```

pub mod acquire;
pub mod converters;
pub mod detect;
pub mod error;
pub mod registry;
pub mod source;
pub mod util;

pub use detect::DetectedFormat;
pub use error::MarkItDownError;
pub use registry::{ConversionResult, Converter, Registry};
pub use source::{Origin, SourceContent};

/// Fetches a URI, converts it to Markdown, and returns the result (body + title).
pub fn convert(uri: &str) -> Result<ConversionResult, MarkItDownError> {
    let source = acquire::acquire(uri)?;
    let registry = Registry::with_defaults();
    registry.convert(&source)
}

/// Convenience wrapper that converts a URI and returns only the Markdown body.
pub fn convert_to_string(uri: &str) -> Result<String, MarkItDownError> {
    Ok(convert(uri)?.markdown)
}

/// Converts an already-fetched byte slice (filename/MIME hints are optional).
pub fn convert_bytes(
    bytes: Vec<u8>,
    filename: Option<String>,
    mime: Option<String>,
) -> Result<ConversionResult, MarkItDownError> {
    let source = SourceContent {
        bytes,
        mime,
        filename,
        charset: None,
        origin: Origin::Inline,
    };
    Registry::with_defaults().convert(&source)
}
