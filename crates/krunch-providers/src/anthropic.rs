//! Anthropic native adapter: `/v1/messages` with `x-api-key` + `anthropic-version`
//! and the Anthropic SSE event framing (PLAN §2).

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::agent::{Agent, TokenSink};
use crate::error::ProviderError;
use crate::http::{build_client, run_stream};
use crate::types::{
    AuthScheme, Budget, Capabilities, Completion, CompletionRequest, MessageRole, SseFlavor,
};

/// Anthropic requires `max_tokens`; used when a seat leaves it unset.
const DEFAULT_MAX_TOKENS: u32 = 4096;
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// An Anthropic Messages endpoint.
pub struct AnthropicAgent {
    base_url: String,
    api_key: String,
}

impl AnthropicAgent {
    /// `base_url` is the API root, e.g. `https://api.anthropic.com`.
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self { base_url: base_url.into(), api_key: api_key.into() }
    }

    fn endpoint(&self) -> String {
        format!("{}/v1/messages", self.base_url.trim_end_matches('/'))
    }

    fn body(&self, req: &CompletionRequest) -> Value {
        // Anthropic takes `system` as a top-level field; messages hold only
        // user/assistant turns.
        let mut system = String::new();
        let mut messages: Vec<Value> = Vec::new();
        for m in &req.messages {
            match m.role {
                MessageRole::System => {
                    if !system.is_empty() {
                        system.push_str("\n\n");
                    }
                    system.push_str(&m.content);
                }
                MessageRole::User => messages.push(json!({ "role": "user", "content": m.content })),
                MessageRole::Assistant => {
                    messages.push(json!({ "role": "assistant", "content": m.content }))
                }
            }
        }

        let mut body = json!({
            "model": req.model,
            "messages": messages,
            "stream": true,
            "max_tokens": req.sampling.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
        });
        let obj = body.as_object_mut().unwrap();
        if !system.is_empty() {
            obj.insert("system".into(), json!(system));
        }
        if let Some(t) = req.sampling.temperature {
            obj.insert("temperature".into(), json!(t));
        }
        if let Some(p) = req.sampling.top_p {
            obj.insert("top_p".into(), json!(p));
        }
        body
    }
}

#[async_trait]
impl Agent for AnthropicAgent {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_system_role: true,
            sse_flavor: SseFlavor::Anthropic,
            auth_scheme: AuthScheme::XApiKey,
        }
    }

    async fn stream_completion(
        &self,
        req: &CompletionRequest,
        budget: Budget,
        cancel: CancellationToken,
        sink: &mut dyn TokenSink,
    ) -> Result<Completion, ProviderError> {
        let client = build_client(budget.connect_timeout)?;
        let request = client
            .post(self.endpoint())
            .timeout(budget.total_timeout)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&self.body(req));
        run_stream(request, SseFlavor::Anthropic, budget, cancel, sink).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;
    use krunch_core::config::SamplingParams;

    #[test]
    fn endpoint_appends_v1_messages() {
        let a = AnthropicAgent::new("https://api.anthropic.com/", "k");
        assert_eq!(a.endpoint(), "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn system_is_hoisted_out_of_messages() {
        let a = AnthropicAgent::new("https://x", "k");
        let req = CompletionRequest {
            model: "claude-x".into(),
            messages: vec![
                Message::system("you are a juror"),
                Message::system("be terse"),
                Message::user("decide"),
            ],
            sampling: SamplingParams::default(),
        };
        let b = a.body(&req);
        assert_eq!(b["system"], json!("you are a juror\n\nbe terse"));
        // messages contain only the user turn.
        assert_eq!(b["messages"].as_array().unwrap().len(), 1);
        assert_eq!(b["messages"][0]["role"], json!("user"));
        // max_tokens defaulted since sampling left it None.
        assert_eq!(b["max_tokens"], json!(DEFAULT_MAX_TOKENS));
    }
}
