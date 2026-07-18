//! The shared SSE streaming loop: enforces per-seat budgets (idle/total timeouts,
//! byte + token caps → deterministic truncation) and cancellation, feeding raw
//! `data:` payloads through the pure [`crate::sse`] parser. Both adapters share it.

use std::time::{Duration, Instant};

use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::agent::TokenSink;
use crate::error::{ProviderError, TransientKind};
use crate::sse::parse_data;
use crate::types::{Budget, Completion, FinishReason, SseFlavor, TruncationCause, Usage};

/// Rough output-token estimate when the provider doesn't report usage.
fn approx_tokens(text: &str) -> u32 {
    text.split_whitespace().count() as u32
}

/// Send `req`, then stream + parse the SSE body under `budget`/`cancel`.
pub async fn run_stream(
    req: reqwest::RequestBuilder,
    flavor: SseFlavor,
    budget: Budget,
    cancel: CancellationToken,
    sink: &mut dyn TokenSink,
) -> Result<Completion, ProviderError> {
    let deadline = Instant::now() + budget.total_timeout;

    // --- send request (cancellable) ---
    let resp = tokio::select! {
        _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
        r = req.send() => r.map_err(|e| ProviderError::from_reqwest(&e))?,
    };

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(500).collect();
        return Err(ProviderError::from_status(status.as_u16(), snippet));
    }

    // --- stream the body ---
    let mut stream = resp.bytes_stream();
    let mut line_buf = String::new();
    let mut text = String::new();
    let mut finish_reason: Option<FinishReason> = None;
    let mut usage = Usage::default();
    let mut truncated: Option<TruncationCause> = None;
    let mut done = false;

    'outer: while !done {
        let now = Instant::now();
        if now >= deadline {
            return Err(ProviderError::Transient {
                kind: TransientKind::TotalTimeout,
                status: None,
                detail: "total budget elapsed".into(),
            });
        }
        let remaining = deadline - now;
        let wait = remaining.min(budget.idle_timeout);

        let next = tokio::select! {
            _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
            r = tokio::time::timeout(wait, stream.next()) => r,
        };

        let chunk = match next {
            Err(_elapsed) => {
                // Distinguish idle stall from total-budget exhaustion.
                let kind = if remaining <= budget.idle_timeout {
                    TransientKind::TotalTimeout
                } else {
                    TransientKind::IdleTimeout
                };
                return Err(ProviderError::Transient {
                    kind,
                    status: None,
                    detail: "stream stalled".into(),
                });
            }
            Ok(None) => break, // stream ended without an explicit done marker
            Ok(Some(Err(e))) => return Err(ProviderError::from_reqwest(&e)),
            Ok(Some(Ok(bytes))) => bytes,
        };

        line_buf.push_str(&String::from_utf8_lossy(&chunk));

        // Process all complete lines currently in the buffer.
        while let Some(nl) = line_buf.find('\n') {
            let line: String = line_buf.drain(..=nl).collect();
            let line = line.trim_end_matches(['\r', '\n']);
            let Some(payload) = line.strip_prefix("data:") else {
                continue;
            };
            let Some(delta) = parse_data(flavor, payload) else {
                continue;
            };

            if let Some(t) = delta.text {
                text.push_str(&t);
                sink.on_token(&t);

                // Byte cap → deterministic truncation.
                if text.len() > budget.max_response_bytes {
                    text.truncate(budget.max_response_bytes);
                    truncated = Some(TruncationCause::MaxBytes);
                    finish_reason.get_or_insert(FinishReason::Stop);
                    break 'outer;
                }
                // Token cap → deterministic truncation.
                if let Some(cap) = budget.max_output_tokens {
                    let count = usage.output_tokens.unwrap_or_else(|| approx_tokens(&text));
                    if count > cap {
                        truncated = Some(TruncationCause::MaxOutputTokens);
                        finish_reason.get_or_insert(FinishReason::Stop);
                        break 'outer;
                    }
                }
            }
            if let Some(fr) = delta.finish_reason {
                finish_reason = Some(fr);
            }
            if let Some(u) = delta.usage {
                if u.input_tokens.is_some() {
                    usage.input_tokens = u.input_tokens;
                }
                if u.output_tokens.is_some() {
                    usage.output_tokens = u.output_tokens;
                }
            }
            if delta.done {
                done = true;
                break;
            }
        }
    }

    Ok(Completion {
        text,
        finish_reason: finish_reason.unwrap_or(FinishReason::Stop),
        truncated,
        usage,
    })
}

/// Build a `reqwest::Client` honoring the per-call connect timeout. Cross-origin
/// redirects are disabled so credentials are never replayed to another host
/// (PLAN §9 — the app layer additionally binds keys to a canonical origin).
pub fn build_client(connect_timeout: Duration) -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| ProviderError::Permanent {
            kind: crate::error::PermanentKind::Other,
            status: None,
            detail: format!("client build failed: {e}"),
        })
}
