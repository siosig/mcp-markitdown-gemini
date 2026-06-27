//! XML 変換 → インデント整形して ```xml コードフェンスに包む。
//! 整形に失敗した場合は元テキストをそのまま包む (頑健性優先)。

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;
use crate::util::decode_text;

/// XML を整形する。失敗時は入力をそのまま返す。
fn pretty_print(text: &str) -> String {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(event) => {
                if writer.write_event(event).is_err() {
                    return text.to_string();
                }
            }
            Err(_) => return text.to_string(),
        }
    }
    String::from_utf8(writer.into_inner()).unwrap_or_else(|_| text.to_string())
}

/// XML を Markdown コードフェンスに包んで返す。
pub struct XmlConverter;

impl Converter for XmlConverter {
    fn convert(
        &self,
        src: &SourceContent,
        _ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        let text = decode_text(&src.bytes, src.charset.as_deref());
        let pretty = pretty_print(&text);
        Ok(ConversionResult::text(format!("```xml\n{pretty}\n```\n")))
    }
}
