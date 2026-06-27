//! ZIP 変換。内包の各対応ファイルを Registry で再帰変換し、見出し付きで連結する (FR-008)。
//! ネストした ZIP も再帰的に処理する。

use std::io::{Cursor, Read};

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::{Origin, SourceContent};

/// ZIP を展開し、各エントリを変換して連結する。
pub struct ZipConverter;

impl Converter for ZipConverter {
    fn convert(
        &self,
        src: &SourceContent,
        ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        let mut archive = zip::ZipArchive::new(Cursor::new(&src.bytes[..]))
            .map_err(|e| MarkItDownError::decode("zip", e.to_string()))?;

        let mut markdown = String::new();

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| MarkItDownError::decode("zip", e.to_string()))?;

            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_string();

            let mut bytes = Vec::new();
            if entry.read_to_end(&mut bytes).is_err() {
                markdown.push_str(&format!("### {name}\n\n_(skipped: read error)_\n\n"));
                continue;
            }

            let inner = SourceContent {
                bytes,
                mime: None,
                filename: Some(name.clone()),
                charset: None,
                origin: Origin::Inline,
            };

            markdown.push_str(&format!("### {name}\n\n"));
            match ctx.convert(&inner) {
                Ok(result) => {
                    markdown.push_str(result.markdown.trim_end());
                    markdown.push_str("\n\n");
                }
                Err(err) => {
                    markdown.push_str(&format!("_(skipped: {err})_\n\n"));
                }
            }
        }

        Ok(ConversionResult::text(markdown.trim_end().to_string()))
    }
}
