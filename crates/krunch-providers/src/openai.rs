//! OpenAI-compatible adapter: OpenAI, Ollama, LM Studio, OpenRouter, vLLM, … via
//! a configurable `base_url` and `/chat/completions` (PLAN §2/§5).

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::agent::{Agent, TokenSink};
use crate::error::ProviderError;
use crate::http::{build_client, run_stream};
use crate::types::{
    AuthScheme, Budget, Capabilities, Completion, CompletionRequest, MessageRole, SseFlavor,
};

/// An OpenAI-compatible chat-completions endpoint.
pub struct OpenAiCompatibleAgent {
    base_url: String,
    api_key: String,
}

impl OpenAiCompatibleAgent {
    /// `base_url` should include any version prefix, e.g. `https://api.openai.com/v1`.
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self { base_url: base_url.into(), api_key: api_key.into() }
    }

    fn endpoint(&self) -> String {
        let trimmed = self.base_url.trim_end_matches('/');
        // If the user gave only a host (no path, e.g. `http://127.0.0.1:1234`),
        // assume the conventional `/v1` prefix (LM Studio, vLLM, etc.). If they
        // already included a path (`.../v1`), append only the method.
        let after_scheme = trimmed.splitn(2, "://").nth(1).unwrap_or(trimmed);
        if after_scheme.contains('/') {
            format!("{trimmed}/chat/completions")
        } else {
            format!("{trimmed}/v1/chat/completions")
        }
    }

    fn body(&self, req: &CompletionRequest) -> Value {
        let messages: Vec<Value> = req
            .messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                };
                json!({ "role": role, "content": m.content })
            })
            .collect();

        let mut body = json!({
            "model": req.model,
            "messages": messages,
            "stream": true,
            "stream_options": { "include_usage": true },
        });
        let obj = body.as_object_mut().unwrap();
        if let Some(t) = req.sampling.temperature {
            obj.insert("temperature".into(), json!(t));
        }
        if let Some(p) = req.sampling.top_p {
            obj.insert("top_p".into(), json!(p));
        }
        if let Some(m) = req.sampling.max_tokens {
            obj.insert("max_tokens".into(), json!(m));
        }
        body
    }
}

#[async_trait]
impl Agent for OpenAiCompatibleAgent {
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
        budget: Budget,
        cancel: CancellationToken,
        sink: &mut dyn TokenSink,
    ) -> Result<Completion, ProviderError> {
        let client = build_client(budget.connect_timeout)?;
        let request = client
            .post(self.endpoint())
            .timeout(budget.total_timeout)
            .bearer_auth(&self.api_key)
            .json(&self.body(req));
        run_stream(request, SseFlavor::OpenAi, budget, cancel, sink).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use krunch_core::config::SamplingParams;
    use crate::types::Message;

    fn req() -> CompletionRequest {
        CompletionRequest {
            model: "gpt-x".into(),
            messages: vec![Message::system("sys"), Message::user("hi")],
            sampling: SamplingParams { temperature: Some(0.4), top_p: None, max_tokens: Some(256) },
        }
    }

    #[test]
    fn endpoint_respects_an_explicit_version_path() {
        let a = OpenAiCompatibleAgent::new("https://api.openai.com/v1/", "k");
        assert_eq!(a.endpoint(), "https://api.openai.com/v1/chat/completions");
        let b = OpenAiCompatibleAgent::new("http://localhost:11434/v1", "k");
        assert_eq!(b.endpoint(), "http://localhost:11434/v1/chat/completions");
    }

    #[test]
    fn endpoint_adds_v1_when_only_a_host_is_given() {
        // LM Studio / vLLM default: user pastes just host:port.
        let a = OpenAiCompatibleAgent::new("http://127.0.0.1:1234", "k");
        assert_eq!(a.endpoint(), "http://127.0.0.1:1234/v1/chat/completions");
        let b = OpenAiCompatibleAgent::new("http://127.0.0.1:1234/", "k");
        assert_eq!(b.endpoint(), "http://127.0.0.1:1234/v1/chat/completions");
    }

    #[test]
    fn body_streams_with_usage_and_sampling() {
        let a = OpenAiCompatibleAgent::new("https://x", "k");
        let b = a.body(&req());
        assert_eq!(b["stream"], json!(true));
        assert_eq!(b["stream_options"]["include_usage"], json!(true));
        assert!((b["temperature"].as_f64().unwrap() - 0.4).abs() < 1e-6);
        assert_eq!(b["max_tokens"], json!(256));
        assert!(b.get("top_p").is_none());
        assert_eq!(b["messages"][0]["role"], json!("system"));
        assert_eq!(b["messages"][1]["content"], json!("hi"));
    }
}
