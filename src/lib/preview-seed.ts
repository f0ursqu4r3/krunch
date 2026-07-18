// DEV-ONLY: seed the store with representative data so the Room / Verdict screens
// can be designed and screenshotted without the Tauri backend. Activated via
// ?preview=room|verdict|deadlock|halted|awaiting. No-op in production builds.

import type { useDeliberation } from "@/stores/deliberation";
import type { RoundSnapshot } from "@/stores/deliberation";

type Store = ReturnType<typeof useDeliberation>;

const P1 =
  "A cozy farming-sim leans on a proven, loyal Steam audience — Stardew's long tail shows wishlists compound. Two people can ship a tight vertical slice of farming, seasons, and one town in a month if we ruthlessly cut multiplayer.";
const P2 =
  "Scope is the killer. A horror roguelite is more novel and streams well, but procedural generation plus enemy AI plus juice is a six-month job disguised as one. If we must ship in 30 days, the cozy sim is the only honestly-finishable option.";
const MED =
  "Both jurors now favor the cozy farming-sim on feasibility and audience grounds, and each explicitly agrees with the other's core point about scope. Confidence is high and mutual. I am ready to rule.";

export function seedPreview(store: Store, kind: string) {
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
    stance: "Cozy farming-sim — proven audience", confidence: 0.86, lastSeq: 9,
  };
  store.live[p2.id] = {
    id: p2.id, text: P2, status: "stance",
    stance: "Cozy sim is the only finishable option", confidence: 0.79, lastSeq: 9,
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
