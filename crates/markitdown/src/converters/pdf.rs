//! PDF conversion.
//!
//! Two routes (research.md §5, data-model.md §5):
//! - No `GeminiConfig` (GEMINI_API_KEY unset): local text extraction via `pdf-extract`.
//! - With `GeminiConfig`: Gemini flash(low) → accuracy assessment → escalate to pro(medium) →
//!   assessment → error. No fallback to local on Gemini failure (FR-006a).

use crate::error::MarkItDownError;
use crate::gemini::assess::{assess, Accuracy};
use crate::gemini::{client, GeminiConfig};
use crate::registry::{ConversionResult, Converter, Registry};
use crate::source::SourceContent;

/// Converts PDFs, optionally via Gemini when a config is injected.
pub struct PdfConverter {
    gemini: Option<GeminiConfig>,
}

impl PdfConverter {
    /// Creates a converter. `gemini = Some` enables the Gemini route; `None` uses local extraction.
    pub fn new(gemini: Option<GeminiConfig>) -> Self {
        Self { gemini }
    }

    /// Local extraction (backward-compatible path, no external network access).
    fn convert_local(&self, src: &SourceContent) -> Result<ConversionResult, MarkItDownError> {
        let text = pdf_extract::extract_text_from_mem(&src.bytes)
            .map_err(|e| MarkItDownError::decode("pdf", e.to_string()))?;
        Ok(ConversionResult::text(text.trim().to_string()))
    }

    /// Gemini route: flash → assess → (escalate) pro → assess → error.
    fn convert_gemini(
        &self,
        cfg: &GeminiConfig,
        src: &SourceContent,
    ) -> Result<ConversionResult, MarkItDownError> {
        let pdf_len = src.bytes.len();
        if pdf_len > cfg.inline_max_bytes {
            return Err(MarkItDownError::decode(
                "pdf-gemini",
                format!(
                    "file too large for inline conversion ({pdf_len} bytes > limit {})",
                    cfg.inline_max_bytes
                ),
            ));
        }

        // Primary tier (flash / low).
        let primary = client::generate(cfg, &cfg.primary, &src.bytes)?;
        match assess(
            &primary.text,
            &primary.finish_reason,
            pdf_len,
            &cfg.thresholds,
        ) {
            Accuracy::Sufficient => {
                tracing::info!(route = "gemini-primary", model = %cfg.primary.model, "pdf converted via gemini");
                return Ok(ConversionResult::text(primary.text.trim().to_string()));
            }
            Accuracy::Insufficient(reason) => {
                tracing::warn!(reason = %reason, model = %cfg.primary.model, "gemini primary insufficient; escalating");
            }
        }

        // Escalation tier (pro / medium).
        let escalated = client::generate(cfg, &cfg.escalation, &src.bytes)?;
        match assess(
            &escalated.text,
            &escalated.finish_reason,
            pdf_len,
            &cfg.thresholds,
        ) {
            Accuracy::Sufficient => {
                tracing::info!(route = "gemini-escalated", model = %cfg.escalation.model, "pdf converted via gemini escalation");
                Ok(ConversionResult::text(escalated.text.trim().to_string()))
            }
            Accuracy::Insufficient(reason) => {
                tracing::error!(reason = %reason, model = %cfg.escalation.model, "gemini escalation still insufficient");
                Err(MarkItDownError::decode(
                    "pdf-gemini",
                    format!("escalation insufficient: {reason}"),
                ))
            }
        }
    }
}

impl Converter for PdfConverter {
    fn convert(
        &self,
        src: &SourceContent,
        _ctx: &Registry,
    ) -> Result<ConversionResult, MarkItDownError> {
        match &self.gemini {
            Some(cfg) => self.convert_gemini(cfg, src),
            None => {
                tracing::debug!(route = "local", "pdf converted via local extraction");
                self.convert_local(src)
            }
        }
    }
}
