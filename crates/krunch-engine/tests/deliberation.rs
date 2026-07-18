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
use krunch_engine::{
    AgentProvider, Engine, EngineConfig, EngineError, EngineEvent, NoopGate,
};
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
        self.scripts.insert(seat, Arc::new(Mutex::new(responses.into())));
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

fn seat(id: SeatId, role: Role) -> SeatConfig {
    SeatConfig {
        id,
        display_name: format!("seat-{}", &id.to_string()[..8]),
        provider: Provider::OpenAiCompatible,
        base_url: "https://mock".into(),
        model: "mock".into(),
        system_prompt: String::new(),
        sampling: SamplingParams::default(),
        credential_ref: "cred".into(),
        role,
    }
}

fn stance(confidence: f64, agree: &[SeatId]) -> String {
    let agree_json =
        agree.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(",");
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
    EngineConfig { backoff_base: std::time::Duration::ZERO, ..EngineConfig::default() }
}

async fn drive(
    cfg: SessionConfig,
    provider: MockProvider,
) -> (SessionState, Vec<EngineEvent>) {
    drive_with(cfg, Arc::new(provider)).await
}

async fn drive_with(
    cfg: SessionConfig,
    provider: Arc<dyn AgentProvider>,
) -> (SessionState, Vec<EngineEvent>) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("k.sqlite")).unwrap();
    let created = store.create_session("idem".into(), cfg.clone()).await.unwrap();

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
        .run(created.session_id, cfg, Arc::new(NoopGate), tx, CancellationToken::new())
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
        seats: vec![seat(med, Role::Mediator), seat(a, Role::Panelist), seat(b, Role::Panelist)],
    };
    let mut provider = MockProvider::default();
    provider.script(a, vec![stance(0.9, &[b])]);
    provider.script(b, vec![stance(0.9, &[a])]);
    provider.script(med, vec![ruling("CONSENSUS"), "FINAL VERDICT: adopt it.".into()]);

    let (state, events) = drive(cfg, provider).await;
    assert_eq!(state, SessionState::Converged);
    assert!(events.iter().any(|e| matches!(
        e,
        EngineEvent::Verdict { outcome: SessionState::Converged, text, .. } if text.contains("adopt it")
    )));
}

#[tokio::test]
async fn deadlocks_at_the_round_cap() {
    let (a, b, med) = (SeatId::new(), SeatId::new(), SeatId::new());
    let cfg = SessionConfig {
        problem: "p".into(),
        mode: InteractionMode::Autonomous,
        max_rounds: 1,
        guard: GuardThresholds::default(),
        seats: vec![seat(med, Role::Mediator), seat(a, Role::Panelist), seat(b, Role::Panelist)],
    };
    let mut provider = MockProvider::default();
    provider.script(a, vec![stance(0.9, &[])]); // no agreement
    provider.script(b, vec![stance(0.9, &[])]);
    provider.script(med, vec![ruling("CONTINUE"), "No agreement reached.".into()]);

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
        seats: vec![seat(med, Role::Mediator), seat(a, Role::Panelist), seat(b, Role::Panelist)],
    };
    let mut provider = MockProvider::default();
    provider.script(a, vec![stance(0.9, &[b])]); // valid
    provider.script(b, vec!["no json block here".into()]); // abstains -> only 1 survivor
    provider.script(med, vec![ruling("CONTINUE")]);

    let (state, events) = drive(cfg, provider).await;
    assert_eq!(state, SessionState::Halted);
    assert!(events.iter().any(|e| matches!(e, EngineEvent::SeatAbstained { .. })));
}

/// The built-in offline demo provider: every seat is a `DemoAgent`.
struct DemoProvider;
impl AgentProvider for DemoProvider {
    fn build(&self, _seat: &SeatConfig) -> Result<Box<dyn Agent>, EngineError> {
        Ok(Box::new(krunch_providers::demo::DemoAgent { chunk_delay: std::time::Duration::ZERO }))
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
        seats: vec![seat(med, Role::Mediator), seat(a, Role::Panelist), seat(b, Role::Panelist)],
    };
    let (state, events) = drive_with(cfg, Arc::new(DemoProvider)).await;
    assert_eq!(state, SessionState::Converged);
    // The demo streamed tokens through the real event path.
    assert!(events.iter().any(|e| matches!(e, EngineEvent::Token { .. })));
    assert!(events.iter().any(|e| matches!(e, EngineEvent::Verdict { .. })));
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
        seats: vec![seat(med, Role::Mediator), seat(a, Role::Panelist), seat(b, Role::Panelist)],
    };
    let mut provider = MockProvider::default();
    provider.script(a, vec![stance(0.9, &[])]); // no reciprocal edges
    provider.script(b, vec![stance(0.9, &[])]);
    provider.script(med, vec![ruling("CONSENSUS"), "deadlocked verdict".into()]);

    let (state, events) = drive(cfg, provider).await;
    assert_eq!(state, SessionState::Deadlocked);
    assert!(events.iter().any(|e| matches!(e, EngineEvent::ConsensusDowngraded { .. })));
}
