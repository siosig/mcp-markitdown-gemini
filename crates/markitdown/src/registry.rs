//! Converter registry and dispatch (data-model.md).
//! The ZIP converter recursively reuses `Registry` to convert files contained within the archive (FR-008).

use std::collections::HashMap;

use crate::config::EngineConfig;
use crate::converters;
use crate::detect::{detect, DetectedFormat};
use crate::error::MarkItDownError;
use crate::source::SourceContent;

/// Conversion result.
#[derive(Debug, Clone, Default)]
pub struct ConversionResult {
    /// Converted Markdown body.
    pub markdown: String,
    /// Document title (if one could be extracted).
    pub title: Option<String>,
}

impl ConversionResult {
    /// Creates a result with no title.
    pub fn text(markdown: impl Into<String>) -> Self {
        Self {
            markdown: markdown.into(),
            title: None,
        }
    }
}

/// Common interface for per-format converters.
pub trait Converter: Send + Sync {
    /// Converts `src` to Markdown. `ctx` is used for recursive conversion of nested containers.
    fn convert(
        &self,
        src: &SourceContent,
        ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError>;
}

/// Registry mapping formats to their respective converters.
pub struct Registry {
    converters: HashMap<DetectedFormat, Box<dyn Converter>>,
}

impl Registry {
    /// Builds the default registry, resolving configuration (incl. Gemini) from the environment.
    pub fn with_defaults() -> Self {
        Self::with_config(&EngineConfig::from_env())
    }

    /// Builds the registry using an explicit [`EngineConfig`] (dependency injection for tests
    /// and embedders). Only the PDF converter is configuration-aware; all other formats are
    /// unaffected (FR-004, SC-005).
    pub fn with_config(config: &EngineConfig) -> Self {
        let mut converters: HashMap<DetectedFormat, Box<dyn Converter>> = HashMap::new();
        converters.insert(
            DetectedFormat::PlainText,
            Box::new(converters::text::TextConverter),
        );
        converters.insert(
            DetectedFormat::Html,
            Box::new(converters::html::HtmlConverter),
        );
        converters.insert(DetectedFormat::Csv, Box::new(converters::csv::CsvConverter));
        converters.insert(
            DetectedFormat::Json,
            Box::new(converters::json::JsonConverter),
        );
        converters.insert(DetectedFormat::Xml, Box::new(converters::xml::XmlConverter));
        converters.insert(
            DetectedFormat::Pdf,
            Box::new(converters::pdf::PdfConverter::new(config.gemini.clone())),
        );
        converters.insert(
            DetectedFormat::Docx,
            Box::new(converters::docx::DocxConverter),
        );
        converters.insert(
            DetectedFormat::Pptx,
            Box::new(converters::pptx::PptxConverter),
        );
        converters.insert(
            DetectedFormat::Xlsx,
            Box::new(converters::xlsx::XlsxConverter),
        );
        converters.insert(
            DetectedFormat::Epub,
            Box::new(converters::epub::EpubConverter),
        );
        converters.insert(DetectedFormat::Zip, Box::new(converters::zip::ZipConverter));
        Self { converters }
    }

    /// Detects the format of `src` and converts it using the matching converter. Returns `UnsupportedFormat` if no converter is registered.
    pub fn convert(&self, src: &SourceContent) -> Result<ConversionResult, MarkItDownError> {
        let fmt = detect(src);
        match self.converters.get(&fmt) {
            Some(converter) => converter.convert(src, self),
            None => {
                let hint = src
                    .filename
                    .clone()
                    .or_else(|| src.mime.clone())
                    .unwrap_or_else(|| fmt.label().to_string());
                Err(MarkItDownError::UnsupportedFormat(hint))
            }
        }
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::with_defaults()
    }
}
