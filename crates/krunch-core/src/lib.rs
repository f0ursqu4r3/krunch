//! krunch-core — pure, side-effect-free domain logic for the deliberation engine.
//!
//! This crate holds the parts of krunch that are deterministic and independently
//! testable: domain types + validation ([`config`]), newtype [`ids`], the versioned
//! wire [`schema`], fenced-JSON [`parse`]ing, the reciprocal-edge consensus guard
//! ([`consensus`]), and the [`state`] machine. It has no knowledge of Tauri, HTTP,
//! or SQLite — those live in the app crate.

pub mod config;
pub mod consensus;
pub mod ids;
pub mod parse;
pub mod schema;
pub mod state;

// Convenience re-exports for the app crate.
pub use config::{
    GuardThresholds, InteractionMode, Provider, Role, SamplingParams, SeatConfig, SessionConfig,
    ValidationError,
};
pub use consensus::{evaluate_consensus, GuardOutcome, SurvivorStance};
pub use ids::{AttemptId, RoundId, SeatId, SessionId};
pub use parse::{
    parse_ruling, parse_stance, should_pause, ParsedStance, RulingParseError, StanceParseError,
};
pub use schema::{MediatorRuling, Ruling, Stance};
pub use state::{transition, Event, IllegalTransition, SessionState};

/// The crate's semantic version string.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_reported() {
        assert_eq!(version(), "0.1.0");
    }
}
