//! Pure Server-Sent-Events delta parsing for both provider flavors (PLAN §2).
//!
//! Kept separate + side-effect-free so the tricky wire differences (OpenAI vs
//! Anthropic framing, `[DONE]` sentinel, event types) are exhaustively unit-tested
//! without any network. The streaming loop in `http.rs` only does I/O + budgets.

use serde_json::Value;

use crate::types::{FinishReason, SseFlavor, Usage};

/// The meaning extracted from a single `data:` payload.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Delta {
    /// Text to append, if this frame carried any.
    pub text: Option<String>,
    /// A finish reason, if this frame reported one.
    pub finish_reason: Option<FinishReason>,
    /// Usage, if this frame reported it.
    pub usage: Option<Usage>,
    /// True if this frame signals end-of-stream.
    pub done: bool,
}

/// Map a provider `finish_reason`/`stop_reason` string to our enum.
fn map_finish(s: &str) -> FinishReason {
    match s {
        "stop" | "end_turn" | "stop_sequence" => FinishReason::Stop,
        "length" | "max_tokens" => FinishReason::Length,
        "tool_use" | "tool_calls" | "function_call" => FinishReason::ToolUse,
        other => FinishReason::Other(other.to_string()),
    }
}

/// Parse one SSE `data:` payload (already stripped of the `data:` prefix).
///
/// Returns `None` when the payload is not a delta we care about (e.g. an
/// Anthropic `ping` or `content_block_start`), so the caller can skip it.
pub fn parse_data(flavor: SseFlavor, payload: &str) -> Option<Delta> {
    let payload = payload.trim();
    if payload.is_empty() {
        return None;
    }
    // OpenAI's end-of-stream sentinel.
    if flavor == SseFlavor::OpenAi && payload == "[DONE]" {
        return Some(Delta { done: true, ..Default::default() });
    }

    let v: Value = serde_json::from_str(payload).ok()?;

    match flavor {
        SseFlavor::OpenAi => parse_openai(&v),
        SseFlavor::Anthropic => parse_anthropic(&v),
    }
}

fn parse_openai(v: &Value) -> Option<Delta> {
    let choice = v.get("choices")?.get(0)?;
    let text = choice
        .get("delta")
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let finish_reason = choice
        .get("finish_reason")
        .and_then(|f| f.as_str())
        .map(map_finish);
    let usage = v.get("usage").map(parse_openai_usage);

    if text.is_none() && finish_reason.is_none() && usage.is_none() {
        return None;
    }
    Some(Delta { text, finish_reason, usage, done: false })
}

fn parse_openai_usage(u: &Value) -> Usage {
    Usage {
        input_tokens: u.get("prompt_tokens").and_then(|x| x.as_u64()).map(|x| x as u32),
        output_tokens: u.get("completion_tokens").and_then(|x| x.as_u64()).map(|x| x as u32),
    }
}

fn parse_anthropic(v: &Value) -> Option<Delta> {
    let ty = v.get("type")?.as_str()?;
    match ty {
        "content_block_delta" => {
            let text = v
                .get("delta")
                .and_then(|d| d.get("text"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());
            text.map(|t| Delta { text: Some(t), ..Default::default() })
        }
        "message_delta" => {
            let finish_reason = v
                .get("delta")
                .and_then(|d| d.get("stop_reason"))
                .and_then(|s| s.as_str())
                .map(map_finish);
            let usage = v.get("usage").map(|u| Usage {
                input_tokens: None,
                output_tokens: u.get("output_tokens").and_then(|x| x.as_u64()).map(|x| x as u32),
            });
            if finish_reason.is_none() && usage.is_none() {
                return None;
            }
            Some(Delta { finish_reason, usage, ..Default::default() })
        }
        "message_stop" => Some(Delta { done: true, ..Default::default() }),
        // ping, message_start, content_block_start, content_block_stop, error handled upstream
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_content_delta() {
        let d = parse_data(SseFlavor::OpenAi, r#"{"choices":[{"delta":{"content":"Hel"}}]}"#).unwrap();
        assert_eq!(d.text.as_deref(), Some("Hel"));
        assert!(!d.done);
    }

    #[test]
    fn openai_done_sentinel() {
        let d = parse_data(SseFlavor::OpenAi, "[DONE]").unwrap();
        assert!(d.done);
    }

    #[test]
    fn openai_finish_reason_and_usage() {
        let d = parse_data(
            SseFlavor::OpenAi,
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#,
        )
        .unwrap();
        assert_eq!(d.finish_reason, Some(FinishReason::Stop));
        assert_eq!(d.usage.unwrap().output_tokens, Some(5));
    }

    #[test]
    fn openai_length_finish_maps() {
        let d = parse_data(SseFlavor::OpenAi, r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#).unwrap();
        assert_eq!(d.finish_reason, Some(FinishReason::Length));
    }

    #[test]
    fn openai_empty_delta_is_skipped() {
        assert!(parse_data(SseFlavor::OpenAi, r#"{"choices":[{"delta":{}}]}"#).is_none());
    }

    #[test]
    fn anthropic_text_delta() {
        let d = parse_data(
            SseFlavor::Anthropic,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}"#,
        )
        .unwrap();
        assert_eq!(d.text.as_deref(), Some("Hi"));
    }

    #[test]
    fn anthropic_message_delta_stop_and_usage() {
        let d = parse_data(
            SseFlavor::Anthropic,
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":42}}"#,
        )
        .unwrap();
        assert_eq!(d.finish_reason, Some(FinishReason::Stop));
        assert_eq!(d.usage.unwrap().output_tokens, Some(42));
    }

    #[test]
    fn anthropic_message_stop_is_done() {
        let d = parse_data(SseFlavor::Anthropic, r#"{"type":"message_stop"}"#).unwrap();
        assert!(d.done);
    }

    #[test]
    fn anthropic_ping_ignored() {
        assert!(parse_data(SseFlavor::Anthropic, r#"{"type":"ping"}"#).is_none());
    }

    #[test]
    fn garbage_payload_ignored() {
        assert!(parse_data(SseFlavor::OpenAi, "not json").is_none());
        assert!(parse_data(SseFlavor::Anthropic, "").is_none());
    }
}
