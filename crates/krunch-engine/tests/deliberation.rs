//! End-to-end orchestrator tests with scripted mock agents (PLAN §3/§6/§12):
//! consensus, deadlock, halt-on-too-few-survivors, and the consensus-guard
//! downgrade. No network, no Tauri — the engine is driven purely through its
//! injected `AgentProvider`.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use krunch_core::config::{
    GuardThresholds, InteractionMode, Provider, Role, SamplingParams, SeatConfig, SessionConfig,
};
use krunch_core::ids::SeatId;
use krunch_core::state::SessionState;
use krunch_engine::{AgentProvider, Engine, EngineConfig, EngineError, EngineEvent, NoopGate};
use krunch_providers::agent::{Agent, TokenSink};
use krunch_providers::error::ProviderError;
use krunch_providers::types::{
    AuthScheme, Budget, Capabilities, Completion, CompletionRequest, FinishReason, SseFlavor,
};
use krunch_store::Store;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// A mock agent that streams scripted responses, one per call.
struct MockAgent {
    script: Arc<Mutex<VecDeque<String>>>,
}

/// Fails exactly once before producing the normal scripted completion. This
/// exercises the accepted-attempt telemetry fence without changing engine logic.
struct FlakyAgent {
    script: Arc<Mutex<VecDeque<String>>>,
    failed: Arc<Mutex<bool>>,
}

#[async_trait]
impl Agent for FlakyAgent {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_system_role: true,
            sse_flavor: SseFlavor::OpenAi,
            auth_scheme: AuthScheme::Bearer,
        }
    }

    async fn stream_completion(
        &self,
        _req: &CompletionRequest,
        _budget: Budget,
        _cancel: CancellationToken,
        sink: &mut dyn TokenSink,
    ) -> Result<Completion, ProviderError> {
        let mut failed = self.failed.lock().unwrap();
        if !*failed {
            *failed = true;
            return Err(ProviderError::from_status(500, "retry me"));
        }
        drop(failed);
        let text = self.script.lock().unwrap().pop_front().unwrap_or_default();
        let mid = text.len() / 2;
        if mid > 0 {
            sink.on_token(&text[..mid]);
            sink.on_token(&text[mid..]);
        }
        Ok(Completion {
            text,
            finish_reason: FinishReason::Stop,
            truncated: None,
            usage: Default::default(),
        })
    }
}

#[async_trait]
impl Agent for MockAgent {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_system_role: true,
            sse_flavor: SseFlavor::OpenAi,
            auth_scheme: AuthScheme::Bearer,
        }
    }

    async fn stream_completion(
        &self,
        _req: &CompletionRequest,
        _budget: Budget,
        _cancel: CancellationToken,
        sink: &mut dyn TokenSink,
    ) -> Result<Completion, ProviderError> {
        let text = self.script.lock().unwrap().pop_front().unwrap_or_default();
        // Stream in a couple of chunks so the sink/seq path is exercised.
        let mid = text.len() / 2;
        if mid > 0 {
            sink.on_token(&text[..mid]);
            sink.on_token(&text[mid..]);
        }
        Ok(Completion {
            text,
            finish_reason: FinishReason::Stop,
            truncated: None,
            usage: Default::default(),
        })
    }
}

#[derive(Default)]
struct MockProvider {
    scripts: HashMap<SeatId, Arc<Mutex<VecDeque<String>>>>,
}

impl MockProvider {
    fn script(&mut self, seat: SeatId, responses: Vec<String>) {
        self.scripts
            .insert(seat, Arc::new(Mutex::new(responses.into())));
    }
}

impl AgentProvider for MockProvider {
    fn build(&self, seat: &SeatConfig) -> Result<Box<dyn Agent>, EngineError> {
        let script = self
            .scripts
            .get(&seat.id)
            .cloned()
            .unwrap_or_else(|| Arc::new(Mutex::new(VecDeque::new())));
        Ok(Box::new(MockAgent { script }))
    }
}

struct RetryProvider {
    inner: MockProvider,
    flaky: SeatId,
    failed: Arc<Mutex<bool>>,
}
impl AgentProvider for RetryProvider {
    fn build(&self, seat: &SeatConfig) -> Result<Box<dyn Agent>, EngineError> {
        let script = self
            .inner
            .scripts
            .get(&seat.id)
            .cloned()
            .unwrap_or_else(|| Arc::new(Mutex::new(VecDeque::new())));
        if seat.id == self.flaky {
            Ok(Box::new(FlakyAgent {
                script,
                failed: self.failed.clone(),
            }))
        } else {
            Ok(Box::new(MockAgent { script }))
        }
    }
}

/// Delays before delegating to a normal scripted agent — for asserting that
/// fast seats' stances are emitted while slow seats are still running.
struct SlowAgent {
    inner: MockAgent,
    delay: std::time::Duration,
}

#[async_trait]
impl Agent for SlowAgent {
    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }

    async fn stream_completion(
        &self,
        req: &CompletionRequest,
        budget: Budget,
        cancel: CancellationToken,
        sink: &mut dyn TokenSink,
    ) -> Result<Completion, ProviderError> {
        tokio::time::sleep(self.delay).await;
        self.inner.stream_completion(req, budget, cancel, sink).await
    }
}

struct SlowSeatProvider {
    inner: MockProvider,
    slow: SeatId,
    delay: std::time::Duration,
}

impl AgentProvider for SlowSeatProvider {
    fn build(&self, seat: &SeatConfig) -> Result<Box<dyn Agent>, EngineError> {
        let script = self
            .inner
            .scripts
            .get(&seat.id)
            .cloned()
            .unwrap_or_else(|| Arc::new(Mutex::new(VecDeque::new())));
        let agent = MockAgent { script };
        if seat.id == self.slow {
            Ok(Box::new(SlowAgent { inner: agent, delay: self.delay }))
        } else {
            Ok(Box::new(agent))
        }
    }
}

fn seat(id: SeatId, role: Role) -> SeatConfig {
    SeatConfig {
        id,
        display_name: format!("seat-{}", &id.to_string()[..8]),
        provider: Provider::OpenAiCompatible,
        base_url: "https://mock".into(),
        model: "mock".into(),
        system_prompt: String::new(),
        sampling: SamplingParams::default(),
        personas: vec![],
        credential_ref: "cred".into(),
        role,
    }
}

fn stance(confidence: f64, agree: &[SeatId]) -> String {
    let agree_json = agree
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "I have considered it.\n```json\n{{\"v\":1,\"stance\":\"yes\",\"confidence\":{confidence},\"agree_with\":[{agree_json}],\"open_questions\":[]}}\n```"
    )
}

fn ruling(ruling: &str) -> String {
    format!(
        "Here is my summary of the round.\n```json\n{{\"v\":1,\"ruling\":\"{ruling}\",\"request_user_input\":false,\"next_focus\":\"keep going\",\"questions_for_user\":[],\"assumptions\":[],\"summary\":\"running synthesis\"}}\n```"
    )
}

fn engine_config() -> EngineConfig {
    EngineConfig {
        backoff_base: std::time::Duration::ZERO,
        ..EngineConfig::default()
    }
}

async fn drive(cfg: SessionConfig, provider: MockProvider) -> (SessionState, Vec<EngineEvent>) {
    drive_with(cfg, Arc::new(provider)).await
}

async fn drive_with(
    cfg: SessionConfig,
    provider: Arc<dyn AgentProvider>,
) -> (SessionState, Vec<EngineEvent>) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("k.sqlite")).unwrap();
    let created = store
        .create_session("idem".into(), cfg.clone())
        .await
        .unwrap();

    let engine = Engine::new(store, provider, engine_config());
    let (tx, mut rx) = mpsc::channel::<EngineEvent>(4096);
    let collector = tokio::spawn(async move {
        let mut evs = Vec::new();
        while let Some(e) = rx.recv().await {
            evs.push(e);
        }
        evs
    });

    let state = engine
        .run(
            created.session_id,
            cfg,
            Arc::new(NoopGate),
            tx,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let events = collector.await.unwrap();
    // keep tempdir alive until here
    drop(dir);
    (state, events)
}

#[tokio::test]
async fn reaches_consensus_and_synthesizes_a_verdict() {
    let (a, b, med) = (SeatId::new(), SeatId::new(), SeatId::new());
    let cfg = SessionConfig {
        problem: "adopt a monorepo?".into(),
        mode: InteractionMode::Autonomous,
        max_rounds: 5,
        guard: GuardThresholds::default(),
        seats: vec![
            seat(med, Role::Mediator),
            seat(a, Role::Panelist),
            seat(b, Role::Panelist),
        ],
    };
    let mut provider = MockProvider::default();
    provider.script(a, vec![stance(0.9, &[b])]);
    provider.script(b, vec![stance(0.9, &[a])]);
    provider.script(
        med,
        vec![ruling("CONSENSUS"), "FINAL VERDICT: adopt it.".into()],
    );

    let (state, events) = drive(cfg, provider).await;
    assert_eq!(state, SessionState::Converged);
    assert!(events.iter().any(|e| matches!(
        e,
        EngineEvent::Verdict { outcome: SessionState::Converged, text, .. } if text.contains("adopt it")
    )));
    let usage: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::SeatUsage {
                input_tokens,
                output_tokens,
                emitted_seat_chunk_count,
                ..
            } => Some((*input_tokens, *output_tokens, *emitted_seat_chunk_count)),
            _ => None,
        })
        .collect();
    // Two panelists, mediator ruling, and final synthesis: exactly one
    // accepted-completion usage event each, with no retry double-counting.
    assert_eq!(usage.len(), 4);
    assert!(usage
        .iter()
        .all(|(input, output, chunks)| input.is_none() && output.is_none() && *chunks == 2));
}

#[tokio::test]
async fn stance_is_emitted_as_each_seat_finishes_not_at_round_end() {
    let (a, b, med) = (SeatId::new(), SeatId::new(), SeatId::new());
    let cfg = SessionConfig {
        problem: "p".into(),
        mode: InteractionMode::Autonomous,
        max_rounds: 5,
        guard: GuardThresholds::default(),
        seats: vec![
            seat(med, Role::Mediator),
            seat(a, Role::Panelist),
            seat(b, Role::Panelist),
        ],
    };
    let mut inner = MockProvider::default();
    inner.script(a, vec![stance(0.9, &[b])]);
    inner.script(b, vec![stance(0.9, &[a])]);
    inner.script(med, vec![ruling("CONSENSUS"), "FINAL VERDICT: yes.".into()]);
    // Seat `b` is much slower than seat `a`.
    let provider = SlowSeatProvider { inner, slow: b, delay: std::time::Duration::from_millis(250) };

    let (state, events) = drive_with(cfg, Arc::new(provider)).await;
    assert_eq!(state, SessionState::Converged);

    let a_stance = events
        .iter()
        .position(|e| matches!(e, EngineEvent::Stance { seat, .. } if *seat == a))
        .expect("a filed a stance");
    let b_usage = events
        .iter()
        .position(|e| matches!(e, EngineEvent::SeatUsage { seat, .. } if *seat == b))
        .expect("b completed");
    // The fast seat's stance must land while the slow seat is still running —
    // not batched after the whole round joins.
    assert!(
        a_stance < b_usage,
        "stance for fast seat (idx {a_stance}) should precede slow seat completion (idx {b_usage})"
    );
}

#[tokio::test]
async fn deadlocks_at_the_round_cap() {
    let (a, b, med) = (SeatId::new(), SeatId::new(), SeatId::new());
    let cfg = SessionConfig {
        problem: "p".into(),
        mode: InteractionMode::Autonomous,
        max_rounds: 1,
        guard: GuardThresholds::default(),
        seats: vec![
            seat(med, Role::Mediator),
            seat(a, Role::Panelist),
            seat(b, Role::Panelist),
        ],
    };
    let mut provider = MockProvider::default();
    provider.script(a, vec![stance(0.9, &[])]); // no agreement
    provider.script(b, vec![stance(0.9, &[])]);
    provider.script(
        med,
        vec![ruling("CONTINUE"), "No agreement reached.".into()],
    );

    let (state, _events) = drive(cfg, provider).await;
    assert_eq!(state, SessionState::Deadlocked);
}

#[tokio::test]
async fn halts_when_too_few_survivors() {
    let (a, b, med) = (SeatId::new(), SeatId::new(), SeatId::new());
    let cfg = SessionConfig {
        problem: "p".into(),
        mode: InteractionMode::Autonomous,
        max_rounds: 3,
        guard: GuardThresholds::default(),
        seats: vec![
            seat(med, Role::Mediator),
            seat(a, Role::Panelist),
            seat(b, Role::Panelist),
        ],
    };
    let mut provider = MockProvider::default();
    provider.script(a, vec![stance(0.9, &[b])]); // valid
    provider.script(b, vec!["no json block here".into()]); // abstains -> only 1 survivor
    provider.script(med, vec![ruling("CONTINUE")]);

    let (state, events) = drive(cfg, provider).await;
    assert_eq!(state, SessionState::Halted);
    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::SeatAbstained { .. })));
}

/// The built-in offline demo provider: every seat is a `DemoAgent`.
struct DemoProvider;
impl AgentProvider for DemoProvider {
    fn build(&self, _seat: &SeatConfig) -> Result<Box<dyn Agent>, EngineError> {
        Ok(Box::new(krunch_providers::demo::DemoAgent {
            chunk_delay: std::time::Duration::ZERO,
        }))
    }
}

#[tokio::test]
async fn demo_provider_drives_a_full_deliberation_to_consensus() {
    let (a, b, med) = (SeatId::new(), SeatId::new(), SeatId::new());
    let cfg = SessionConfig {
        problem: "adopt a four-day week?".into(),
        mode: InteractionMode::Autonomous,
        max_rounds: 3,
        guard: GuardThresholds::default(),
        seats: vec![
            seat(med, Role::Mediator),
            seat(a, Role::Panelist),
            seat(b, Role::Panelist),
        ],
    };
    let (state, events) = drive_with(cfg, Arc::new(DemoProvider)).await;
    assert_eq!(state, SessionState::Converged);
    // The demo streamed tokens through the real event path.
    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::Token { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::Verdict { .. })));
}

#[tokio::test]
async fn consensus_is_downgraded_when_the_guard_fails() {
    // Mediator claims CONSENSUS, but panelists don't reciprocally agree -> guard
    // downgrades to CONTINUE; at max_rounds=1 that becomes a deadlock.
    let (a, b, med) = (SeatId::new(), SeatId::new(), SeatId::new());
    let cfg = SessionConfig {
        problem: "p".into(),
        mode: InteractionMode::Autonomous,
        max_rounds: 1,
        guard: GuardThresholds::default(),
        seats: vec![
            seat(med, Role::Mediator),
            seat(a, Role::Panelist),
            seat(b, Role::Panelist),
        ],
    };
    let mut provider = MockProvider::default();
    provider.script(a, vec![stance(0.9, &[])]); // no reciprocal edges
    provider.script(b, vec![stance(0.9, &[])]);
    provider.script(med, vec![ruling("CONSENSUS"), "deadlocked verdict".into()]);

    let (state, events) = drive(cfg, provider).await;
    assert_eq!(state, SessionState::Deadlocked);
    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::ConsensusDowngraded { .. })));
    let telemetry = events
        .iter()
        .position(|e| matches!(e, EngineEvent::RoundTelemetry { .. }))
        .unwrap();
    let complete = events
        .iter()
        .position(|e| matches!(e, EngineEvent::RoundComplete { .. }))
        .unwrap();
    assert!(
        telemetry < complete,
        "post-guard telemetry precedes the round snapshot"
    );
}

#[tokio::test]
async fn usage_is_emitted_once_for_the_accepted_retry() {
    let (a, b, med) = (SeatId::new(), SeatId::new(), SeatId::new());
    let cfg = SessionConfig {
        problem: "p".into(),
        mode: InteractionMode::Autonomous,
        max_rounds: 2,
        guard: GuardThresholds::default(),
        seats: vec![
            seat(med, Role::Mediator),
            seat(a, Role::Panelist),
            seat(b, Role::Panelist),
        ],
    };
    let mut inner = MockProvider::default();
    inner.script(a, vec![stance(0.9, &[b])]);
    inner.script(b, vec![stance(0.9, &[a])]);
    inner.script(med, vec![ruling("CONSENSUS"), "final".into()]);
    let provider = RetryProvider {
        inner,
        flaky: a,
        failed: Arc::new(Mutex::new(false)),
    };
    let (_state, events) = drive_with(cfg, Arc::new(provider)).await;
    let retries: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::SeatUsage { seat, attempt, .. } if *seat == a => Some(*attempt),
            _ => None,
        })
        .collect();
    assert_eq!(
        retries,
        vec![1],
        "discarded attempt 0 never contributes usage"
    );
}
