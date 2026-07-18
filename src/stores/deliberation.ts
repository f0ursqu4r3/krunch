import { defineStore } from "pinia";
import { computed, reactive, ref } from "vue";
import { api, onEngineEvent } from "@/lib/api";
import type {
  EngineEvent,
  InteractionMode,
  RulingKind,
  SeatConfig,
  SessionConfig,
  SessionState,
} from "@/lib/types";
import { isTerminal, isLoopbackUrl, providerIsHttp } from "@/lib/types";

type Phase = "setup" | "room" | "verdict";

export type SeatStatus = "idle" | "streaming" | "stance" | "abstained" | "truncated";

export interface SeatLive {
  id: string;
  text: string;
  status: SeatStatus;
  stance?: string;
  confidence?: number;
  reason?: string;
  lastSeq: number;
}

export interface RoundSnapshot {
  round: number;
  stances: { seat: string; stance: string; confidence: number }[];
  ruling?: RulingKind;
  summary?: string;
  downgraded?: boolean;
}

let uid = 0;
function newSeat(role: SeatConfig["role"], partial: Partial<SeatConfig> = {}): SeatConfig {
  uid += 1;
  return {
    id: crypto.randomUUID(),
    display_name: role === "mediator" ? "Mediator" : `Panelist ${uid}`,
    provider: "anthropic",
    base_url: role === "mediator" ? "https://api.anthropic.com" : "https://api.anthropic.com",
    model: "claude-sonnet-5",
    system_prompt: "",
    sampling: { temperature: 0.7 },
    credential_ref: "anthropic-default",
    role,
    ...partial,
  };
}

export const useDeliberation = defineStore("deliberation", () => {
  // --- config draft ---
  const problem = ref("");
  const mode = ref<InteractionMode>("batched");
  const maxRounds = ref(8);
  const quorumFraction = ref(2 / 3);
  const confidenceFloor = ref(0.6);
  const seats = ref<SeatConfig[]>([
    newSeat("mediator"),
    newSeat("panelist"),
    newSeat("panelist"),
  ]);

  // --- runtime ---
  const phase = ref<Phase>("setup");
  const sessionId = ref<string | null>(null);
  const running = ref(false);
  const currentRound = ref(0);
  const live = reactive<Record<string, SeatLive>>({});
  const mediatorId = ref<string | null>(null);
  const mediatorText = ref("");
  const rounds = ref<RoundSnapshot[]>([]);
  const awaiting = ref<{ round: number; questions: string[] } | null>(null);
  const verdict = ref<{ text: string; outcome: SessionState } | null>(null);
  const failure = ref<{ state: SessionState; reason: string } | null>(null);
  const finalState = ref<SessionState | null>(null);
  const startError = ref<string | null>(null);

  // per-round in-progress ruling capture
  let pendingRuling: RulingKind | undefined;
  let pendingSummary: string | undefined;
  let pendingDowngraded = false;

  const panelists = computed(() => seats.value.filter((s) => s.role === "panelist"));
  const mediator = computed(() => seats.value.find((s) => s.role === "mediator") ?? null);

  const validation = computed<string[]>(() => {
    const errs: string[] = [];
    if (!problem.value.trim()) errs.push("Problem statement is empty.");
    if (seats.value.filter((s) => s.role === "mediator").length !== 1)
      errs.push("Exactly one mediator is required.");
    const p = panelists.value.length;
    if (p < 2 || p > 6) errs.push(`Need 2–6 panelists (have ${p}).`);
    if (maxRounds.value < 1 || maxRounds.value > 64) errs.push("Rounds must be 1–64.");
    for (const s of seats.value) {
      // HTTP providers need url + model; a credential too unless the endpoint is
      // loopback. CLI/Demo providers need none of these (M7).
      if (providerIsHttp(s.provider)) {
        if (!s.model.trim()) errs.push(`${s.display_name}: model is empty.`);
        if (!s.base_url.trim()) errs.push(`${s.display_name}: base URL is empty.`);
        if (!s.credential_ref.trim() && !isLoopbackUrl(s.base_url))
          errs.push(`${s.display_name}: credential is empty.`);
      }
    }
    return errs;
  });

  function buildConfig(): SessionConfig {
    return {
      problem: problem.value,
      mode: mode.value,
      max_rounds: maxRounds.value,
      guard: { quorum_fraction: quorumFraction.value, confidence_floor: confidenceFloor.value },
      seats: JSON.parse(JSON.stringify(seats.value)),
    };
  }

  function addPanelist() {
    if (panelists.value.length < 6) seats.value.push(newSeat("panelist"));
  }

  /** Fill the roster with offline demo seats — runs with no keys/network. */
  function loadDemoPanel() {
    const demo = (role: SeatConfig["role"], name: string) =>
      newSeat(role, {
        display_name: name,
        provider: "demo",
        base_url: "",
        model: "demo",
        credential_ref: "",
      });
    seats.value = [
      demo("mediator", "Foreman (demo)"),
      demo("panelist", "Juror A (demo)"),
      demo("panelist", "Juror B (demo)"),
    ];
    if (!problem.value.trim()) {
      problem.value = "Should our team adopt a four-day work week?";
    }
  }
  function removeSeat(id: string) {
    seats.value = seats.value.filter((s) => s.id !== id);
  }

  function resetRuntime() {
    for (const k of Object.keys(live)) delete live[k];
    for (const s of seats.value) {
      live[s.id] = { id: s.id, text: "", status: "idle", lastSeq: -1 };
    }
    mediatorId.value = mediator.value?.id ?? null;
    mediatorText.value = "";
    rounds.value = [];
    awaiting.value = null;
    verdict.value = null;
    failure.value = null;
    finalState.value = null;
    currentRound.value = 0;
    pendingRuling = undefined;
    pendingSummary = undefined;
    pendingDowngraded = false;
  }

  async function start() {
    startError.value = null;
    if (validation.value.length) {
      startError.value = validation.value.join(" ");
      return;
    }
    resetRuntime();
    try {
      const res = await api.startDeliberation(crypto.randomUUID(), buildConfig());
      sessionId.value = res.session_id;
      running.value = true;
      phase.value = "room";
    } catch (e) {
      startError.value = String(e);
    }
  }

  async function submitAnswers(answers: [string, string][]) {
    if (!sessionId.value) return;
    awaiting.value = null;
    await api.answerQuestions(sessionId.value, answers);
  }

  async function abandon() {
    if (sessionId.value) await api.abandon(sessionId.value);
  }

  function seatOf(id: string): SeatLive {
    if (!live[id]) live[id] = { id, text: "", status: "idle", lastSeq: -1 };
    return live[id];
  }

  function snapshotRound(round: number) {
    const stances = seats.value
      .filter((s) => s.role === "panelist")
      .map((s) => {
        const l = live[s.id];
        return { seat: s.id, stance: l?.stance ?? "", confidence: l?.confidence ?? 0 };
      });
    rounds.value.push({
      round,
      stances,
      ruling: pendingRuling,
      summary: pendingSummary,
      downgraded: pendingDowngraded,
    });
    pendingRuling = undefined;
    pendingSummary = undefined;
    pendingDowngraded = false;
  }

  function handle(e: EngineEvent) {
    // Fence: ignore events for other sessions.
    if ("session" in e && sessionId.value && e.session !== sessionId.value) return;

    switch (e.type) {
      case "round_started":
        currentRound.value = e.round;
        mediatorText.value = "";
        for (const s of seats.value) {
          const l = seatOf(s.id);
          l.text = "";
          l.status = "idle";
          l.stance = undefined;
          l.confidence = undefined;
          l.reason = undefined;
        }
        break;
      case "seat_started": {
        const l = seatOf(e.seat);
        l.text = "";
        l.status = "streaming";
        if (e.seat === mediatorId.value) mediatorText.value = "";
        break;
      }
      case "token": {
        const l = seatOf(e.seat);
        if (e.seq <= l.lastSeq) return; // fence: stale/out-of-order
        l.lastSeq = e.seq;
        l.text += e.text;
        if (e.seat === mediatorId.value) mediatorText.value += e.text;
        break;
      }
      case "stance": {
        const l = seatOf(e.seat);
        l.status = "stance";
        l.stance = e.stance;
        l.confidence = e.confidence;
        break;
      }
      case "seat_abstained": {
        const l = seatOf(e.seat);
        l.status = "abstained";
        l.reason = e.reason;
        break;
      }
      case "seat_truncated": {
        const l = seatOf(e.seat);
        if (l.status === "streaming") l.status = "truncated";
        break;
      }
      case "ruling":
        pendingRuling = e.ruling;
        pendingSummary = e.summary;
        break;
      case "consensus_downgraded":
        pendingDowngraded = true;
        break;
      case "round_complete":
        snapshotRound(e.round);
        break;
      case "awaiting_user":
        awaiting.value = { round: e.round, questions: e.questions };
        break;
      case "verdict":
        verdict.value = { text: e.text, outcome: e.outcome };
        finalState.value = e.outcome;
        running.value = false;
        phase.value = "verdict";
        break;
      case "failed":
        failure.value = { state: e.state, reason: e.reason };
        finalState.value = e.state;
        awaiting.value = null;
        running.value = false;
        phase.value = "verdict";
        break;
      case "state_changed":
        finalState.value = e.state;
        if (isTerminal(e.state)) running.value = false;
        break;
    }
  }

  let unlisten: (() => void) | null = null;
  async function init() {
    if (unlisten) return;
    unlisten = await onEngineEvent(handle);
  }

  function backToSetup() {
    phase.value = "setup";
    sessionId.value = null;
  }

  async function exportMarkdown(): Promise<string> {
    if (!sessionId.value) return "";
    return api.exportSession(sessionId.value);
  }

  return {
    // config
    problem, mode, maxRounds, quorumFraction, confidenceFloor, seats,
    panelists, mediator, validation,
    addPanelist, removeSeat, loadDemoPanel,
    // runtime
    phase, sessionId, running, currentRound, live, mediatorId, mediatorText,
    rounds, awaiting, verdict, failure, finalState, startError,
    // actions
    init, start, submitAnswers, abandon, backToSetup, exportMarkdown,
  };
});
