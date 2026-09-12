//! PDF conversion.
//!
//! Two routes (research.md §5, data-model.md §5):
//! - No `GeminiConfig` (GEMINI_API_KEY unset): local text extraction via `pdf-extract`.
//! - With `GeminiConfig`: Gemini flash-lite(thinking low) → accuracy assessment →
//!   escalate to the same model at thinking high → assessment → accept-or-error. No
//!   fallback to local on Gemini failure (FR-006a). "Accept-or-error" because not every
//!   insufficiency reason means the same thing after escalation has already been tried
//!   once: see [`crate::gemini::InsufficientReason`] and the escalation-tier match below.

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

    /// Gemini route: primary → assess → (escalate to thinking high) → assess → error.
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
            // A low chars-per-KB ratio is not, by itself, reliable evidence of a bad
            // extraction (see InsufficientReason::LowDensity): image/diagram-heavy PDFs
            // can legitimately produce sparse text even on a complete, accurate read,
            // since Gemini reads PDF pages as images natively regardless of thinking
            // level — there is no separate "OCR mode" a low ratio means was skipped.
            // Once the escalation tier has already been tried, discarding a complete
            // (finishReason=STOP) and uncorrupted result over this heuristic alone would
            // throw away genuine output in exchange for nothing (there is no next tier to
            // retry with), so accept it as best-effort instead of hard-failing.
            Accuracy::Insufficient(reason) if !reason.is_reliable_failure() => {
                tracing::warn!(reason = %reason, model = %cfg.escalation.model, "gemini escalation still low-density; accepting as best-effort");
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
