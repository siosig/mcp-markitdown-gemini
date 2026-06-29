//! Heuristic accuracy assessment of a Gemini conversion result (research.md §4, data-model.md §3).
//!
//! Pure function: decides whether the primary (flash) output is good enough or whether to
//! escalate to the pro tier. Page-count-free; uses output text shape and source byte size.

use super::config::AccuracyThresholds;

/// Outcome of an accuracy assessment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accuracy {
    /// Output is good enough; no escalation needed.
    Sufficient,
    /// Output is inadequate; carries a human-readable reason.
    Insufficient(String),
}

/// Assesses `markdown` produced for a PDF of `pdf_len` bytes that finished with `finish_reason`.
///
/// Triggers `Insufficient` when any of the following holds (FR-002b):
/// 1. `finish_reason != "STOP"` (truncated / blocked / incomplete)
/// 2. trimmed output is empty
/// 3. U+FFFD replacement-character ratio exceeds `thresholds.garble_ratio`
/// 4. for PDFs larger than `density_apply_min_bytes`, output chars-per-KB is below `min_chars_per_kb`
pub fn assess(
    markdown: &str,
    finish_reason: &str,
    pdf_len: usize,
    thresholds: &AccuracyThresholds,
) -> Accuracy {
    if finish_reason != "STOP" {
        return Accuracy::Insufficient(format!("finishReason={finish_reason}"));
    }

    if markdown.trim().is_empty() {
        return Accuracy::Insufficient("empty output".to_string());
    }

    let total = markdown.chars().count();
    let garble = markdown.chars().filter(|&c| c == '\u{FFFD}').count();
    let garble_ratio = garble as f64 / total as f64;
    if garble_ratio > thresholds.garble_ratio {
        return Accuracy::Insufficient(format!("garble ratio {garble_ratio:.3}"));
    }

    if pdf_len > thresholds.density_apply_min_bytes {
        let kb = pdf_len as f64 / 1024.0;
        let density = total as f64 / kb;
        if density < thresholds.min_chars_per_kb {
            return Accuracy::Insufficient(format!("low density {density:.2} chars/KB"));
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
            Accuracy::Insufficient(_)
        ));
    }

    #[test]
    fn insufficient_when_finish_reason_not_stop() {
        assert!(matches!(
            assess("# Plenty of text here", "MAX_TOKENS", 1000, &t()),
            Accuracy::Insufficient(_)
        ));
    }

    #[test]
    fn insufficient_when_garbled() {
        // 10 chars, 3 replacement characters => 30% > 2%.
        let md = "ab\u{FFFD}\u{FFFD}\u{FFFD}cdefg";
        assert!(matches!(
            assess(md, "STOP", 1000, &t()),
            Accuracy::Insufficient(_)
        ));
    }

    #[test]
    fn insufficient_when_low_density_on_large_pdf() {
        // 2 MB PDF but only a few chars => far below 5 chars/KB.
        let md = "tiny";
        assert!(matches!(
            assess(md, "STOP", 2 * 1024 * 1024, &t()),
            Accuracy::Insufficient(_)
        ));
    }

    #[test]
    fn sufficient_short_text_on_small_pdf() {
        // Small PDF (< density_apply_min_bytes) => low-density check does not apply.
        let md = "short";
        assert_eq!(assess(md, "STOP", 1024, &t()), Accuracy::Sufficient);
    }
}
