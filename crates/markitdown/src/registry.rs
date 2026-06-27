//! コンバーターのレジストリと dispatch (data-model.md)。
//! ZIP コンバーターは `Registry` を再帰利用して内包ファイルを変換する (FR-008)。

use std::collections::HashMap;

use crate::converters;
use crate::detect::{detect, DetectedFormat};
use crate::error::MarkItDownError;
use crate::source::SourceContent;

/// 変換結果。
#[derive(Debug, Clone, Default)]
pub struct ConversionResult {
    /// 変換後 Markdown 本文。
    pub markdown: String,
    /// 文書タイトル (取得できた場合)。
    pub title: Option<String>,
}

impl ConversionResult {
    /// タイトル無しの結果を作る。
    pub fn text(markdown: impl Into<String>) -> Self {
        Self {
            markdown: markdown.into(),
            title: None,
        }
    }
}

/// フォーマット別コンバーターの共通インターフェース。
pub trait Converter: Send + Sync {
    /// `src` を Markdown へ変換する。`ctx` は内包コンテナの再帰変換に使う。
    fn convert(
        &self,
        src: &SourceContent,
        ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError>;
}

/// フォーマット → コンバーターのレジストリ。
pub struct Registry {
    converters: HashMap<DetectedFormat, Box<dyn Converter>>,
}

impl Registry {
    /// 全フォーマットのコンバーターを登録した既定レジストリを構築する (FR-006)。
    pub fn with_defaults() -> Self {
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
        converters.insert(DetectedFormat::Pdf, Box::new(converters::pdf::PdfConverter));
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

    /// `src` を判定し、対応コンバーターで変換する。未対応は `UnsupportedFormat`。
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
