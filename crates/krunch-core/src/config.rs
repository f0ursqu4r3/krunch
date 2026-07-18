//! Session configuration + up-front validation (PLAN §10).
//!
//! All of this is pure data with no HTTP/secret handling — the app crate resolves
//! `credential_ref` against the OS keychain. Validation runs *before* any provider
//! task launches, so an invalid roster never reaches spend.

use serde::{Deserialize, Serialize};

use crate::ids::SeatId;

/// Provider family for a seat. HTTP details live in the app crate; this is only
/// the audit-relevant discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// Anthropic Messages API over HTTP (needs a key).
    Anthropic,
    /// Any OpenAI-compatible `/chat/completions` endpoint (key, or key-free loopback).
    OpenAiCompatible,
    /// Local `claude` CLI using the user's subscription auth (no key).
    ClaudeCli,
    /// Local `codex` CLI using the user's subscription auth (no key).
    CodexCli,
    /// Built-in offline demo agent (no key, no network).
    Demo,
}

impl Provider {
    /// True for providers that make an HTTP request with a stored credential.
    pub fn is_http(self) -> bool {
        matches!(self, Provider::Anthropic | Provider::OpenAiCompatible)
    }
}

/// Generation parameters recorded in the audit snapshot and sent to the provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SamplingParams {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
}

/// A seat's role. Exactly one Mediator per session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Panelist,
    Mediator,
}

/// Interaction mode — governs *when* the mediator surfaces questions (PLAN §5).
/// The parallel round is always atomic regardless of mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionMode {
    /// Never pause; every suppressed question becomes a recorded assumption.
    Autonomous,
    /// Mediator decides at round boundaries whether to interrupt.
    Batched,
    /// Pause at every round boundary that has open questions.
    Interactive,
}

/// One configured seat. `credential_ref` is an opaque keychain-item id (never a key).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeatConfig {
    pub id: SeatId,
    pub display_name: String,
    pub provider: Provider,
    pub base_url: String,
    pub model: String,
    pub system_prompt: String,
    #[serde(default)]
    pub sampling: SamplingParams,
    pub credential_ref: String,
    pub role: Role,
}

/// Deterministic consensus-guard thresholds (PLAN §3h).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GuardThresholds {
    /// Fraction of survivors the largest reciprocal-agreement cluster must cover.
    pub quorum_fraction: f64,
    /// Minimum mean survivor confidence.
    pub confidence_floor: f64,
}

impl Default for GuardThresholds {
    fn default() -> Self {
        // ⌈2/3⌉ coverage + 0.6 mean confidence (PLAN §3h defaults).
        Self { quorum_fraction: 2.0 / 3.0, confidence_floor: 0.6 }
    }
}

/// The full configuration for a session, validated by [`SessionConfig::validate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionConfig {
    pub problem: String,
    pub mode: InteractionMode,
    pub max_rounds: u32,
    #[serde(default)]
    pub guard: GuardThresholds,
    pub seats: Vec<SeatConfig>,
}

/// Cheap loopback detection for the validation relaxation (core has no URL parser;
/// the app layer does a stricter canonical check before attaching a credential).
pub fn is_loopback_url(base_url: &str) -> bool {
    let u = base_url.to_ascii_lowercase();
    u.contains("localhost") || u.contains("127.0.0.1") || u.contains("[::1]") || u.contains("::1")
}

/// Bounds enforced by validation (PLAN §10 / Terminology).
pub const MAX_ROUNDS_MIN: u32 = 1;
pub const MAX_ROUNDS_MAX: u32 = 64;
pub const PANELISTS_MIN: usize = 2;
pub const PANELISTS_MAX: usize = 6;
pub const PROBLEM_MAX_CHARS: usize = 20_000;

/// A single reason a [`SessionConfig`] is invalid. Validation returns *all* of them.
/// (No `Eq`: two variants carry `f64` threshold values.)
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    EmptyProblem,
    ProblemTooLong { chars: usize, max: usize },
    MaxRoundsOutOfRange { got: u32, min: u32, max: u32 },
    NoMediator,
    MultipleMediators { count: usize },
    PanelistCountOutOfRange { got: usize, min: usize, max: usize },
    DuplicateSeatId { id: SeatId },
    EmptyBaseUrl { seat: SeatId },
    EmptyModel { seat: SeatId },
    EmptyCredentialRef { seat: SeatId },
    QuorumFractionOutOfRange { got: f64 },
    ConfidenceFloorOutOfRange { got: f64 },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ValidationError::*;
        match self {
            EmptyProblem => write!(f, "problem statement is empty"),
            ProblemTooLong { chars, max } => {
                write!(f, "problem is {chars} chars, max {max}")
            }
            MaxRoundsOutOfRange { got, min, max } => {
                write!(f, "max_rounds {got} out of range {min}..={max}")
            }
            NoMediator => write!(f, "no seat is assigned the mediator role"),
            MultipleMediators { count } => write!(f, "{count} mediators; exactly one required"),
            PanelistCountOutOfRange { got, min, max } => {
                write!(f, "{got} panelists; need {min}..={max}")
            }
            DuplicateSeatId { id } => write!(f, "duplicate seat id {id}"),
            EmptyBaseUrl { seat } => write!(f, "seat {seat} has empty base_url"),
            EmptyModel { seat } => write!(f, "seat {seat} has empty model"),
            EmptyCredentialRef { seat } => write!(f, "seat {seat} has empty credential_ref"),
            QuorumFractionOutOfRange { got } => {
                write!(f, "quorum_fraction {got} must be in (0, 1]")
            }
            ConfidenceFloorOutOfRange { got } => {
                write!(f, "confidence_floor {got} must be in [0, 1]")
            }
        }
    }
}

impl SessionConfig {
    /// Validate the whole config, collecting *every* problem (not just the first),
    /// so the UI can show all fixes at once. `Ok(())` means safe to start.
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errs = Vec::new();

        if self.problem.trim().is_empty() {
            errs.push(ValidationError::EmptyProblem);
        }
        let problem_chars = self.problem.chars().count();
        if problem_chars > PROBLEM_MAX_CHARS {
            errs.push(ValidationError::ProblemTooLong {
                chars: problem_chars,
                max: PROBLEM_MAX_CHARS,
            });
        }

        if !(MAX_ROUNDS_MIN..=MAX_ROUNDS_MAX).contains(&self.max_rounds) {
            errs.push(ValidationError::MaxRoundsOutOfRange {
                got: self.max_rounds,
                min: MAX_ROUNDS_MIN,
                max: MAX_ROUNDS_MAX,
            });
        }

        // Exactly one mediator.
        let mediators = self.seats.iter().filter(|s| s.role == Role::Mediator).count();
        match mediators {
            0 => errs.push(ValidationError::NoMediator),
            1 => {}
            n => errs.push(ValidationError::MultipleMediators { count: n }),
        }

        // 2..=6 distinct panelists.
        let panelists = self.seats.iter().filter(|s| s.role == Role::Panelist).count();
        if !(PANELISTS_MIN..=PANELISTS_MAX).contains(&panelists) {
            errs.push(ValidationError::PanelistCountOutOfRange {
                got: panelists,
                min: PANELISTS_MIN,
                max: PANELISTS_MAX,
            });
        }

        // Unique seat ids.
        let mut seen = std::collections::HashSet::new();
        for seat in &self.seats {
            if !seen.insert(seat.id) {
                errs.push(ValidationError::DuplicateSeatId { id: seat.id });
            }
        }

        // Per-seat required fields depend on the provider. HTTP providers need a
        // base_url + model; a credential is required except for loopback endpoints
        // (local servers like Ollama often need no auth). CLI/Demo providers need
        // none of these — they use subscription auth or nothing (PLAN §9 / M7).
        for seat in &self.seats {
            if seat.provider.is_http() {
                if seat.base_url.trim().is_empty() {
                    errs.push(ValidationError::EmptyBaseUrl { seat: seat.id });
                }
                if seat.model.trim().is_empty() {
                    errs.push(ValidationError::EmptyModel { seat: seat.id });
                }
                if seat.credential_ref.trim().is_empty() && !is_loopback_url(&seat.base_url) {
                    errs.push(ValidationError::EmptyCredentialRef { seat: seat.id });
                }
            }
        }

        // Guard thresholds in range.
        if !(self.guard.quorum_fraction > 0.0 && self.guard.quorum_fraction <= 1.0) {
            errs.push(ValidationError::QuorumFractionOutOfRange {
                got: self.guard.quorum_fraction,
            });
        }
        if !(0.0..=1.0).contains(&self.guard.confidence_floor) {
            errs.push(ValidationError::ConfidenceFloorOutOfRange {
                got: self.guard.confidence_floor,
            });
        }

        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(role: Role) -> SeatConfig {
        SeatConfig {
            id: SeatId::new(),
            display_name: "seat".into(),
            provider: Provider::OpenAiCompatible,
            base_url: "https://api.example.com".into(),
            model: "gpt-x".into(),
            system_prompt: "you are a juror".into(),
            sampling: SamplingParams::default(),
            credential_ref: "keychain-item-1".into(),
            role,
        }
    }

    fn valid_config() -> SessionConfig {
        SessionConfig {
            problem: "Should we adopt a monorepo?".into(),
            mode: InteractionMode::Batched,
            max_rounds: 8,
            guard: GuardThresholds::default(),
            seats: vec![seat(Role::Mediator), seat(Role::Panelist), seat(Role::Panelist)],
        }
    }

    #[test]
    fn a_well_formed_config_validates() {
        assert_eq!(valid_config().validate(), Ok(()));
    }

    #[test]
    fn empty_problem_is_rejected() {
        let mut c = valid_config();
        c.problem = "   ".into();
        let errs = c.validate().unwrap_err();
        assert!(errs.contains(&ValidationError::EmptyProblem));
    }

    #[test]
    fn missing_and_extra_mediators_rejected() {
        let mut c = valid_config();
        c.seats[0].role = Role::Panelist; // now zero mediators
        assert!(c.validate().unwrap_err().contains(&ValidationError::NoMediator));

        let mut c2 = valid_config();
        c2.seats.push(seat(Role::Mediator)); // now two mediators
        assert!(c2
            .validate()
            .unwrap_err()
            .contains(&ValidationError::MultipleMediators { count: 2 }));
    }

    #[test]
    fn panelist_count_bounds_enforced() {
        // Only one panelist -> too few.
        let mut c = valid_config();
        c.seats = vec![seat(Role::Mediator), seat(Role::Panelist)];
        assert!(c.validate().unwrap_err().iter().any(|e| matches!(
            e,
            ValidationError::PanelistCountOutOfRange { got: 1, .. }
        )));

        // Seven panelists -> too many.
        let mut c2 = valid_config();
        c2.seats = std::iter::once(seat(Role::Mediator))
            .chain((0..7).map(|_| seat(Role::Panelist)))
            .collect();
        assert!(c2.validate().unwrap_err().iter().any(|e| matches!(
            e,
            ValidationError::PanelistCountOutOfRange { got: 7, .. }
        )));
    }

    #[test]
    fn duplicate_seat_ids_rejected() {
        let mut c = valid_config();
        let dup = c.seats[1].id;
        c.seats[2].id = dup;
        assert!(c
            .validate()
            .unwrap_err()
            .contains(&ValidationError::DuplicateSeatId { id: dup }));
    }

    #[test]
    fn max_rounds_out_of_range_rejected() {
        let mut c = valid_config();
        c.max_rounds = 0;
        assert!(c.validate().unwrap_err().iter().any(|e| matches!(
            e,
            ValidationError::MaxRoundsOutOfRange { got: 0, .. }
        )));
        c.max_rounds = 65;
        assert!(c.validate().unwrap_err().iter().any(|e| matches!(
            e,
            ValidationError::MaxRoundsOutOfRange { got: 65, .. }
        )));
    }

    #[test]
    fn out_of_range_guard_thresholds_rejected() {
        let mut c = valid_config();
        c.guard.quorum_fraction = 0.0;
        c.guard.confidence_floor = 1.5;
        let errs = c.validate().unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ValidationError::QuorumFractionOutOfRange { .. })));
        assert!(errs.iter().any(|e| matches!(e, ValidationError::ConfidenceFloorOutOfRange { .. })));
    }

    #[test]
    fn loopback_http_needs_no_credential() {
        let mut c = valid_config();
        c.seats[1].provider = Provider::OpenAiCompatible;
        c.seats[1].base_url = "http://localhost:11434/v1".into();
        c.seats[1].credential_ref = "".into();
        assert_eq!(c.validate(), Ok(()));
    }

    #[test]
    fn cli_and_demo_seats_need_no_url_or_credential() {
        let mut c = valid_config();
        c.seats[1].provider = Provider::ClaudeCli;
        c.seats[1].base_url = "".into();
        c.seats[1].credential_ref = "".into();
        c.seats[2].provider = Provider::Demo;
        c.seats[2].base_url = "".into();
        c.seats[2].model = "".into();
        c.seats[2].credential_ref = "".into();
        assert_eq!(c.validate(), Ok(()));
    }

    #[test]
    fn remote_http_still_needs_a_credential() {
        let mut c = valid_config();
        c.seats[1].provider = Provider::OpenAiCompatible;
        c.seats[1].base_url = "https://api.example.com/v1".into();
        c.seats[1].credential_ref = "".into();
        assert!(c
            .validate()
            .unwrap_err()
            .iter()
            .any(|e| matches!(e, ValidationError::EmptyCredentialRef { .. })));
    }

    #[test]
    fn validation_collects_all_errors_at_once() {
        let mut c = valid_config();
        c.problem = "".into();
        c.max_rounds = 0;
        c.seats[0].role = Role::Panelist; // no mediator
        let errs = c.validate().unwrap_err();
        // At least the three distinct problems above.
        assert!(errs.len() >= 3);
    }
}
