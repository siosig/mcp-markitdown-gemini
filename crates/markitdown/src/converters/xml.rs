//! XML conversion — indent-formats the XML and wraps it in a ```xml code fence.
//! Falls back to wrapping the raw text if formatting fails (robustness first).

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

use crate::error::MarkItDownError;
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;
use crate::util::decode_text;

/// Pretty-prints XML. Returns the input as-is on failure.
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

/// Wraps XML in a Markdown code fence and returns the result.
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
