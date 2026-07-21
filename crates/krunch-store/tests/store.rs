//! Persistence tests (PLAN §7/§12): idempotent start, attempt/chunk lifecycle,
//! the generation fence, and crash recovery.

use krunch_core::config::{
    GuardThresholds, InteractionMode, Provider, Role, SamplingParams, SeatConfig, SessionConfig,
};
use krunch_core::ids::{SeatId, SessionId};
use krunch_core::state::SessionState;
use krunch_store::{RoundKind, Store, StoreError};

fn temp_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("krunch.sqlite")).unwrap();
    (dir, store)
}

fn seat(role: Role) -> SeatConfig {
    SeatConfig {
        id: SeatId::new(),
        display_name: "seat".into(),
        provider: Provider::OpenAiCompatible,
        base_url: "https://api.example.com".into(),
        model: "m".into(),
        system_prompt: "sys".into(),
        sampling: SamplingParams::default(),
        personas: vec![],
        credential_ref: "cred-1".into(),
        role,
    }
}

fn config() -> SessionConfig {
    SessionConfig {
        problem: "decide".into(),
        mode: InteractionMode::Batched,
        max_rounds: 8,
        guard: GuardThresholds::default(),
        seats: vec![seat(Role::Mediator), seat(Role::Panelist), seat(Role::Panelist)],
    }
}

#[tokio::test]
async fn create_session_is_idempotent() {
    let (_d, store) = temp_store();
    let first = store.create_session("key-1".into(), config()).await.unwrap();
    assert!(first.created);

    let second = store.create_session("key-1".into(), config()).await.unwrap();
    assert!(!second.created);
    assert_eq!(first.session_id, second.session_id);

    // Only one session row exists.
    assert_eq!(store.list_sessions().await.unwrap().len(), 1);
}

#[tokio::test]
async fn attempt_lifecycle_accepts_one_and_concatenates_chunks() {
    let (_d, store) = temp_store();
    let s = store.create_session("k".into(), config()).await.unwrap().session_id;
    let round = store.begin_round(s, 0, RoundKind::Deliberation, None).await.unwrap();
    let seat = SeatId::new();

    let attempt = store.start_attempt(round, seat).await.unwrap();
    store.append_chunks(attempt, vec!["Hel".into(), "lo".into()]).await.unwrap();
    store.accept_attempt(attempt).await.unwrap();

    assert_eq!(store.attempt_status(attempt).await.unwrap().as_deref(), Some("accepted"));
    assert_eq!(store.accepted_text(round, seat).await.unwrap().as_deref(), Some("Hello"));
}

#[tokio::test]
async fn retry_discards_the_prior_attempt_and_fences_its_chunks() {
    let (_d, store) = temp_store();
    let s = store.create_session("k".into(), config()).await.unwrap().session_id;
    let round = store.begin_round(s, 0, RoundKind::Deliberation, None).await.unwrap();
    let seat = SeatId::new();

    // First attempt streams partial output, then a retry supersedes it.
    let first = store.start_attempt(round, seat).await.unwrap();
    store.append_chunks(first, vec!["partial".into()]).await.unwrap();

    let second = store.start_attempt(round, seat).await.unwrap();
    // The first attempt is now discarded (fence advanced).
    assert_eq!(store.attempt_status(first).await.unwrap().as_deref(), Some("discarded"));

    // Appending to the superseded attempt is a fence violation.
    let err = store.append_chunks(first, vec!["late".into()]).await.unwrap_err();
    assert!(matches!(err, StoreError::FenceViolation { .. }));

    // The retry's output is the accepted one.
    store.append_chunks(second, vec!["final".into()]).await.unwrap();
    store.accept_attempt(second).await.unwrap();
    assert_eq!(store.accepted_text(round, seat).await.unwrap().as_deref(), Some("final"));
}

#[tokio::test]
async fn accept_discards_sibling_attempts() {
    let (_d, store) = temp_store();
    let s = store.create_session("k".into(), config()).await.unwrap().session_id;
    let round = store.begin_round(s, 0, RoundKind::Deliberation, None).await.unwrap();
    let seat = SeatId::new();

    let a1 = store.start_attempt(round, seat).await.unwrap();
    let a2 = store.start_attempt(round, seat).await.unwrap();
    store.accept_attempt(a2).await.unwrap();

    assert_eq!(store.attempt_status(a1).await.unwrap().as_deref(), Some("discarded"));
    assert_eq!(store.attempt_status(a2).await.unwrap().as_deref(), Some("accepted"));
}

#[tokio::test]
async fn crash_recovery_interrupts_unfinished_sessions() {
    let (_d, store) = temp_store();
    let s = store.create_session("k".into(), config()).await.unwrap().session_id;
    store.set_state(s, SessionState::Running).await.unwrap();
    let round = store.begin_round(s, 0, RoundKind::Deliberation, None).await.unwrap();
    let seat = SeatId::new();
    let attempt = store.start_attempt(round, seat).await.unwrap();

    let interrupted = store.recover_on_startup().await.unwrap();
    assert_eq!(interrupted, 1);

    assert_eq!(store.get_session(s).await.unwrap().state, SessionState::Interrupted);
    assert_eq!(store.attempt_status(attempt).await.unwrap().as_deref(), Some("discarded"));
}

#[tokio::test]
async fn recovery_leaves_terminal_sessions_untouched() {
    let (_d, store) = temp_store();
    let s = store.create_session("k".into(), config()).await.unwrap().session_id;
    store.set_state(s, SessionState::Converged).await.unwrap();

    let interrupted = store.recover_on_startup().await.unwrap();
    assert_eq!(interrupted, 0);
    assert_eq!(store.get_session(s).await.unwrap().state, SessionState::Converged);
}

#[tokio::test]
async fn read_models_round_trip_for_export() {
    use krunch_core::schema::{MediatorRuling, Ruling, Stance};

    let (_d, store) = temp_store();
    let cfg = config();
    let panelist = cfg.seats[1].id;
    let s = store.create_session("k".into(), cfg).await.unwrap().session_id;
    let round = store.begin_round(s, 0, RoundKind::Deliberation, Some("focus".into())).await.unwrap();

    store
        .record_stance(
            round,
            panelist,
            Stance { v: 1, stance: "yes".into(), confidence: 0.8, agree_with: vec![], open_questions: vec![] },
        )
        .await
        .unwrap();
    store
        .record_ruling(
            round,
            MediatorRuling {
                v: 1,
                ruling: Ruling::Continue,
                request_user_input: false,
                next_focus: "next".into(),
                questions_for_user: vec![],
                assumptions: vec!["assumed X".into()],
                summary: "synthesis".into(),
            },
        )
        .await
        .unwrap();
    store.record_user_qa(s, 0, "why?".into(), "because".into()).await.unwrap();

    // Reads used by the Markdown export.
    let rounds = store.rounds(s).await.unwrap();
    assert_eq!(rounds.len(), 1);
    assert_eq!(rounds[0].focus.as_deref(), Some("focus"));

    let stances = store.stances(round).await.unwrap();
    assert_eq!(stances[0].stance, "yes");

    let ruling = store.ruling(round).await.unwrap().unwrap();
    assert_eq!(ruling.ruling, "CONTINUE");
    assert_eq!(ruling.assumptions, vec!["assumed X".to_string()]);

    let seats = store.seats(s).await.unwrap();
    assert_eq!(seats.len(), 3);
    assert_eq!(seats.iter().filter(|s| s.role == "mediator").count(), 1);

    let qa = store.user_qa(s).await.unwrap();
    assert_eq!(qa[0].answer, "because");
}

#[tokio::test]
async fn unknown_session_state_update_is_not_found() {
    let (_d, store) = temp_store();
    let err = store.set_state(SessionId::new(), SessionState::Running).await.unwrap_err();
    assert!(matches!(err, StoreError::NotFound(_)));
}

#[test]
fn schema_version_is_two() {
    assert_eq!(krunch_store::schema::SCHEMA_VERSION, 2);
}

#[tokio::test]
async fn migrate_opens_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("krunch.sqlite");
    let _first = Store::open(&path).unwrap();  // creates + migrates (fresh DB → v2)
    let _second = Store::open(&path).unwrap(); // re-open runs migrate() again, no error
}
