//! # markitdown
//!
//! URI (`http`/`https`/`file`/`data`) の指す文書を Markdown に変換する変換エンジン。
//! `microsoft/markitdown` 相当のフォーマットに対応する (spec FR-006)。
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

/// URI を取得して Markdown へ変換し、結果 (本文 + タイトル) を返す。
pub fn convert(uri: &str) -> Result<ConversionResult, MarkItDownError> {
    let source = acquire::acquire(uri)?;
    let registry = Registry::with_defaults();
    registry.convert(&source)
}

/// URI を変換し、Markdown 本文だけを返す簡易版。
pub fn convert_to_string(uri: &str) -> Result<String, MarkItDownError> {
    Ok(convert(uri)?.markdown)
}

/// 既に取得済みのバイト列を変換する (ファイル名/MIME ヒントを指定可能)。
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
