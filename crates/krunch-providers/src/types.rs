//! Provider-facing request/response types + per-seat budgets (PLAN §2/§8).

use std::time::Duration;

use krunch_core::config::SamplingParams;

/// The role of a message in a provider request (distinct from a seat's `Role`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// One message in a completion request.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: MessageRole::System, content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: MessageRole::User, content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: MessageRole::Assistant, content: content.into() }
    }
}

/// A normalized completion request. Adapters translate this to provider wire form.
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub sampling: SamplingParams,
}

/// Why a completion stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    /// Natural stop / end of turn.
    Stop,
    /// Provider hit its own length limit.
    Length,
    /// Tool/function call (not used in v1, mapped through for completeness).
    ToolUse,
    /// Anything the adapter couldn't map.
    Other(String),
}

/// Set when *we* truncated the stream due to a per-seat budget (PLAN §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncationCause {
    MaxBytes,
    MaxOutputTokens,
}

/// Token usage if the provider reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

/// A completed (possibly truncated) generation.
#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub text: String,
    pub finish_reason: FinishReason,
    /// `Some` if a budget forced truncation — recorded in the transcript.
    pub truncated: Option<TruncationCause>,
    pub usage: Usage,
}

/// Per-seat budgets enforced by the adapter (PLAN §8).
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// Max time to establish the connection.
    pub connect_timeout: Duration,
    /// Max time to wait between streamed chunks before treating it as a stall.
    pub idle_timeout: Duration,
    /// Max total wall-clock for the whole generation.
    pub total_timeout: Duration,
    /// Hard ceiling on accumulated response bytes.
    pub max_response_bytes: usize,
    /// Optional ceiling on output tokens (approximated by provider usage / whitespace).
    pub max_output_tokens: Option<u32>,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(15),
            idle_timeout: Duration::from_secs(60),
            total_timeout: Duration::from_secs(300),
            max_response_bytes: 2 * 1024 * 1024,
            max_output_tokens: None,
        }
    }
}

/// Which SSE dialect an adapter speaks (part of its capability contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseFlavor {
    OpenAi,
    Anthropic,
}

/// How the adapter authenticates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScheme {
    /// `Authorization: Bearer <key>`
    Bearer,
    /// `x-api-key: <key>` (+ anthropic-version)
    XApiKey,
}

/// The normalized capability contract for an adapter (PLAN §2). Lets the
/// orchestrator + tests reason about provider differences explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub supports_system_role: bool,
    pub sse_flavor: SseFlavor,
    pub auth_scheme: AuthScheme,
}
