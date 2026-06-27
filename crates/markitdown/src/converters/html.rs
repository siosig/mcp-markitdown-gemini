//! HTML 変換。`htmd` で Markdown 化し、`scraper` で `<title>` を抽出する。

use scraper::{Html, Selector};

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;
use crate::util::decode_text;

/// HTML を Markdown へ変換する。
pub struct HtmlConverter;

/// HTML 文字列から `<title>` を取り出す。
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
