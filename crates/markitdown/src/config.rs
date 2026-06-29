//! Engine-level runtime configuration (data-model.md §1).
//!
//! Resolved once at the composition root so converters receive their settings by injection
//! rather than reading the environment themselves (keeps converters pure and testable).

use crate::gemini::GeminiConfig;

/// Runtime configuration for the conversion engine.
#[derive(Debug, Clone, Default)]
pub struct EngineConfig {
    /// Gemini PDF-conversion settings. `Some` only when `GEMINI_API_KEY` is set and non-empty;
    /// `None` means PDFs use the built-in local extractor (FR-003/FR-007).
    pub gemini: Option<GeminiConfig>,
}

impl EngineConfig {
    /// Resolves configuration from the process environment.
    ///
    /// `GEMINI_API_KEY` is trimmed; empty or whitespace-only values are treated as unset
    /// (FR-009), yielding `gemini = None`.
    pub fn from_env() -> Self {
        let gemini = std::env::var("GEMINI_API_KEY")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(GeminiConfig::from_env);
        Self { gemini }
    }
}
