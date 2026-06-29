//! Gemini-based PDF conversion: configuration, REST client, and accuracy assessment.
//!
//! Used by [`crate::converters::pdf::PdfConverter`] when a `GeminiConfig` is injected
//! (i.e. `GEMINI_API_KEY` is set). See research.md and contracts/gemini_pdf_conversion.md.

pub mod assess;
pub mod client;
pub mod config;

pub use config::{AccuracyThresholds, GeminiConfig, ModelTier};
