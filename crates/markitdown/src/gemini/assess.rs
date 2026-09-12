//! Heuristic accuracy assessment of a Gemini conversion result (research.md §4, data-model.md §3).
//!
//! Pure function: decides whether the primary (flash) output is good enough or whether to
//! escalate to the pro tier. Page-count-free; uses output text shape and source byte size.

use super::config::AccuracyThresholds;

/// Outcome of an accuracy assessment.
#[derive(Debug, Clone, PartialEq)]
pub enum Accuracy {
    /// Output is good enough; no escalation needed.
    Sufficient,
    /// Output is inadequate; carries a typed, human-readable reason.
    Insufficient(InsufficientReason),
}

/// Why a single Gemini attempt's output was judged insufficient.
///
/// Three of these four are reliable evidence of a bad extraction on their own: the model
/// itself reports it didn't finish, the output is empty, or the output is corrupted. The
/// fourth, [`LowDensity`](InsufficientReason::LowDensity), is not — see its doc comment —
/// which is why callers should consult [`is_reliable_failure`](Self::is_reliable_failure)
/// rather than treating every `Insufficient` the same way once escalation has already run
/// once (see `crates/markitdown/src/converters/pdf.rs`).
#[derive(Debug, Clone, PartialEq)]
pub enum InsufficientReason {
    /// `finishReason != "STOP"`: truncated, blocked, or otherwise incomplete.
    NotStopped(String),
    /// Trimmed output was empty.
    Empty,
    /// U+FFFD replacement-character ratio exceeded `thresholds.garble_ratio`.
    Garbled(f64),
    /// Output characters per KB of source PDF fell below `thresholds.min_chars_per_kb`.
    ///
    /// Unlike the other three reasons, this is *not* on its own reliable evidence of a bad
    /// extraction. PDF byte size is dominated by embedded raster images for scanned or
    /// diagram-heavy documents, so a large PDF with genuinely sparse text (an illustrated
    /// manual where most pages are diagrams with a caption or two, say) can legitimately
    /// produce a low characters-per-KB ratio even on a complete, accurate read — Gemini
    /// reads PDF pages natively as images regardless of thinking level, so there is no
    /// separate "OCR mode" that a low ratio indicates was skipped. It remains useful as an
    /// *escalation* trigger (worth trying the higher-thinking tier once, in case that tier
    /// genuinely does better), but a low ratio that persists after that retry is not, by
    /// itself, grounds to discard a complete and uncorrupted result.
    LowDensity(f64),
}

impl InsufficientReason {
    /// Whether this reason is reliable evidence of a bad extraction on its own — i.e.
    /// whether it should still cause a hard failure even after the escalation tier has
    /// already been tried once. Only [`LowDensity`](Self::LowDensity) is not; see its doc
    /// comment.
    pub fn is_reliable_failure(&self) -> bool {
        !matches!(self, Self::LowDensity(_))
    }
}

impl std::fmt::Display for InsufficientReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStopped(reason) => write!(f, "finishReason={reason}"),
            Self::Empty => write!(f, "empty output"),
            Self::Garbled(ratio) => write!(f, "garble ratio {ratio:.3}"),
            Self::LowDensity(density) => write!(f, "low density {density:.2} chars/KB"),
        }
    }
}

/// Assesses `markdown` produced for a PDF of `pdf_len` bytes that finished with `finish_reason`.
///
/// Triggers `Insufficient` when any of the following holds (FR-002b):
/// 1. `finish_reason != "STOP"` (truncated / blocked / incomplete)
/// 2. trimmed output is empty
/// 3. U+FFFD replacement-character ratio exceeds `thresholds.garble_ratio`
/// 4. for PDFs larger than `density_apply_min_bytes`, output chars-per-KB is below `min_chars_per_kb`
///
/// This function only classifies a single attempt; it does not decide what to do about an
/// `Insufficient` result. That decision (escalate, hard-fail, or accept as best-effort)
/// belongs to the caller, which has the context of how many attempts have already run — see
/// [`InsufficientReason::is_reliable_failure`].
pub fn assess(
    markdown: &str,
    finish_reason: &str,
    pdf_len: usize,
    thresholds: &AccuracyThresholds,
) -> Accuracy {
    if finish_reason != "STOP" {
        return Accuracy::Insufficient(InsufficientReason::NotStopped(finish_reason.to_string()));
    }

    if markdown.trim().is_empty() {
        return Accuracy::Insufficient(InsufficientReason::Empty);
    }

    let total = markdown.chars().count();
    let garble = markdown.chars().filter(|&c| c == '\u{FFFD}').count();
    let garble_ratio = garble as f64 / total as f64;
    if garble_ratio > thresholds.garble_ratio {
        return Accuracy::Insufficient(InsufficientReason::Garbled(garble_ratio));
    }

    if pdf_len > thresholds.density_apply_min_bytes {
        let kb = pdf_len as f64 / 1024.0;
        let density = total as f64 / kb;
        if density < thresholds.min_chars_per_kb {
            return Accuracy::Insufficient(InsufficientReason::LowDensity(density));
        }
    }

    Accuracy::Sufficient
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> AccuracyThresholds {
        AccuracyThresholds::default()
    }

    #[test]
    fn sufficient_normal_output() {
        let md = "# Title\n\nA reasonable paragraph of text.";
        assert_eq!(assess(md, "STOP", 1000, &t()), Accuracy::Sufficient);
    }

    #[test]
    fn insufficient_when_empty() {
        assert!(matches!(
            assess("   \n  ", "STOP", 1000, &t()),
            Accuracy::Insufficient(InsufficientReason::Empty)
        ));
    }

    #[test]
    fn insufficient_when_finish_reason_not_stop() {
        assert!(matches!(
            assess("# Plenty of text here", "MAX_TOKENS", 1000, &t()),
            Accuracy::Insufficient(InsufficientReason::NotStopped(_))
        ));
    }

    #[test]
    fn insufficient_when_garbled() {
        // 10 chars, 3 replacement characters => 30% > 2%.
        let md = "ab\u{FFFD}\u{FFFD}\u{FFFD}cdefg";
        assert!(matches!(
            assess(md, "STOP", 1000, &t()),
            Accuracy::Insufficient(InsufficientReason::Garbled(_))
        ));
    }

    #[test]
    fn insufficient_when_low_density_on_large_pdf() {
        // 2 MB PDF but only a few chars => far below 5 chars/KB.
        let md = "tiny";
        assert!(matches!(
            assess(md, "STOP", 2 * 1024 * 1024, &t()),
            Accuracy::Insufficient(InsufficientReason::LowDensity(_))
        ));
    }

    #[test]
    fn sufficient_short_text_on_small_pdf() {
        // Small PDF (< density_apply_min_bytes) => low-density check does not apply.
        let md = "short";
        assert_eq!(assess(md, "STOP", 1024, &t()), Accuracy::Sufficient);
    }

    #[test]
    fn only_low_density_is_not_a_reliable_failure() {
        assert!(!InsufficientReason::LowDensity(1.0).is_reliable_failure());
        assert!(InsufficientReason::Empty.is_reliable_failure());
        assert!(InsufficientReason::Garbled(0.5).is_reliable_failure());
        assert!(InsufficientReason::NotStopped("MAX_TOKENS".to_string()).is_reliable_failure());
    }
}
