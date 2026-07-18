//! Versioned wire schemas emitted by seats as fenced ```json blocks (PLAN §3d/§3g).
//!
//! Parsing + validation lives in [`crate::parse`]; this module is the shape only.

use serde::{Deserialize, Serialize};

use crate::ids::SeatId;

/// The only stance schema version this build understands.
pub const STANCE_SCHEMA_VERSION: u32 = 1;
/// The only ruling schema version this build understands.
pub const RULING_SCHEMA_VERSION: u32 = 1;

/// A panelist's per-round structured stance (the fenced json after its prose).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stance {
    /// Schema version; must equal [`STANCE_SCHEMA_VERSION`].
    pub v: u32,
    /// One-line position (rendered/recorded, never semantically compared).
    pub stance: String,
    /// Self-reported confidence in [0, 1].
    pub confidence: f64,
    /// Seats this panelist claims to agree with (validated against the roster).
    #[serde(default)]
    pub agree_with: Vec<SeatId>,
    /// Assumptions/uncertainties this panelist is flagging.
    #[serde(default)]
    pub open_questions: Vec<String>,
}

/// The mediator's ruling on the round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Ruling {
    Consensus,
    Continue,
    Deadlock,
}

/// The mediator's per-round structured ruling (the fenced json after its summary).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediatorRuling {
    /// Schema version; must equal [`RULING_SCHEMA_VERSION`].
    pub v: u32,
    pub ruling: Ruling,
    /// Machine-readable "interrupt now" signal for batched mode (PLAN §5).
    #[serde(default)]
    pub request_user_input: bool,
    /// What the next round should focus on.
    #[serde(default)]
    pub next_focus: String,
    /// Questions to surface to the user (must be non-empty when pausing).
    #[serde(default)]
    pub questions_for_user: Vec<String>,
    /// Concrete assumptions recorded for every suppressed question (PLAN §5 mode 1).
    #[serde(default)]
    pub assumptions: Vec<String>,
    /// Capped running synthesis — the persisted ledger source (PLAN §3g/§4).
    #[serde(default)]
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruling_serializes_uppercase() {
        assert_eq!(serde_json::to_string(&Ruling::Consensus).unwrap(), "\"CONSENSUS\"");
        let r: Ruling = serde_json::from_str("\"DEADLOCK\"").unwrap();
        assert_eq!(r, Ruling::Deadlock);
    }

    #[test]
    fn stance_defaults_absent_arrays() {
        let s: Stance = serde_json::from_str(r#"{"v":1,"stance":"yes","confidence":0.8}"#).unwrap();
        assert!(s.agree_with.is_empty());
        assert!(s.open_questions.is_empty());
    }
}
