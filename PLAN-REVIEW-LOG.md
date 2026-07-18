# Plan Review Log: krunch UI re-imagining — "Mission Control" terminal cockpit
Act 1 (grill) complete — plan locked with the user. MAX_ROUNDS=5.
Reviewer model: gpt-5.6-terra (config default) — codex-cli 0.144.0.

## Round 1 — Codex
Material problems:

- Anthropic input usage is not parsed: `message_start.usage.input_tokens` is ignored, so "real input tokens" is false for Anthropic. Fix: parse+retain message_start input usage, preserve unknown-vs-zero, add fixtures.
- Transient `SeatUsage` can't support telemetry after drops/reload/recovery: no usage record or read API in SQLite. Fix: persist usage per accepted attempt + read model, or scope as explicitly transient.
- `SeatUsage { session, round, seat }` not idempotent: retries/replays double-count. Fix: include `attempt` and dedupe by attempt, count accepted only.
- TTFT/tok-s can't be truthful from client: no engine timestamps; client receipt includes queue/render delay. Fix: emit engine timing or label client calc approximate.
- Live transcript already lossy under load: `StreamSink` drops token events when channel full; no hydration API despite "reload authoritative text" claim. Fix: hydration command, or drop the claim + coalesce.
- Convergence meter has no reliable alignment signal: frontend stance events carry only prose+confidence; consensus is `agree_with`+backend guard. Fix: emit backend cluster/agreement metrics; don't infer from stance strings.
- Finalization pass invalidates round counter: engine emits finalization as round_index+1 → can display `09/08`. Fix: round-kind field, render finalization separately.
- Client pricing fabricates cost for custom OpenAI-compatible endpoints: name match insufficient. Fix: key prices by canonical provider endpoint+revision, else `—`.
- Raw EngineEvent rail overwhelms renderer/memory: token events too numerous + duplicate transcript. Fix: bounded batched lifecycle-log schema, exclude token events.
- Command palette not implementable as written: no shadcn-vue `Command` in repo; reka Listbox alone isn't a palette. Fix: specify exact primitive/path or build from Dialog + combobox.
- Shortcut handling unsafe: excluding only input/textarea misses contenteditable, select, IME, modifiers, phase-invalid actions. Fix: centralize bindings with composition/modifier/focus/phase guards.
- Reduced motion ≠ contrast fix: static scanlines/glow still cut contrast; effects preference unresolved. Fix: measurable contrast targets + persistent effects/contrast toggle.
- 940×640 min window won't fit 6-seat grid + mediator + rail + palette + overlay. Fix: compact breakpoints, pane collapse/scroll, test at min + 200% zoom.
- Verification plan misses risky paths: seeds don't test Rust parsing, event loss/replay, migrations, hotkeys, price. Fix: Rust parser/engine/migration tests + frontend store tests.
- Scope/assets unresolved: feature cuts + display font license/source/bundle still open. Fix: lock before implementation as acceptance criteria.

VERDICT: REVISE

### Claude's response
All 15 accepted — Codex read the real code and every finding checks out (verified: `sse.rs` drops Anthropic input usage; no `src/components/ui/command`; tauri.conf min 940×640; all events keyed by `round`). Folded into `PLAN.md`, three with an arbiter nuance:
- **Usage persistence (#2):** scoped as explicitly **transient UI-only for v1** — telemetry resets on reload, no SQLite migration this pass (persistence deferred to a later data pass). Keeps "small engine work" honest; documented as a known limitation, not hidden.
- **TTFT/tok-s (#4):** labeled **approximate** (`~`, client-derived) rather than adding an engine timing-metrics pipeline. Real token *counts* are exact (from usage); rates are honestly marked approximate.
- **Transcript loss (#5):** dropped the false "reload authoritative text" claim entirely. Token display stays best-effort (existing `StreamSink` behavior unchanged); the new log rail carries **lifecycle events only**, so it never depends on lossy token delivery.

Accepted and added:
- Anthropic `message_start` input-usage parsing + fixtures (#1); `SeatUsage` carries `attempt`, store dedupes by `(round,seat,attempt)`, counts accepted attempt only (#3).
- Convergence readout driven by backend truth: extend `Ruling`/emit cluster metric (`cluster_fraction`, `mean_confidence`) each round; meter never infers consensus from stance prose (#6).
- Finalization rendered distinctly ("SYNTHESIZING"); round counter clamped to max, never shows `NN/<max+1>` (#7).
- Pricing keyed by canonical (provider, model) for anthropic/openai only; openai_compatible/CLI/demo/unknown → `—`, never fabricated (#8).
- Event-log rail = bounded ring buffer, lifecycle events only, token events excluded (#9).
- Command palette built from existing `ui/dialog` + reka `ListboxRoot`/combobox (or shadcn-vue CLI add), path specified (#10).
- Centralized keybinding module: contenteditable + select + IME `isComposing` + modifier + phase-aware + focus-restoration guards (#11).
- Persistent effects/contrast toggle (Off/Ambient/Max, default Ambient) in localStorage; static overlays contrast-capped; reduced-motion still kills motion (#12).
- Compact desktop breakpoints: at min size the log rail collapses + seat grid scrolls; tested at 940×640 and 200% zoom (#13).
- Verification adds Rust tests (Anthropic input-usage fixture, `SeatUsage` emit/dedupe) + frontend store tests (usage dedupe across retries, log-rail cap, shortcut focus guards, reduced-motion) (#14).
- Display font locked: **Departure Mono** (SIL OFL 1.1, self-hosted woff2, ≤ ~40KB budget) as the blocky display face (#15). Feature cuts remain the single item held for user sign-off.

## Round 2 — Codex
Most prior findings addressed. Remaining material issues:

- "Exact token totals" still false when usage unavailable: CLI/demo return `None`, HTTP can omit a count; summing nulls as 0 undercounts. Fix: nullable wire fields, render `—`/`partial`, never "exact" unless every included completion reported both counts.
- Acceptance fence ambiguous: "emit after `Ok(completion)`" permits emitting before `accept_attempt`; a later storage failure shows usage for an unaccepted attempt. Fix: emit `SeatUsage` only after `accept_attempt` succeeds, carry durable attempt identity.
- Convergence event can report wrong ruling: `Ruling` is emitted before the guard can downgrade it. Fix: compute cluster metrics + effective ruling after guard, emit one final per-round telemetry event for UI + log.
- Logs have no authoritative timestamp/ordering: events carry neither; browser receipt time is distorted by concurrent producers. Fix: session-monotonic seq + engine timestamp, or label displayed times as local receipt.
- Best-effort streaming has no visible loss signal: dropped tokens leave a plausible-but-incomplete argument while usage says it was larger. Fix: per-seat/attempt sequencing + `stream_gap` indicator, mark transcript incomplete on gap.
- Transient telemetry unresolved product contract: plan still asks whether reload-loss is acceptable. Fix: get sign-off on transient-only, or add the persisted read model.
- Perf mitigation only a risk note: no measurable budget/rule for when effects degrade under seven concurrent streams. Fix: define a frame-time budget + concrete degradation policy, test at max seats.

VERDICT: REVISE

### Claude's response
All 7 accepted — verified against real code (`accept_attempt`/`discard_attempt` lifecycle at `lib.rs:554-556`; guard downgrades after `Ruling` with `cluster_fraction`/`mean_confidence` in hand at `lib.rs:380-394`). Both major fixes land on existing structure. Folded in:
- **Nullable usage → honest totals (#1):** wire fields nullable; per-seat/session totals shown "exact" only when every included completion reported both counts, else `partial`; providers with no usage (CLI/demo) → `—`. No summing nulls as zero.
- **Emit-after-accept (#2):** `SeatUsage` emitted immediately after `accept_attempt(attempt_id)` succeeds (`lib.rs:556`), never on a discarded/retried attempt; carries `attempt` (consistent with `SeatStarted`/`Token`), store dedupes by `(round,seat,attempt)`.
- **Single post-guard telemetry event (#3):** cluster/convergence telemetry (`effective_ruling`, `cluster_fraction`, `mean_confidence`) computed AFTER the guard and emitted at `RoundComplete` (`lib.rs:396`); the readout uses the effective (post-downgrade) ruling and also folds `ConsensusDowngraded` live during the round. Never trusts the pre-guard `Ruling`.
- **Honest log timestamps (#4):** log rail labels times as **local receipt time**, ordered by receipt — no claim of engine-authoritative ordering. (Engine timestamp envelope noted as a later enhancement.)
- **Visible stream-gap signal (#5):** the store already tracks `lastSeq` per seat; on a seq gap (received > `lastSeq`+1) set a per-seat `gap` flag and render a "⚠ stream incomplete" marker, so a dropped-token transcript can't masquerade as complete.
- **Transient telemetry = decided, not asked (#6):** v1 is transient-only by decision (resets on reload); persisted read-model is an explicit deferred follow-up. Removed the waffle; still surfaced to the user at sign-off.
- **Perf budget + degradation policy (#7):** type-on batched via `requestAnimationFrame`, text repaint capped (~≤30fps); target ~60fps idle and ≥30fps under 7 concurrent streams; if the frame budget is exceeded the console auto-drops to reduced effects (independent of the manual Off/Ambient/Max toggle). Tested with 6 panelists + mediator streaming.

## Round 3 — Codex
Most prior findings addressed. Remaining material issues:

- Stream-gap detector is incorrect: `seq` is session-global and interleaves concurrent seats, so normal A(0),B(1),A(2) falsely marks A incomplete. Fix: emit a counter scoped to `(round,seat,attempt)`, reset on `seat_started`, add a concurrency test.
- "Exact" usage underreports actual consumption on retries: counting only accepted attempts ignores billable tokens a failed/retried streamed request consumed. Fix: label "accepted-completion tokens/cost", never "total billing".
- Post-guard telemetry ordering unspecified: if the per-round event arrives after `round_complete`, the store snapshots before it has the effective ruling/metrics. Fix: emit post-guard telemetry before `round_complete`, snapshot by round id defensively, test ordering.
- Perf test assumes impossible shape: panelists run concurrently then mediator runs after `join_all`, so there are never seven simultaneous streams. Fix: benchmark the real six-panelist worst case; label seven-stream as synthetic.
- "Canonical OpenAI host" not executable: a loose base-URL check can price a third-party compatible endpoint as OpenAI. Fix: parse URL, price only when normalized origin is exactly the approved OpenAI origin, else `—`.
- Auto-degradation needs a state policy: without thresholds/hysteresis/precedence, effects flap or override the user. Fix: define sampling window, degrade/recover thresholds, hysteresis, and auto-vs-manual precedence.

VERDICT: REVISE

### Claude's response
All 6 accepted — verified the seq claim against `lib.rs:142` (`seq: Arc<AtomicU64>` is shared session-global; it's also what lets retries survive the store's `e.seq <= lastSeq` stale-fence, so it must NOT be reset). Folded in:
- **Correct gap detection (#1):** add a separate per-sink `seat_seq: u64` (each `StreamSink` gets its own counter from 0, reset per attempt) to the `Token` event — leave the shared global `seq` untouched for the stale-fence. Gap detection uses `seat_seq` contiguity, baseline reset on `seat_started`. Add a concurrency test proving no false gaps under interleaved seats.
- **Honest billing label (#2):** the metric is "accepted-completion tokens" / "est. accepted-completion cost", never "total billing"; note that discarded/retried attempts may have consumed provider tokens not counted.
- **Ordering (#3):** the post-guard per-round telemetry event is emitted **before** `RoundComplete` (between `lib.rs:394` and `:397`); the store stashes pending metrics and `snapshotRound` consumes them, keyed by round id. Add an event-ordering test.
- **Real perf shape (#4):** panelists run concurrently then the mediator runs after `join_all` — max real concurrency is the panelist count (≤6). Retarget the budget to the **6-panelist worst case**; keep any 7-stream figure only as a separately-labeled synthetic stress test.
- **Executable pricing origin (#5):** parse the seat `base_url`, normalize the origin, and allow OpenAI pricing only when it exactly equals `https://api.openai.com` (and Anthropic pricing only for `https://api.anthropic.com`); any other origin → `—`.
- **Degradation state policy (#6):** sample frame time over a 1s rolling window; degrade after 2 consecutive windows with mean frame > 24 ms (~<42fps); recover after 3s sustained < 18 ms (hysteresis prevents flapping). Precedence: manual **Off** always wins; manual **Max** opts out of auto-degrade (explicit user override); **Ambient** (default) is the only mode auto-degradation touches.

## Round 4 — Codex
Prior ordering, pricing-origin, performance, and per-seat sequencing issues are addressed. One remaining:

- Terminal token drops remain undetectable: contiguous `seat_seq` catches a missing middle token, but not a dropped final token or an entirely dropped stream, since no later token exposes the gap. Fix: include each attempt's final emitted `seat_seq` in `SeatUsage` (or a completion event); mark the transcript incomplete unless the client received every sequence through that terminal value; test final/all-token drop.

VERDICT: REVISE

### Claude's response
Accepted — genuinely sharp, and `SeatUsage` (emitted right after `accept_attempt`) is the natural carrier. Folded in: `SeatUsage` gains `final_seat_seq` = the terminal `seat_seq` the sink emitted for the accepted attempt. The store marks a seat's transcript **incomplete** unless it received a contiguous run through `final_seat_seq` (0-token accepted attempts included). This closes middle-drop, final-token-drop, and whole-stream-drop. Tests: final-token drop and whole-stream drop both surface the "⚠ stream incomplete" marker.

## Round 5 — Codex (MAX_ROUNDS cap)
Prior findings addressed. One remaining edge case:

- `final_seat_seq: u64` has no representation for a successful completion that emitted zero chunks: `0` could mean "first chunk expected" or "no chunks existed" → false incompleteness or an unmarked empty transcript. Fix: send `emitted_seat_chunk_count: u64` (zero unambiguous) or make it nullable; test empty successful completions + text-without-streamed-chunks fallback.

VERDICT: REVISE

### Claude's response
Accepted in full — no counter-position. Verified against `lib.rs:548`: the `chunks.is_empty() && !completion.text.is_empty()` fallback path emits zero `Token` events, so a terminal-seq of 0 is genuinely ambiguous. Changed the wire field from `final_seat_seq` to **`emitted_seat_chunk_count: u64`** (0 unambiguous); a seat is incomplete unless the client received exactly that many contiguous chunks. Added a zero-chunk successful-completion test alongside the drop tests.

---
**Resolution: CONVERGED IN SUBSTANCE — round cap reached, no open disagreement.**
Act 1 (grill, 11 questions) locked intent with the user; Act 2 ran the full MAX_ROUNDS=5 with Codex (gpt-5.6-terra). The literal Round-5 verdict is REVISE, but this is **not a deadlock**: every one of Codex's ~29 findings across 5 rounds was *accepted* (a handful with logged arbiter nuances, zero rejected outright), and the Round-5 item was a single trivial one-field fix I agreed with and folded in. Codex was narrowing to progressively smaller edges (R3: 6 findings → R4: 1 → R5: 1 micro edge). There is **no outstanding point of disagreement to hand the user as a tie-break** — the only truly open item is a product decision the grill deferred to sign-off ("which cuts, if any"). No code written during either act.

Cuts decision (user, at sign-off): *"dealer's choice, make it pretty and useful."* → all three restructures adopted, nothing removed: verdict → dismissible overlay; setup → one dense pane; awaiting-user → inline log-rail prompt.

## Act 3 — Build
Builder: Codex (gpt-5.6-terra, reasoning high) — codex-cli 0.144.0, `codex exec --yolo`, one shot, 0 fix rounds. Branch `feat/mission-control-ui`.

### Round 1 — Codex build
Implemented the full Mission Control cockpit + additive Rust telemetry. Files: `sse.rs` (Anthropic `message_start` input-usage + fixture), `krunch-engine/lib.rs` (`SeatUsage`, `RoundTelemetry`, `seat_seq`, post-guard metrics, accepted-attempt usage emission), `deliberation.rs` tests (telemetry ordering + retry dedupe), `types.ts` (wire mirror), `deliberation.ts` (totals/pricing/stream-gap/RAF/log-ring/convergence), new `telemetry.ts` + `shortcuts.ts`, `App.vue` cockpit shell, `style.css` tactical palette + CRT + reduced-motion, Departure Mono font (13.9 KB), new `CockpitStatusBar`/`CommandPalette`/`EventLogRail`, rebuilt `SeatCard`/`MediatorPanel`/`ConvergenceStrip`/`SeatEditor`/screens, `AwaitingUserModal` → inline stub. Proof reported green.
Deviations: (1) no frontend test runner exists → store tests enumerated but not added; (2) no browser surface in Codex's env → preview left to Claude.

### Claude's verdict — APPROVED
Read the full diff like a PR; verified the high-risk surfaces against real code:
- **Anthropic input usage** parsed on `message_start`, nullability preserved, and `http.rs` merges usage across deltas with `is_some()` guards so `message_delta`'s `None` can't clobber input — the real-input-tokens gap is genuinely closed.
- **Engine emission** faithful: `SeatUsage` only post-`accept_attempt` with real usage + `emitted_seat_chunk_count`; `RoundTelemetry` (post-guard `effective`) before `RoundComplete`; per-sink `seat_seq` from 0 alongside untouched shared `seq`.
- **Store** correct: usage dedup by `round:seat:attempt`; stream-gap handles the zero-chunk completion (`0===0 && -1===-1` → not incomplete); RAF token batching ~30fps with `instantTokens` bypass; convergence from backend telemetry only.
- **Pricing** exact-origin match (`api.openai.com`/`api.anthropic.com`), `null` unless every completion has both counts + known model — never fabricated; totals labeled `exact`/`partial`.
Proof re-run by Claude: `npm run build` ✓ (type-check + bundle, Departure Mono 13.9 KB), `cargo test` ✓ **99 passed / 0 failed**.
Browser verification (Codex couldn't): setup (dense config pane, preflight, shortcut hints), room (cockpit grid, cyan mediator, LATCHED seats + stances, convergence meter, event-log rail, `est. —`), `⌘K` palette, verdict overlay ("CONSENSUS LOCKED" in Departure Mono, "accepted-completion tokens" label, × dismiss), deadlock (ember-red + session-dump export). **No console errors on any phase.**
Minor polish (non-blocking, for user's call): QUORUM input shows the raw float `0.6666…`; default model `claude-sonnet-5` not in the price table so cost shows `—` (honest). 0 fix rounds needed.

