# Plan Review Log: krunch — jury-room LLM deliberation engine
Act 1 (grill) complete — plan locked with the user. MAX_ROUNDS=5 (Act-2 review-round cap; distinct from krunch's in-app deliberation cap).
Reviewer model: gpt-5.6-terra (config default) — codex-cli 0.144.0. THREAD_ID=019f72be-4c6f-7993-b579-f9a576bbfdff.

## Round 1 — Codex
Material problems:

- `PLAN.md` says default `MAX_ROUNDS=8`, while `PLAN-REVIEW-LOG.md` locks it at 5. Fix: define one authoritative default and persist it in the session snapshot.
- The generic endpoint model can send a stored API key to an attacker-controlled `base_url`. Fix: bind credentials to a validated provider/host and require explicit confirmation before changing endpoint hosts.
- Raw panelist output is injected into mediator context, enabling prompt injection and forged "rulings." Fix: delimit outputs as untrusted data and instruct the mediator to treat them only as evidence.
- User problem/Q&A are also untrusted prompt material with no isolation strategy. Fix: place them in clearly delimited data blocks and keep control instructions exclusively in system/developer messages.
- No response-size, context-size, or output-token limits are specified; one seat can exhaust memory, disk, context, or spend. Fix: enforce per-seat byte/token/time budgets and deterministic truncation recorded in the transcript.
- "Mediator context grows" conflicts with bounded-context safety and will eventually overflow on long answers. Fix: retain a bounded structured ledger plus capped excerpts, not all prior summaries/raw text.
- `abandon` has no cancellation semantics, so requests may continue spending money and emit stale tokens afterward. Fix: propagate cancellation tokens to every task/HTTP stream and reject post-cancel events.
- Event tags omit `session_id` and sequence numbers, allowing stale/out-of-order events to corrupt a new or reopened view. Fix: include session UUID, round, seat, and monotonic event sequence on every event.
- "Atomic round" is not actually defined transactionally: partial streamed output, retries, and UI events can diverge from SQLite after a crash. Fix: persist round-start/status and append outputs durably, then atomically finalize the round.
- A crash can leave a session permanently marked Running despite "no mid-run resume." Fix: mark unfinished sessions `Interrupted` during startup recovery and expose them read-only.
- Timeout behavior is mentioned but neither timeout values nor retry classification/backoff are defined. Fix: specify connect/idle/total deadlines, exponential jittered retries, and a strict transient-error allowlist.
- The mediator's malformed or missing ruling has no state-transition rule. Fix: validate against a versioned schema and fail safely to a recorded mediator-error/halted state.
- Consensus is entirely subjective despite collecting structured stances, so the mediator can declare consensus with contradictory evidence. Fix: require deterministic minimum agreement/confidence checks before accepting `CONSENSUS`.
- `agree_with[]` is unvalidated and can reference nonexistent/self/duplicate seats. Fix: validate UUID references against the round roster and record invalid claims as malformed stance fields.
- Batched mode has no machine-readable "interrupt now" field, yet says the mediator decides when to interrupt. Fix: add a validated `request_user_input: bool` field distinct from `questions_for_user`.
- Autonomous-mode questions are neither answered nor converted into explicit assumptions by a defined rule. Fix: require the mediator to emit a concrete assumption for every suppressed question.
- Keychain storage alone does not prevent a compromised webview from invoking privileged Rust commands or rendering hostile output. Fix: lock Tauri capabilities to required commands, use a strict CSP, and sanitize all rendered Markdown/HTML.
- The plan stores potentially sensitive problems and model outputs as plaintext SQLite without a privacy policy, retention control, or filesystem-permission requirement. Fix: document plaintext-at-rest behavior and add encrypted storage or an explicit opt-in/retention control.
- "Reproducible" roster snapshots cannot reproduce remote model behavior without provider/model revision and full generation parameters. Fix: call these audit snapshots and store endpoint, model revision where available, and all sampling parameters.
- Live token streaming acknowledges backpressure but leaves it unresolved, making UI freezes and unbounded queues likely. Fix: use bounded per-session channels with timed token coalescing and explicit overflow handling.
- The provider abstraction hides essential incompatibilities: OpenAI-compatible servers vary in SSE framing, auth, system-message support, and finish reasons. Fix: define a normalized capability contract and test adapters against a compatibility matrix.
- The final Markdown export can contain hostile provider/user content and potentially become executable HTML if rendered later. Fix: escape exported fields by default and render transcript Markdown with sanitization.
- There is no test/fixture plan for malformed SSE, cancellation, retries, crash recovery, schema migration, or adversarial model output. Fix: add adapter contract tests and state-machine persistence tests before UI implementation.

VERDICT: REVISE

### Claude's response
Strong round — 24 of 25 findings accepted and folded into `PLAN.md`. One reframed rather than fixed as a bug:
- **MAX_ROUNDS "inconsistency" (8 vs 5):** these are two different counters. `5` in this log is the Act-2 Codex review-round cap (skill template). `8` is krunch's in-app deliberation cap. Codex conflated them — fair, the naming collided. Resolved by renaming the app setting to **`max_rounds` (deliberation)** and disambiguating in both files; default stays 8 and is persisted in the audit snapshot.

Accepted and added to the plan:
- **Security (new section):** base_url host allowlist + explicit confirmation on endpoint-host change (SSRF/key-exfil); prompt-injection isolation — panelist output and user problem/Q&A wrapped in delimited untrusted-data blocks, all control instructions kept in system messages, mediator instructed to treat panelist text as evidence only; Tauri capability lockdown + strict CSP + sanitized Markdown rendering; plaintext-at-rest documented with restrictive file perms (encryption = v2 opt-in).
- **Budgets & backpressure:** per-seat connect/idle/total deadlines, max response bytes + output-token cap, deterministic truncation recorded in transcript; bounded per-session channels with timed token coalescing + explicit overflow handling.
- **Mediator context:** replaced unbounded "growing summary" with a bounded structured ledger + capped per-round excerpts.
- **Robustness:** cancellation tokens on abandon (reject post-cancel events); events carry session UUID + round + seat + monotonic sequence; transactional round (persist round-start/status → append outputs durably → atomic finalize); startup recovery marks unfinished sessions `Interrupted`; timeout/retry spec (deadlines + exponential jittered backoff + transient allowlist).
- **Schema/validation:** versioned stance + ruling schemas; malformed/missing mediator ruling → recorded `MediatorError`/`Halted`; `agree_with[]` validated against round roster seat ids; deterministic minimum agreement/confidence guard before `CONSENSUS` is accepted (mediator still arbiter, but can't override contradictory evidence into consensus); `request_user_input: bool` added for batched mode; autonomous mode must emit a concrete recorded assumption for every suppressed question.
- **Adapters:** normalized capability contract + compatibility matrix; adapter contract tests + state-machine/persistence tests + adversarial fixtures added as a testing section, before UI work.
- **Audit snapshot:** renamed from "reproducible"; stores endpoint, model revision where available, and all sampling parameters.

## Round 2 — Codex
The Round 1 findings are substantively addressed; the `max_rounds` distinction is now clear. Remaining material gaps:

- Concurrent `start_deliberation`, `answer_questions`, or `abandon` commands can still race and launch duplicate rounds/spend. Fix: enforce one per-session state-transition lock plus idempotency/expected-state tokens on every mutating command.
- Retrying after a stream has emitted partial tokens is ambiguous: durable partial output may be mixed with the retry's output. Fix: persist each provider attempt separately and select exactly one successful attempt as the seat's round output.
- "Atomic finalize" conflicts with durable token-by-token appends, and cancellation mid-round has no explicit terminal round status. Fix: define append chunks as provisional, then atomically mark them accepted/discarded and mark cancelled rounds `abandoned`.
- Backpressure's "explicit overflow handling" does not say whether UI tokens are dropped, which can make the displayed transcript diverge from persisted output. Fix: make overflow lossless via coalesced resync chunks, or visibly mark dropped UI ranges and reload authoritative persisted text.
- The consensus guard cannot be deterministic while "compatible stance" and quorum denominator are undefined for arbitrary free-text stances. Fix: specify canonical compatibility rules, quorum denominator, confidence aggregation, and validated default thresholds.
- `max_rounds` and seat-count constraints are described but not validated, allowing zero/invalid caps, no mediator, duplicate IDs, or unsupported rosters. Fix: validate a bounded `max_rounds`, exactly one mediator, 2–6 distinct panelists, and usable credentials before creating a session.
- A mediator can request user input with an empty question list, or return incompatible fields for an interaction mode. Fix: add semantic ruling validation requiring non-empty bounded questions whenever pausing and mode-consistent field combinations.
- Final synthesis is a separate provider call with no state, budget, retry, persistence, or failure behavior. Fix: add a persisted `Finalizing` state with mediator budgets and a defined terminal fallback that exports the transcript when synthesis fails.
- `Halted`, `MediatorError`, and `Interrupted` sessions have no stated export/view outcome beyond read-only reopening. Fix: provide a terminal failure view and self-contained export with reason, last durable round, and transcript.
- "Validated provider/host" is underspecified for generic endpoints and redirects; host confirmation alone does not define safe credential routing. Fix: canonicalize key-to-origin bindings, require HTTPS except explicit loopback opt-in, and disable cross-origin redirects before attaching credentials.
- The credential model lacks a non-secret credential reference, so multiple accounts for one provider/host cannot be selected reliably. Fix: store an opaque keychain-item ID in each seat configuration and snapshot its non-secret identity.
- Per-token durable SQLite writes can cause lock contention and severe streaming latency, especially with concurrent seats. Fix: append bounded token chunks through a single writer with defined flush intervals and crash-recovery semantics.
- Error handling lacks a diagnostic record for provider failures, making adapter and production failures hard to distinguish. Fix: persist redacted error class, adapter, HTTP status, retry count, deadline hit, and provider request ID when available.

VERDICT: REVISE

### Claude's response
All 13 findings accepted and folded into `PLAN.md`, one with a nuance:
- **Consensus-guard determinism (#5):** rather than invent canonical free-text "compatibility" (inherently non-deterministic without an extra LLM/embedding call), the guard is now defined purely over **structured fields**: denominator = surviving non-abstained panelists; agreement measured via the validated `agree_with[]` **graph** forming a single cluster ≥ quorum fraction (default ⌈2/3⌉); confidence aggregation = mean survivor confidence ≥ floor (default 0.6). **Stance prose is explicitly never compared semantically.** Added the honesty limit (self-reported agreement) to Risks.

Accepted and added:
- Per-session state-transition lock + `expected_state` tokens on all mutating commands (§1) — no racing duplicate rounds/spend.
- Per-attempt isolation (§3b–c, Terminology): provisional chunks tagged `(round, seat, attempt)`, discarded on retry, exactly one accepted; retries never mix partial output.
- Provisional→accepted/discarded chunk lifecycle + atomic finalize (§3i); cancelled round → `abandoned` + terminal `Abandoned` state (§8).
- Lossless backpressure (§8): overflow never silently drops tokens — coalesced resync or reload authoritative persisted text; UI can't diverge from DB.
- Session-creation validation (§10): `max_rounds` 1..=64, exactly one mediator, 2–6 distinct panelists, resolvable credentials + reachable origin — before any spend.
- Semantic ruling validation (§3g): `request_user_input==true` ⇒ non-empty bounded questions; mode-consistent field combos.
- `Finalizing` state (§6): synthesis under mediator budgets + retry + terminal fallback exporting transcript on synthesis failure.
- Terminal failure view + export (§11) for `Halted`/`MediatorError`/`Interrupted`/`Abandoned` with reason + last durable round.
- Credential routing (§9): opaque keychain-item `credential_ref` (multi-account), canonical key-to-origin binding, HTTPS-except-loopback, no cross-origin redirects before attaching credentials, origin-change confirmation.
- Single-writer chunked persistence (§7): bounded chunks flushed on interval, no per-token writes; crash-recovery rollback of provisional rounds.
- Diagnostic `error_records` (§7): redacted error class, adapter, HTTP status, retry count, deadline-hit, provider request id.

## Round 3 — Codex
Round 2's listed findings are addressed. Remaining material issues:

- A per-session lock cannot deduplicate two simultaneous `start_deliberation` calls because neither call necessarily has an existing session yet. Fix: require a client idempotency key or atomically create a unique "starting" session before any provider task launches.
- Problem text and accumulated user Q&A remain unbounded, so they can still overflow provider context windows despite the bounded ledger. Fix: enforce input-size limits and construct every provider request under an explicit total context-token budget.
- The ledger promises a "capped running synthesis," but the ruling schema has no `summary` field and persistence names only rulings/chunks. Fix: add a versioned, capped mediator-summary field and persist it explicitly as the ledger source.
- The agreement guard leaves "single agreement cluster" undefined for a directed `agree_with[]` graph; weak connectivity lets one-sided claims create consensus. Fix: define the exact graph algorithm and require reciprocal agreement edges for all quorum members.
- Live events, the single writer, cancellation, and finalization lack a stated ordering/fencing invariant, so queued pre-cancel chunks can be persisted or displayed after `Abandoned`. Fix: attach a round/attempt generation fence to every queued chunk and emit only writer-acknowledged chunks.
- The relational model is `sessions → rounds → attempts → chunks`, but final synthesis is outside a round and has no defined place for its attempts/chunks. Fix: add a persisted finalization-attempt entity (or an explicit synthetic finalization round) with the same accept/discard lifecycle.

VERDICT: REVISE

### Claude's response
All 6 accepted — consistency/completeness fixes, all folded in:
- **start dedup (§1, §3 state machine):** `start_deliberation` carries a client `idempotency_key`; core atomically inserts a `Starting` session row under UNIQUE(idempotency_key) before spawning any task — duplicate start returns the existing session. Added `Starting` to the state list.
- **Input budgets (§8):** enforced input-size limits on problem + each Q&A at capture; every provider request assembled under an explicit total context-token budget with deterministic elision recorded — bounded ledger no longer masks unbounded raw input.
- **Ledger summary field (§3g, §7):** added capped `summary` to the versioned ruling schema, persisted in the `rulings` row as the explicit ledger source.
- **Reciprocal agreement graph (§3h):** guard now builds an undirected graph from **reciprocal `agree_with` edges only** (A→B counts iff B→A), takes the largest connected component, requires ≥ quorum coverage + mean-confidence floor. One-sided claims can't manufacture consensus.
- **Generation fence (§7):** every chunk/event carries a `(round, attempt)` fence; writer persists / UI displays only live-generation chunks — queued pre-cancel/pre-retry chunks are dropped, none land after `Abandoned` or a discarded attempt.
- **Finalization as synthetic round (§6, §7):** synthesis is a `kind=finalization` round reusing the exact `attempts → chunks` accept/discard lifecycle — no special-case entity outside the relational model.

## Round 4 — Codex
All Round 3 findings are resolved in the plan: start idempotency, bounded inputs/context, persisted ledger summaries, reciprocal agreement semantics, generation-fenced persistence/events, and synthetic finalization rounds are specified coherently.

I found no new material implementation blocker; the remaining listed risks are acknowledged tradeoffs with defined boundaries and tests.

VERDICT: APPROVED

---
**Resolution: CONVERGED.** Grilled (Act 1) + survived 4 rounds of Codex adversarial review (Act 2, cap 5). No code written during either act. Awaiting user sign-off on implementation.
