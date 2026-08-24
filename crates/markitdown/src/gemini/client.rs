//! Blocking Gemini `generateContent` REST client (research.md §2/§8, contracts/gemini_pdf_conversion.md §3).
//!
//! Sends a PDF as inline base64 data and returns the generated Markdown text plus the
//! `finishReason`. Hard failures (auth/rate/server/network/timeout) map to typed `Decode` errors.

use base64::Engine as _;

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
    let encoded = base64::engine::general_purpose::STANDARD.encode(pdf_bytes);
    let url = format!(
        "{}/v1beta/models/{}:generateContent",
        cfg.base_url.trim_end_matches('/'),
        tier.model
    );

    let body = serde_json::json!({
        "contents": [{
            "parts": [
                { "inline_data": { "mime_type": "application/pdf", "data": encoded } },
                { "text": PROMPT }
            ]
        }],
        "generationConfig": {
            "thinkingConfig": { "thinkingLevel": tier.thinking_level },
            "maxOutputTokens": 8192,
            "temperature": 0
        }
    });

    let payload_out = serde_json::to_vec(&body).map_err(|e| {
        MarkItDownError::decode("pdf-gemini", format!("failed to encode request: {e}"))
    })?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(cfg.timeout_secs))
        .build()
        .map_err(|e| MarkItDownError::decode("pdf-gemini", format!("client build failed: {e}")))?;

    let response = client
        .post(&url)
        .header("x-goog-api-key", &cfg.api_key)
        .header("content-type", "application/json")
        .body(payload_out)
        .send()
        .map_err(|e| {
            let kind = if e.is_timeout() { "timeout" } else { "network" };
            MarkItDownError::decode("pdf-gemini", format!("{kind} error: {e}"))
        })?;

    let status = response.status();
    let payload = response
        .text()
        .map_err(|e| MarkItDownError::decode("pdf-gemini", format!("failed to read body: {e}")))?;

    if !status.is_success() {
        let detail = serde_json::from_str::<serde_json::Value>(&payload)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| payload.clone());
        let kind = match status.as_u16() {
            400 | 403 => "auth",
            429 => "rate-limit",
            500..=599 => "server",
            _ => "http",
        };
        return Err(MarkItDownError::decode(
            "pdf-gemini",
            format!("{kind} error ({}): {detail}", status.as_u16()),
        ));
    }

    let value: serde_json::Value = serde_json::from_str(&payload).map_err(|e| {
        MarkItDownError::decode("pdf-gemini", format!("invalid response JSON: {e}"))
    })?;

    let has_candidate = value
        .get("candidates")
        .and_then(|c| c.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if !has_candidate {
        let block = value
            .get("promptFeedback")
            .and_then(|p| p.get("blockReason"))
            .and_then(|b| b.as_str())
            .unwrap_or("no candidates returned");
        return Err(MarkItDownError::decode(
            "pdf-gemini",
            format!("prompt blocked: {block}"),
        ));
    }

    let candidate = &value["candidates"][0];
    let finish_reason = candidate
        .get("finishReason")
        .and_then(|f| f.as_str())
        .unwrap_or("STOP")
        .to_string();

    let mut text = String::new();
    if let Some(parts) = candidate
        .get("content")
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
    {
        for part in parts {
            if part
                .get("thought")
                .and_then(|t| t.as_bool())
                .unwrap_or(false)
            {
                continue;
            }
            if let Some(chunk) = part.get("text").and_then(|t| t.as_str()) {
                text.push_str(chunk);
            }
        }
    }

    Ok(GeminiResponse {
        text,
        finish_reason,
    })
}
