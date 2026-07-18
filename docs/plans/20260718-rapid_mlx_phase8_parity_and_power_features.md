# Rapid-MLX Phase 8: Parity, Diagnostics, and Power Features

This phase closes the remaining gaps between llama.cpp and Rapid-MLX support surfaced
after Phase 7's cross-backend release gate landed. It follows the Builder/Verifier/
Coordinator checkpoint protocol established in
`docs/plans/archived/20260710-rapid_mlx_roadmap.md`.

## Execution and Checkpoint Protocol

Unchanged from the archived roadmap:

1. **Builder** implements only the phase scope and records files changed, tests run,
   assumptions, and remaining risks.
2. **Verifier** reviews the exact diff against this plan, adds or requests missing
   regression/contract/security/platform tests, and signs off or rejects.
3. **Coordinator** runs the hard gates, removes stale artifacts, updates this plan if
   reality invalidated an assumption, and creates one Conventional Commit checkpoint
   only after sign-off.
4. No later item may be used to excuse a failed earlier gate.

Required evidence at every checkpoint: targeted tests for changed contracts,
`cargo clippy -- -D warnings`, `cargo test`, `git diff --check`, real command/HTTP
fixtures (not mocks that just mirror the implementation), a real runtime smoke test on
Apple Silicon whenever discovery/launch/readiness/lifecycle/chat routing changes, and
the screenshot harness plus focused Playwright coverage whenever UI changes.

## Verified CLI Reality (preflight, `rapid-mlx` 0.10.12, Apple Silicon M5 Max)

Captured from the installed binary on the dev machine on 2026-07-18. Builders must not
re-derive these; they are the ground truth this phase parses against.

- **No machine-readable output anywhere.** `info`, `doctor`, `models`, and `bench` emit
  human-formatted Unicode box-drawing tables only. `--json` is rejected
  (`error: unrecognized arguments: --json`). **This phase commits to text-scraping** the
  observed 0.10.x layouts as the primary path, each with a raw-text fallback and a
  version guard (`rapid-mlx version`) so a layout change degrades to "no recommendation"
  rather than a silent misparse. There is no stable contract — treat every parser as
  best-effort and never gate launch on it.
- **`rapid-mlx info <alias|repo>`** reports, per model:
  `Tool format`, `Reasoning parser`, `Architecture`, `Spec decode` (✓/✗),
  `MTP path` (enabled/disabled), `KV-share`, `Throttle`, `Suffix tier`, plus separate
  `DFlash eligibility` and `DDTree eligibility` blocks with per-criterion reasons
  (declared support, MoE, precision, drafter, runtime present). It does **not** print a
  PFlash line in its table (verified on `qwen3-0.6b-4bit` and `gemma3-1b-4bit`), so a
  PFlash disposition cannot be *scraped from `info`* — see the PFlash bullet below for how
  it is surfaced instead.
- **PFlash is real and per-alias-defaulted** (contra any reading that it's not
  model-specific). `rapid-mlx serve --pflash {off,auto,always}` defaults to `always` for
  verified aliases (Qwen3.5 / Qwen3.6 family, per upstream #287) and `off` otherwise, with
  a bench-validated `--pflash-keep-ratio 0.20` profile (PR #649: TTFT 3.87x–8.5x, needle
  recall 5/5) and an auto threshold of 32768 prompt tokens. Because `info` doesn't emit
  this, llama-monitor surfaces PFlash as an **advanced serve control** (the `--pflash`
  mode + `--pflash-threshold`/`--pflash-keep-ratio`/etc. knobs) in Item 3's structured
  allowlist, letting the user override serve's alias-aware auto-default rather than
  scraping a nonexistent `info` field. If a future `info` adds a PFlash line, promote it
  to a scraped recommendation then.
- **`rapid-mlx doctor`** is an **environment** health check only (System, Python,
  Required/Optional Packages, HF Cache, Network, Shell Integration, Optional Tools). It
  emits nothing model- or flag-specific. Severity is glyph-coded (`✓` ok / `⚠` warning /
  `✗` issue), sections are `◆`-prefixed, and it ends with `Summary: N ok, M warnings,
  K issues` — all cleanly mappable to a `DoctorFinding.severity`.
- **Spec-decode methods are `--speculative-config` JSON, not toggles:**
  `{"method":"dflash"}`, `{"method":"ddtree"}`, `{"method":"mtp",...}`,
  `{"method":"suffix",...}`. Eligibility is what `info` surfaces; the config is what
  `serve` consumes. Force overrides exist: `--force-spec-decode` / `--no-spec-decode`.
- **Vision** = `--mllm` / `--no-mllm` (a boolean escape hatch) + the `[vision]` extra
  (mlx-vlm). **Embeddings** = `--embedding-model <repo>` (a value) + the `[embeddings]`
  extra. These are the only two genuine per-model "extras" toggles/values.
- **Audio exists in the CLI** (`serve --enable-audio`, `[audio]` extra, cached TTS/ASR
  models) but is deliberately out of scope — see the audio scope decision below.

## Scope Decisions (resolved before implementation)

- **No local alias sync/cache.** `rapid-mlx info` and `rapid-mlx models` are shelled out
  to live at wizard-open and preset-edit time, for **display/recommendation enrichment
  only**. This does not change launch-time resolution, which stays free-form and
  validated by Rapid-MLX at launch (preserving the existing `model_resolver.rs` stance
  that "no unstable CLI catalog was scraped" for the launch path — see Item 1). `aliases.json`
  is Rapid-MLX's internal data file, not a contract llama-monitor should parse directly.
- **Extras in scope:** vision (`--mllm`) and embeddings (`--embedding-model`) as
  first-class per-model surfaces; spec-decode methods (MTP/DFlash/DDTree/suffix) as an
  **eligibility display sourced from `rapid-mlx info` plus a `--speculative-config` JSON
  builder**, not as boolean toggles. Audio is excluded: this phase is scoped to standard
  LLM OpenAI/Anthropic-style endpoints (text / tool-calling / MCP). Audio/TTS is handled
  by a separate project and is not surfaced here despite `rapid-mlx`'s `--enable-audio`
  support.
- **Finetunes get identical treatment to verified aliases** wherever `rapid-mlx info`
  returns a structured recommendation — no "unverified" downgrade badge for
  parser/spec-decode/PFlash recommendations sourced from the live CLI.
- **Doctor/troubleshooting extends existing diagnostics UI** (`local-server-error-details`
  / `server-error-details-panel` in `static/index.html`), not a new panel. Rapid-MLX
  checks are primary; a handful of llama.cpp checks are folded in for parity.
  `tool-call-parser`/`auto-tool-choice`/`no-thinking` flag fixes are wired as diagnostic
  fixes under this item, not exposed as standalone escape hatches.
- **Escape hatches are a structured allowlist**, not a free-text field, with full
  tooltip/education treatment. Scoped to Rapid-MLX only. `--watchdog-ppid` and
  `--listen-fd` are excluded from the UI entirely (process-lifecycle-internal, unsafe to
  expose).
- **Spawn-wizard.js refactor is deferred.** The 9,711-line monolith with 52 Rapid-MLX
  conditionals is a known problem but out of scope for this phase; items below add to it
  using its existing patterns rather than restructuring it.
- **HF/MLX model search parity is in scope**, first-class across the models modal,
  wizard, and preset editor — not a Rapid-MLX-only bolt-on.
- **`rapid-mlx bench` integrates into the existing benchmark dropdown**
  (`static/js/features/tune-panel.js`, `#benchmark-dropdown`), not a separate surface.
- **Changelog/"what's new" lives in the updater surface**
  (`static/js/features/rapid-mlx-updater.js`), sourced from the GitHub compare API,
  alongside the already-flagged updater-dedup work between `llama-updater.js` and
  `rapid-mlx-updater.js`.

---

## Item 1: Live Alias, Extras, and Recommendation Resolution

### Objective
Replace any assumption of a synced/cached alias registry with live queries to the
installed `rapid-mlx` CLI, and surface vision/MTP-DFlash/embeddings extras and
spec-decode/PFlash recommendations sourced from that live data — with finetunes treated
identically to verified aliases.

### Precise Scope
**Files to create:**
- `src/inference/rapid_mlx/info_query.rs`: thin wrapper that shells out to
  `rapid-mlx info <model>` and `rapid-mlx models`, parses their structured output into
  Rust types, and caches only in-memory per-process-lifetime (no disk cache, no sync
  job).

**Files to modify:**
- `src/inference/rapid_mlx/model_resolver.rs`: consume `info_query` results for
  **display/recommendation enrichment only**, without changing the launch-time resolution
  path. Note the deliberate existing stance at `model_resolver.rs:1259` ("Free-form alias
  will be validated by Rapid-MLX at launch; no unstable CLI catalog was scraped") — Item 1
  must preserve it: live `info` enriches the *wizard/preset UI*, but launch resolution
  stays free-form and launch-validated. Ensure finetune-derived recommendations use the
  same struct/fields as verified-alias recommendations (no separate "unverified" enum
  variant). Here "finetune" = an HF repo passed to `info` that isn't a named verified
  alias (`info` accepts `alias | HF repo` identically).
- `src/web/api/rapid_mlx_runtime.rs`: add an endpoint (e.g.
  `GET /api/rapid-mlx/models/:id/profile`) that the frontend calls at wizard-open /
  preset-edit time to fetch the live profile (tool-parser, reasoning-parser, spec-decode
  eligibility, PFlash tier, extras: vision / mtp-dflash / embeddings).
- `static/js/features/spawn-wizard.js`: call the new endpoint when a model is selected;
  render the extras surfaces contextually. Note these are **not** three uniform toggles
  (see Verified CLI Reality): **vision** is a boolean `--mllm`/`--no-mllm` toggle shown
  only when the `[vision]` extra is present and the model declares a vision tower;
  **embeddings** is a model-*value* control (`--embedding-model <repo>`), not a toggle;
  **spec-decode methods (MTP/DFlash/DDTree/suffix)** render as an *eligibility readout*
  from `info` (with the per-criterion reasons `info` already gives) plus a
  `--speculative-config` JSON builder — never as a plain on/off toggle.
- `static/js/features/presets.js`: same live-resolution call when editing an existing
  preset's model reference, so recommendations don't go stale between wizard runs.

### Implementation Steps
1. Implement `info_query::fetch_model_profile(binary, model_id)` and
   `info_query::fetch_model_list(binary)` using `tokio::process::Command`, bounded
   output size and timeout (mirror `compatibility::run_probe`'s pattern).
2. Define `ModelProfile` from the **actual** `info` fields (no invented `pflash_tier` —
   `info` doesn't emit one; PFlash is handled as an advanced serve control in Item 3):
   `ModelProfile { tool_format, reasoning_parser, architecture, spec_decode:
   SpecDecodeSupport, mtp_path, kv_share: bool, throttle: bool, suffix_tier,
   dflash_eligibility: Eligibility, ddtree_eligibility: Eligibility, extras:
   ExtraCapabilities, is_finetune: bool }`, where `Eligibility` carries the pass/fail
   plus the per-criterion reasons `info` prints (declared support, MoE, precision,
   drafter, runtime). No separate confidence/verification field distinguishing finetunes
   from verified aliases. Parse defensively: any field `info` omits maps to
   `Unknown`/`None`, never a parse failure.
3. Wire `model_resolver.rs` to call `info_query` on demand rather than reading any local
   alias file.
4. Add the profile-fetch API endpoint; return `404`/typed error if `rapid-mlx` binary is
   missing or the model isn't recognized (surfaced by the wizard as "run rapid-mlx doctor"
   guidance, feeding into Item 4).
5. Update wizard/preset-editor JS to request the profile on model selection and render
   extras toggles + spec-decode/PFlash defaults from it.

### Hard Gates
- `cargo clippy -- -D warnings`, `cargo test`.
- Contract test: `info_query` parses real `rapid-mlx info`/`rapid-mlx models` output
  captured from the installed binary on the dev machine (Apple Silicon M5 Max), not a
  hand-written fixture that only mirrors the parser.
- Verify finetune and verified-alias profiles pass through identical struct fields end to
  end — no UI branch that downgrades finetune-derived recommendations.
- Verify no disk-persisted alias cache file is created.
- Playwright: selecting a model in the wizard populates the extras surfaces (vision
  toggle if applicable, embedding-model field, spec-decode eligibility readout) matching a
  live `rapid-mlx info` call for that model.
- Real runtime smoke test: launch a Rapid-MLX preset using a resolved profile's
  recommended flags; confirm the server starts.

### Known Pitfalls
- `rapid-mlx info`/`models` emit **only human-formatted box-drawing tables** — there is
  no `--json` (verified: `--json` is rejected). The parser scrapes the observed 0.10.x
  layout; guard it with `rapid-mlx version` and fall back to "no recommendation" on any
  layout mismatch rather than failing the wizard. Do not assume a stable output contract.
- Don't block preset launch on this endpoint being reachable; degrade to manual flag
  entry if `rapid-mlx` isn't installed or the query times out.

---

## Item 2: Doctor / Troubleshooting Integration

### Objective
Extend the existing diagnostics panel with Rapid-MLX-primary health checks, folding in a
handful of llama.cpp checks for parity, and wire flag-level fixes
(`tool-call-parser`, `auto-tool-choice`, `no-thinking`) as one-click remediations inside
this panel.

### Two distinct data sources (do not conflate — verified against the real CLI)
`rapid-mlx doctor` is an **environment** health check only (System / Python / Packages /
HF Cache / Network / Shell Integration). It emits **nothing** about `tool-call-parser`,
`auto-tool-choice`, or `no-thinking`. Those flag remediations therefore cannot come from
`doctor` — they come from **diffing the model's `rapid-mlx info` profile (tool format,
reasoning parser) against the active preset's launch args**. So this item has two
finding producers, rendered into the same panel:
- **Environment findings** ← `rapid-mlx doctor` (map its `✓`/`⚠`/`✗` glyphs and
  `Summary: N ok, M warnings, K issues` line to `DoctorFinding.severity`).
- **Preset flag advisor findings** ← `info`-vs-preset diff; these are the ones that carry
  a `FixAction` (the `tool-call-parser`/`auto-tool-choice`/`no-thinking` fixes).

### Verified Current State (read this before changing anything)
- There are two distinct existing diagnostics surfaces in `static/index.html`, not one:
  - `#local-server-error-details-btn` / `#local-server-error-details-body` /
    `#local-server-error-details-close` (`static/index.html:461-468`) — shown on the
    welcome/Local Server panel per `dashboard-ws.js:742`.
  - `#server-error-details-panel` / `#server-error-details-body` /
    `#server-error-details-close` (`static/index.html:579-584`) — the main control-bar
    diagnostics panel, driven by `#server-error-details-wrapper` /
    `#btn-server-error-details` in `static/js/features/dashboard-ws.js:1544-1623`
    (`dashboard-ws.js:739` shows it "in the main control bar if present").
  Extend both, matching each panel's existing scope (local-server panel = local spawn
  failures; main panel = active-session diagnostics) rather than merging them into one.
- `inference_diagnostics_available()` in `src/web/api/sessions.rs:857` already does a
  lightweight backend-aware reachability check (`/health` for `LlamaCpp`, `/v1/status`
  for `RapidMlx`) — this is a boolean liveness probe, not a diagnostics/findings source.
  It is the right place to see how backend-specific paths are already branched in Rust,
  but it is not what Item 2's doctor findings extend; the new doctor endpoint is
  additive, not a modification of this function.

### Precise Scope
**Files to modify:**
- `static/index.html`: extend both `#local-server-error-details-body` and
  `#server-error-details-body` with a Rapid-MLX doctor-findings section, matching each
  panel's existing scope.
- `src/web/api/rapid_mlx_runtime.rs`: add `GET /api/rapid-mlx/doctor` that shells out to
  `rapid-mlx doctor` and scrapes its glyph-coded, `◆`-sectioned text output (no `--json`
  exists) into structured `DoctorFinding`s, with the raw output retained as a
  fallback/detail view. This endpoint carries only **environment** findings.
- A second producer (no new endpoint needed — reuse the Item 1 profile fetch): the
  **preset flag advisor** compares the selected model's `rapid-mlx info` profile against
  the active preset's launch args and emits the `FixAction`-bearing findings.
- llama.cpp-side parity checks: locate the existing health-check logic backing
  `inference_diagnostics_available()` (`src/web/api/sessions.rs:857`) and the
  `/health`-poll path used to populate `local-server-error-details`/
  `server-error-details` today (trace it from `dashboard-ws.js:1544-1623` back through
  whatever endpoint currently populates those panels — do not assume a file path without
  tracing the actual call chain first) and add the small set of llama.cpp parity checks
  (server binary reachable, GGUF path valid, port free) to the same response shape as the
  new Rapid-MLX doctor findings.
- `static/js/features/dashboard-ws.js`: render doctor findings in both panels; for
  findings tagged with a known fixable flag (`tool-call-parser`, `auto-tool-choice`,
  `no-thinking`), show an inline "Apply fix" action that patches the current preset's
  launch args.

### Implementation Steps
1. Design a `DoctorFinding { severity, message, fix: Option<FixAction> }` shape shared
   between backends; `FixAction` enumerates the small set of known-safe flag patches.
   Only the preset-flag-advisor findings ever set `fix`; environment/doctor findings
   never do.
2. Implement the `/api/rapid-mlx/doctor` endpoint, bounded timeout, scraping-parse of
   `rapid-mlx doctor`'s text (map `✓`/`⚠`/`✗` → severity; retain `Summary: N ok, M
   warnings, K issues` as a rollup) with raw-text fallback for anything unparsed. Guard
   with `rapid-mlx version` like Item 1.
3. Add the llama.cpp-side equivalent checks (reuse whatever health-check logic already
   backs `local-server-error-details`, don't reimplement).
4. Extend the diagnostics panel UI to render both backends' findings, with "Apply fix"
   wired to the preset's stored launch args (write-through, not just in-memory).
5. Confirm `tool-call-parser`/`auto-tool-choice`/`no-thinking` are removed from any
   escape-hatch consideration — they live only as diagnostic fixes here.

### Hard Gates
- `cargo clippy -- -D warnings`, `cargo test`.
- Contract test against real `rapid-mlx doctor` output on Apple Silicon.
- Verify "Apply fix" persists to the preset and is reflected on next launch (real
  spawn smoke test).
- Playwright: diagnostics panel renders Rapid-MLX findings and at least one llama.cpp
  parity finding without layout regression.
- Verify the panel degrades gracefully (llama.cpp-only findings) when `rapid-mlx` binary
  is absent.

### Known Pitfalls
- Don't let doctor findings block server start — this is a diagnostics surface, not a
  gate.
- Keep `FixAction` a closed enum; do not let it become a general arbitrary-flag-patch
  mechanism (that's Item 3's job, deliberately separated).

---

## Item 3: Structured Escape-Hatch Flag Allowlist

### Objective
Let advanced users pass a curated set of additional Rapid-MLX CLI flags through the UI
without free-text injection risk or exposing process-lifecycle-internal flags.

### Precise Scope
**Files to create:**
- `src/inference/rapid_mlx/escape_hatch.rs`: `ALLOWED_ESCAPE_FLAGS` static list, each
  entry carrying flag name, value type (bool/int/string/enum), description, and
  docs-tooltip text. Explicitly excludes `--watchdog-ppid` and `--listen-fd` (both verified
  to exist in `rapid-mlx serve --help`; both process-lifecycle-internal).
  Candidate allowlist entries (from the verified `serve --help` surface): the **PFlash
  family** — `--pflash {off,auto,always}` (enum) plus `--pflash-threshold` (int, default
  32768), `--pflash-keep-ratio` (float, default 0.20), `--pflash-min-keep-tokens`,
  `--pflash-sink-tokens`, `--pflash-tail-tokens`, `--pflash-block-size`,
  `--pflash-query-window`, `--pflash-stride-blocks`, `--pflash-include-tools` (bool) — this
  is where PFlash lives (see Verified CLI Reality; it is a real, per-alias-defaulted
  feature the user overrides here); spec-decode force overrides `--force-spec-decode` /
  `--no-spec-decode`; hybrid `--force-hybrid` / `--no-hybrid`; cache-tuning knobs. Curate
  deliberately rather than exposing the entire `serve` flag set.

**Files to modify:**
- `src/inference/rapid_mlx/command.rs`: apply escape-hatch flags from preset config
  through the allowlist validator before appending to the launch command; reject/strip
  anything not on the list rather than passing through raw strings.
- `static/index.html` + `static/js/features/spawn-wizard.js`: render the allowlisted
  flags as individual structured controls (toggle/number/select per their type) in a
  Rapid-MLX-only "Advanced" section, each with a tooltip explaining what it does and why
  it's gated. No free-text CLI-args box for Rapid-MLX.

### Implementation Steps
1. Define the allowlist as data, not string parsing — each entry is a typed control
   descriptor, so the frontend can render it generically without per-flag JS branches.
2. Validate on the Rust side too (never trust the frontend to have enforced the
   allowlist) — reject any preset-stored flag not in `ALLOWED_ESCAPE_FLAGS` at launch
   time with a clear error.
3. Render the section only when the active/selected backend is Rapid-MLX.
4. Write tooltip copy for each flag (what it does, when to use it, risk if misused).

### Hard Gates
- `cargo clippy -- -D warnings`, `cargo test`.
- Unit test: a preset with a flag not on the allowlist fails validation at launch, not
  silently dropped or silently passed through.
- Unit test: `--watchdog-ppid` and `--listen-fd` are provably absent from
  `ALLOWED_ESCAPE_FLAGS` (test asserts the list, not just current behavior).
- Playwright: advanced section only renders for Rapid-MLX backend selection; llama.cpp
  selection shows none of it.
- Real spawn smoke test with at least one allowlisted flag applied end to end.

### Known Pitfalls
- Resist the urge to add a "custom flag" fallback field "just in case" — that reopens
  the injection surface this item exists to close.

---

## Item 4: HF/MLX Model Search Parity

### Objective
Give the preset editor the same model search the models modal and spawn wizard already
have, and add an explicit MLX-format filter, since no such filter currently exists
anywhere in the app.

### Verified Current State (read this before changing anything)
- `hfSearch()` in `static/js/features/hf-browse.js:76` is already the single shared
  search function, imported and called by both `static/js/features/models.js`
  (`models.js:11`, called at `models.js:1841/1862/1880/1913/1934` for different result
  panes) and `static/js/features/spawn-wizard.js` (imported at `spawn-wizard.js:32`,
  wrapped by `hfSearchForWizard()` at `spawn-wizard.js:1985`). There is **no divergence
  between the modal and the wizard** — both already call the same function with the same
  request shape (`query`, `author`, `sort`, `limit`, `minParamB`, `cursor`).
- `static/js/features/presets.js` has **zero** model-search integration (`grep -c
  'hfSearch\|search' presets.js` returns 0 matches). This is the actual gap: editing an
  existing preset's model reference has no search UI at all today, not an inconsistent
  one.
- **The real blocker (verified in source): the shared search is hardwired to GGUF.**
  `hf_search_models()` at `src/hf/mod.rs:709` unconditionally appends `filter=gguf`
  (`src/hf/mod.rs:733`, comment "Always filter for GGUF"). GGUF and MLX are mutually
  exclusive repo formats, so the current search **structurally cannot return MLX repos** —
  this is not "add an optional filter," it's "make the currently-hardcoded format filter a
  parameter." The URL is built in `src/hf/mod.rs`, **not** `src/web/api/hf.rs` (the warp
  route there only packs `HfSearchParams { query, author, sort, limit, cursor }` and calls
  `crate::hf::hf_search_models`).
- **Server-side MLX filtering works via the `filter` (tag) param, not `library`**
  (verified live against the HF API on 2026-07-18): `?filter=mlx&sort=downloads` returns
  genuinely MLX-tagged repos; `?library=mlx` returns unrelated
  transformers/sentence-transformers repos (the `library` param is effectively ignored).
  So the fix is to make the hardcoded `filter=gguf` a caller-selected format
  (`gguf` | `mlx`), threaded through `HfSearchParams` → the route → `hfSearch()` → UI.
- Today, finding an MLX-format repo is impossible through the app's search (GGUF-locked);
  a user can only reach MLX repos by other means. MLX-specific *resolution* (once a repo
  is picked) goes through `/api/models/rapid-mlx/resolve/preview`
  (`src/web/api/rapid_mlx_runtime.rs:144`), a single-repo preview/validate endpoint, not a
  search endpoint.

### Precise Scope
**Files to modify:**
- `static/js/features/presets.js`: wire the existing `hfSearch()` (import from
  `hf-browse.js`, same pattern as `spawn-wizard.js:1985`'s wrapper) into the preset
  editor's model-reference field, reusing the same result-card rendering `hf-browse.js`
  already provides rather than building new markup.
- `src/hf/mod.rs`: change the hardcoded `filter=gguf` (line 733) into a caller-selected
  format on `HfSearchParams` (e.g. `format: SearchFormat` = `Gguf` | `Mlx`, defaulting to
  `Gguf` to preserve every existing caller), emitting `filter=gguf` or `filter=mlx`
  accordingly. This is the actual query-building layer.
- `src/web/api/hf.rs`: thread the new `format` field from the request body into
  `HfSearchParams` (the route currently constructs it with only query/author/sort/limit/
  cursor at `hf.rs:141`).
- `static/js/features/hf-browse.js`: add a `format`/`mlxOnly` option to `hfSearch()`'s
  params that sets it; add a visible toggle/chip in the shared search UI so it's usable
  from all three entry points once presets.js is wired up.

### Implementation Steps
1. (Already confirmed — no need to re-investigate.) HF supports server-side filtering via
   the `filter` tag param: `filter=mlx` works, `filter=gguf` is what the code already
   uses, `library=mlx` does not. Client-side filtering is not needed.
2. Convert the hardcoded `filter=gguf` in `src/hf/mod.rs` to a caller-selected format,
   threaded end to end (query builder → `HfSearchParams` → route → `hfSearch()` params →
   UI toggle), defaulting to `Gguf` so every existing caller (models.js, spawn-wizard.js)
   is byte-for-byte unchanged until it opts into MLX.
3. Add search to `presets.js` using the existing `hfSearch()` function and existing
   result-rendering conventions from `hf-browse.js` — no new search implementation.

### Hard Gates
- `cargo clippy -- -D warnings`, `cargo test`.
- Playwright: preset editor can search and select a model where it previously had no
  search UI at all.
- Playwright: MLX-format filter, once toggled, visibly narrows results in at least one
  of the three entry points (verify against a real HF API response, not a mock).
- No regression in existing `models.js`/`spawn-wizard.js` search behavior — this item
  adds a filter param and a new call site, it does not rewrite `hfSearch()`'s existing
  contract.

### Known Pitfalls
- Don't reintroduce a second search implementation for presets.js "to save time" — the
  whole point of this item is that a shared function already exists.

---

## Item 5: `rapid-mlx bench` Integration

### Objective
Extend the existing benchmark dropdown (`static/js/features/tune-panel.js`,
`#benchmark-dropdown` in `static/index.html`) to run `rapid-mlx bench` when the active
backend is Rapid-MLX, alongside the existing llama-bench-driven flows (quick benchmark,
batch sweep, depth sweep, MoE auto-tune).

### Verified Current State (read this before changing anything)
- The existing benchmark flow is **HTTP-driven against an already-running server** —
  `benchmark.rs` uses `reqwest` clients (e.g. lines 108/713/783) and has **no
  `Command::new`**; it does not spawn a `llama-bench` subprocess. Results are normalized
  to `prompt_tokens_per_second` / `gen_tokens_per_second` (`benchmark.rs:247-248`).
- Sweep endpoints already exist: `api/bench/sweep` (`benchmark.rs:548`),
  `api/bench/batch-sweep` (`benchmark.rs:622`), `api/bench/mtp-sweep` (`benchmark.rs:808`).
- `rapid-mlx bench` is a **CLI subprocess** that itself drives a running server via
  `--base-url`, with `--tier {smoke,speed,harness,all}`, `--num-prompts`, `--max-tokens`,
  `--submit`, `--sampled`. It has **no `--json`** (same scraping constraint as Items 1/2)
  and **no depth-sweep concept** (only single `--completion-batch-size` /
  `--prefill-batch-size` values, not sweeps).

### Precise Scope
**Files to modify:**
- `src/web/api/benchmark.rs`: add a Rapid-MLX branch. Because the existing flow already
  targets a running server over HTTP, prefer either (a) shelling out to
  `rapid-mlx bench --base-url http://127.0.0.1:<port>` and scraping its text output
  (bounded, version-guarded, raw fallback — no JSON exists), or (b) replicating bench's
  request pattern over HTTP in Rust if scraping proves too brittle. Normalize into the
  existing `prompt_tokens_per_second`/`gen_tokens_per_second` response shape.
- `static/js/features/tune-panel.js`: dispatch to the Rapid-MLX path when appropriate;
  reuse existing result-rendering code rather than forking a parallel UI. Map
  `rapid-mlx bench --tier` onto the quick-benchmark control; MTP already has an
  `mtp-sweep` endpoint to align with.

### Implementation Steps
1. (`bench --help` already inspected — see Verified Current State.) Map `--tier` and
   `--num-prompts`/`--max-tokens` onto the existing response contract.
2. Add the backend branch in `benchmark.rs`, normalizing timing/throughput fields to
   match what `tune-panel.js` already renders (`prompt_tokens_per_second` /
   `gen_tokens_per_second`).
3. Batch-sweep can map to `rapid-mlx bench`'s `--completion-batch-size` /
   `--prefill-batch-size` iterated host-side; **depth-sweep has no `rapid-mlx bench`
   equivalent** — scope it out explicitly (disable with a clear reason for the Rapid-MLX
   backend), do not fake it.

### Hard Gates
- `cargo clippy -- -D warnings`, `cargo test`.
- Real benchmark run against a loaded Rapid-MLX server on Apple Silicon, comparing
  reported numbers against a manual `rapid-mlx bench` run for sanity.
- Playwright: benchmark dropdown runs and renders results with Rapid-MLX backend active,
  with no llama.cpp-specific labels leaking into the output (e.g. "llama-bench").

### Known Pitfalls
- Don't silently no-op sweep buttons for Rapid-MLX — either wire them or hide/disable
  them with a clear reason, per the app's existing degrade pattern for backend-specific
  capabilities.

---

## Item 6: GitHub Compare-API Changelog in the Updater Surface

### Objective
Surface a real "what's new" changelog when a Rapid-MLX update is available, since
`rapid-mlx`'s GitHub release bodies are sparse/inconsistent and there is currently no
per-version changelog data anywhere in the app. Source it from GitHub's compare API
between the active installed version's tag and the target release's tag.

### Verified Current State (read this before changing anything)
- The upstream repo is **`raullenchai/Rapid-MLX`** (`https://github.com/raullenchai/Rapid-MLX`),
  confirmed from the two existing constants in `src/web/api/rapid_mlx_runtime.rs:23-26`:
  ```rust
  const RELEASES_URL: &str =
      "https://api.github.com/repos/raullenchai/Rapid-MLX/releases?per_page=30";
  const RELEASE_BY_TAG_URL: &str = "https://api.github.com/repos/raullenchai/Rapid-MLX/releases/tags";
  ```
  The compare-API URL for this feature is therefore:
  `https://api.github.com/repos/raullenchai/Rapid-MLX/compare/{base}...{head}`.
- Tag format is confirmed `v{version}` (e.g. `v0.10.10`), not an assumption — see
  `decode_github_releases()` in `rapid_mlx_runtime.rs` (`item.tag_name.strip_prefix('v')`)
  and the real fixture in the same file's test module:
  `{"tag_name":"v0.10.10","draft":false,"prerelease":false}`. `base`/`head` for the
  compare call are `v{version}` strings built the same way `select_published_release`
  already builds `format!("{RELEASE_BY_TAG_URL}/v{version}")`.
- `RuntimeApiState` (`rapid_mlx_runtime.rs`) already holds a `reqwest::Client` and a
  release cache (`ReleaseCache`, `RELEASE_CACHE_TTL = Duration::from_secs(300)`) — reuse
  this client and this 300s-TTL caching pattern for compare responses rather than
  inventing a new HTTP client or cache shape.
- `PublishedRelease { version, tag, channel, published_at }` (decoded in
  `decode_github_releases`) is what `fetchReleases()` in `rapid-mlx-updater.js:188`
  already returns to the frontend — the new "What's new" UI hooks off this existing list,
  it does not need its own release-listing call.
- No `GITHUB_TOKEN`/auth header is set on any existing GitHub call in this file — GitHub
  API calls from this app are unauthenticated today (60 req/hr/IP limit). Confirmed by
  absence of any `Authorization`/token constant near `RELEASES_URL`.

### Precise Scope
**Files to create:**
- `src/inference/rapid_mlx/changelog.rs`: fetch and summarize the compare-API commit
  list between two `v{version}` tags, bounded output size, cached in-memory per
  version-pair for the process lifetime (mirror `RELEASE_CACHE_TTL`'s 300s pattern rather
  than caching forever).

**Files to modify:**
- `src/web/api/rapid_mlx_runtime.rs`: add `GET /api/rapid-mlx/runtime/changelog?from=&to=`
  (plain version strings, not tags — build the `v` prefix server-side, matching
  `select_published_release`'s existing convention) returning a structured commit
  summary (message, author, sha, URL) for the modal to render. Reuse `RuntimeApiState`'s
  existing `client: reqwest::Client` field rather than constructing a new one.
- `static/js/features/rapid-mlx-updater.js`: in the update/release-selection flow
  (`fetchReleases()` at line 188, modal rendering around `openRapidMlxModal()` at line
  234), add a "What's new" expandable section per release entry that calls the new
  endpoint and renders the commit list.
- `static/js/features/updater-shared.js`: this is also where the already-flagged
  updater-dedup work between `llama-updater.js` (446 lines) and `rapid-mlx-updater.js`
  (517 lines) should land, done as part of this item rather than deferred again —
  `updater-shared.js` currently only holds focus-trap helpers (`attachModalFocusTrap`/
  `detachModalFocusTrap`, `updater-shared.js:1-64`); extend it with shared
  fetch-release/render-list logic, keeping backend-specific bits (install/upgrade
  endpoints, confirm tokens) in their respective files.

### Implementation Steps
1. Implement `changelog::fetch_compare(client: &reqwest::Client, base_version: &str,
   head_version: &str)` hitting
   `https://api.github.com/repos/raullenchai/Rapid-MLX/compare/v{base}...v{head}`, with
   GitHub API rate-limit handling (respect `X-RateLimit-Remaining` response header,
   surface a clear "changelog unavailable, rate-limited" state rather than erroring the
   whole update flow — the app has no GitHub token, so treat rate-limiting as an expected
   steady-state condition, not an edge case).
2. Map the compare-API commit list to a compact structured summary — do not dump raw
   commit messages 1:1 into the UI; check the real commit history at
   `github.com/raullenchai/Rapid-MLX` for merge-commit noise before deciding on filtering
   rules (don't assume noise exists without checking).
3. Add the endpoint, wire it to accept plain versions and build `v`-prefixed tags
   internally, consistent with the rest of `rapid_mlx_runtime.rs`.
4. Add the frontend "What's new" section to the existing release list rendering; lazy-load
   per release row (don't fetch compare data for every listed release up front).
5. Fold in the updater-dedup refactor as described above.

### Hard Gates
- `cargo clippy -- -D warnings`, `cargo test`.
- Contract test against a real GitHub compare-API response for two actual
  `raullenchai/Rapid-MLX` tags (fixture captured from a real call, not hand-authored) —
  follow the existing test pattern at `rapid_mlx_runtime.rs`'s
  `release_discovery_filters_drafts_and_versions_below_floor` test, which already
  captures real-shaped JSON fixtures inline.
- Verify rate-limit and network-failure paths degrade to "changelog unavailable" without
  blocking the install/upgrade action itself.
- Playwright: opening the Rapid-MLX updater modal and expanding "What's new" on a release
  shows commit entries; llama.cpp updater modal is unaffected in behavior (only shared
  code moved, not semantics).
- Diff review: confirm the dedup refactor didn't change either updater's existing
  install/upgrade/focus-trap behavior — this is a refactor-plus-feature commit, so
  behavioral parity on the refactored half is a hard gate in itself.

### Known Pitfalls
- GitHub API is unauthenticated from this app (verified — no token anywhere near the
  existing `RELEASES_URL`/`RELEASE_BY_TAG_URL` calls) — 60 req/hr/IP is the real ceiling;
  cache aggressively (reuse the 300s `RELEASE_CACHE_TTL` pattern) and never make the
  update-check flow depend on changelog fetch succeeding.
- Compare-API responses can be large for big version jumps (e.g. skipping several
  releases) — enforce a response byte bound the same way `fetch_bounded_release_body`
  already does (`MAX_RELEASE_RESPONSE_BYTES = 512 * 1024` in `rapid_mlx_runtime.rs`) and
  the GitHub API's own commit-count truncation (`compare` responses cap at 250 commits
  server-side) — pick whichever bound bites first as the visible truncation point.

---

## Explicitly Out of Scope for Phase 8

- Spawn-wizard.js monolith refactor (deferred to a later PR).
- Any local disk-persisted alias/model registry sync job.
- Audio extras. Rapid-MLX **does** support audio (`serve --enable-audio`, `[audio]` extra,
  cached TTS/ASR models), but Phase 8 is scoped to standard LLM OpenAI/Anthropic-style
  endpoints (text / tool-calling / MCP). Audio/TTS is handled by a separate project.
- Free-text custom CLI args for Rapid-MLX (superseded by Item 3's structured allowlist).
