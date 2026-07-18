//! krunch-engine — the deliberation orchestrator (PLAN §3/§5/§6).
//!
//! Tauri-free so it can be driven end-to-end with mock agents. It wires the pure
//! [`krunch_core`] domain to [`krunch_providers`] adapters and [`krunch_store`]
//! persistence: concurrent panelist rounds with one-retry-then-abstain, the
//! deterministic consensus guard, mode-driven user pauses, and a finalization
//! round that synthesizes the verdict. Dependencies (agent construction, the user
//! gate, the event sink) are injected as traits so every path is testable.

pub mod prompts;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::Serialize;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use krunch_core::config::{Role, SeatConfig, SessionConfig};
use krunch_core::consensus::{evaluate_consensus, SurvivorStance};
use krunch_core::ids::{RoundId, SeatId, SessionId};
use krunch_core::parse::{parse_ruling, parse_stance, should_pause, ParsedStance};
use krunch_core::schema::Ruling;
use krunch_core::state::{transition, Event as StateEvent, SessionState};
use krunch_providers::agent::{Agent, TokenSink};
use krunch_providers::backoff::backoff_delay;
use krunch_providers::error::ProviderError;
use krunch_providers::types::{Budget, CompletionRequest};
use krunch_store::{ErrorRecord, RoundKind, Store, StoreError};

/// Errors that abort a run (as opposed to a seat abstaining, which is normal).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("store: {0}")]
    Store(#[from] StoreError),
    #[error("no mediator seat in roster")]
    NoMediator,
    #[error("fewer than two panelists in roster")]
    NoPanelists,
    #[error("agent build failed for seat {seat}: {detail}")]
    AgentBuild { seat: SeatId, detail: String },
    #[error("illegal state transition: {0}")]
    Transition(String),
}

/// Builds a provider [`Agent`] for a seat (resolving its credential internally).
pub trait AgentProvider: Send + Sync {
    fn build(&self, seat: &SeatConfig) -> Result<Box<dyn Agent>, EngineError>;
}

/// The user's answer to a pause (PLAN §5).
#[derive(Debug, Clone)]
pub enum GateResponse {
    Answers(Vec<(String, String)>),
    Abandon,
}

/// Called when the mediator pauses for user input.
#[async_trait]
pub trait UserGate: Send + Sync {
    async fn ask(&self, round_index: u32, questions: Vec<String>) -> GateResponse;
}

/// A gate that always abandons (used for autonomous mode where it's never called).
pub struct NoopGate;

#[async_trait]
impl UserGate for NoopGate {
    async fn ask(&self, _round_index: u32, _questions: Vec<String>) -> GateResponse {
        GateResponse::Abandon
    }
}

/// Streamed + lifecycle events. Every event carries the fence fields the UI needs
/// to reject stale/out-of-order updates (PLAN §7/§11).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    StateChanged {
        session: SessionId,
        state: SessionState,
    },
    RoundStarted {
        session: SessionId,
        round: u32,
    },
    SeatStarted {
        session: SessionId,
        round: u32,
        seat: SeatId,
        attempt: u32,
    },
    Token {
        session: SessionId,
        round: u32,
        seat: SeatId,
        attempt: u32,
        seq: u64,
        seat_seq: u64,
        text: String,
    },
    SeatUsage {
        session: SessionId,
        round: u32,
        seat: SeatId,
        attempt: u32,
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
        emitted_seat_chunk_count: u64,
    },
    SeatTruncated {
        session: SessionId,
        round: u32,
        seat: SeatId,
        cause: String,
    },
    Stance {
        session: SessionId,
        round: u32,
        seat: SeatId,
        stance: String,
        confidence: f64,
    },
    SeatAbstained {
        session: SessionId,
        round: u32,
        seat: SeatId,
        reason: String,
    },
    Ruling {
        session: SessionId,
        round: u32,
        ruling: Ruling,
        summary: String,
        next_focus: String,
    },
    ConsensusDowngraded {
        session: SessionId,
        round: u32,
        cluster_fraction: f64,
        mean_confidence: f64,
    },
    RoundTelemetry {
        session: SessionId,
        round: u32,
        effective_ruling: Ruling,
        cluster_fraction: f64,
        mean_confidence: f64,
    },
    RoundComplete {
        session: SessionId,
        round: u32,
    },
    AwaitingUser {
        session: SessionId,
        round: u32,
        questions: Vec<String>,
    },
    Verdict {
        session: SessionId,
        outcome: SessionState,
        text: String,
    },
    Failed {
        session: SessionId,
        state: SessionState,
        reason: String,
    },
}

/// Engine tunables (PLAN §3c/§8).
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub panelist_budget: Budget,
    pub mediator_budget: Budget,
    pub backoff_base: Duration,
    pub backoff_max: Duration,
    /// Cap on the mediator ledger passed forward each round.
    pub ledger_cap_chars: usize,
    pub event_channel_capacity: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            panelist_budget: Budget::default(),
            mediator_budget: Budget::default(),
            backoff_base: Duration::from_millis(500),
            backoff_max: Duration::from_secs(8),
            ledger_cap_chars: 8_000,
            event_channel_capacity: 1_024,
        }
    }
}

/// The orchestrator.
pub struct Engine {
    store: Store,
    provider: Arc<dyn AgentProvider>,
    config: EngineConfig,
}

enum AttemptResult {
    Completed(String),
    Abstained(String),
    Cancelled,
}

/// A panelist's round outcome after stance parsing (events already emitted
/// per-seat inside the concurrent future; persistence happens sequentially).
enum SeatOutcome {
    /// Valid stance; `text` still feeds the mediator's round transcript.
    Stance { text: String, parsed: Box<ParsedStance> },
    /// No valid stance. `Some(text)` = completed but unparseable (text still
    /// reaches the mediator); `None` = hard abstain (provider failure).
    NoStance { text: Option<String> },
    Cancelled,
}

/// A token sink that both streams tokens to the UI (with a monotonic seq) and
/// buffers them for one batched persist per attempt.
struct StreamSink {
    session: SessionId,
    round: u32,
    seat: SeatId,
    attempt: u32,
    seq: Arc<AtomicU64>,
    seat_seq: u64,
    events: mpsc::Sender<EngineEvent>,
    buffer: Vec<String>,
}

impl TokenSink for StreamSink {
    fn on_token(&mut self, chunk: &str) {
        self.buffer.push(chunk.to_string());
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let seat_seq = self.seat_seq;
        self.seat_seq += 1;
        // Lossless against the DB: if the UI channel is full we drop the live
        // event. The cockpit marks its best-effort transcript incomplete rather
        // than claiming to reload an authoritative stream.
        let _ = self.events.try_send(EngineEvent::Token {
            session: self.session,
            round: self.round,
            seat: self.seat,
            attempt: self.attempt,
            seq,
            seat_seq,
            text: chunk.to_string(),
        });
    }
}

fn jitter01() -> f64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (nanos % 1000) as f64 / 1000.0
}

fn cap_chars(s: String, n: usize) -> String {
    if s.chars().count() <= n {
        s
    } else {
        s.chars().take(n).collect()
    }
}

impl Engine {
    pub fn new(store: Store, provider: Arc<dyn AgentProvider>, config: EngineConfig) -> Self {
        Self {
            store,
            provider,
            config,
        }
    }

    /// Drive a created session to a terminal state, streaming events. The session
    /// row must already exist (created idempotently by the caller, PLAN §1).
    pub async fn run(
        &self,
        session: SessionId,
        cfg: SessionConfig,
        gate: Arc<dyn UserGate>,
        events: mpsc::Sender<EngineEvent>,
        cancel: CancellationToken,
    ) -> Result<SessionState, EngineError> {
        let panelists: Vec<SeatConfig> = cfg
            .seats
            .iter()
            .filter(|s| s.role == Role::Panelist)
            .cloned()
            .collect();
        let mediator = cfg
            .seats
            .iter()
            .find(|s| s.role == Role::Mediator)
            .cloned()
            .ok_or(EngineError::NoMediator)?;
        if panelists.len() < 2 {
            return Err(EngineError::NoPanelists);
        }

        // Build one agent per seat up front.
        let mut agents: HashMap<SeatId, Box<dyn Agent>> = HashMap::new();
        for seat in &cfg.seats {
            agents.insert(seat.id, self.provider.build(seat)?);
        }

        let seq = Arc::new(AtomicU64::new(0));
        let mut state = SessionState::Starting;

        state = self
            .advance(session, state, StateEvent::BeginRound, &events)
            .await?;

        let mut ledger = String::new();
        let mut focus = String::new();
        let mut qa: Vec<(String, String)> = Vec::new();
        let mut round_index = 0u32;
        let converged;

        loop {
            if cancel.is_cancelled() {
                return self.abandon(session, state, &events).await;
            }

            let round_id = self
                .store
                .begin_round(
                    session,
                    round_index,
                    RoundKind::Deliberation,
                    non_empty(&focus),
                )
                .await?;
            let _ = events
                .send(EngineEvent::RoundStarted {
                    session,
                    round: round_index,
                })
                .await;

            // --- panelists, concurrently ---
            // Each seat's future parses its stance and emits Stance/SeatAbstained
            // the moment THAT seat finishes, so the UI updates per-seat instead of
            // waiting for the slowest panelist. Persistence stays sequential below.
            let peers: Vec<String> = panelists.iter().map(|s| s.id.to_string()).collect();
            let events_ref = &events;
            let cancel_ref = &cancel;
            let seq_ref = &seq;
            let mut futs = Vec::new();
            for p in &panelists {
                let others: Vec<String> = peers
                    .iter()
                    .filter(|id| **id != p.id.to_string())
                    .cloned()
                    .collect();
                let other_ids: Vec<SeatId> = panelists
                    .iter()
                    .filter(|x| x.id != p.id)
                    .map(|x| x.id)
                    .collect();
                let msgs = prompts::panelist_messages(
                    &p.id.to_string(),
                    &p.system_prompt,
                    &cfg.problem,
                    &qa,
                    &ledger,
                    &focus,
                    &others,
                );
                let agent = agents.get(&p.id).unwrap().as_ref();
                futs.push(async move {
                    let res = self
                        .run_attempt(
                            session,
                            round_id,
                            round_index,
                            p,
                            agent,
                            msgs,
                            self.config.panelist_budget,
                            events_ref,
                            cancel_ref,
                            seq_ref,
                        )
                        .await?;
                    Ok::<SeatOutcome, EngineError>(match res {
                        AttemptResult::Completed(text) => {
                            match parse_stance(&text, p.id, &other_ids) {
                                Ok(parsed) => {
                                    let _ = events_ref
                                        .send(EngineEvent::Stance {
                                            session,
                                            round: round_index,
                                            seat: p.id,
                                            stance: parsed.stance.stance.clone(),
                                            confidence: parsed.stance.confidence,
                                        })
                                        .await;
                                    SeatOutcome::Stance { text, parsed: Box::new(parsed) }
                                }
                                Err(e) => {
                                    let _ = events_ref
                                        .send(EngineEvent::SeatAbstained {
                                            session,
                                            round: round_index,
                                            seat: p.id,
                                            reason: e.to_string(),
                                        })
                                        .await;
                                    // Completed text still reaches the mediator.
                                    SeatOutcome::NoStance { text: Some(text) }
                                }
                            }
                        }
                        AttemptResult::Abstained(reason) => {
                            let _ = events_ref
                                .send(EngineEvent::SeatAbstained {
                                    session,
                                    round: round_index,
                                    seat: p.id,
                                    reason,
                                })
                                .await;
                            SeatOutcome::NoStance { text: None }
                        }
                        AttemptResult::Cancelled => SeatOutcome::Cancelled,
                    })
                });
            }
            let results = futures_util::future::join_all(futs).await;

            let mut survivors: Vec<SurvivorStance> = Vec::new();
            let mut round_outputs: Vec<(String, String)> = Vec::new();
            for (p, res) in panelists.iter().zip(results) {
                match res? {
                    SeatOutcome::Stance { text, parsed } => {
                        self.store
                            .record_stance(round_id, p.id, parsed.stance.clone())
                            .await?;
                        round_outputs.push((format!("{} [{}]", p.display_name, p.id), text));
                        survivors.push(SurvivorStance { seat: p.id, stance: parsed.stance });
                    }
                    SeatOutcome::NoStance { text: Some(text) } => {
                        round_outputs.push((format!("{} [{}]", p.display_name, p.id), text));
                    }
                    SeatOutcome::NoStance { text: None } => {}
                    SeatOutcome::Cancelled => {
                        return self.abandon(session, state, &events).await;
                    }
                }
            }

            // Survivor floor (PLAN §3e).
            if survivors.len() < 2 {
                state = self
                    .advance(session, state, StateEvent::TooFewSurvivors, &events)
                    .await?;
                let _ = events
                    .send(EngineEvent::Failed {
                        session,
                        state,
                        reason: "fewer than two panelists produced a valid stance".into(),
                    })
                    .await;
                return Ok(state);
            }

            // --- mediator ---
            let med_msgs = prompts::mediator_round_messages(
                cfg.mode,
                &cfg.problem,
                &qa,
                &ledger,
                &round_outputs,
                round_index,
                cfg.max_rounds,
            );
            let med_agent = agents.get(&mediator.id).unwrap().as_ref();
            let med_text = match self
                .run_attempt(
                    session,
                    round_id,
                    round_index,
                    &mediator,
                    med_agent,
                    med_msgs,
                    self.config.mediator_budget,
                    &events,
                    &cancel,
                    &seq,
                )
                .await?
            {
                AttemptResult::Completed(t) => t,
                AttemptResult::Cancelled => return self.abandon(session, state, &events).await,
                AttemptResult::Abstained(reason) => {
                    return self.mediator_error(session, state, reason, &events).await;
                }
            };

            let ruling = match parse_ruling(&med_text, cfg.mode) {
                Ok(r) => r,
                Err(e) => {
                    return self
                        .mediator_error(session, state, e.to_string(), &events)
                        .await;
                }
            };
            self.store.record_ruling(round_id, ruling.clone()).await?;
            let _ = events
                .send(EngineEvent::Ruling {
                    session,
                    round: round_index,
                    ruling: ruling.ruling,
                    summary: ruling.summary.clone(),
                    next_focus: ruling.next_focus.clone(),
                })
                .await;

            // Deterministic consensus guard (PLAN §3h).
            let guard = evaluate_consensus(&survivors, cfg.guard);
            let mut effective = ruling.ruling;
            if ruling.ruling == Ruling::Consensus && !guard.consensus_ok {
                effective = Ruling::Continue;
                let _ = events
                    .send(EngineEvent::ConsensusDowngraded {
                        session,
                        round: round_index,
                        cluster_fraction: guard.cluster_fraction,
                        mean_confidence: guard.mean_confidence,
                    })
                    .await;
            }

            // This is deliberately after the deterministic guard and before
            // RoundComplete. The UI snapshots backend truth from this event.
            let _ = events
                .send(EngineEvent::RoundTelemetry {
                    session,
                    round: round_index,
                    effective_ruling: effective,
                    cluster_fraction: guard.cluster_fraction,
                    mean_confidence: guard.mean_confidence,
                })
                .await;

            self.store.finalize_round(round_id).await?;
            let _ = events
                .send(EngineEvent::RoundComplete {
                    session,
                    round: round_index,
                })
                .await;

            if !ruling.summary.is_empty() {
                ledger = cap_chars(ruling.summary.clone(), self.config.ledger_cap_chars);
            }
            focus = ruling.next_focus.clone();

            match effective {
                Ruling::Consensus => {
                    converged = true;
                    break;
                }
                Ruling::Deadlock => {
                    converged = false;
                    break;
                }
                Ruling::Continue => {
                    if round_index + 1 >= cfg.max_rounds {
                        converged = false;
                        break;
                    }
                    if should_pause(cfg.mode, &ruling) {
                        state = self
                            .advance(session, state, StateEvent::PauseForUser, &events)
                            .await?;
                        let _ = events
                            .send(EngineEvent::AwaitingUser {
                                session,
                                round: round_index,
                                questions: ruling.questions_for_user.clone(),
                            })
                            .await;
                        match gate
                            .ask(round_index, ruling.questions_for_user.clone())
                            .await
                        {
                            GateResponse::Answers(ans) => {
                                for (q, a) in ans {
                                    self.store
                                        .record_user_qa(session, round_index, q.clone(), a.clone())
                                        .await?;
                                    qa.push((q, a));
                                }
                                state = self
                                    .advance(session, state, StateEvent::UserAnswered, &events)
                                    .await?;
                            }
                            GateResponse::Abandon => {
                                cancel.cancel();
                                return self.abandon(session, state, &events).await;
                            }
                        }
                    }
                    round_index += 1;
                }
            }
        }

        // --- finalization (PLAN §6) ---
        let enter = if converged {
            StateEvent::EnterFinalizeConsensus
        } else {
            StateEvent::EnterFinalizeDeadlock
        };
        state = self.advance(session, state, enter, &events).await?;

        let fin_round = self
            .store
            .begin_round(session, round_index + 1, RoundKind::Finalization, None)
            .await?;
        let fin_msgs = prompts::finalize_messages(converged, &cfg.problem, &qa, &ledger);
        let fin_agent = agents.get(&mediator.id).unwrap().as_ref();
        let verdict = match self
            .run_attempt(
                session,
                fin_round,
                round_index + 1,
                &mediator,
                fin_agent,
                fin_msgs,
                self.config.mediator_budget,
                &events,
                &cancel,
                &seq,
            )
            .await?
        {
            AttemptResult::Completed(t) => t,
            AttemptResult::Cancelled => return self.abandon(session, state, &events).await,
            AttemptResult::Abstained(reason) => {
                return self.mediator_error(session, state, reason, &events).await;
            }
        };
        self.store.finalize_round(fin_round).await?;

        let outcome = if converged {
            StateEvent::SynthesisConverged
        } else {
            StateEvent::SynthesisDeadlocked
        };
        state = self.advance(session, state, outcome, &events).await?;
        let _ = events
            .send(EngineEvent::Verdict {
                session,
                outcome: state,
                text: verdict,
            })
            .await;
        Ok(state)
    }

    /// Run one seat's completion with one retry on transient errors, then abstain.
    #[allow(clippy::too_many_arguments)]
    async fn run_attempt(
        &self,
        session: SessionId,
        round: RoundId,
        round_index: u32,
        seat: &SeatConfig,
        agent: &dyn Agent,
        messages: Vec<krunch_providers::types::Message>,
        budget: Budget,
        events: &mpsc::Sender<EngineEvent>,
        cancel: &CancellationToken,
        seq: &Arc<AtomicU64>,
    ) -> Result<AttemptResult, EngineError> {
        let req = CompletionRequest {
            model: seat.model.clone(),
            messages,
            sampling: seat.sampling.clone(),
        };
        const MAX_ATTEMPTS: u32 = 2; // one try + one retry

        for attempt_no in 0..MAX_ATTEMPTS {
            if cancel.is_cancelled() {
                return Ok(AttemptResult::Cancelled);
            }
            let attempt_id = self.store.start_attempt(round, seat.id).await?;
            let _ = events
                .send(EngineEvent::SeatStarted {
                    session,
                    round: round_index,
                    seat: seat.id,
                    attempt: attempt_no,
                })
                .await;

            let mut sink = StreamSink {
                session,
                round: round_index,
                seat: seat.id,
                attempt: attempt_no,
                seq: seq.clone(),
                seat_seq: 0,
                events: events.clone(),
                buffer: Vec::new(),
            };
            let res = agent
                .stream_completion(&req, budget, cancel.clone(), &mut sink)
                .await;
            let chunks = std::mem::take(&mut sink.buffer);
            let emitted_seat_chunk_count = sink.seat_seq;

            match res {
                Ok(completion) => {
                    let to_store = if chunks.is_empty() && !completion.text.is_empty() {
                        vec![completion.text.clone()]
                    } else {
                        chunks
                    };
                    if !to_store.is_empty() {
                        self.store.append_chunks(attempt_id, to_store).await?;
                    }
                    self.store.accept_attempt(attempt_id).await?;
                    // Usage is transient telemetry for accepted completions
                    // only. Discarded retries are intentionally never emitted.
                    let _ = events
                        .send(EngineEvent::SeatUsage {
                            session,
                            round: round_index,
                            seat: seat.id,
                            attempt: attempt_no,
                            input_tokens: completion.usage.input_tokens,
                            output_tokens: completion.usage.output_tokens,
                            emitted_seat_chunk_count,
                        })
                        .await;
                    if let Some(cause) = completion.truncated {
                        let _ = events
                            .send(EngineEvent::SeatTruncated {
                                session,
                                round: round_index,
                                seat: seat.id,
                                cause: format!("{cause:?}"),
                            })
                            .await;
                    }
                    return Ok(AttemptResult::Completed(completion.text));
                }
                Err(ProviderError::Cancelled) => {
                    self.store.discard_attempt(attempt_id).await?;
                    return Ok(AttemptResult::Cancelled);
                }
                Err(e) => {
                    self.store.discard_attempt(attempt_id).await?;
                    self.store
                        .record_error(
                            session,
                            ErrorRecord {
                                round_id: Some(round),
                                seat_id: Some(seat.id),
                                attempt_id: Some(attempt_id),
                                kind: e.kind_label(),
                                http_status: e.status(),
                                retry_count: attempt_no,
                                deadline_hit: e.kind_label().contains("timeout"),
                                provider_request_id: None,
                                detail: e.to_string(),
                            },
                        )
                        .await?;

                    if e.is_transient() && attempt_no + 1 < MAX_ATTEMPTS {
                        let delay = backoff_delay(
                            attempt_no,
                            self.config.backoff_base,
                            self.config.backoff_max,
                            jitter01(),
                        );
                        tokio::select! {
                            _ = cancel.cancelled() => return Ok(AttemptResult::Cancelled),
                            _ = tokio::time::sleep(delay) => {}
                        }
                        continue;
                    }
                    return Ok(AttemptResult::Abstained(e.kind_label()));
                }
            }
        }
        Ok(AttemptResult::Abstained("retries exhausted".into()))
    }

    /// Apply a state transition, persist it, and emit a `StateChanged`.
    async fn advance(
        &self,
        session: SessionId,
        from: SessionState,
        event: StateEvent,
        events: &mpsc::Sender<EngineEvent>,
    ) -> Result<SessionState, EngineError> {
        let next = transition(from, event).map_err(|e| EngineError::Transition(e.to_string()))?;
        self.store.set_state(session, next).await?;
        let _ = events
            .send(EngineEvent::StateChanged {
                session,
                state: next,
            })
            .await;
        Ok(next)
    }

    async fn abandon(
        &self,
        session: SessionId,
        from: SessionState,
        events: &mpsc::Sender<EngineEvent>,
    ) -> Result<SessionState, EngineError> {
        let state = self
            .advance(session, from, StateEvent::Cancelled, events)
            .await?;
        let _ = events
            .send(EngineEvent::Failed {
                session,
                state,
                reason: "abandoned".into(),
            })
            .await;
        Ok(state)
    }

    async fn mediator_error(
        &self,
        session: SessionId,
        from: SessionState,
        reason: String,
        events: &mpsc::Sender<EngineEvent>,
    ) -> Result<SessionState, EngineError> {
        let state = self
            .advance(session, from, StateEvent::MediatorFailed, events)
            .await?;
        let _ = events
            .send(EngineEvent::Failed {
                session,
                state,
                reason,
            })
            .await;
        Ok(state)
    }
}

fn non_empty(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}
