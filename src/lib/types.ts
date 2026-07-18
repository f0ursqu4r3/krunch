// TypeScript mirrors of the Rust wire types (krunch-core serde shapes + EngineEvent).

export type Provider = "anthropic" | "open_ai_compatible";
export type Role = "panelist" | "mediator";
export type InteractionMode = "autonomous" | "batched" | "interactive";
export type RulingKind = "CONSENSUS" | "CONTINUE" | "DEADLOCK";

export type SessionState =
  | "configuring"
  | "starting"
  | "running"
  | "awaiting_user"
  | "finalizing"
  | "converged"
  | "deadlocked"
  | "halted"
  | "mediator_error"
  | "interrupted"
  | "abandoned";

export interface SamplingParams {
  temperature?: number | null;
  top_p?: number | null;
  max_tokens?: number | null;
}

export interface SeatConfig {
  id: string;
  display_name: string;
  provider: Provider;
  base_url: string;
  model: string;
  system_prompt: string;
  sampling: SamplingParams;
  credential_ref: string;
  role: Role;
}

export interface GuardThresholds {
  quorum_fraction: number;
  confidence_floor: number;
}

export interface SessionConfig {
  problem: string;
  mode: InteractionMode;
  max_rounds: number;
  guard: GuardThresholds;
  seats: SeatConfig[];
}

export interface SessionDto {
  id: string;
  state: SessionState;
  max_rounds: number;
  problem: string;
  created_at: number;
  updated_at: number;
}

export interface StartDto {
  session_id: string;
  created: boolean;
}

// Discriminated union mirroring EngineEvent (serde tag = "type").
export type EngineEvent =
  | { type: "state_changed"; session: string; state: SessionState }
  | { type: "round_started"; session: string; round: number }
  | { type: "seat_started"; session: string; round: number; seat: string; attempt: number }
  | { type: "token"; session: string; round: number; seat: string; attempt: number; seq: number; text: string }
  | { type: "seat_truncated"; session: string; round: number; seat: string; cause: string }
  | { type: "stance"; session: string; round: number; seat: string; stance: string; confidence: number }
  | { type: "seat_abstained"; session: string; round: number; seat: string; reason: string }
  | { type: "ruling"; session: string; round: number; ruling: RulingKind; summary: string; next_focus: string }
  | { type: "consensus_downgraded"; session: string; round: number; cluster_fraction: number; mean_confidence: number }
  | { type: "round_complete"; session: string; round: number }
  | { type: "awaiting_user"; session: string; round: number; questions: string[] }
  | { type: "verdict"; session: string; outcome: SessionState; text: string }
  | { type: "failed"; session: string; state: SessionState; reason: string };

export const TERMINAL_STATES: SessionState[] = [
  "converged",
  "deadlocked",
  "halted",
  "mediator_error",
  "interrupted",
  "abandoned",
];

export const SUCCESS_STATES: SessionState[] = ["converged", "deadlocked"];

export function isTerminal(s: SessionState): boolean {
  return TERMINAL_STATES.includes(s);
}
