//! HTML conversion. Converts to Markdown via `htmd` and extracts `<title>` via `scraper`.

use scraper::{Html, Selector};

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;
use crate::util::decode_text;

/// Converts HTML to Markdown.
pub struct HtmlConverter;

/// Extracts `<title>` from an HTML string.
pub(crate) fn extract_title(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("title").ok()?;
    let title = document
        .select(&selector)
        .next()?
        .text()
        .collect::<String>()
        .trim()
        .to_string();
    (!title.is_empty()).then_some(title)
}

impl Converter for HtmlConverter {
    fn convert(
        &self,
        src: &SourceContent,
        _ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        let html = decode_text(&src.bytes, src.charset.as_deref());
        let markdown =
            htmd::convert(&html).map_err(|e| MarkItDownError::decode("html", e.to_string()))?;
        let title = extract_title(&html);
        Ok(ConversionResult { markdown, title })
    }
}
