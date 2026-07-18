//! Robust extraction + validation of the fenced ```json blocks seats emit
//! (PLAN §3d/§3g/§7). Lowest-common-denominator parsing: works on any provider,
//! including local models, because it only relies on a trailing fenced block.
//!
//! Failure policy (PLAN):
//! - panelist: any hard parse failure ⇒ abstain (caller records + flags).
//! - mediator: any hard parse failure ⇒ `MediatorError` (caller halts cleanly).
//! - `agree_with` refs that are self / unknown / duplicate are *dropped* and noted,
//!   but do not by themselves invalidate an otherwise-good stance.

use crate::config::InteractionMode;
use crate::ids::SeatId;
use crate::schema::{
    MediatorRuling, Stance, RULING_SCHEMA_VERSION, STANCE_SCHEMA_VERSION,
};

/// Extract the content of the last fenced code block, preferring one tagged
/// `json`. Returns `None` if there is no closed fenced block.
pub fn extract_last_json_block(raw: &str) -> Option<String> {
    let mut blocks: Vec<(String, String)> = Vec::new(); // (lang, content)
    let mut in_block = false;
    let mut lang = String::new();
    let mut buf = String::new();

    for line in raw.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("```") {
            if in_block {
                // closing fence
                blocks.push((lang.clone(), buf.clone()));
                buf.clear();
                lang.clear();
                in_block = false;
            } else {
                // opening fence; rest may be an info string like "json"
                lang = rest.trim().to_ascii_lowercase();
                in_block = true;
            }
        } else if in_block {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    // An unclosed final fence is ignored (not a complete block).

    blocks
        .iter()
        .rev()
        .find(|(l, _)| l == "json")
        .or_else(|| blocks.last())
        .map(|(_, c)| c.trim().to_string())
}

/// Why a panelist stance could not be accepted (⇒ abstain).
#[derive(Debug, Clone, PartialEq)]
pub enum StanceParseError {
    NoJsonBlock,
    InvalidJson(String),
    UnsupportedVersion { got: u32, expected: u32 },
    ConfidenceOutOfRange(f64),
}

impl std::fmt::Display for StanceParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoJsonBlock => write!(f, "no fenced json block found"),
            Self::InvalidJson(e) => write!(f, "invalid stance json: {e}"),
            Self::UnsupportedVersion { got, expected } => {
                write!(f, "stance schema v{got}, expected v{expected}")
            }
            Self::ConfidenceOutOfRange(c) => write!(f, "confidence {c} out of [0,1]"),
        }
    }
}

/// A validated stance plus notes on any dropped `agree_with` references.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedStance {
    pub stance: Stance,
    pub dropped_self: bool,
    pub dropped_unknown: Vec<SeatId>,
    pub dropped_duplicate: Vec<SeatId>,
}

impl ParsedStance {
    /// True if any `agree_with` reference was dropped (for UI flagging).
    pub fn has_malformed_refs(&self) -> bool {
        self.dropped_self || !self.dropped_unknown.is_empty() || !self.dropped_duplicate.is_empty()
    }
}

/// Parse + validate a panelist stance from raw completion text.
///
/// `author` is the emitting seat (self-references are dropped); `valid_seats` are
/// the other panelist seats in this round that an `agree_with` ref may name.
pub fn parse_stance(
    raw: &str,
    author: SeatId,
    valid_seats: &[SeatId],
) -> Result<ParsedStance, StanceParseError> {
    let block = extract_last_json_block(raw).ok_or(StanceParseError::NoJsonBlock)?;
    let mut stance: Stance = serde_json::from_str(&block)
        .map_err(|e| StanceParseError::InvalidJson(e.to_string()))?;

    if stance.v != STANCE_SCHEMA_VERSION {
        return Err(StanceParseError::UnsupportedVersion {
            got: stance.v,
            expected: STANCE_SCHEMA_VERSION,
        });
    }
    if !(0.0..=1.0).contains(&stance.confidence) {
        return Err(StanceParseError::ConfidenceOutOfRange(stance.confidence));
    }

    // Validate agree_with against the roster; drop self / unknown / duplicate.
    let mut dropped_self = false;
    let mut dropped_unknown = Vec::new();
    let mut dropped_duplicate = Vec::new();
    let mut kept: Vec<SeatId> = Vec::new();
    for r in std::mem::take(&mut stance.agree_with) {
        if r == author {
            dropped_self = true;
        } else if !valid_seats.contains(&r) {
            dropped_unknown.push(r);
        } else if kept.contains(&r) {
            dropped_duplicate.push(r);
        } else {
            kept.push(r);
        }
    }
    stance.agree_with = kept;

    Ok(ParsedStance { stance, dropped_self, dropped_unknown, dropped_duplicate })
}

/// Why a mediator ruling could not be accepted (⇒ `MediatorError`).
#[derive(Debug, Clone, PartialEq)]
pub enum RulingParseError {
    NoJsonBlock,
    InvalidJson(String),
    UnsupportedVersion { got: u32, expected: u32 },
    /// Pausing (request_user_input) with no questions is meaningless.
    PauseWithoutQuestions,
    /// Fields inconsistent with the active interaction mode.
    ModeInconsistent(String),
}

impl std::fmt::Display for RulingParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoJsonBlock => write!(f, "no fenced json block found"),
            Self::InvalidJson(e) => write!(f, "invalid ruling json: {e}"),
            Self::UnsupportedVersion { got, expected } => {
                write!(f, "ruling schema v{got}, expected v{expected}")
            }
            Self::PauseWithoutQuestions => {
                write!(f, "request_user_input is true but questions_for_user is empty")
            }
            Self::ModeInconsistent(m) => write!(f, "ruling inconsistent with mode: {m}"),
        }
    }
}

/// Parse + validate a mediator ruling, including semantic checks (PLAN §3g).
pub fn parse_ruling(
    raw: &str,
    mode: InteractionMode,
) -> Result<MediatorRuling, RulingParseError> {
    let block = extract_last_json_block(raw).ok_or(RulingParseError::NoJsonBlock)?;
    let ruling: MediatorRuling = serde_json::from_str(&block)
        .map_err(|e| RulingParseError::InvalidJson(e.to_string()))?;

    if ruling.v != RULING_SCHEMA_VERSION {
        return Err(RulingParseError::UnsupportedVersion {
            got: ruling.v,
            expected: RULING_SCHEMA_VERSION,
        });
    }
    if ruling.request_user_input && ruling.questions_for_user.is_empty() {
        return Err(RulingParseError::PauseWithoutQuestions);
    }
    if mode == InteractionMode::Autonomous && ruling.request_user_input {
        return Err(RulingParseError::ModeInconsistent(
            "autonomous mode must never request user input".into(),
        ));
    }

    Ok(ruling)
}

/// Whether the orchestrator should pause for the user after this ruling (PLAN §5).
pub fn should_pause(mode: InteractionMode, ruling: &MediatorRuling) -> bool {
    match mode {
        InteractionMode::Autonomous => false,
        InteractionMode::Batched => {
            ruling.request_user_input && !ruling.questions_for_user.is_empty()
        }
        InteractionMode::Interactive => !ruling.questions_for_user.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Ruling;

    #[test]
    fn extracts_last_json_block_preferring_json_tag() {
        let raw = "prose here\n```json\n{\"a\":1}\n```\nmore prose";
        assert_eq!(extract_last_json_block(raw).unwrap(), "{\"a\":1}");
    }

    #[test]
    fn prefers_json_tagged_over_a_later_untagged_block() {
        let raw = "```json\n{\"real\":1}\n```\nthen\n```\nnot json really\n```";
        assert_eq!(extract_last_json_block(raw).unwrap(), "{\"real\":1}");
    }

    #[test]
    fn no_block_returns_none() {
        assert!(extract_last_json_block("just prose, no fence").is_none());
        // Unclosed fence is not a complete block.
        assert!(extract_last_json_block("```json\n{\"a\":1}").is_none());
    }

    fn stance_json(confidence: f64, agree: &str) -> String {
        format!(
            "I think yes.\n```json\n{{\"v\":1,\"stance\":\"yes\",\"confidence\":{confidence},\"agree_with\":[{agree}],\"open_questions\":[]}}\n```"
        )
    }

    #[test]
    fn parses_a_clean_stance() {
        let a = SeatId::new();
        let b = SeatId::new();
        let raw = stance_json(0.9, &format!("\"{}\"", b.0));
        let parsed = parse_stance(&raw, a, &[b]).unwrap();
        assert_eq!(parsed.stance.confidence, 0.9);
        assert_eq!(parsed.stance.agree_with, vec![b]);
        assert!(!parsed.has_malformed_refs());
    }

    #[test]
    fn drops_self_unknown_and_duplicate_refs() {
        let a = SeatId::new();
        let b = SeatId::new();
        let unknown = SeatId::new();
        let agree = format!("\"{}\",\"{}\",\"{}\",\"{}\"", a.0, b.0, b.0, unknown.0);
        let raw = stance_json(0.5, &agree);
        let parsed = parse_stance(&raw, a, &[b]).unwrap();
        assert_eq!(parsed.stance.agree_with, vec![b]); // only the valid, de-duped ref
        assert!(parsed.dropped_self);
        assert_eq!(parsed.dropped_duplicate, vec![b]);
        assert_eq!(parsed.dropped_unknown, vec![unknown]);
        assert!(parsed.has_malformed_refs());
    }

    #[test]
    fn abstains_on_missing_block_bad_json_version_and_confidence() {
        let a = SeatId::new();
        assert_eq!(parse_stance("no fence", a, &[]), Err(StanceParseError::NoJsonBlock));
        assert!(matches!(
            parse_stance("```json\nnot json\n```", a, &[]),
            Err(StanceParseError::InvalidJson(_))
        ));
        let wrong_v = "```json\n{\"v\":2,\"stance\":\"x\",\"confidence\":0.5}\n```";
        assert_eq!(
            parse_stance(wrong_v, a, &[]),
            Err(StanceParseError::UnsupportedVersion { got: 2, expected: 1 })
        );
        let bad_conf = "```json\n{\"v\":1,\"stance\":\"x\",\"confidence\":1.7}\n```";
        assert_eq!(parse_stance(bad_conf, a, &[]), Err(StanceParseError::ConfidenceOutOfRange(1.7)));
    }

    fn ruling_json(ruling: &str, req: bool, questions: &str) -> String {
        format!(
            "Summary of round.\n```json\n{{\"v\":1,\"ruling\":\"{ruling}\",\"request_user_input\":{req},\"next_focus\":\"x\",\"questions_for_user\":[{questions}],\"assumptions\":[],\"summary\":\"s\"}}\n```"
        )
    }

    #[test]
    fn parses_a_clean_ruling() {
        let raw = ruling_json("CONTINUE", false, "");
        let r = parse_ruling(&raw, InteractionMode::Batched).unwrap();
        assert_eq!(r.ruling, Ruling::Continue);
    }

    #[test]
    fn rejects_pause_without_questions() {
        let raw = ruling_json("CONTINUE", true, "");
        assert_eq!(
            parse_ruling(&raw, InteractionMode::Batched),
            Err(RulingParseError::PauseWithoutQuestions)
        );
    }

    #[test]
    fn rejects_autonomous_requesting_input() {
        let raw = ruling_json("CONTINUE", true, "\"why?\"");
        assert!(matches!(
            parse_ruling(&raw, InteractionMode::Autonomous),
            Err(RulingParseError::ModeInconsistent(_))
        ));
    }

    #[test]
    fn should_pause_follows_mode() {
        let with_q = parse_ruling(&ruling_json("CONTINUE", true, "\"q?\""), InteractionMode::Batched).unwrap();
        assert!(should_pause(InteractionMode::Batched, &with_q));
        assert!(should_pause(InteractionMode::Interactive, &with_q));
        assert!(!should_pause(InteractionMode::Autonomous, &with_q));

        let no_q = parse_ruling(&ruling_json("CONTINUE", false, ""), InteractionMode::Interactive).unwrap();
        assert!(!should_pause(InteractionMode::Interactive, &no_q));
    }
}
