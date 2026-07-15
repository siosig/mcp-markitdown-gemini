//! Gemini conversion settings and heuristic thresholds (data-model.md §2/§3).
//!
//! Values are resolved from environment variables in [`GeminiConfig::from_env`], but the
//! type itself carries no I/O so it can be constructed directly in tests (dependency injection).

/// Default Gemini API base URL.
pub const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com";

/// Primary-tier model id.
pub const PRIMARY_MODEL: &str = "gemini-2.5-flash-lite";
/// Escalation-tier model id.
pub const ESCALATION_MODEL: &str = "gemini-2.5-pro";

/// Per-tier model settings.
#[derive(Debug, Clone)]
pub struct ModelTier {
    /// Model id placed in the `models/{model}:generateContent` path.
    pub model: String,
    /// `generationConfig.thinkingConfig.thinkingLevel` value (Gemini 3.x/2.5 series enum).
    pub thinking_level: &'static str,
}

/// Thresholds for the accuracy heuristic that decides flash→pro escalation (research.md §4).
#[derive(Debug, Clone)]
pub struct AccuracyThresholds {
    /// Maximum tolerated ratio of U+FFFD replacement characters.
    pub garble_ratio: f64,
    /// Minimum tolerated output characters per KB of source PDF.
    pub min_chars_per_kb: f64,
    /// The low-density check only applies to PDFs at least this large (bytes).
    pub density_apply_min_bytes: usize,
}

impl Default for AccuracyThresholds {
    fn default() -> Self {
        Self {
            garble_ratio: 0.02,
            min_chars_per_kb: 5.0,
            density_apply_min_bytes: 50 * 1024,
        }
    }
}

/// Settings for converting PDFs via the Gemini API.
#[derive(Debug, Clone)]
pub struct GeminiConfig {
    /// API key (trimmed, non-empty); sent as the `x-goog-api-key` header.
    pub api_key: String,
    /// API base URL (overridable for tests).
    pub base_url: String,
    /// Primary conversion tier (flash / low).
    pub primary: ModelTier,
    /// Escalation conversion tier (pro / medium).
    pub escalation: ModelTier,
    /// Maximum PDF size sent via inline_data; larger inputs are rejected (research.md §3).
    pub inline_max_bytes: usize,
    /// HTTP timeout in seconds.
    pub timeout_secs: u64,
    /// Accuracy heuristic thresholds.
    pub thresholds: AccuracyThresholds,
}

impl GeminiConfig {
    /// Builds a config for `api_key` with default models/thresholds.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            primary: ModelTier {
                model: PRIMARY_MODEL.to_string(),
                thinking_level: "low",
            },
            escalation: ModelTier {
                model: ESCALATION_MODEL.to_string(),
                thinking_level: "medium",
            },
            inline_max_bytes: 20 * 1024 * 1024,
            timeout_secs: 120,
            thresholds: AccuracyThresholds::default(),
        }
    }

    /// Builds a config from `api_key`, applying optional environment overrides
    /// (`GEMINI_BASE_URL`, `GEMINI_INLINE_MAX_BYTES`, `GEMINI_TIMEOUT_SECS`,
    /// `GEMINI_GARBLE_RATIO`, `GEMINI_MIN_CHARS_PER_KB`, `GEMINI_DENSITY_MIN_BYTES`).
    pub fn from_env(api_key: String) -> Self {
        let mut cfg = Self::new(api_key);
        if let Some(url) = env_nonempty("GEMINI_BASE_URL") {
            cfg.base_url = url;
        }
        cfg.inline_max_bytes = env_parse("GEMINI_INLINE_MAX_BYTES", cfg.inline_max_bytes);
        cfg.timeout_secs = env_parse("GEMINI_TIMEOUT_SECS", cfg.timeout_secs);
        cfg.thresholds.garble_ratio = env_parse("GEMINI_GARBLE_RATIO", cfg.thresholds.garble_ratio);
        cfg.thresholds.min_chars_per_kb =
            env_parse("GEMINI_MIN_CHARS_PER_KB", cfg.thresholds.min_chars_per_kb);
        cfg.thresholds.density_apply_min_bytes = env_parse(
            "GEMINI_DENSITY_MIN_BYTES",
            cfg.thresholds.density_apply_min_bytes,
        );
        cfg
    }
}

/// Reads an env var, returning `Some` only if present and non-empty after trimming.
fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Reads and parses an env var, falling back to `default` on absence or parse failure.
fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    env_nonempty(key)
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}
