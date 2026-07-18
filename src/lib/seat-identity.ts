// Compact identity line for a seat header: engine (provider/model) plus the
// temperament / domain / mediator persona labels, joined with " · ".

import { personaById } from "@/lib/personas";
import type { SeatConfig } from "@/lib/types";

/** Human label for what actually generates this seat's tokens. */
export function engineLabel(seat: SeatConfig): string {
  const model = seat.model.trim();
  switch (seat.provider) {
    case "claude_cli":
      return model ? `claude cli · ${model}` : "claude cli";
    case "codex_cli":
      return "codex cli"; // model is pinned to the CLI's configured default
    case "demo":
      return "demo";
    default:
      return model || seat.provider;
  }
}

/** "engine · Temperament · Domain" (parts omitted when unset). */
export function seatIdentity(seat: SeatConfig): string {
  const personas = seat.personas
    .map((id) => personaById(id)?.label)
    .filter((label): label is string => Boolean(label));
  return [engineLabel(seat), ...personas].join(" · ");
}
