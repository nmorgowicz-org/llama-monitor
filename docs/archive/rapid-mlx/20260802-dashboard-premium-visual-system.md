# Dashboard premium visual system for Rapid-MLX telemetry

Status: **done (2026-08-03), archived.** All six cards from "Recommended card composition"
are built and live-verified where a live server was available; step 5's capture/test
corrections are complete; validation gates (ESLint, `npm run validate-js`,
`rtk cargo build --release`, `rapid-mlx-cards.spec.js` 5/5, `spawn-wizard.spec.js` 31/31)
all pass. See "Progress notes" below for the full trail, and the "What's NOT done yet"
section near the end for the one explicitly out-of-scope item (upstream rapid-mlx pp/tg
bug) and the one follow-up suggestion (real exploratory use of a live session, now that
Rapid-MLX is actually launchable day-to-day) that were still open when this was archived.

Originally split out of `20260731-phase7_8_remainder-handoff.md`
("Dashboard premium visual system: implement after Wizard/Preset contracts settle",
lines 1548-1587) because it is a standalone feature-sized effort, not a polish pass.
Wizard/Preset contracts have now settled (Spawn Wizard MLX IA redesign and
`buildRapidMlxConfig` extraction are done), so this is unblocked and next in queue
after capture-script cleanup and the tag-cloud flake.

## Why this is scoped separately

The existing `rapid-mlx-cards.js` renders six label/value cards (runtime, throughput,
queue, memory, cache, totals) plus an occasional progress/activity card, all through one
generic `card(title, rows, state)` builder. This is functional but visually flat next to
the llama.cpp GPU/system cards in `dashboard-render.js`, which already have sparklines,
ring gauges, and stale/zero/unavailable states. Bringing Rapid telemetry to the same bar
means designing and building new visual components (not just reflowing existing DOM), so
it belongs in its own plan with its own review/verification pass.

## Current state (read before starting)

- `static/js/features/rapid-mlx-cards.js` (417 lines) — card registry
  (`CARD_REGISTRY`), `parkLlamaCards()`/`restoreLlamaCards()` (swaps the llama.cpp
  `#inference`/`.inference-detail-grid` DOM out and back in), staleness tracking
  (`STALE_POLLS = 3`, per-card `cardHistory`), and a DOM-diffing `syncCard`/`syncCardBody`
  pair that patches existing card nodes in place rather than re-rendering — **this
  in-place patching is the pattern to keep**, since it's what makes stale/live chip
  transitions and progress bars animate smoothly instead of flashing.
- `static/js/features/dashboard-ws.js` — `updateInferenceMetrics()` (~line 815) is the
  single call site: `renderRapidMlxCards(rm, pollSequence, pollFailed, sessionId,
  sampledAtUnixMs)` for `backend === 'rapid_mlx'`, else `restoreLlamaCards()`.
  `rm` is `d.inference` when `d.backend === 'rapid_mlx'` (set via
  `setLastRapidMlxMetrics` at ~line 770). This call site and its five-argument shape are
  the integration contract — do not change without updating dashboard-ws.js in the same
  change.
- `static/js/features/dashboard-render.js` (1678 lines) — the premium primitives to reuse
  or extract: `buildSparklineSVG` (multiple variants around lines 104-198 and 751-805 and
  974-982; note there are at least three near-duplicate sparkline builders already —
  don't add a fourth, extract one shared variant if reasonably scoped),
  `hw-ring-viz` gauge rendering (~line 742), and the GPU/system card visualization-mode
  system (`vizPrefs`, bar/ring toggle, ~lines 1006-1023). These are llama.cpp/hardware
  card-specific today; check what's genuinely backend-neutral before extracting vs.
  copying.
- `static/css/cards-inference.css` (2745 lines) — `.widget-card` and its state modifiers
  (`is-live`, `is-idle`, `is-unavailable`, `is-dormant`, `is-blocked`, ~lines 501-638),
  `.metric-card-topline` (~684), `.rapid-telemetry-grid`/`.rapid-telemetry-card` (~2666-2743,
  the current Rapid-specific block — this is what grows), light theme and reduced-motion
  overrides (~2162-2177, 2331-2367). New Rapid card CSS should live near the existing
  ~2666-2743 block and follow the same state-modifier vocabulary as `.widget-card`.
- `tests/ui/core/rapid-mlx-cards.spec.js` (118 lines) — current test contract:
  `[data-card-id]` count and per-id content assertions, stale-chip text format
  (`stale · Ns ago · M/3`), in-place node identity checks (`card ===
  document.querySelector(...)` across re-renders), and the session-reset/backend-switch
  restore path. **Any redesign must preserve `data-card-id` values and the
  `RAPID_MLX_CARD_IDS` export** — other code/tests may key off card identity even if the
  internal DOM structure and visuals change.

## Trustworthy telemetry fields (do not invent anything beyond this list)

Confirmed available today, per the handoff doc and current `CARD_REGISTRY.available()`
predicates in `rapid-mlx-cards.js`:

- runtime: `model`, `health`, `ready`, `uptime_seconds`
- throughput: `prompt_tokens_per_second`, `generation_tokens_per_second`
- queue: `running_requests`, `waiting_requests`
- Metal memory: `active_memory_bytes`, `peak_memory_bytes`, `cache_memory_bytes`
- cache: `global_cache_hit_rate`, `global_cache_entries`,
  `cache_metrics.current_memory_bytes`, `cache_metrics.multimodal_cache_kinds`
- cumulative totals: `completed_requests_total`, `prompt_tokens_total`,
  `completion_tokens_total`, `steps_executed`
- optional: `active_requests[]` (sanitized), `backend_details.progress`
  (normalized 0-1 via `normalizedProgress()`)

**Explicitly not available: TTFT history, speculative-decoding acceptance rate, or
speedup.** Do not add UI slots for these. If a future backend change adds them, that's a
separate follow-up, not something to stub out now with fake/zero data.

Before building each card, re-verify the field is still actually populated by hitting a
live Rapid-MLX session and reading the raw WS payload (or `/api/rapid-mlx/telemetry` if
that's still the live path — confirm the current transport in `dashboard-ws.js` first,
since polling was described elsewhere in the handoff as replaced by WebSocket push).
Don't trust this list against months-old memory; the whole point of the "no invented
TTFT" rule is that this changes and needs re-checking against the live server, not
copy-pasted from a doc.

## Recommended card composition (from the handoff, refined)

1. **Hero throughput card** — prompt/generation sparklines (reuse/extract one
   `buildSparklineSVG` variant) plus current generation stage (idle/prefill/decode, if
   derivable from `running_requests`/`backend_details`). This replaces the current flat
   "Inference throughput" card.
2. **Unified-memory composition visual** — stacked bar or ring showing active/cache/peak
   against a capacity, if a trustworthy capacity value exists (check: does Rapid report
   total unified memory, or only active/peak/cache deltas? If there's no reliable
   denominator, this has to be a relative/delta visualization, not a percentage-of-total
   gauge — do not fabricate a denominator from system RAM).
3. **Queue/admission card** — running/waiting pressure, plus the active-request rail from
   `active_requests[]` when present.
4. **Prefix-cache card** — hit-rate gauge (ring, matching the GPU/system ring style),
   entries, memory. Explicit unavailable/zero/stale states — zero hit rate and "no data
   yet" must render differently (this exact distinction is called out in the handoff as a
   required test assertion).
5. **Cumulative work card** — compact totals + deltas computed client-side between polls.
   Deltas are session-local and must be labeled as such (see below).
6. **Runtime/effective-policy status strip** — model/health/uptime plus the evidence/
   staleness grade already computed elsewhere (`telemetry-grade.js`,
   `deriveTelemetryGrade`/`gradeLabel`/`gradeStatusClass` — reuse this, don't reinvent a
   second grading scheme).

Client-side sparkline history is live-session-only (lost on reload) — every sparkline
label must make this explicit (e.g. "this session" in a tooltip or subheading), so it's
never mistaken for persisted server-side history.

## Explicit non-goals

- No llama.cpp DOM/ID reuse while `parkLlamaCards()` owns `#inference` /
  `.inference-detail-grid` — build Rapid's own grid, as today.
- No invented TTFT or speculative-acceptance metrics.
- No chat UI changes (out of scope per the handoff, unchanged).
- Prefer a capability-driven metric model (card `available()` predicates, as today) so
  MTPLX can plug in a subset/superset later without a second dashboard rewrite — don't
  hard-code "Rapid-MLX" assumptions any deeper than the current registry already does.

## Suggested execution order

1. Confirm the live telemetry field list against a real running Rapid-MLX session
   (`rtk cargo build --release`, spawn a real local model, inspect the WS payload) before
   writing any card code — the "trustworthy fields" list above is a starting point, not
   gospel.
2. Extract one shared sparkline SVG builder if the three existing variants in
   `dashboard-render.js` are close enough to unify without behavior change to the
   llama.cpp/hardware cards that use them today. If they've diverged for good reason,
   leave them and write a new Rapid-specific one instead of forcing a bad abstraction.
3. Build cards one at a time against `rapid-mlx-cards.spec.js`, keeping `data-card-id`
   stable, extending (not replacing) the existing stale/live/degraded state machine.
4. New CSS additions land in the existing `.rapid-telemetry-*` block in
   `cards-inference.css`, following `.widget-card`'s state-modifier naming.
5. Capture/test corrections (already scoped in the handoff, carry over unchanged):
   - `dashboard-rapid-mlx` capture must assert visual hierarchy, zero/unavailable/stale
     states, DOM stability, accessibility, dark/light, narrow, and reduced motion.
   - `rapid-mlx-live` currently captures telemetry *before* chat, so the dashboard shot
     has zero totals — move/add a live dashboard capture after a request and before stop,
     keeping the stopped/historic frame separate.
   - `rapid-mlx-live`'s status assertion reports `model stopped: false` even when process
     inspection finds no Rapid process — fix the assertion to inspect the active
     session/process contract, not the managed-runtime installation status.
6. Validation gates before calling this done: `npm run validate-js`, ESLint on touched
   files, `rtk cargo build --release`, full `rapid-mlx-cards.spec.js` plus whatever
   capture specs get touched, and a manual visual pass (screenshot against a throwaway
   dev server, same technique used for the Wizard IA redesign) across dark/light/narrow/
   reduced-motion — Playwright pass/fail alone does not verify visual quality.

## Open questions to resolve during step 1 (not assumed here)

- Does Rapid-MLX report any usable unified-memory capacity/denominator, or only
  active/peak/cache deltas? Determines whether the memory card can be a true
  percentage-of-capacity gauge or must stay delta-only.
- Is `backend_details.progress` reliably populated during normal generation, or only
  during model load? Determines whether "current generation stage" in the hero card is
  derivable now or needs to wait on a backend addition.
- Confirm current telemetry transport (WS push vs. poll) directly in `dashboard-ws.js`
  before writing capture-timing fixes, since the handoff text says polling was replaced
  but this plan doc has not independently re-verified that against current code.

## Progress notes (2026-08-02, session continuation)

Ran step 1 (source-level, no live server was available — see caveat below) and made a
small, safe start on step 2. Status: **not done, safe to resume**; no card UI written yet.

### Step 1 findings — verified against source, not yet against a live payload

- **Transport confirmed: WebSocket push, not polling.** `dashboard-ws.js:317` opens one
  `new WebSocket(...)`; inbound messages dispatch through `updateInferenceMetrics(d)`
  (~line 452), which calls `renderRapidMlxCards(...)` for `backend === 'rapid_mlx'`. The
  handoff doc's claim that polling was replaced is correct as far as the frontend call
  site goes.
- **Field list confirmed against `src/inference/rapid_mlx/poller.rs` `StatusResponse` /
  `MetalMetrics` structs and the `InferenceMetricsSnapshot` construction (~lines 16-184).**
  All fields in this plan's "trustworthy telemetry fields" list map to real Rust struct
  fields (`num_running`→`running_requests`, `total_requests_processed`→
  `completed_requests_total`, etc.) with the exception noted below.
- **No unified-memory capacity/denominator field exists anywhere in the poller.**
  `MetalMetrics` has only `active_memory_gb`, `peak_memory_gb`, `cache_memory_gb`. Answers
  open question #1: the memory card **must be delta-only**, not a percentage-of-capacity
  gauge — there is no total-memory value to divide by, and none should be fabricated from
  system RAM.
  - Item 2 in "Recommended card composition" above should be read as decided: relative/
    delta visualization, not a gauge.
- **`ttft` and `speculative_acceptance_rate` are hardcoded `None` at the Rust
  construction site** (poller.rs:163-164), not just currently-empty-but-sometimes-present.
  This is a structural absence, not a transient gap — reinforces the "do not add UI slots
  for these" rule with more certainty than before.
- **`backend_details.progress` open question RESOLVED (2026-08-02, live-verified):**
  spawned `rapid-mlx serve mlx-community/Qwen3-0.6B-4bit` directly (bypassing the
  dashboard) on port 19321, fired a real ~400-token chat completion, and polled
  `/v1/status` every second during generation. `progress` was `null` on every poll while
  `status` read `"generating"` — confirmed live, not inferred from source. **`progress` is
  not populated during normal text generation** (it exists in the schema for a different
  case, likely model-load download/weight-loading progress, not decode progress). The
  hero card's "current stage" indicator can use `status` (`idle`/`generating`/etc.) as a
  coarse state signal, but must not build a percentage/progress-bar UI keyed off
  `backend_details.progress` for generation — there is nothing there to show today.

Also ran the full `rapid-mlx-live` Playwright capture scenario
(`RUNNING_PORT=17778 node tests/ui/capture.mjs --scenario rapid-mlx-live`) against a
newly built release binary standing at `http://127.0.0.1:17778` (config dir + logs in
this session's scratchpad, kept off port 7778 per `run-server.mjs`'s live-instance
warning). Real spawn → real chat ("Say hello and stop." → actual model response) → real
telemetry captured idle and post-chat. Screenshots landed in
`docs/screenshots/artifacts/rapid-mlx-live-dashboard-{idle,active}.png` and confirm the
plan's flat-card baseline description is accurate: four label/value cards (RAPID-MLX
RUNTIME, INFERENCE THROUGHPUT, REQUEST QUEUE, METAL RUNTIME MEMORY) plus CUMULATIVE
TOTALS, no sparklines/rings/hero card yet, and — notably — **no prefix-cache card
rendered at all** in this run (cache hit-rate was 0/no data, so `available()` presumably
suppressed it). Both step-1 open questions from this plan are now closed:
1. No unified-memory capacity/denominator — confirmed (source-level, above).
2. `backend_details.progress` not populated during generation — confirmed (live, above).

Both the source-level and live verification passes are now done. No caveat remains on
step 1 — safe to proceed to step 3 (build cards) in a future session.

### Step 2 findings — sparkline extraction

- The plan's premise that there are "three near-duplicate sparkline builders" in
  `dashboard-render.js` was **checked and is wrong** — there is exactly one
  `buildSparklineSVG(points, cssClass, color)` (was line 953), called from two sites
  (~751, ~918). It's already a single shared, backend-neutral pure function (no DOM
  writes, no llama.cpp-specific assumptions) — no de-duplication work was actually
  needed.
- Changed: exported it (`export function buildSparklineSVG`) so `rapid-mlx-cards.js` can
  import it directly. Its three private helper dependencies
  (`nextSparklineGradientId`, `getThemedSparklineFillColor`, `buildSparklineFillDefs`) are
  self-contained inside `dashboard-render.js` and don't need to move — the exported
  function is usable as-is from another module. ESLint clean on the touched file.
  `rapid-mlx-cards.js` does not yet import it — that's the next concrete step.
- `renderHwRing` (dashboard-render.js:740) is **not** a reusable pure builder like
  `buildSparklineSVG` — it's imperative and writes directly into a passed DOM container.
  Reusing the ring-gauge visual style for the prefix-cache hit-rate card (composition
  item 4) will need either an adapter or a small new ring-specific pure builder matching
  `rapid-mlx-cards.js`'s existing `syncCard`/`syncCardBody` DOM-diffing pattern — do not
  call `renderHwRing` directly from Rapid card code without adapting it first.

### Card 1 (hero throughput sparklines) — done, live-verified (2026-08-02)

Implemented in `rapid-mlx-cards.js`: client-side `throughputHistory` (prompt/generation
t/s, capped at 60 samples, reset on session change), a new `sparklineRow()` builder using
the now-exported `buildSparklineSVG`, and sync support for it in `syncCardBody`/`rowKey`
(new `rapid-sparkline-row` row type, diffed by innerHTML replacement rather than the
existing progress-row/metric-row branches). CSS added at the end of the existing
`.rapid-telemetry-*` block in `cards-inference.css` (`.rapid-sparkline-row`,
`.rapid-sparkline-chart`) — reuses `.metric-sparkline`'s existing dark/light/reduced-motion
rules rather than duplicating them.

**Deliberately did not touch** the existing `.rapid-metric` rows (Prompt/Generation t/s
labels) — `rapid-mlx-cards.spec.js` pins those by exact `span`/`strong` structure
(lines 67-78), so the sparklines are additive rows appended after them, not a
replacement. All 5 existing spec assertions still pass unchanged (verified via
`LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17779 npx playwright test
core/rapid-mlx-cards.spec.js` — 5/5 pass).

Live-verified end to end: built the release binary, stood up a throwaway instance on
port 17778, ran the `rapid-mlx-live` capture scenario with a temporarily lengthened chat
prompt (reverted after) to get a ~130s sustained generation instead of the scenario's
default "say hello" — this produced enough telemetry polls for the sparklines to
actually accumulate a real multi-point trace, confirmed in
`rapid-mlx-live-dashboard-active.png`. `Steps: 11,778` in that capture confirms real
sustained generation occurred. Throwaway server and `rapid-mlx serve` child process were
torn down afterward, and the temporary prompt edit to `tests/ui/capture.mjs` was
reverted (confirmed via `git diff` — no stray changes left in that file beyond
pre-existing unrelated edits).

Not yet exposed elsewhere: no CSS/test changes were needed beyond the above; no
regressions expected in other specs since the change is additive-only within the
`throughput` card.

**Correction (2026-08-02, later same session):** the `rapid-mlx-live-dashboard-active.png`
capture above was against `mlx-community/Qwen3-0.6B-4bit` and showed `pp/tg 0.0/0.0` with
an empty sparkline. Re-investigated with a proper model
(`nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx`, launched with the validated
chat-template override + argv — see `reference_defiant9b_validated_launch` memory) and
confirmed via direct `/v1/status` polling during real sustained generation that
`prompt_tps`/`generation_tps` are **permanently `0.0` on every rapid-mlx build**, not a
capture-timing or wiring artifact. Root cause: an upstream rapid-mlx bug —
`BatchedEngine.get_stats()` only forwards a populated `batch_generator` dict on the
MLLM/vision scheduler path; the standard text-serving path (what essentially all
non-vision models use, `--no-mllm` or not) never tracks or forwards throughput at all.
Confirmed still present on latest upstream `main` (commit `a73cf8cdd...`, 2026-08-02) and
not fixed in any release through v0.11.9. Full writeup, repro, and root-cause trace filed
at `docs/plans/20260802-rapid-mlx_metric_issue.md`, ready to submit upstream.

**Decision:** leave Card 1's code as-is — the poller (`poller.rs`) and card
(`rapid-mlx-cards.js`) are both correctly reading the documented/only-available fields;
there is no client-side bug to fix, and building a derived-tps fallback (e.g. diffing
`steps_executed`/`total_completion_tokens` over wall-clock time between polls) would be
throwaway code duplicating exactly the "don't invent metrics" concern this plan already
guards against elsewhere. Card 1 is code-complete and wiring-verified; it will render real
numbers once upstream ships a fix (or the issue gets a maintainer response suggesting
otherwise). Proceeding to cards 2-6 without further live pp/tg verification blocking
progress.

### Card 4 (prefix-cache hit-rate ring) — done, live-verified (2026-08-03)

Extracted `buildRingMarkup(pct, colorHex, isAlert)` out of `renderHwRing`
(`dashboard-render.js`) as a pure, backend-neutral markup builder — no behavior change to
existing hardware ring cards, now reusable elsewhere. Added `hitRateRingRow()` to
`rapid-mlx-cards.js`, rewired the `cache` card's `render()` to lead with a ring row
(sourced from `s.global_cache_hit_rate`, itself populated by `poller.rs` from
`/v1/status`'s `cache.hit_rate` field) with a dashed "no data yet" fallback state when
no lookups have occurred yet (`hits + misses === 0`), followed by entries/memory metric
rows as before. New `.rapid-ring-row`/`.rapid-ring-viz`/`.rapid-ring-text` CSS added to
`cards-inference.css`.

Live-verified end-to-end against the Defiant 9B server (validated launch config, chat
template override applied — see `reference_defiant9b_validated_launch` memory), attached
via `/api/attach` from a throwaway `llama-monitor` instance
(`--config-dir <scratch-dir>`, **never the user's real `~/.config/llama-monitor`** — that
holds production session/chat data): initial state showed the dashed "no data yet" ring
(0 lookups, `hits=0 misses=1` edge case where `hit_rate=0.0` but a miss has occurred —
card correctly still gated this to no-data since it reads `hits+misses>0` as the "has
data" signal, so this was actually the boundary the card should light up on and did once
a second request landed). After a few repeated chat completions against the same prompt
prefix, `/v1/status` reported `hits=2 misses=2 hit_rate=0.5`, and the dashboard ring
correctly filled to a 50% conic-gradient arc with matching "50.0%" text, `Entries: 4`,
and `Memory` tracking the live `current_memory_bytes` value — confirmed visually via
screenshot zoom, not just API polling. `core/rapid-mlx-cards.spec.js` (5 tests) still
passes with no regressions.

### Cards 2, 3, 5 — already built, this note was stale

Re-checked `CARD_REGISTRY` directly: card 2 (`memory`) already has `memoryBarRow()`, card 3
(`queue`) already has `requestRailRow()` for `active_requests[]`, and card 5 (`totals`) had
row-level delta plumbing (`metric()`'s `rawDelta` param + `updateMetricDelta`) for two of
its four rows. Extended it to all four (`Requests`, `Prompt tokens`, `Completion tokens`,
`Steps` all now flash a session-local `+N` delta on change) so the card is fully consistent.

### Card 6 (runtime status strip) — decided against literal `telemetry-grade.js` reuse

Investigated wiring `deriveTelemetryGrade`/`gradeLabel`/`gradeStatusClass` from
`telemetry-grade.js` directly into the runtime card as this plan originally suggested.
Found that `dashboard-ws.js` already renders that exact grade as a single page-level chip
(`gradeChip`, ~line 603) covering local/remote/agent connectivity — duplicating it inside
the runtime card would just repeat the same information in a second place, not add value.
The "don't reinvent a second grading scheme" instruction is better read as: don't invent a
*new* ad hoc color scheme for the card's own health state. Found (and fixed) a real bug in
that spirit instead — `card()`'s status chip only ever applied the green `.live` CSS class
regardless of state text, so a `degraded` or `not ready` runtime silently rendered as a
green "live" pill. Added `chipClassForState()` to map `degraded`/`not ready` → `.critical`
(existing, previously-unused red chip style) and `stale`/`idle` → `.idle`. Verified visually
via the new stale-state capture (`dashboard-rapid-mlx-stale.png`): a stale+degraded runtime
now renders a red "DEGRADED · STALE · LAST SAMPLE · N/3" chip instead of a green one.

### Sparkline clipping bug found and fixed (2026-08-03)

User flagged that `dashboard-rapid-mlx-{dark,light}.png` showed the throughput sparkline
pair spilling out past the bottom of its card. Root cause: `buildSparklineSVG` used
`viewBox="0 0 120 24"` (5:1 aspect) with `preserveAspectRatio="xMidYMid slice"` — "slice"
scales content to *cover* the container like `background-size:cover`, and the
`.rapid-sparkline-pair` chart boxes are far wider than 5:1 relative to their 32px height, so
the scaled content overshot vertically. Combined with `.metric-sparkline`'s intentional
`overflow:visible` (meant only to let the current-point glow halo bleed a few px), the
overscaled line spilled well past the card edge instead of a subtle glow. Fixed by changing
`preserveAspectRatio` to `"none"` (stretch to exactly fill the container, no crop/overscale)
— shared by both Rapid-MLX and llama.cpp cards, verified visually on both, no regression.

### Step 5 capture/test corrections — mostly already done, one gap closed

Re-checked `rapid-mlx-live` directly: the idle-vs-active dashboard capture split (item 2)
and the stopped-status assertion using `/api/sessions/active` instead of the stale
runtime-install endpoint (item 3) were **already fixed** in an earlier session — this plan's
progress notes just hadn't been updated. The one real gap was item 1, `dashboard-rapid-mlx`
capture assertions for zero/stale states, DOM stability, and a11y — added in this session:
- DOM node-identity check (re-render same card set, assert `data-card-id` nodes are patched
  in place, not replaced) plus `aria-labelledby`/heading-id wiring check.
- Zero/no-data state: asserts the cache-hit-rate ring shows its dashed "no data yet" state
  (not a filled 0% ring) when `hits+misses === 0`.
- Stale state: asserts a failed poll marks the card `is-stale`; new
  `dashboard-rapid-mlx-stale.png` capture (also incidentally verifies the chip-color fix
  above).
- Narrow (430px) + `prefers-reduced-motion: reduce` capture
  (`dashboard-rapid-mlx-narrow-reduced-motion.png`), matching the pattern used elsewhere
  (e.g. `models-import-lab-reduced-narrow.png`).

All of the above passes: `core/rapid-mlx-cards.spec.js` (5/5), `core/spawn-wizard.spec.js`
(31/31), `npm run validate-js`, ESLint clean, `rtk cargo build --release` clean.

### What's NOT done yet

Nothing from "Recommended card composition" — all six items are now built (card 1's
pp/tg numbers remain permanently `0.0` pending the upstream rapid-mlx fix, tracked
separately in `docs/plans/20260802-rapid-mlx_metric_issue.md`; this is a data availability
issue, not a missing card). Step 5's capture/test corrections are also complete. Remaining
open item, not previously scoped in this plan: put Rapid-MLX through its paces against a
real spawned session end-to-end now that it's actually launchable/usable day-to-day (per
user, "we hadn't really ever been in a proper place to actually launch and use rapid-mlx
before the last day or so") — i.e. a fresh live `rapid-mlx-live` capture run plus manual
exploratory use, not just the synthetic-data captures this plan has relied on so far.

### How to reproduce the live check (for future sessions)

- Build once: `rtk cargo build --release`.
- Stand up a throwaway instance off the live dev port:
  `./target/release/llama-monitor --port 17778 --config-dir <scratch-dir>` (never 7778 —
  that's the live coding-session instance per `run-server.mjs`'s warning).
- Run the real end-to-end capture: `cd tests/ui && RUNNING_PORT=17778 node capture.mjs
  --scenario rapid-mlx-live` — this drives the actual dashboard through spawn → chat →
  telemetry → stop and writes screenshots to `docs/screenshots/artifacts/`. Requires
  `rapid-mlx` on PATH and `mlx-community/Qwen3-0.6B-4bit` cached (both present on this
  machine).
- For raw-payload questions not visible in a screenshot (like `progress` behavior), it's
  faster to bypass the dashboard and hit rapid-mlx directly: `rapid-mlx serve
  mlx-community/Qwen3-0.6B-4bit --port 19321 --host 127.0.0.1`, then poll
  `curl -s http://127.0.0.1:19321/v1/status` during a real
  `POST /v1/chat/completions` call.
