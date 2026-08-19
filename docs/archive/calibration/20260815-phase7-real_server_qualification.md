# Phase 7 — Pre-spawn real-server qualification

**Status:** Code implementation complete for the bounded 2.0 slice. The macOS representative dense-Qwen managed-server receipt is recorded in [`docs/plans/evidence/20260813-llama-optimize/phase-07/README.md`](evidence/20260813-llama-optimize/phase-07/README.md).
Native Windows managed-binary qualification remains the release gate and
must use native binaries and models rather than fixtures.

**Validation (2026-08-15):** `cargo clippy -- -D warnings`, the complete Rust
suite with `cargo test -- --test-threads=1`, JavaScript validation/lint, release
build, formatting, and `git diff --check` pass. The calibration and
Spawn-Wizard calibration capture scenarios were rerun against the release
build; the focused calibration Playwright suite passed all six tests. The
calibration capture uses an explicit local GGUF opt-in and does not run a
benchmark as part of screenshot capture.

**Qwen template fixture:** Qwen models use froggeric’s pinned
`qwen3.8-froggeric-v22` template at revision
`9f14778c92c3b5ed3e0738085694c0d3452802dd`, consumed as published without the
historical v21.3 transform. The maintainer has confirmed tool-calling success
with this v22 template on Qwen3.6-27B, Qwen3.6-35B-A3B, and Qwen3.8-27B.

## Qualification run status (2026-08-16)

The first disposable balanced run (`4d3d63a471a667f923b48a23`) was cancelled after 6/17 trials because every `llama-bench` child reached the old 1,324-second timeout before emitting JSON; the resulting EOF parse errors were not benchmark measurements. The timeout budget is now depth/decode scaled to 2,648 seconds for the 131K/8K workload, with a focused regression test. A corrected retry (`b3ba88d6cdccb8523d75ba16`) completed with 17/17 successful llama-bench trials and selected `balanced-l9-r04` (median generation approximately 22.5 tok/s). This receipt is benchmark evidence only; the short managed-server MTP/no-spec qualification remains to be rerun with the new `-lv 4` log capture.

## Post-run analysis gate

Do not shrink or redesign the qualification matrix until the first complete run has been reviewed. Preserve the raw manifest, journal, per-trial receipts, selected-candidate analysis, and managed MTP/no-spec receipts. The review must compare success/failure and timeout/OOM rates, repetition variance, prompt/decode throughput, memory telemetry, fixture-scoped speculative acceptance telemetry from `-lv 4`, and the selected candidate against the existing Phase 4 comparator. Acceptance is observational for the exact prompt/template/runtime fixture; it is not a universal model or workload guarantee. Identify factors that materially change behavior and document any redundant rows before proposing a smaller future matrix; a shorter run must not be adopted solely because this run is slow. A receipt without `-lv 4` acceptance evidence cannot claim an acceptance result, but may still report independent latency or functional-smoke results.

## Purpose

Quick and Balanced calibration measure `llama-bench` performance.
Phase 7 adds server-realistic evidence before a saved preset launches:
TTFT, prompt/decode behavior, memory, tool correctness, and speculative
decoding behavior.

The normal workflow is pre-spawn. The user saves a Spawn Wizard or Preset
Editor preset, optionally adjusts `batch`/`ubatch`, KV, context, GPU layers,
and chat-template/tool settings, then qualifies it before ordinary launch.
It does not require a second model instance or user session.

## Current implementation status

- [x] Typed `load_mode` policy is implemented in the preset schema, legacy `no_mmap` migration, llama.cpp launch builder, batch importer, Spawn Wizard Pro surface, Preset Editor, and focused launch coverage. New and migrated presets carry an explicit mode; legacy `--no-mmap` remains a compatibility fallback only.
- [x] Advanced controls are registered with the Pro Spawn Wizard information architecture: batching/SWA/load mode, diagnostics verbosity, and prompt-cache checkpoints/reuse. Review and payload receipts include the selected values.
- [x] `repeat_last_n`, `no_cont_batching`, `swa_full`, `ctx_checkpoints`, `checkpoint_min_step`, `cache_reuse`, and `-lv` are typed across the llama.cpp preset/launch path and both UI surfaces.
- [x] Run repeated managed-server qualification against the completed Balanced sweep. Used `-lv 4`, retained raw stderr, replayed reasoning, multi-turn, tool-shaped, synthesis, and realistic 32k-cap workloads, and compared an explicit no-spec control without rerunning the 17-cell llama-bench sweep.
- [x] Parse and classify fixture-scoped speculative acceptance telemetry from the captured log tail. Acceptance is reported as observational per request/method only; it is never presented as a universal model score.

## Release boundary
The bounded pre-spawn qualification slice is part of 2.0.0. Broader overnight
search, arbitrary custom factors, and automatic multi-GPU placement remain later.
Rapid-MLX calibration remains a separate backend phase.

The default lane is single-user `--parallel 1`. Concurrency is explicit opt-in,
especially for unified-memory systems such as Apple Silicon and DGX Spark when
preflight shows enough capacity. Current MTP qualification remains
`--parallel 1`.

## Execution model

1. Preflight resolves the saved preset, exact runtime, model fingerprint,
   selected tracks, capability evidence, and expected VRAM.
2. The supervisor starts a calibration-owned llama-server before normal user
   spawn, bound to loopback on an isolated port with logs and timeouts.
3. Candidates that change load-time settings receive fresh server processes.
   A server is reused only when load-time configuration is identical.
4. Runs are serialized behind the calibration/GPU lease, journaled,
   cancellable, and receipt-backed.
5. An already-active server defers qualification by default. Stopping and
   restoring it is an explicit disruptive fallback, not the normal path.

## Implementation progress

- [x] Typed track/request/receipt contract with `--parallel 1` as the default.
- [x] Explicit validation for concurrency opt-in and MTP single-user safety.
- [x] Calibration-owned loopback process with health timeout and cleanup.
- [x] Latency/response and deterministic tool-correctness probes.
- [x] Process RSS before/peak/after telemetry is captured in the receipt.
- [x] Exact llama-server help plus GGUF metadata gate MTP and n-gram tracks;
  DFLASH-specific qualification remains unavailable without a verified
  llama.cpp capability signal; this does not reject DFLASH-capable presets.
- [x] Optional request persisted through calibration manifests and receipts.
- [x] Existing Preset Editor and Spawn Wizard request the default tracks and
  render qualification results when a receipt is available.
- [x] Preset Editor track controls and pre-spawn receipt review; Spawn Wizard
  uses the safe default bundle.
- [x] Capability-backed MTP and n-gram execution with an explicit no-spec
  baseline; DFLASH remains fail-closed because llama.cpp exposes no verified
  DFLASH capability signal.
- [x] Process memory telemetry; GPU-specific telemetry remains hardware-gated.
- [ ] Representative hardware receipts and Windows qualification.

## Selectable workload tracks

### Automated qualification bundle

The default bundle runs `--parallel 1` and measures generation throughput,
TTFT, prompt/decode throughput, wall time, memory/VRAM, correctness markers,
and bounded tool behavior. It produces no-spec baseline and candidate receipts.

### Latency and memory

Measures TTFT, prompt processing, generation speed, startup/load time, peak
and steady memory, and context behavior from one single-user server run.

### Tool correctness

Independently selectable deterministic constrained-tool fixtures test
chat-template, grammar/schema, tool parser, and structured output. Users can
rerun only this track after changing those settings. Correctness is an
independent pass/fail gate; a faster incorrect candidate never qualifies.

### MTP

This is a bounded compatibility/smoke track, not a speculative-decoding
optimizer. Run only when GGUF introspection and managed runtime help prove exact
MTP/NextN capability. Preserve the preset's existing `n_max`, `p_min`, draft
device, and related values; do not sweep them. Use `--parallel 1`, issue a
representative request, record server response, runtime capability evidence,
and latency/throughput, then compare with an explicit no-spec baseline. Do not
claim an acceptance rate unless the managed runtime exposes one explicitly.
Unsupported or inert flags are excluded. Workload-specific MTP performance
remains user-driven and outside the 2.0 release gate.

### DFLASH

Run only when model metadata and runtime capability evidence prove DFLASH is
supported and active. A requested flag alone is never evidence.

### N-gram speculation

Use conditional staged search: screen for an opportunity, retain promising
variants, then tune only meaningful knobs for those variants. Inactive knobs
never enter the base orthogonal array.

### Concurrency

Opt-in only. Begin with the `--parallel 1` baseline and increase parallelism
only after VRAM preflight and explicit user selection. Unified-memory systems
are included. A concurrency result cannot replace or invalidate the default
single-user MTP recommendation.

## Rapid-MLX and future MLX loaders

The workload contract may eventually be shared for track selection, correctness
scoring, TTFT, memory, and receipt presentation. The 2.0 implementation remains
llama.cpp-only. Rapid-MLX and a future MLX/MTP loader require separate capability
evidence, factor catalogs, command builders, and receipts.

No llama.cpp batch, KV, MTP, DFLASH, or n-gram advice may be translated into
Rapid-MLX or an MLX backend.

## Acceptance gates

- Fake-server tests cover health/load failures, bounded response parsing,
  tool/schema failures, failed probes, and readiness/port cleanup; managed
  lifecycle tests cover cancellation, restart/restore, and concurrency cleanup.
- Unsupported or inert MTP/DFLASH/n-gram flags cannot enter a design; active
  capability evidence is recorded. DFLASH remains fail-closed without a
  verified runtime signal.
- Tool correctness is independent of speed; a faster incorrect candidate
  cannot qualify.
- `--parallel 1` remains the default recommendation; concurrency receipts are
  explicitly opt-in and hardware-scoped.
- Representative hardware receipts show repeatable benefit over no-spec
  baselines without correctness regression.
- Failed, unsupported, or declined tracks degrade to Quick/Balanced results
  rather than blocking model launch.
## Qwen MTP qualification preset (2026-08-15)

The next real-server smoke receipt must use an explicitly speculative Qwen preset. The earlier Qwen3.5-9B Balanced receipt was a regular batch/ubatch/context sweep; its `-MTP-` filename did not make it an MTP test.
This fixture is a compatibility check, not an MTP optimizer: preserve the requested speculative values, run `--parallel 1`, and compare with an explicit no-spec baseline.

### Requested effective configuration

| Area | Requested value | Current contract / qualification note |
|---|---|---|
| Sampling | `--temp 1.0 --top-p 0.95 --top-k 20 --min-p 0.0 --repeat-penalty 1.0 --presence-penalty 0.0` | Already represented by `ModelPreset` and emitted by the llama.cpp adapter. |
| Repeat window | `--repeat-last-n 64` | Runtime help confirms the flag, but `ModelPreset`, API serialization, adapter, Spawn Wizard, and Preset Editor do not expose it yet. Leave it out of this fixture and track it below. |
| Context / attention | `--ctx-size 200000 --flash-attn auto` | Use `auto` for the qualification preset; on this host it resolves to the same active Flash Attention path as `on`, while preserving model/runtime portability. An explicit `on` override remains user-selectable and must be recorded separately. |
| Main KV | `--cache-type-k q8_0 --cache-type-v q8_0` | Supported as `ctk`/`ctv`. |
| Draft KV / placement | `-ctkd q8_0 -ctvd q8_0 --spec-draft-ngl all` | Supported. `all` is represented as `gpu_layers=-1` / `spec_draft_ngl=-1` and emitted as `all`. |
| Fit / KV mode | `--fit off --no-kv-unified --cache-idle-slots` | Supported as `fit_enabled=false`, `kv_unified=false`, and `cache_idle_slots=true`. Cache-idle behavior remains subject to available memory. |
| Batching | Requested `-b 2048 -ub 512` | Both are supported. The measured Phase 4 comparator was `-t 6 -tb 6 -b 512 -ub 512`; retain that as the evidence-backed alternate, not as an unannounced override. |
| Threads | Requested `--threads -1` | Supported. Phase 4 measured `threads=6` with batch threads following at `6` on this host; the receipt must show which value was actually used. |
| Slots | `--parallel 1` | Supported and required for the default MTP smoke lane. |
| Vision | `--image-min-tokens 1024 --image-max-tokens 4096` | Supported; only affects models with a multimodal projector. |
| Reasoning | `--reasoning on --reasoning-budget 8192 --reasoning-budget-message "\nFinal Answer:"` | Supported. `reasoning-preserve` is not forced for Qwen3.5; older templates may ignore it harmlessly, while Qwen3.6/3.8 may opt into it when capability evidence says it is meaningful. |
| Speculation | `--spec-type draft-mtp,ngram-mod,ngram-map-k4v --spec-draft-n-max 3 --spec-draft-p-min 0.20 --spec-default` | Supported typed fields. Capability gating must prove each requested method is active; unsupported methods are omitted and reported, never silently treated as measured. |
| Template | `--chat-template-file <app-root>/chat-templates/qwen-froggeric-fixed/chat_template.jinja` | Use the pinned froggeric v22 artifact (revision `9f14778c92c3b5ed3e0738085694c0d3452802dd`), resolved below the active application root. |

The MTP smoke matrix contains two independent load-time cells: (A) the measured Balanced comparator (`threads=6`, `threads_batch=6`, `batch=512`, `ubatch=512`) and (B) the requested MTP configuration (`threads=-1`, `batch=2048`, `ubatch=512`). Each cell runs its own explicit no-spec baseline with all other settings held constant. The result is reported per cell; this bounded smoke check does not silently choose a universal batch winner.

The following requested switches are already backend-enforced defaults rather than editable preset fields: `--jinja`, `--no-context-shift`, `--ctx-checkpoints 32`, `--no-warmup`, and `--metrics` (plural). The requested `--metric` spelling is invalid for this managed binary. The requested `--repeat-penality` spelling is corrected above.

### Load mode policy

The current Phase 7 cells intentionally use llama.cpp's default `--load-mode mmap` (the generated `llama-bench` command omits the deprecated `--mmap` switch). This keeps the MTP/no-spec comparison focused on batching, KV quality, and speculation with identical model-loading behavior, and matches the runtime default reported by the managed `--help` probe. `--load-mode none` is a valid newer alternative (the old `--no-mmap`/`--mmap 0` spelling is deprecated), but it changes startup and memory behavior rather than decode quality. It should be a separate, explicitly selected load-mode experiment—not mixed into the current two-cell MTP matrix or presented as a universal recommendation.

Evolve the existing typed `no_mmap` boolean into a typed `load_mode` policy (`mmap`, `none`, `mlock`, `mmap+mlock`, `dio`) in a later backend/frontend slice; preserve a migration path for existing presets. It must be serialized in presets and receipts, emitted by both `llama-bench` and `llama-server`, capability/platform-gated, visible in Spawn Wizard and Preset Editor advanced controls, and covered by a matching-load-mode/no-spec receipt. Until then, users who require `--load-mode none` remain a follow-up request rather than an untracked `extra_args` override.

### Explicit follow-up work list

- [x] Evolve existing `no_mmap` into typed `load_mode` (`mmap` default; `none`/`mlock`/`mmap+mlock`/`dio` opt-in) across the backend, both frontend surfaces, command builders, migrations, and focused tests. Keep non-default load modes out of the Phase 7 MTP matrix.

- [ ] Add `repeat_last_n: Option<u32>` to `ModelPreset`, API request/response contracts, llama.cpp command construction, migration/defaulting, Spawn Wizard advanced controls, Preset Editor advanced controls, receipts, and focused UI/API tests. Default to omitted/managed runtime behavior until the field is available end to end.
- [ ] Add a typed `no_cont_batching` policy to the backend and both preset surfaces. Qualification must record whether continuous batching was disabled; it must not be smuggled through `extra_args`.
- [ ] Add typed `swa_full` policy and a fail-closed qualification branch. Compare SWA-full versus the normal cache only on opt-in, model/context-compatible runs; treat context-creation failure or OOM as a recorded unsupported result, not as a recommendation.
- [ ] Add typed `checkpoint_min_step` and `cache_reuse` policies, with receipt provenance and UI explanations. Keep the current backend `ctx-checkpoints=32` default explicit while deciding whether its count should become user-editable.
- [ ] Add a typed server log-verbosity field (`-lv/--verbosity`) with qualification default `4`; MTP qualification must emit `-lv 4` and capture fixture-scoped acceptance telemetry, while the no-spec control records acceptance as not applicable. Do not overload `extra_args`.
- [ ] Add model-capability handling for `reasoning-preserve`: Qwen3.5 must remain valid without it; Qwen3.6/3.8 may expose it when the template/runtime capability snapshot supports preserved reasoning.
- [ ] Add a canonical Qwen MTP fixture receipt containing the effective preset, no-spec baseline, active capability evidence, template revision, and the Phase 4 `threads=6, threads_batch=6, batch=512, ubatch=512` comparator before calling the smoke lane representative.

No requested option may be added through arbitrary `extra_args`; each new control must be typed, capability-gated, serialized, visible on both frontend surfaces, and covered by a receipt-backed test.
## Repeated managed-server qualification (2026-08-16)

The selected Qwen3.5 9B MTP GGUF was run at 131,072 context with q8_0 main KV, batch/ubatch 1024/512, `--parallel 1`, Flash Attention `auto`, reasoning enabled with an 8,192-token server budget, and the pinned Froggeric v22 template. The speculative candidate explicitly used `--spec-type draft-mtp,ngram-mod,ngram-map-k4v --spec-default --spec-draft-ngl all --spec-draft-n-max 3 --spec-draft-p-min 0.20`; the control used the same settings with `--spec-type none`.

Six candidate requests were retained: two long reasoning/design tasks, a multi-turn qualification task, a declared-tool request, a synthesis task, and a realistic request with API `max_tokens=32768`. The matched control replayed the initial reasoning task, multi-turn task, tool-shaped task, and realistic 32k-cap task. Candidate `-lv 4` logs proved active `draft-mtp` and `ngram-map-k4v`; per-request draft acceptance was 0.40785, 0.54619, 0.44287, 0.85897, 0.38435, and 0.51537. Aggregate draft-MTP counters reached 6,090 generated drafts and 4,065 accepted drafts, while ngram-map-k4v emitted activity. The tool-shaped request emitted streamed `tool_calls` in both candidate and no-spec responses. The no-spec control emitted no speculative counters, as expected.

These are fixture/workload-specific observations, not a universal acceptance score or speed guarantee. Response lengths and stochastic reasoning paths differ, so latency is retained as raw per-request evidence only. Raw response captures and `-lv 4` logs are retained outside the repository for this run and are not portable fixtures. The short managed-server gate is closed for this host/model/config; Windows and representative-hardware qualification remain open.
## Runtime-reduction policy (2026-08-16)

The repeated server run showed no stalls; long reasoning requests completed normally. Therefore server qualification keeps explicit cancellation and bounded request controls, but does not turn a fixed wall-clock deadline into a correctness gate. The full 32k-output request remains finalist-only evidence. The default bundle uses one substantial reasoning request, one tool-shaped request, and one no-spec comparison; expanded deep workloads remain opt-in.

Calibration-owned `llama-bench` sweeps no longer apply the former 2,648-second hard deadline. They remain cancellable through the calibration job and kill the owned process group on cancellation or drop. Quick screens use one repetition and confirm at most the baseline plus the strongest survivor with two; Balanced screens use one repetition and confirm the baseline plus two strongest survivors with two; three repetitions remain reserved for standalone/Thorough work. Balanced screens use 512 prompt tokens, 1,024 generation tokens, and a 32k context ceiling, then rerun finalists at the full requested workload.

For the dense-Qwen3.5/3.6/3.8 evidence set, the default batch screen is `512`, `1024`, and `2048` with `ubatch=512`. `1536` and `4096` remain explicit deeper-search values; `4096` was high-noise in the Phase 4 receipt and no derived candidate beat the baseline reliably. This reduction is intentionally not a MoE policy and must not be generalized to MoE models without a separate catalog and receipts.

### Validation record (2026-08-16)

- [x] Rust formatting, Clippy, full serialized test suite (1,316 passed, 14 ignored), release build, and Windows GNU-target check.
- [x] JavaScript syntax validation, ESLint, and `git diff --check`.
- [ ] New PR CI run remains pending push. Historical PR 314 Playwright artifacts expired; failed job logs remain available locally for diagnosis.
