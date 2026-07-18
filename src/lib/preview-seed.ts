// DEV-ONLY: seed the store with representative data so the Room / Verdict screens
// can be designed and screenshotted without the Tauri backend. Activated via
// ?preview=room|verdict|deadlock|halted|awaiting|stream. No-op in production
// builds. `stream` replays a full scripted deliberation through store.handle()
// with realistic timing — a live rehearsal of streaming, per-seat stances,
// abstain + recovery, round resets, and the verdict, no backend required.

import type { useDeliberation } from "@/stores/deliberation";
import type { RoundSnapshot } from "@/stores/deliberation";
import type { EngineEvent } from "@/lib/types";

type Store = ReturnType<typeof useDeliberation>;

const P1 =
  "A cozy farming-sim leans on a proven, loyal Steam audience — Stardew's long tail shows wishlists compound. Two people can ship a tight vertical slice of farming, seasons, and one town in a month if we ruthlessly cut multiplayer.";
const P2 =
  "Scope is the killer. A horror roguelite is more novel and streams well, but procedural generation plus enemy AI plus juice is a six-month job disguised as one. If we must ship in 30 days, the cozy sim is the only honestly-finishable option.";
const MED =
  "Both jurors now favor the cozy farming-sim on feasibility and audience grounds, and each explicitly agrees with the other's core point about scope. Confidence is high and mutual. I am ready to rule.";

export function seedPreview(store: Store, kind: string) {
  if (kind === "stream") {
    void playScriptedStream(store);
    return;
  }
  store.loadDemoPanel();
  store.problem =
    "What game should a two-person team build in one month to make a little money on Steam? We want a genre, theme, and style.";
  const seats = store.seats;
  const med = seats.find((s) => s.role === "mediator")!;
  const [p1, p2] = seats.filter((s) => s.role === "panelist");
  med.display_name = "Foreman";
  med.provider = "claude_cli";
  p1.display_name = "The Optimist";
  p1.provider = "codex_cli";
  p2.display_name = "The Scarred Shipper";
  p2.provider = "open_ai_compatible";
  p2.model = "qwen/qwen3.6-27b";

  store.mediatorId = med.id;
  store.currentRound = 2;

  store.live[p1.id] = {
    id: p1.id, text: P1, status: "stance",
    stance: "Cozy farming-sim — proven audience", confidence: 0.86, lastSeq: 9, seatLastSeq: 9, receivedChunkCount: 10, streamIncomplete: false,
  };
  store.live[p2.id] = {
    id: p2.id, text: P2, status: "stance",
    stance: "Cozy sim is the only finishable option", confidence: 0.79, lastSeq: 9, seatLastSeq: 9, receivedChunkCount: 10, streamIncomplete: false,
  };
  store.mediatorText = MED;

  const mkRound = (round: number, c1: number, c2: number, ruling: RoundSnapshot["ruling"], summary: string): RoundSnapshot => ({
    round,
    stances: [
      { seat: p1.id, stance: "Cozy farming-sim", confidence: c1 },
      { seat: p2.id, stance: "Cozy sim, cut scope", confidence: c2 },
    ],
    ruling,
    summary,
  });
  store.rounds = [
    mkRound(0, 0.55, 0.4, "CONTINUE", "The jurors split on ambition vs. feasibility; sharpening the 30-day constraint."),
    mkRound(1, 0.86, 0.79, "CONSENSUS", "Both converge on a cozy farming-sim, agreeing scope is decisive."),
  ];

  if (kind === "room") {
    store.running = true;
    store.phase = "room";
  } else if (kind === "awaiting") {
    store.running = true;
    store.phase = "room";
    store.awaiting = {
      round: 1,
      questions: [
        "What's your hard budget ceiling for art assets — can you buy a tileset, or must it be original?",
        "Is 'a little money' ~$2k or ~$20k? That changes how much polish the launch needs.",
      ],
    };
  } else if (kind === "verdict") {
    store.finalState = "converged";
    store.verdict = {
      outcome: "converged",
      text:
        "The panel's verdict: build a cozy farming-and-foraging sim set on a small tidal island, with a hand-drawn, limited-palette art style and a single-season (autumn) scope.\n\nWhy this wins on all three lenses:\n\n- Audience — cozy sims have a proven, compounding wishlist base on Steam and forgiving production values.\n- Feasibility — one biome, one town, one season is genuinely finishable by two people in ~30 days if multiplayer and procedural systems are cut on day one.\n- Differentiation — the tidal-island hook (the map floods and reshapes daily) gives streamers and screenshots a distinct identity without new tech.\n\n## Assumptions made\n- 'A little money' means low-thousands, not a full-time income.\n- The team has one programmer and one artist, or can contract art.\n- A 30-day scope means a polished vertical slice, not a content-complete game.",
    };
    store.phase = "verdict";
  } else if (kind === "deadlock") {
    store.finalState = "deadlocked";
    store.rounds[1].ruling = "DEADLOCK";
    store.verdict = {
      outcome: "deadlocked",
      text:
        "The panel could not reach consensus within the round cap. The unresolved split:\n\n- The Optimist holds that a horror roguelite's novelty is worth the scope risk and would sell more per-unit.\n- The Scarred Shipper maintains that anything with procedural generation cannot be honestly finished in 30 days by two people.\n\nThe disagreement is fundamentally about risk tolerance and the definition of 'finished', which the panel could not reconcile.",
    };
    store.phase = "verdict";
  } else if (kind === "halted") {
    store.failure = { state: "halted", reason: "fewer than two panelists produced a valid stance" };
    store.finalState = "halted";
    store.phase = "verdict";
  }
}

// --- ?preview=stream: scripted live replay ---------------------------------

const STREAM_TEXTS = {
  fast: "**Feasibility first.** Two people, one month — the budget is ~160 hours total.\n\n- Content scales linearly with time; systems don't.\n- A roguelite loop ships small and still holds hours.\n- Multiplayer alone eats the month in netcode.\n\nPick systems-driven, not content-driven.\n\n```json\n{\"v\":1,\"stance\":\"Systems-driven roguelite; cut multiplayer day one\",\"confidence\":0.84,\"agree_with\":[],\"open_questions\":[]}\n```",
  medium: "The market evidence cuts the other way. *Short-form horror* has an accepted price point:\n\n1. Chilla's Art ships 1–2h games at $4–6, repeatedly.\n2. The PS1 aesthetic hides art weakness — a feature, not a compromise.\n3. Streamers do the marketing for free.\n\n**Biggest risk isn't dev — it's the launch pipeline.** Wishlists take weeks; the store page must go live in week 2.\n\n```json\n{\"v\":1,\"stance\":\"Short PS1-horror walking sim, store page by week 2\",\"confidence\":0.77,\"agree_with\":[],\"open_questions\":[\"budget ceiling?\"]}\n```",
  slowR1: "I keep coming back to scope math. Every genre we've named needs either content breadth or novel tech, and we have neither the hours nor the",
  slowR2: "Conceded on the evidence: the scope math only closes for a systems game. My residual is the *retention layer* — daily seeded challenge plus leaderboards, as retention after launch, not the purchase justification.\n\n```json\n{\"v\":1,\"stance\":\"Systems game with seeded-challenge retention layer\",\"confidence\":0.81,\"agree_with\":[],\"open_questions\":[]}\n```",
  med1: "Two stances filed, one abstention. The split is systems-versus-market, but both accept the 160-hour ceiling as binding. Focus next round: does the retention layer justify itself, and who concedes on genre?\n\n```json\n{\"v\":1,\"ruling\":\"CONTINUE\",\"request_user_input\":false,\"next_focus\":\"retention layer vs launch pipeline\",\"questions_for_user\":[],\"assumptions\":[],\"summary\":\"scope ceiling accepted; genre unresolved\"}\n```",
  med2: "All three panelists now back a systems-driven game with a seeded-challenge retention layer; confidence is high and mutual. I am ready to rule.\n\n```json\n{\"v\":1,\"ruling\":\"CONSENSUS\",\"request_user_input\":false,\"next_focus\":\"\",\"questions_for_user\":[],\"assumptions\":[],\"summary\":\"unanimous on systems-driven + retention layer\"}\n```",
  verdict: "The panel's verdict: build a **systems-driven arcade roguelite** with a seeded daily challenge and leaderboards as the retention layer.\n\nWhy this wins:\n\n- **Feasibility** — systems scale with skill, not hours; two people can ship the loop in a month.\n- **Market** — score-chasing plus a daily seed gives streamers and friends a reason to return.\n- **Pipeline** — the store page goes live week 2, wishlisting while content is polished.\n\n## Assumptions made\n- ~160 total dev hours between the two of you.\n- Store fee and review lead time are paid for on day one.",
} as const;

/** Split into word-preserving chunks so the stream reads naturally. */
function chunks(text: string, size: number): string[] {
  const words = text.split(/(?<=\s)/);
  const out: string[] = [];
  let buf = "";
  for (const w of words) {
    buf += w;
    if (buf.length >= size) { out.push(buf); buf = ""; }
  }
  if (buf) out.push(buf);
  return out;
}

async function playScriptedStream(store: Store) {
  store.loadDemoPanel();
  store.addPanelist();
  store.problem = "What game should a two-person team build in one month to make a little money on Steam?";
  const seats = store.seats;
  const med = seats.find((s) => s.role === "mediator")!;
  const [p1, p2, p3] = seats.filter((s) => s.role === "panelist");
  med.display_name = "Foreman";
  med.provider = "claude_cli";
  med.model = "claude-opus-4-8";
  med.personas = ["med.neutral_foreman"];
  p1.display_name = "Panelist 2";
  p1.provider = "claude_cli";
  p1.model = "claude-sonnet-5";
  p1.personas = ["temp.pragmatist", "dom.engineer"];
  p2.display_name = "Panelist 3";
  p2.provider = "codex_cli";
  p2.model = "";
  p2.personas = ["temp.skeptic", "dom.economist"];
  p3.display_name = "Panelist 4";
  p3.provider = "open_ai_compatible";
  p3.model = "qwen/qwen3.6-27b";
  p3.personas = ["temp.contrarian", "dom.designer"];
  store.mediatorId = med.id;
  store.running = true;
  store.phase = "room";

  const session = "preview-stream";
  let seq = 0;
  type Sessionless<T> = T extends unknown ? Omit<T, "session"> : never; // distribute over the union
  const emit = (e: Sessionless<EngineEvent>) => store.handle({ ...e, session } as EngineEvent);
  const sleep = (ms: number) => new Promise((r) => window.setTimeout(r, ms));

  async function streamSeat(seatId: string, round: number, text: string, msPerChunk: number, outcome?: { stance?: string; confidence?: number; abstain?: string }) {
    emit({ type: "seat_started", round, seat: seatId, attempt: 0 });
    const parts = chunks(text, 18);
    for (let i = 0; i < parts.length; i += 1) {
      emit({ type: "token", round, seat: seatId, attempt: 0, seq: seq += 1, seat_seq: i, text: parts[i] });
      await sleep(msPerChunk);
    }
    emit({ type: "seat_usage", round, seat: seatId, attempt: 0, input_tokens: 800 + Math.round(text.length / 3), output_tokens: Math.round(text.length / 4), emitted_seat_chunk_count: parts.length });
    if (outcome?.abstain) emit({ type: "seat_abstained", round, seat: seatId, reason: outcome.abstain });
    else if (outcome?.stance) emit({ type: "stance", round, seat: seatId, stance: outcome.stance, confidence: outcome.confidence ?? 0 });
  }

  // Round 1 — staggered finishes: p1's stance latches while p3 still streams;
  // p3 runs out of tokens mid-thought and abstains (red reason on the card).
  emit({ type: "round_started", round: 0 });
  await Promise.all([
    streamSeat(p1.id, 0, STREAM_TEXTS.fast, 24, { stance: "Systems-driven roguelite; cut multiplayer", confidence: 0.84 }),
    streamSeat(p2.id, 0, STREAM_TEXTS.medium, 42, { stance: "Short PS1-horror walking sim", confidence: 0.77 }),
    streamSeat(p3.id, 0, STREAM_TEXTS.slowR1, 80, { abstain: "no fenced json block found" }),
  ]);
  await streamSeat(med.id, 0, STREAM_TEXTS.med1, 22);
  emit({ type: "ruling", round: 0, ruling: "CONTINUE", summary: "scope ceiling accepted; genre unresolved", next_focus: "retention layer vs launch pipeline" });
  emit({ type: "round_telemetry", round: 0, effective_ruling: "CONTINUE", cluster_fraction: 0.5, mean_confidence: 0.8 });
  emit({ type: "round_complete", round: 0 });
  await sleep(2200);

  // Round 2 — everything from round 1 must visibly clear (stances, abstain
  // reason, usage), then p3 recovers and consensus forms.
  emit({ type: "round_started", round: 1 });
  await Promise.all([
    streamSeat(p1.id, 1, STREAM_TEXTS.fast, 22, { stance: "Systems-driven roguelite + retention layer", confidence: 0.88 }),
    streamSeat(p2.id, 1, STREAM_TEXTS.medium, 30, { stance: "Concede genre; keep week-2 store page", confidence: 0.82 }),
    streamSeat(p3.id, 1, STREAM_TEXTS.slowR2, 36, { stance: "Systems game with seeded-challenge retention", confidence: 0.81 }),
  ]);
  await streamSeat(med.id, 1, STREAM_TEXTS.med2, 22);
  emit({ type: "ruling", round: 1, ruling: "CONSENSUS", summary: "unanimous on systems-driven + retention layer", next_focus: "" });
  emit({ type: "round_telemetry", round: 1, effective_ruling: "CONSENSUS", cluster_fraction: 1, mean_confidence: 0.84 });
  emit({ type: "round_complete", round: 1 });
  await sleep(1200);
  emit({ type: "verdict", outcome: "converged", text: STREAM_TEXTS.verdict });
}
