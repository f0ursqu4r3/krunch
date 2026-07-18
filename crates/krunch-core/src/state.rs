//! The deliberation state machine (PLAN §3).
//!
//! Kept pure and total: `transition(state, event)` is the single source of truth
//! for what may follow what. The orchestrator (app crate) owns side effects and
//! decides *which* event to raise; this module only says whether it is legal.

use serde::{Deserialize, Serialize};

/// Every state a session can occupy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Being assembled in the UI; not yet started.
    Configuring,
    /// Atomic pre-spawn row created under the idempotency key (PLAN §1/§3).
    Starting,
    /// A round is running (or between rounds).
    Running,
    /// Paused for the user to answer the mediator's questions.
    AwaitingUser,
    /// Running the synthetic finalization round (PLAN §6).
    Finalizing,
    // --- terminal: success ---
    Converged,
    Deadlocked,
    // --- terminal: failure ---
    /// Too few survivors to proceed (PLAN §3e).
    Halted,
    /// The mediator failed or returned an unusable ruling (PLAN §3g).
    MediatorError,
    /// Crash recovery marked an unfinished session interrupted (PLAN §7).
    Interrupted,
    /// The user abandoned the run (PLAN §8).
    Abandoned,
}

impl SessionState {
    /// Terminal states never transition again.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Converged
                | Self::Deadlocked
                | Self::Halted
                | Self::MediatorError
                | Self::Interrupted
                | Self::Abandoned
        )
    }

    /// A successful terminal outcome (produced a verdict).
    pub fn is_success(self) -> bool {
        matches!(self, Self::Converged | Self::Deadlocked)
    }
}

/// Events the orchestrator raises to advance a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Validation + idempotent insert succeeded.
    StartAccepted,
    /// The first (or a subsequent) round is now executing.
    BeginRound,
    /// Round ruled CONTINUE and the cap is not yet reached.
    RoundContinued,
    /// Mediator asked to pause for the user (mode-dependent).
    PauseForUser,
    /// User answered; resume deliberation.
    UserAnswered,
    /// Guard-passed CONSENSUS — enter finalization for a converged verdict.
    EnterFinalizeConsensus,
    /// CONTINUE at the round cap, or an explicit DEADLOCK — finalize a deadlock.
    EnterFinalizeDeadlock,
    /// Finalization synthesis succeeded for a consensus verdict.
    SynthesisConverged,
    /// Finalization synthesis succeeded for a deadlock verdict.
    SynthesisDeadlocked,
    /// Fewer than two panelists survived the round.
    TooFewSurvivors,
    /// The mediator failed (round or finalization).
    MediatorFailed,
    /// The user abandoned the run.
    Cancelled,
    /// Startup recovery found this unfinished session after a crash.
    CrashRecovered,
}

/// Raised when an event is not legal from the current state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IllegalTransition {
    pub from: SessionState,
    pub event: Event,
}

impl std::fmt::Display for IllegalTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "illegal transition: {:?} on {:?}", self.event, self.from)
    }
}

impl std::error::Error for IllegalTransition {}

/// The single transition table. Returns the next state or an [`IllegalTransition`].
pub fn transition(from: SessionState, event: Event) -> Result<SessionState, IllegalTransition> {
    use Event::*;
    use SessionState::*;

    // Cancellation and crash recovery may fire from any *non-terminal* state.
    match event {
        Cancelled if !from.is_terminal() => return Ok(Abandoned),
        CrashRecovered if !from.is_terminal() => return Ok(Interrupted),
        _ => {}
    }

    let next = match (from, event) {
        (Configuring, StartAccepted) => Starting,
        (Starting, BeginRound) => Running,

        (Running, RoundContinued) => Running,
        (Running, BeginRound) => Running,
        (Running, PauseForUser) => AwaitingUser,
        (AwaitingUser, UserAnswered) => Running,

        (Running, EnterFinalizeConsensus) => Finalizing,
        (Running, EnterFinalizeDeadlock) => Finalizing,

        (Finalizing, SynthesisConverged) => Converged,
        (Finalizing, SynthesisDeadlocked) => Deadlocked,

        (Running, TooFewSurvivors) => Halted,

        (Running, MediatorFailed) => MediatorError,
        (Finalizing, MediatorFailed) => MediatorError,

        _ => return Err(IllegalTransition { from, event }),
    };
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Event::*;
    use SessionState::*;

    #[test]
    fn happy_path_to_consensus() {
        let mut s = Configuring;
        for (ev, expect) in [
            (StartAccepted, Starting),
            (BeginRound, Running),
            (RoundContinued, Running),
            (EnterFinalizeConsensus, Finalizing),
            (SynthesisConverged, Converged),
        ] {
            s = transition(s, ev).unwrap();
            assert_eq!(s, expect);
        }
        assert!(s.is_terminal() && s.is_success());
    }

    #[test]
    fn pause_and_resume_cycle() {
        let s = transition(Running, PauseForUser).unwrap();
        assert_eq!(s, AwaitingUser);
        assert_eq!(transition(s, UserAnswered).unwrap(), Running);
    }

    #[test]
    fn deadlock_path() {
        let s = transition(Running, EnterFinalizeDeadlock).unwrap();
        assert_eq!(s, Finalizing);
        let s = transition(s, SynthesisDeadlocked).unwrap();
        assert_eq!(s, Deadlocked);
        assert!(s.is_terminal() && s.is_success());
    }

    #[test]
    fn cancellation_from_any_nonterminal_state() {
        for st in [Configuring, Starting, Running, AwaitingUser, Finalizing] {
            assert_eq!(transition(st, Cancelled).unwrap(), Abandoned);
        }
    }

    #[test]
    fn crash_recovery_from_any_nonterminal_state() {
        for st in [Starting, Running, AwaitingUser, Finalizing] {
            assert_eq!(transition(st, CrashRecovered).unwrap(), Interrupted);
        }
    }

    #[test]
    fn terminal_states_reject_all_events() {
        for st in [Converged, Deadlocked, Halted, MediatorError, Interrupted, Abandoned] {
            assert!(st.is_terminal());
            for ev in [StartAccepted, BeginRound, Cancelled, CrashRecovered, UserAnswered] {
                assert!(transition(st, ev).is_err(), "{st:?} should reject {ev:?}");
            }
        }
    }

    #[test]
    fn mediator_failure_and_too_few_survivors() {
        assert_eq!(transition(Running, MediatorFailed).unwrap(), MediatorError);
        assert_eq!(transition(Finalizing, MediatorFailed).unwrap(), MediatorError);
        assert_eq!(transition(Running, TooFewSurvivors).unwrap(), Halted);
    }

    #[test]
    fn nonsense_transitions_are_illegal() {
        assert!(transition(Configuring, UserAnswered).is_err());
        assert!(transition(AwaitingUser, SynthesisConverged).is_err());
        assert!(transition(Starting, PauseForUser).is_err());
    }
}
