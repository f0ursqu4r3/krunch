//! Built-in offline demo agent (M7). Streams a real, parseable deliberation with
//! no key and no network — panelists reciprocally agree (so the guard reaches
//! consensus), the mediator rules, and finalization synthesizes a verdict. Also
//! doubles as a deterministic end-to-end exercise of the streaming/event path.

use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::agent::{Agent, TokenSink};
use crate::error::ProviderError;
use crate::types::{
    AuthScheme, Budget, Capabilities, Completion, CompletionRequest, FinishReason, MessageRole,
    SseFlavor, Usage,
};

/// An offline agent that fabricates a coherent deliberation.
pub struct DemoAgent {
    /// Delay between streamed chunks, to make the room feel alive.
    pub chunk_delay: Duration,
}

impl Default for DemoAgent {
    fn default() -> Self {
        Self { chunk_delay: Duration::from_millis(25) }
    }
}

/// Extract the substring following `marker` up to the next newline.
fn after<'a>(haystack: &'a str, marker: &str) -> Option<&'a str> {
    let start = haystack.find(marker)? + marker.len();
    let rest = &haystack[start..];
    let end = rest.find('\n').unwrap_or(rest.len());
    Some(rest[..end].trim().trim_end_matches('.'))
}

fn build_response(req: &CompletionRequest) -> String {
    let system = req
        .messages
        .iter()
        .find(|m| m.role == MessageRole::System)
        .map(|m| m.content.as_str())
        .unwrap_or("");
    let user = req
        .messages
        .iter()
        .find(|m| m.role == MessageRole::User)
        .map(|m| m.content.as_str())
        .unwrap_or("");

    if system.contains("writing the final verdict") {
        return "After weighing every panelist's position, the panel's synthesized answer is: \
proceed with the proposal, adopting the strongest safeguards raised during deliberation.\n\n\
## Assumptions made\n- The panel assumed good-faith constraints as stated in the problem.\n\
- Where specifics were missing, the most conventional interpretation was used."
            .to_string();
    }

    if system.contains("neutral mediator") {
        return "All panelists have converged on a compatible position with high confidence, and \
their agreement is mutual.\n```json\n{\"v\":1,\"ruling\":\"CONSENSUS\",\"request_user_input\":false,\
\"next_focus\":\"finalize\",\"questions_for_user\":[],\"assumptions\":[\"assumed standard constraints\"],\
\"summary\":\"The panel agrees; proceeding to a verdict.\"}\n```"
            .to_string();
    }

    // Panelist: agree reciprocally with all named peers.
    let peers = after(user, "Other panelist seat ids:").unwrap_or("");
    let agree_json = peers
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "Having considered the problem and the mediator's framing, I support the proposal on balance; \
the tradeoffs are acceptable given the stated constraints.\n```json\n{{\"v\":1,\"stance\":\"support the proposal\",\
\"confidence\":0.9,\"agree_with\":[{agree_json}],\"open_questions\":[]}}\n```"
    )
}

#[async_trait]
impl Agent for DemoAgent {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_system_role: true,
            sse_flavor: SseFlavor::OpenAi,
            auth_scheme: AuthScheme::Bearer,
        }
    }

    async fn stream_completion(
        &self,
        req: &CompletionRequest,
        _budget: Budget,
        cancel: CancellationToken,
        sink: &mut dyn TokenSink,
    ) -> Result<Completion, ProviderError> {
        let text = build_response(req);
        // Stream word-by-word so the UI shows live typing.
        let words: Vec<&str> = text.split_inclusive(char::is_whitespace).collect();
        for w in words {
            if cancel.is_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            sink.on_token(w);
            if !self.chunk_delay.is_zero() {
                tokio::select! {
                    _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
                    _ = tokio::time::sleep(self.chunk_delay) => {}
                }
            }
        }
        Ok(Completion {
            text,
            finish_reason: FinishReason::Stop,
            truncated: None,
            usage: Usage::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;
    use krunch_core::config::SamplingParams;

    fn req(system: &str, user: &str) -> CompletionRequest {
        CompletionRequest {
            model: "demo".into(),
            messages: vec![Message::system(system), Message::user(user)],
            sampling: SamplingParams::default(),
        }
    }

    #[test]
    fn panelist_agrees_with_named_peers() {
        let r = req(
            "You are a juror on a deliberation panel.",
            "Your seat id is aaaa.\nOther panelist seat ids: bbbb, cccc.\n\nPROBLEM",
        );
        let out = build_response(&r);
        assert!(out.contains("\"bbbb\""));
        assert!(out.contains("\"cccc\""));
        assert!(!out.contains("\"ruling\""));
        assert!(out.contains("\"stance\""));
    }

    #[test]
    fn mediator_rules_consensus() {
        let r = req("You are the neutral mediator (jury foreman).", "round data");
        let out = build_response(&r);
        assert!(out.contains("CONSENSUS"));
    }

    #[test]
    fn finalize_writes_a_verdict() {
        let r = req("You are the neutral mediator writing the final verdict.", "summary");
        let out = build_response(&r);
        assert!(out.contains("Assumptions made"));
    }

    #[tokio::test]
    async fn streams_to_the_sink() {
        let agent = DemoAgent { chunk_delay: Duration::ZERO };
        let mut got = String::new();
        let r = req("You are the neutral mediator (jury foreman).", "x");
        let c = agent
            .stream_completion(&r, Budget::default(), CancellationToken::new(), &mut |t: &str| {
                got.push_str(t)
            })
            .await
            .unwrap();
        assert_eq!(got, c.text);
        assert!(c.text.contains("CONSENSUS"));
    }
}
