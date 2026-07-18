//! krunch-providers — HTTP provider adapters behind one `Agent` trait (PLAN §2).
//!
//! Two v1 adapters ([`OpenAiCompatibleAgent`], [`AnthropicAgent`]) share a single
//! streaming loop ([`http::run_stream`]) that enforces per-seat budgets, honors
//! cancellation, and feeds the pure [`sse`] parser. Error [`error`] classification
//! drives the strict retry allowlist.

pub mod agent;
pub mod anthropic;
pub mod backoff;
pub mod cli;
pub mod demo;
pub mod error;
pub mod http;
pub mod openai;
pub mod sse;
pub mod types;

pub use agent::{Agent, TokenSink};
pub use anthropic::AnthropicAgent;
pub use cli::{CliAgent, CliKind};
pub use demo::DemoAgent;
pub use error::{PermanentKind, ProviderError, TransientKind};
pub use openai::OpenAiCompatibleAgent;
pub use types::{
    AuthScheme, Budget, Capabilities, Completion, CompletionRequest, FinishReason, Message,
    MessageRole, SseFlavor, TruncationCause, Usage,
};
