import { defineStore } from "pinia";
import { computed, reactive, ref } from "vue";
import { api, onEngineEvent } from "@/lib/api";
import { estimateAcceptedCost, summarizeAcceptedUsage, type AcceptedUsage } from "@/lib/telemetry";
import { resolveSystemPrompt } from "@/lib/personas";
import type {
  EngineEvent, InteractionMode, RulingKind, SeatConfig, SessionConfig, SessionState,
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
  seatLastSeq: number;
  receivedChunkCount: number;
  expectedChunkCount?: number;
  streamIncomplete: boolean;
  attempt?: number;
  startedAt?: number;
  firstTokenAt?: number;
  usage?: AcceptedUsage;
}

export interface RoundSnapshot {
  round: number;
  stances: { seat: string; stance: string; confidence: number }[];
  ruling?: RulingKind;
  summary?: string;
  downgraded?: boolean;
  clusterFraction?: number;
  meanConfidence?: number;
}

export interface LogLine { id: number; receipt: number; kind: string; text: string }
export interface ConvergenceTelemetry { round: number; effectiveRuling: RulingKind; clusterFraction: number; meanConfidence: number; downgraded: boolean }

let uid = 0;
let logId = 0;
const LOG_CAP = 180;

function newSeat(role: SeatConfig["role"], partial: Partial<SeatConfig> = {}): SeatConfig {
  uid += 1;
  return {
    id: crypto.randomUUID(), display_name: role === "mediator" ? "Mediator" : `Panelist ${uid}`,
    provider: "anthropic", base_url: "https://api.anthropic.com", model: "claude-sonnet-5",
    system_prompt: "", sampling: { temperature: 0.7 }, personas: [], credential_ref: "anthropic-default", role, ...partial,
  };
}

function blankLive(id: string): SeatLive {
  return { id, text: "", status: "idle", lastSeq: -1, seatLastSeq: -1, receivedChunkCount: 0, streamIncomplete: false };
}

function resetSeatForRound(seat: SeatLive) {
  const lastSeq = seat.lastSeq;
  Object.assign(seat, blankLive(seat.id), { lastSeq });
}

export const useDeliberation = defineStore("deliberation", () => {
  const problem = ref("");
  const mode = ref<InteractionMode>("batched");
  const maxRounds = ref(8);
  const quorumFraction = ref(2 / 3);
  const confidenceFloor = ref(0.6);
  const seats = ref<SeatConfig[]>([newSeat("mediator"), newSeat("panelist"), newSeat("panelist")]);

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
  const acceptedUsage = ref<AcceptedUsage[]>([]);
  const logLines = ref<LogLine[]>([]);
  const convergence = ref<ConvergenceTelemetry | null>(null);
  const instantTokens = ref(false);

  let pendingRuling: RulingKind | undefined;
  let pendingSummary: string | undefined;
  let pendingDowngraded = false;
  const pendingMetrics = new Map<number, ConvergenceTelemetry>();
  const usageKeys = new Set<string>();
  const tokenBuffers = new Map<string, string>();
  let flushHandle = 0;
  let lastFlush = 0;

  const panelists = computed(() => seats.value.filter((seat) => seat.role === "panelist"));
  const mediator = computed(() => seats.value.find((seat) => seat.role === "mediator") ?? null);
  const usageSummary = computed(() => summarizeAcceptedUsage(acceptedUsage.value));
  const estimatedCost = computed(() => estimateAcceptedCost(acceptedUsage.value, seats.value));
  const approximateOutputRate = computed(() => {
    const starts = Object.values(live).map((seat) => seat.startedAt).filter((value): value is number => Boolean(value));
    const output = acceptedUsage.value.reduce((sum, usage) => sum + (usage.outputTokens ?? 0), 0);
    if (!starts.length || !output) return null;
    return output / Math.max(1, (Date.now() - Math.min(...starts)) / 1000);
  });

  const validation = computed<string[]>(() => {
    const errs: string[] = [];
    if (!problem.value.trim()) errs.push("Problem statement is empty.");
    if (seats.value.filter((seat) => seat.role === "mediator").length !== 1) errs.push("Exactly one mediator is required.");
    if (panelists.value.length < 2 || panelists.value.length > 6) errs.push(`Need 2–6 panelists (have ${panelists.value.length}).`);
    if (maxRounds.value < 1 || maxRounds.value > 64) errs.push("Rounds must be 1–64.");
    for (const seat of seats.value) {
      if (!providerIsHttp(seat.provider)) continue;
      if (!seat.model.trim()) errs.push(`${seat.display_name}: model is empty.`);
      if (!seat.base_url.trim()) errs.push(`${seat.display_name}: base URL is empty.`);
      if (!seat.credential_ref.trim() && !isLoopbackUrl(seat.base_url)) errs.push(`${seat.display_name}: credential is empty.`);
    }
    return errs;
  });

  function appendLog(kind: string, text: string) {
    logLines.value.push({ id: ++logId, receipt: Date.now(), kind, text });
    if (logLines.value.length > LOG_CAP) logLines.value.splice(0, logLines.value.length - LOG_CAP);
  }
  function buildConfig(): SessionConfig {
    const seatsResolved = seats.value.map((seat) => ({
      ...JSON.parse(JSON.stringify(seat)),
      system_prompt: resolveSystemPrompt(seat.personas, seat.system_prompt),
    }));
    return { problem: problem.value, mode: mode.value, max_rounds: maxRounds.value, guard: { quorum_fraction: quorumFraction.value, confidence_floor: confidenceFloor.value }, seats: seatsResolved };
  }
  function addPanelist() { if (panelists.value.length < 6) seats.value.push(newSeat("panelist")); }
  function loadDemoPanel() {
    const demo = (role: SeatConfig["role"], display_name: string, personas: string[]) => newSeat(role, { display_name, provider: "demo", base_url: "", model: "demo", credential_ref: "", personas });
    seats.value = [
      demo("mediator", "Foreman (demo)", ["med.neutral_foreman"]),
      demo("panelist", "Juror A (demo)", ["temp.optimist", "dom.designer"]),
      demo("panelist", "Juror B (demo)", ["temp.skeptic", "dom.engineer"]),
    ];
    if (!problem.value.trim()) problem.value = "Should our team adopt a four-day work week?";
  }
  function removeSeat(id: string) { seats.value = seats.value.filter((seat) => seat.id !== id); }
  function resetRuntime() {
    for (const key of Object.keys(live)) delete live[key];
    seats.value.forEach((seat) => { live[seat.id] = blankLive(seat.id); });
    mediatorId.value = mediator.value?.id ?? null; mediatorText.value = ""; rounds.value = []; awaiting.value = null;
    verdict.value = null; failure.value = null; finalState.value = null; currentRound.value = 0; acceptedUsage.value = [];
    logLines.value = []; convergence.value = null; pendingMetrics.clear(); usageKeys.clear(); tokenBuffers.clear();
    pendingRuling = undefined; pendingSummary = undefined; pendingDowngraded = false;
  }
  async function start() {
    startError.value = null;
    if (validation.value.length) { startError.value = validation.value.join(" "); return; }
    resetRuntime();
    try { const result = await api.startDeliberation(crypto.randomUUID(), buildConfig()); sessionId.value = result.session_id; running.value = true; phase.value = "room"; appendLog("state_changed", "session convened"); }
    catch (error) { startError.value = String(error); }
  }
  async function submitAnswers(answers: [string, string][]) { if (!sessionId.value) return; awaiting.value = null; await api.answerQuestions(sessionId.value, answers); }
  async function abandon() { if (sessionId.value) await api.abandon(sessionId.value); }
  function seatOf(id: string): SeatLive { return live[id] ?? (live[id] = blankLive(id)); }
  function flushTokens() {
    flushHandle = 0; lastFlush = performance.now();
    for (const [id, text] of tokenBuffers) {
      const seat = seatOf(id); seat.text += text;
      if (id === mediatorId.value) mediatorText.value += text;
    }
    tokenBuffers.clear();
  }
  function queueToken(id: string, text: string) {
    if (instantTokens.value) {
      const seat = seatOf(id); seat.text += text;
      if (id === mediatorId.value) mediatorText.value += text;
      return;
    }
    tokenBuffers.set(id, `${tokenBuffers.get(id) ?? ""}${text}`);
    if (!flushHandle) {
      const delay = Math.max(0, 33 - (performance.now() - lastFlush));
      flushHandle = window.setTimeout(() => requestAnimationFrame(flushTokens), delay);
    }
  }
  function snapshotRound(round: number) {
    flushTokens();
    const metrics = pendingMetrics.get(round);
    const stances = panelists.value.map((seat) => ({ seat: seat.id, stance: live[seat.id]?.stance ?? "", confidence: live[seat.id]?.confidence ?? 0 }));
    rounds.value.push({ round, stances, ruling: metrics?.effectiveRuling ?? pendingRuling, summary: pendingSummary, downgraded: metrics?.downgraded ?? pendingDowngraded, clusterFraction: metrics?.clusterFraction, meanConfidence: metrics?.meanConfidence });
    pendingMetrics.delete(round); pendingRuling = undefined; pendingSummary = undefined; pendingDowngraded = false;
  }
  function handle(e: EngineEvent) {
    if (sessionId.value && e.session !== sessionId.value) return;
    switch (e.type) {
      case "round_started":
        currentRound.value = e.round; mediatorText.value = ""; appendLog(e.type, `round ${e.round + 1} started`);
        seats.value.forEach((seat) => { resetSeatForRound(seatOf(seat.id)); }); break;
      case "seat_started": {
        { const seat = seatOf(e.seat); const lastSeq = seat.lastSeq;
          Object.assign(seat, blankLive(e.seat), { lastSeq, status: "streaming", startedAt: Date.now(), attempt: e.attempt }); }
        if (e.seat === mediatorId.value) mediatorText.value = "";
        appendLog(e.type, `${seats.value.find((seat) => seat.id === e.seat)?.display_name ?? e.seat} attempt ${e.attempt + 1}`); break;
      }
      case "token": {
        const seat = seatOf(e.seat);
        if (seat.attempt !== e.attempt) return;
        if (e.seq <= seat.lastSeq) return;
        seat.lastSeq = e.seq;
        if (e.seat_seq !== seat.seatLastSeq + 1) seat.streamIncomplete = true;
        seat.seatLastSeq = e.seat_seq; seat.receivedChunkCount += 1; seat.firstTokenAt ??= Date.now(); queueToken(e.seat, e.text); break;
      }
      case "seat_usage": {
        const key = `${e.round}:${e.seat}:${e.attempt}`;
        if (usageKeys.has(key)) return;
        usageKeys.add(key);
        const usage: AcceptedUsage = { round: e.round, seat: e.seat, attempt: e.attempt, inputTokens: e.input_tokens, outputTokens: e.output_tokens, emittedSeatChunkCount: e.emitted_seat_chunk_count, receivedAt: Date.now() };
        acceptedUsage.value.push(usage);
        const seat = seatOf(e.seat); seat.usage = usage; seat.expectedChunkCount = e.emitted_seat_chunk_count;
        if (seat.receivedChunkCount !== e.emitted_seat_chunk_count || seat.seatLastSeq !== e.emitted_seat_chunk_count - 1) seat.streamIncomplete = true;
        appendLog(e.type, `${seats.value.find((candidate) => candidate.id === e.seat)?.display_name ?? e.seat}: ${e.input_tokens ?? "?"} in / ${e.output_tokens ?? "?"} out`); break;
      }
      case "stance": { const seat = seatOf(e.seat); seat.status = "stance"; seat.stance = e.stance; seat.confidence = e.confidence; appendLog(e.type, `${e.seat.slice(0, 6)} filed stance (${Math.round(e.confidence * 100)}%)`); break; }
      case "seat_abstained": { const seat = seatOf(e.seat); seat.status = "abstained"; seat.reason = e.reason; appendLog(e.type, `${e.seat.slice(0, 6)} abstained: ${e.reason}`); break; }
      case "seat_truncated": { const seat = seatOf(e.seat); if (seat.status === "streaming") seat.status = "truncated"; appendLog(e.type, `${e.seat.slice(0, 6)} truncated: ${e.cause}`); break; }
      case "ruling": pendingRuling = e.ruling; pendingSummary = e.summary; appendLog(e.type, `${e.ruling}: ${e.summary || e.next_focus}`); break;
      case "consensus_downgraded":
        pendingDowngraded = true; convergence.value = { round: e.round, effectiveRuling: "CONTINUE", clusterFraction: e.cluster_fraction, meanConfidence: e.mean_confidence, downgraded: true }; appendLog(e.type, "consensus guard downgraded ruling"); break;
      case "round_telemetry": {
        const metrics = { round: e.round, effectiveRuling: e.effective_ruling, clusterFraction: e.cluster_fraction, meanConfidence: e.mean_confidence, downgraded: pendingDowngraded };
        pendingMetrics.set(e.round, metrics); convergence.value = metrics; appendLog(e.type, `${e.effective_ruling}: cluster ${Math.round(e.cluster_fraction * 100)}%`); break;
      }
      case "round_complete": snapshotRound(e.round); appendLog(e.type, `round ${e.round + 1} closed`); break;
      case "awaiting_user": awaiting.value = { round: e.round, questions: e.questions }; appendLog(e.type, "operator input requested"); break;
      case "verdict": flushTokens(); verdict.value = { text: e.text, outcome: e.outcome }; finalState.value = e.outcome; running.value = false; phase.value = "verdict"; appendLog(e.type, `verdict: ${e.outcome}`); break;
      case "failed": failure.value = { state: e.state, reason: e.reason }; finalState.value = e.state; awaiting.value = null; running.value = false; phase.value = "verdict"; appendLog(e.type, `failed: ${e.reason}`); break;
      case "state_changed": finalState.value = e.state; if (isTerminal(e.state)) running.value = false; appendLog(e.type, e.state); break;
    }
  }
  let unlisten: (() => void) | null = null;
  async function init() { if (!unlisten) unlisten = await onEngineEvent(handle); }
  function backToSetup() { phase.value = "setup"; sessionId.value = null; }
  async function exportMarkdown(): Promise<string> { return sessionId.value ? api.exportSession(sessionId.value) : ""; }
  function setReducedEffects(value: boolean) { instantTokens.value = value; }
  return {
    problem, mode, maxRounds, quorumFraction, confidenceFloor, seats, panelists, mediator, validation, addPanelist, removeSeat, loadDemoPanel,
    phase, sessionId, running, currentRound, live, mediatorId, mediatorText, rounds, awaiting, verdict, failure, finalState, startError,
    acceptedUsage, usageSummary, estimatedCost, approximateOutputRate, logLines, convergence,
    init, start, submitAnswers, abandon, backToSetup, exportMarkdown, handle, setReducedEffects,
  };
});
