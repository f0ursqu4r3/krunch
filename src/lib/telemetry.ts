import type { SeatConfig } from "@/lib/types";

export interface AcceptedUsage {
  round: number;
  seat: string;
  attempt: number;
  inputTokens: number | null;
  outputTokens: number | null;
  emittedSeatChunkCount: number;
  receivedAt: number;
}

export interface TokenSummary {
  total: number;
  completionCount: number;
  exact: boolean;
}

type Price = { input: number; output: number };

// USD per million tokens. Unknown models intentionally do not receive a guess.
const MODEL_PRICES: Record<string, Price> = {
  "gpt-4o": { input: 2.5, output: 10 },
  "gpt-4o-mini": { input: 0.15, output: 0.6 },
  "gpt-4.1": { input: 2, output: 8 },
  "gpt-4.1-mini": { input: 0.4, output: 1.6 },
  "claude-sonnet-4-20250514": { input: 3, output: 15 },
  "claude-sonnet-4-5": { input: 3, output: 15 },
  "claude-sonnet-5": { input: 3, output: 15 },
  "claude-haiku-4-5": { input: 1, output: 5 },
  "claude-opus-4-8": { input: 15, output: 75 },
};

function knownOrigin(baseUrl: string): "openai" | "anthropic" | null {
  try {
    const origin = new URL(baseUrl).origin.toLowerCase();
    if (origin === "https://api.openai.com") return "openai";
    if (origin === "https://api.anthropic.com") return "anthropic";
  } catch {
    // Custom and malformed endpoints have no provider price in v1.
  }
  return null;
}

export function summarizeAcceptedUsage(usages: AcceptedUsage[]): TokenSummary {
  const exact = usages.length > 0 && usages.every((u) => u.inputTokens !== null && u.outputTokens !== null);
  const total = usages.reduce((sum, usage) => {
    // Unknown is omitted, never silently treated as zero.
    return sum + (usage.inputTokens ?? 0) + (usage.outputTokens ?? 0);
  }, 0);
  return { total, completionCount: usages.length, exact };
}

export function estimateAcceptedCost(usages: AcceptedUsage[], seats: SeatConfig[]): number | null {
  if (!usages.length || !usages.every((u) => u.inputTokens !== null && u.outputTokens !== null)) return null;
  let total = 0;
  for (const usage of usages) {
    const seat = seats.find((candidate) => candidate.id === usage.seat);
    if (!seat || !knownOrigin(seat.base_url)) return null;
    const price = MODEL_PRICES[seat.model];
    if (!price) return null;
    total += (usage.inputTokens! * price.input + usage.outputTokens! * price.output) / 1_000_000;
  }
  return total;
}

export function formatTokenTotal(summary: TokenSummary): string {
  return `${summary.total.toLocaleString()} tok ${summary.exact ? "exact" : "partial"}`;
}
