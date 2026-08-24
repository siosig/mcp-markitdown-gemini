//! Gemini `generateContent` client backed by the `gemini-genai` SDK
//! (contracts/gemini_client_contract.md, research.md D3-D8 of 004-genai-rs-migration).
//!
//! Sends a PDF as inline data and returns the generated Markdown text plus the
//! `finishReason`. Hard failures (auth/rate/server/network/timeout) map to typed
//! `Decode` errors with the same category strings as the hand-rolled client this
//! replaced. One call = one HTTP attempt (no SDK retry) on a fresh client.

use gemini_genai::types::{
    GenerateContentConfig, GenerateContentResponse, HttpOptions, Part, ThinkingConfig,
};

use super::config::{GeminiConfig, ModelTier};
use crate::error::MarkItDownError;

/// Instruction sent alongside the PDF.
const PROMPT: &str = "Convert this PDF document to clean GitHub-Flavored Markdown. \
Preserve headings, paragraphs, lists, and tables (render tables as Markdown tables). \
Output only the Markdown, with no preamble or explanation.";

/// Successful Gemini response, distilled to the fields the engine needs.
#[derive(Debug, Clone)]
pub struct GeminiResponse {
    /// Concatenated non-thought text parts.
    pub text: String,
    /// `candidates[0].finishReason` (defaults to `STOP` when absent).
    pub finish_reason: String,
}

/// Calls `generateContent` for `tier`, sending `pdf_bytes` inline.
pub fn generate(
    cfg: &GeminiConfig,
    tier: &ModelTier,
    pdf_bytes: &[u8],
) -> Result<GeminiResponse, MarkItDownError> {
    // The SDK's blocking client refuses to run wherever a tokio runtime context
    // is detectable (`Error::BlockingInsideRuntime`) — and `spawn_blocking`
    // threads, where markitdown-mcp invokes this crate, still carry that
    // context. A fresh OS thread carries none, so the entire client lifecycle
    // (build, call, drop) happens there.
    std::thread::scope(|scope| {
        scope
            .spawn(|| generate_on_clean_thread(cfg, tier, pdf_bytes))
            .join()
            .unwrap_or_else(|_| {
                Err(MarkItDownError::decode(
                    "pdf-gemini",
                    "conversion worker thread panicked",
                ))
            })
    })
}

/// Builds a one-shot SDK client and performs the request. Must run on a thread
/// with no tokio runtime context (see [`generate`]).
fn generate_on_clean_thread(
    cfg: &GeminiConfig,
    tier: &ModelTier,
    pdf_bytes: &[u8],
) -> Result<GeminiResponse, MarkItDownError> {
    let timeout_ms = i64::try_from(cfg.timeout_secs.saturating_mul(1000)).unwrap_or(i64::MAX);
    let client = gemini_genai::blocking::Client::builder()
        .api_key(cfg.api_key.clone())
        .http_options(HttpOptions {
            base_url: Some(cfg.base_url.clone()),
            timeout: Some(timeout_ms),
            // `retry_options` stays `None`: a single attempt, matching the
            // hand-rolled client this replaced. The mock-server tests route
            // requests by call order, so an SDK-level retry would silently
            // reach the escalation tier's canned response.
            ..Default::default()
        })
        .build()
        .map_err(map_sdk_error)?;

    let contents = vec![
        Part::from_bytes(pdf_bytes.to_vec(), "application/pdf"),
        Part::from_text(PROMPT),
    ];
    let config = GenerateContentConfig {
        temperature: Some(0.0),
        max_output_tokens: Some(8192),
        thinking_config: Some(ThinkingConfig {
            thinking_level: Some(tier.thinking_level.clone()),
            ..Default::default()
        }),
        ..Default::default()
    };

    let response = client
        .models()
        .generate_content(&tier.model, contents, Some(config))
        .map_err(map_sdk_error)?;

    distill(response)
}

/// Reduces an SDK response to [`GeminiResponse`], preserving the hand-rolled
/// client's semantics: no candidates is a "prompt blocked" error, thought parts
/// are excluded from the text (the SDK's `text()` already does that), and a
/// missing finish reason defaults to `STOP`.
fn distill(response: GenerateContentResponse) -> Result<GeminiResponse, MarkItDownError> {
    let has_candidate = response
        .candidates
        .as_deref()
        .is_some_and(|c| !c.is_empty());
    if !has_candidate {
        let block = response
            .prompt_feedback
            .as_ref()
            .and_then(|feedback| feedback.block_reason.as_ref())
            .map_or_else(
                || "no candidates returned".to_string(),
                |b| b.as_str().to_string(),
            );
        return Err(MarkItDownError::decode(
            "pdf-gemini",
            format!("prompt blocked: {block}"),
        ));
    }

    let finish_reason = response
        .candidates
        .as_deref()
        .and_then(|c| c.first())
        .and_then(|c| c.finish_reason.as_ref())
        .map_or_else(|| "STOP".to_string(), |r| r.as_str().to_string());

    Ok(GeminiResponse {
        text: response.text().unwrap_or_default(),
        finish_reason,
    })
}

/// Maps SDK errors onto the category strings the engine (and its tests) rely
/// on: auth / rate-limit / server / http by status code, timeout / network for
/// transport failures, and a generic label for anything else.
fn map_sdk_error(err: gemini_genai::Error) -> MarkItDownError {
    use gemini_genai::Error;

    let message = match err {
        Error::Api(api) => {
            let kind = match api.code {
                400 | 403 => "auth",
                429 => "rate-limit",
                500..=599 => "server",
                _ => "http",
            };
            format!("{kind} error ({}): {}", api.code, api.message)
        }
        Error::Http(e) if e.is_timeout() => format!("timeout error: {e}"),
        Error::Http(e) => format!("network error: {e}"),
        other => format!("gemini client error: {other}"),
    };
    MarkItDownError::decode("pdf-gemini", message)
}
