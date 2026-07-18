//! The `Agent` trait + token sink (PLAN §2). Adapters implement `Agent`; the
//! orchestrator holds `Box<dyn Agent>` per seat.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::error::ProviderError;
use crate::types::{Budget, Capabilities, Completion, CompletionRequest};

/// Where streamed token chunks go. The orchestrator's sink forwards to its
/// bounded coalescing channel (PLAN §8); a test sink just accumulates.
pub trait TokenSink: Send {
    fn on_token(&mut self, chunk: &str);
}

/// Any `FnMut(&str)` is a sink.
impl<F: FnMut(&str) + Send> TokenSink for F {
    fn on_token(&mut self, chunk: &str) {
        self(chunk)
    }
}

/// A provider seat that can stream a completion. Object-safe via `async_trait`.
#[async_trait]
pub trait Agent: Send + Sync {
    /// The adapter's normalized capability contract.
    fn capabilities(&self) -> Capabilities;

    /// Stream one completion, forwarding token chunks to `sink`, honoring the
    /// per-seat `budget` and the `cancel` token. Returns the assembled
    /// (possibly truncated) completion, or a classified error.
    async fn stream_completion(
        &self,
        req: &CompletionRequest,
        budget: Budget,
        cancel: CancellationToken,
        sink: &mut dyn TokenSink,
    ) -> Result<Completion, ProviderError>;
}
