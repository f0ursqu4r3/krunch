//! End-to-end streaming tests for the OpenAI-compatible adapter against a canned
//! local mock server — exercises the real reqwest + SSE + budget path (PLAN §2/§8/§12).

use std::net::SocketAddr;
use std::time::Duration;

use krunch_core::config::SamplingParams;
use krunch_providers::agent::Agent;
use krunch_providers::error::{ProviderError, TransientKind};
use krunch_providers::openai::OpenAiCompatibleAgent;
use krunch_providers::types::{Budget, CompletionRequest, Message, TruncationCause};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

/// Spawn a mock server that writes `response` verbatim to every connection, then
/// closes it. Returns the bound address. `None` response = accept and hang (for
/// cancellation tests).
async fn spawn_mock(response: Option<&'static str>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                // Drain a bit of the request so the client can finish sending.
                let mut buf = [0u8; 2048];
                let _ = sock.read(&mut buf).await;
                if let Some(resp) = response {
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.flush().await;
                    let _ = sock.shutdown().await;
                } else {
                    // Hang: never respond; keep the socket open.
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            });
        }
    });
    addr
}

fn req() -> CompletionRequest {
    CompletionRequest {
        model: "mock".into(),
        messages: vec![Message::user("hi")],
        sampling: SamplingParams::default(),
    }
}

const OK_SSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2}}\n\n\
data: [DONE]\n\n";

#[tokio::test]
async fn streams_and_assembles_a_completion() {
    let addr = spawn_mock(Some(OK_SSE)).await;
    let agent = OpenAiCompatibleAgent::new(format!("http://{addr}/v1"), "test-key");

    let mut streamed = String::new();
    let completion = agent
        .stream_completion(&req(), Budget::default(), CancellationToken::new(), &mut |c: &str| {
            streamed.push_str(c)
        })
        .await
        .expect("should succeed");

    assert_eq!(completion.text, "Hello world");
    assert_eq!(streamed, "Hello world"); // sink saw the same tokens live
    assert_eq!(completion.usage.output_tokens, Some(2));
    assert!(completion.truncated.is_none());
}

#[tokio::test]
async fn byte_budget_forces_deterministic_truncation() {
    let addr = spawn_mock(Some(OK_SSE)).await;
    let agent = OpenAiCompatibleAgent::new(format!("http://{addr}/v1"), "k");

    let budget = Budget { max_response_bytes: 3, ..Budget::default() };
    let completion = agent
        .stream_completion(&req(), budget, CancellationToken::new(), &mut |_: &str| {})
        .await
        .expect("truncation is success, not error");

    assert_eq!(completion.truncated, Some(TruncationCause::MaxBytes));
    assert_eq!(completion.text.len(), 3);
}

#[tokio::test]
async fn server_error_is_transient() {
    let resp = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 11\r\nConnection: close\r\n\r\nserver oops";
    let addr = spawn_mock(Some(resp)).await;
    let agent = OpenAiCompatibleAgent::new(format!("http://{addr}/v1"), "k");

    let err = agent
        .stream_completion(&req(), Budget::default(), CancellationToken::new(), &mut |_: &str| {})
        .await
        .expect_err("500 should error");

    assert!(err.is_transient());
    assert!(matches!(
        err,
        ProviderError::Transient { kind: TransientKind::ServerError, status: Some(500), .. }
    ));
}

#[tokio::test]
async fn auth_error_is_permanent() {
    let resp = "HTTP/1.1 401 Unauthorized\r\nContent-Length: 7\r\nConnection: close\r\n\r\nno auth";
    let addr = spawn_mock(Some(resp)).await;
    let agent = OpenAiCompatibleAgent::new(format!("http://{addr}/v1"), "bad");

    let err = agent
        .stream_completion(&req(), Budget::default(), CancellationToken::new(), &mut |_: &str| {})
        .await
        .expect_err("401 should error");

    assert!(!err.is_transient());
    assert_eq!(err.status(), Some(401));
}

#[tokio::test]
async fn cancellation_stops_a_hung_request() {
    let addr = spawn_mock(None).await; // server accepts but never responds
    let agent = OpenAiCompatibleAgent::new(format!("http://{addr}/v1"), "k");

    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(80)).await;
        c2.cancel();
    });

    let err = agent
        .stream_completion(&req(), Budget::default(), cancel, &mut |_: &str| {})
        .await
        .expect_err("cancellation should stop it");

    assert!(matches!(err, ProviderError::Cancelled));
}
