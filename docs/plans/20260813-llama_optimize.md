# Hardware-native llama.cpp calibration inspired by `llama-optimize`

**State:** Approved as a bounded Local LLM Foundry 2.0.0 release requirement; the Phase 5 UI boundary is in progress. Rapid-MLX benchmark classification is informational-only (`classify_rapid_mlx_benchmark_result`); typed contracts, active-root persistence, durable recovery, authenticated preflight/start/poll/cancel, deterministic typed candidate planning, sequential bounded Quick execution with per-candidate receipts/winner selection, db-admin transactional apply with optimistic conflict checks and derived-preset default, and the Preset Editor Calibrate flow pass full Rust/JS validation, release build, clippy, diff checks, and a focused release-built Playwright test. Remaining Phase 5 gates are complete result-review/apply-conflict/rollback coverage, release-built screenshots with human acceptance, and post-apply validation. Upstream pin/evidence, full runtime capability probing, Balanced search, Phase 6, and Phase 10 gates remain open.
**Date:** 2026-08-13
**Primary owner:** Luna, executed phase-by-phase with one focused verifier after each hard gate.
**Target release:** 2.0.0 for the bounded llama.cpp Calibration v1 scope below; advanced stages remain post-2.0 follow-ups.
**Upstream research target:** [`bigattichouse/llama-optimize`](https://github.com/bigattichouse/llama-optimize) at commit `1d9e7d7fc2c94675362673983ea4fa1e756e0a0a` (2026-08-11).
**Upstream DOE dependency:** [`bigattichouse/robust`](https://github.com/bigattichouse/robust) at commit `a457b7f7f4a7a06b183fd55be4b8aced5d7f2541`.
**Licenses:** `llama-optimize` is MIT; `robust` is CC0-1.0. Preserve the MIT notice for any copied or substantially derived code.

## 1. Executive decision

Implement this capability. It would be materially more useful than the current fixed advice and one-dimensional sweeps because it can measure interactions among llama.cpp parameters on the user's actual machine, model, runtime build, context requirement, and workload.

The product should call the capability **Calibration**, not “llama-optimize,” and should describe its evidence honestly:

- Current defaults and `/api/advise` are estimates or static recommendations.
- Existing batch, context, MoE, and MTP tools measure isolated ladders.
- Calibration performs a bounded, resumable experiment and produces measured alternatives such as **Fastest**, **Balanced**, and **Max context**.
- A calibration applies only to the exact hardware, model, llama.cpp runtime, workload, and baseline configuration recorded in its receipt.

The recommended implementation is a **native Rust calibration service** that borrows and verifies the upstream methodology while using llama-monitor's existing typed launch, model introspection, managed binaries, auth, job, preset, and UI systems. Do not ship the upstream Python program as the permanent product integration.

### 2.0.0 release boundary

Calibration v1 is a required part of the Local LLM Foundry 2.0.0 product and release story. It must land before the rebrand plan's Phase 12 cross-platform/security qualification begins. Phase 12 and Phase 13 must qualify and visually accept Calibration together with migration, packaging, auth, runtime, and the rest of the release candidate; adding or materially changing it afterward reopens those gates.

The 2.0.0 release-blocking scope is deliberately bounded:

- Phase 0 correctness, attribution, binary-contract, backend-patch, and introspection gates;
- Phases 1–3 job lifecycle, durable receipts, managed sibling tools, typed candidates, and trustworthy `llama-bench` measurement;
- Phase 4 **Quick** and **Balanced** bounded search, measured Pareto picks, noise/confidence reporting, cancellation/resume, and pick verification;
- Phase 5 Preset Editor **Calibrate this preset**, derived-preset-by-default apply, optimistic conflict detection, short post-apply validation, and rollback;
- Phase 6 exact-receipt reuse and explicit optional post-download calibration in the Spawn Wizard;
- the 2.0-owned portion of Phase 8: accurate documentation, backend-safe advice, current/stale/noisy receipt findings, and removal or consolidation of directly superseded narrow tuning behavior;
- Phase 10 security, platform, release-built UI, screenshot, regression, and real-hardware acceptance for the bounded feature.

The following are explicitly **not** 2.0.0 release blockers and remain separately gated follow-ups:

- Thorough/overnight search, multi-pass refinement above the Balanced limits, and automatic context-ceiling exploration;
- Phase 7 multidimensional real-server search for MTP, ngram, and concurrency (2.0 still performs a bounded post-apply validation using the existing typed live-benchmark path);
- automatic multi-GPU placement;
- broad Doctor architecture consolidation beyond the minimum Calibration receipt findings/link;
- Phase 9 Rapid-MLX calibration;
- arbitrary/custom factors, environment variables, RoPE/YaRN, and sampling-quality optimization.

Do not weaken the bounded scope into a static advisor branded as Calibration. The 2.0 feature is complete only when it measures multiple typed candidates on the actual machine, persists reproducible evidence, presents measured alternatives, and can safely apply and roll back a derived preset.

### Recommended product placement

1. **Primary:** Preset Editor action, **Calibrate this preset**.
   - The model is local, the baseline is explicit, and the result can be reviewed before creating or updating a preset.
2. **Secondary:** Spawn Wizard.
   - Reuse a matching measured calibration immediately.
   - If no matching receipt exists, offer an explicit post-download calibration job; never block the ordinary Quick/Guided flow on a long experiment.
3. **Supporting:** Tune panel and Doctor.
   - Tune panel validates an applied winner against the real server workload.
   - Doctor reports missing, stale, noisy, regressed, or unsafe calibration evidence and links to calibration. Doctor never starts a full run automatically.

### Why not invoke the Python script permanently?

- It is one unversioned 5,036-line script with no stable library or machine-readable progress/cancellation API.
- It requires Python 3.10, a compiled C submodule, and platform packaging that is not currently part of llama-monitor.
- It knows nothing about llama-monitor auth, sessions, presets, GPU exclusivity, application-home migration, or typed backend contracts.
- Its CLI and result files are only safe contracts when pinned to a commit; there are no releases or semantic versions.
- Windows is unproven and Metal telemetry is incomplete.
- Directly exposing `--factor` or `--env` would bypass our validation and could persist unsafe or secret-bearing input.

### Why not vendor `robust` immediately?

The C library is viable, but it adds native build, cross-compilation, attribution, and Windows packaging work before product value is proven. Start with a small native Rust design implementation whose output is contract-tested against pinned `robust` fixtures. Revisit linking or invoking `robust` only if parity tests show the Rust implementation is statistically or operationally inadequate.

## 2. Research findings

### 2.1 What upstream actually provides

`llama-optimize` is a designed-experiments orchestrator, not a collection of magic defaults:

1. Detect hardware and GGUF metadata.
2. Build a model- and hardware-sensitive factor space.
3. Optionally use Morris elementary-effects screening to identify consequential factors and interactions.
4. Use a Taguchi orthogonal array (commonly L25 after screening, otherwise potentially L125) instead of a full factorial sweep.
5. Run `llama-bench` for raw prefill/decode measurement or an ephemeral loopback `llama-server` for real generation, MTP/ngram, and concurrency.
6. Randomize execution order, control thermal drift, repeat measurements, and reject physically implausible results.
7. Persist every result incrementally and journal a configuration before executing it so a machine crash is not retried automatically.
8. Select measured Pareto winners and verify the headline picks. The Taguchi predicted optimum is diagnostic, not the applied answer.

Upstream profiles and objectives are useful starting points:

| Product workload | Upstream shape | Driver | Product interpretation |
|---|---:|---|---|
| Interactive | 512 prompt + 256 generated | bench or server | One user, balanced TTFT/decode |
| Agents/RAG | 8192 + 256 | bench or server | Long prefill and several workers |
| Multi-user | 1024 + 256, 8 streams | server | Aggregate concurrent serving |
| Thinking | long generation (~2048) | server preferred | Decode-heavy reasoning |

The upstream effective-throughput objective is:

```text
effective_tps = (prompt_tokens + generated_tokens)
              / (prompt_tokens / pp_tps + generated_tokens / tg_tps)
```

Default upstream ranking uses decode `tg_tps`; `effective_tps` is opt-in. Our product should make the workload/objective explicit and add safety/fidelity gates rather than silently equating “best” with the largest TPS number.

### 2.2 Upstream factors worth considering

The useful llama.cpp surface includes:

- GPU layer offload (`-ngl`)
- context/depth (`-c` / `-d`)
- CPU decode and batch threads (`-t`, `-tb`)
- KV types (`-ctk`, `-ctv`) with a default quality floor of `q8_0`
- logical and physical batch (`-b`, `-ub`)
- KV offload (`-nkvo` / `--no-kv-offload`, version-dependent spelling)
- CPU polling (`--poll`)
- dense tensor placement (`-ot`)
- MoE expert-layer CPU offload (`-ncmoe` / our typed `n_cpu_moe` mapping)
- NUMA mode where multiple nodes are introspected
- MTP enablement and draft/acceptance parameters under the server driver
- ngram self-speculation using conditional staged searches
- real parallelism/concurrency under the server driver

Do not copy the claim that this covers “every knob.” Upstream does not currently tune multi-GPU split mode, tensor split, or main GPU; sampling/model choice is out of scope; RoPE/YaRN changes model behavior rather than merely performance.

### 2.3 Upstream safety and maturity

Positive evidence:

- The pinned checkout's offline `--selftest` passed locally after building `robust`.
- CI builds `robust` and covers L9/L25/L125 design generation and analysis.
- The tool incrementally flushes CSV, records `OOM`, `TIMEOUT`, `ERROR`, `IMPLAUSIBLE`, `CRASH`, and predicted-skip outcomes, and uses a durable crash journal.
- Measurement-validity and conditional-factor failure histories are documented with useful invariants.
- GitHub reported 44 stars, 2 forks, 96 local commits, recent activity, and no tags/releases on 2026-08-13.

Limits:

- No real-GPU CI and Linux is the only CI operating system.
- Windows support is not demonstrated.
- Metal can run llama.cpp but upstream lacks Metal-specific temperature/VRAM telemetry.
- Multi-GPU placement tuning is still a design document, not an implemented feature.
- A default L125 run can take hours and intentionally approaches OOM/driver-failure boundaries.

### 2.4 What llama-monitor already has

The new system should consolidate and reuse, not create a parallel launch stack:

- `ModelPreset` in `src/presets/mod.rs` is the durable, migrated preset schema.
- `LocalLaunchRequest`, `validate_preset_backend_config`, and `request_from_preset` in `src/inference/launch.rs` are the canonical typed launch boundary.
- `LlamaCppAdapter` in `src/inference/llama_cpp.rs` is the canonical argument and environment builder.
- `AppConfig::llama_server_path` and `llama_server_cwd` are the runtime path contract.
- `src/llama/bench_runner.rs` already resolves `llama-bench` as a sibling of the configured `llama-server` and parses `llama-bench -o json`.
- `POST /api/vram-estimate` in `src/web/api/vram.rs` is the canonical introspection-first memory estimator.
- `POST /api/tune/ncpumoe`, `/api/bench/sweep`, `/api/bench/batch-sweep`, and `/api/bench/mtp-sweep` in `src/web/api/benchmark.rs` already provide narrow empirical tools.
- `src/models/import_lab.rs` contains a bounded, cancellable, retained in-memory job pattern with polling endpoints.
- `scripts/model-runtime-benchmark.mjs` and the runtime-specific suites contain receipt and workload-fidelity patterns.
- `static/js/features/tuning-cards.js`, `spawn-wizard-tuning.js`, `presets.js`, and `tune-panel.js` provide the current tuning UI/apply surfaces.
- `DoctorFinding` and `FixAction` in `src/state.rs`, plus `src/web/api/doctor.rs`, provide a typed diagnostic surface.

The managed llama.cpp bundle is the only supported binary source for calibration. Today its default root is `~/.config/llama-monitor/bin/`; after the v2.0.0 application-home migration it is `~/.config/local-llm-foundry/bin/`. Never hardcode either path. Resolve:

```text
llama-server     = AppConfig.llama_server_path
llama-bench      = sibling(AppConfig.llama_server_path, platform executable name)
llama-fit-params = sibling(AppConfig.llama_server_path, platform executable name)
working dir      = AppConfig.llama_server_cwd
environment      = the same backend environment used by LlamaCppAdapter
```

The downloader retains the full extracted llama.cpp bundle, including `llama-bench` and other tools. Add capability preflight for each required sibling; do not assume every release contains or supports every flag.

### 2.5 Correctness defect to fix before expansion

The current live benchmark path sends Rapid-MLX measurements through the same `classify_benchmark_result` used for llama.cpp. That classifier emits llama.cpp-oriented actions such as `flash_attn`, `context_size`, and `batch_size`. The Tune panel then shallow-merges those top-level fields even though Rapid settings live under `rapid_mlx.*`. This can render or “apply” placebo/backend-invalid suggestions.

This is a Phase 0 hard gate. Tuning actions must become backend-qualified typed patches, and Rapid must have its own classifier or informational-only cards until a valid Rapid action exists.

## 3. Frozen product and architecture decisions

Unless Nick explicitly changes them during implementation:

| Area | Decision |
|---|---|
| Product term | **Calibration**; upstream name appears only in attribution/research docs |
| First backend | llama.cpp only |
| Primary entry | Preset Editor, **Calibrate this preset** |
| Wizard | Reuse matching receipt or explicitly queue calibration after local model availability |
| Doctor | Read-only findings/link by default; never auto-run a full calibration |
| Runtime binaries | Exact configured managed `llama-server` plus sibling tools; migration/path agnostic |
| Architecture facts | GGUF/runtime introspection only; never filename-derived |
| Trial launch | Typed `LocalLaunchRequest`; no free-form command strings or writes to `extra_args` |
| Preset mutation | Never during trials; winner applies only after review/confirmation |
| Apply behavior | Default creates a derived preset; updating the source preset is a separate explicit action |
| Default KV floor | `q8_0`; lower precision is an explicitly labeled quality tradeoff |
| Search answer | Measured Pareto candidates; predicted optimum is never auto-applied |
| Execution | One exclusive local calibration job; no parallel scenarios or competing local server |
| Network | Ephemeral loopback only; calibration server never binds publicly |
| Persistence | Durable, versioned receipt and journal under the active application home |
| Security | api-token for status/read/start validation; db-admin-token plus exact confirmation for disruptive apply/restart actions |
| Rapid-MLX | Separate later adapter and factor catalog; never translate llama.cpp factors |
| Python upstream | Research oracle/fixture generator only; not a shipped runtime dependency |
| `robust` | Do not vendor initially; native Rust designs verified against pinned fixtures |

## 4. Target user flow

### 4.1 Preset Editor

1. User opens a local llama.cpp preset and selects **Calibrate this preset**.
2. Preflight reports:
   - exact model, runtime version/hash, hardware identity, and supported factor set;
   - whether the active server must stop;
   - estimated run count and broad duration range;
   - workload, minimum context, time budget, quality floor, and risk level;
   - current matching/stale receipt, if any.
3. User chooses a budget:
   - **Quick check:** small bounded design; useful signal, explicitly lower confidence;
   - **Balanced:** Morris screen plus a small orthogonal design and pick verification;
   - **Thorough:** larger/repeated/refined design with context ceiling validation.
4. Calibration runs as a cancellable job. UI shows phase, current/total trials, elapsed time, ETA range, current factor summary, failures/skips, temperature/noise warnings, and retained progress.
5. Results show measured **Fastest**, **Balanced**, and **Max context** candidates, Pareto tradeoffs, baseline deltas, memory headroom, confidence/noise, and exact changed fields.
6. User can:
   - validate a candidate with the real server workload;
   - create a derived preset;
   - explicitly update the source preset;
   - keep the receipt without applying;
   - discard/forget the receipt.

### 4.2 Spawn Wizard

- Before download/local availability: show estimates only.
- After a local GGUF exists:
  - if a current matching receipt exists, offer its measured Balanced candidate;
  - if no receipt exists, offer **Calibrate after download** as an explicit optional action;
  - keep ordinary spawn available immediately;
  - never silently mutate the wizard's canonical control state from a background result.
- Applying a measured candidate must drive the real shared control registry and dispatch existing events so review/preset payloads remain canonical.

### 4.3 Tune panel and Doctor

- Tune panel runs a short post-apply validation against the active local server. It should compare the result with the receipt rather than invent another unrelated recommendation.
- Doctor reports:
  - no calibration available (informational only);
  - receipt stale because hardware/model/runtime/baseline/workload changed;
  - high measurement spread or thermal contamination;
  - applied preset has regressed materially from its verified receipt;
  - interrupted job is resumable;
  - required managed sibling binary/capability is missing.
- The Doctor action opens Calibration at preflight or results. It does not start benchmarking by itself.

## 5. Data and API contracts

All new HTTP/DB structs use `#[serde(default)]`, reject unknown state-changing input where practical, and return parse failures as 400.

### 5.1 Core identifiers and fingerprints

Create `src/calibration/` with versioned, backend-neutral orchestration and a llama.cpp adapter:

```rust
pub struct CalibrationFingerprint {
    pub schema_version: u32,
    pub backend: InferenceBackend,
    pub hardware: HardwareFingerprint,
    pub model: ModelFingerprint,
    pub runtime: RuntimeFingerprint,
    pub workload: CalibrationWorkload,
    pub baseline_config_hash: String,
    pub factor_catalog_version: u32,
}

pub struct HardwareFingerprint {
    pub os: String,
    pub arch: String,
    pub cpu_identity: Option<String>,
    pub physical_cores: Option<u32>,
    pub logical_cores: u32,
    pub memory_bytes: u64,
    pub gpu_devices: Vec<GpuFingerprint>,
    pub unified_memory: bool,
}

pub struct ModelFingerprint {
    pub canonical_path_id: String,
    pub file_size: u64,
    pub modified_unix_ms: u128,
    pub content_fingerprint: String,
    pub gguf_arch: Option<String>,
    pub metadata_fingerprint: String,
}

pub struct RuntimeFingerprint {
    pub server_path_id: String,
    pub server_sha256: String,
    pub version: Option<String>,
    pub capability_hash: String,
    pub bench_sha256: String,
    pub fit_params_sha256: Option<String>,
}
```

Do not put raw absolute paths in browser responses or portable receipt summaries. Store canonical paths only in the protected local receipt where required, and expose redacted library-relative identities to UI.

### 5.2 Workload, policy, and request

```rust
pub enum CalibrationWorkloadKind {
    Interactive,
    Agents,
    MultiUser,
    Thinking,
    Custom,
}

pub struct CalibrationWorkload {
    pub kind: CalibrationWorkloadKind,
    pub prompt_tokens: u32,
    pub generation_tokens: u32,
    pub parallel_requests: u32,
    pub minimum_context: u64,
    pub objective: CalibrationObjective,
    pub fixture_id: String,
}

pub enum CalibrationBudget { Quick, Balanced, Thorough }
pub enum KvQualityFloor { F16, Q8_0, AnyExplicitlyLossy }

pub struct StartCalibrationRequest {
    pub preset_id: String,
    pub expected_preset_fingerprint: String,
    pub workload: CalibrationWorkload,
    pub budget: CalibrationBudget,
    pub kv_quality_floor: KvQualityFloor,
    pub max_context: Option<u64>,
    pub allow_stop_active_server: bool,
    pub exact_confirmation: Option<String>,
}
```

Custom factor names, arbitrary environment variables, raw argv, model paths outside the configured library, and arbitrary prompts are not accepted from this endpoint.

### 5.3 Typed factor/candidate contract

```rust
pub enum LlamaCppFactor {
    GpuLayers,
    ContextDepth,
    Threads,
    ThreadsBatch,
    CacheTypeK,
    CacheTypeV,
    BatchSize,
    UbatchSize,
    KvOffload,
    Poll,
    Numa,
    TensorPlacement,
    NCpuMoe,
    ParallelSlots,
    SpecType,
    SpecDraftNMin,
    SpecDraftNMax,
    SpecDraftPMin,
    SpecDraftPSplit,
}

pub struct CalibrationCandidate {
    pub id: String,
    pub typed_patch: LlamaCppCalibrationPatch,
    pub effective_launch: LocalLaunchRequest,
    pub capability_evidence: Vec<String>,
    pub predicted_memory: Option<VramEstimate>,
}
```

`LlamaCppCalibrationPatch` must contain typed optional fields corresponding to `ModelPreset`/`ServerConfig`; it must not contain `serde_json::Value`, dotted paths, or `extra_args`. Candidate construction follows:

```text
baseline ModelPreset
  -> request_from_preset
  -> apply typed candidate patch to a clone
  -> validate_preset_backend_config
  -> capability/range/relationship validation
  -> canonical LlamaCppAdapter argument emission
```

Never build candidate argv independently of the production adapter. If `llama-bench` needs a distinct spelling, centralize a typed mapping in the llama.cpp calibration adapter and capability-test it.

### 5.4 Measurement and validity contract

```rust
pub enum TrialStatus {
    Ok,
    Oom,
    Error,
    Timeout,
    ParseFailure,
    Implausible,
    PredictedUnsafe,
    Cancelled,
    SuspectedCrash,
}

pub struct CalibrationMeasurement {
    pub trial_id: String,
    pub status: TrialStatus,
    pub pp_tps_samples: Vec<f64>,
    pub tg_tps_samples: Vec<f64>,
    pub ttft_ms_samples: Vec<f64>,
    pub aggregate_tps_samples: Vec<f64>,
    pub effective_tps_samples: Vec<f64>,
    pub wall_time_ms: u64,
    pub memory_peak_bytes: Option<u64>,
    pub temperature_start_c: Option<f64>,
    pub temperature_end_c: Option<f64>,
    pub correctness: CorrectnessGateResult,
    pub bounded_diagnostics: Vec<String>,
}
```

Required validity gates:

- Use structured `llama-bench -o json`; never parse the human table.
- Cross-check reported tokens and throughput against independent wall time.
- Reject non-finite, negative, zero-duration, physically impossible, truncated, or missing-token results.
- Require expected fixture completion/fidelity markers for server-driver workloads.
- Treat OOM, timeout, parse failure, and crash as data, never as a high score.
- Use medians for headline metrics and record spread/MAD or another robust dispersion statistic.
- Randomize trial/group order with a persisted seed.
- Record thermal availability and contamination; lack of a sensor degrades confidence rather than fabricating a stable state.
- Never recommend a context larger than the largest verified safe context for that candidate.
- Never recommend a lossy KV type below the requested floor.
- Never apply a candidate that fails correctness/tool/protocol gates even if TPS is higher.

### 5.5 Receipt and journal

Store under:

```text
<AppPaths.root>/calibrations/
  index.json
  jobs/<job-id>/journal.jsonl
  jobs/<job-id>/snapshot.json
  receipts/<receipt-id>.json
```

Use atomic write-then-rename, bounded retained jobs, no symlink traversal, and restrictive permissions. The receipt contains:

- schema version and attribution/method version;
- exact fingerprint and staleness inputs;
- redacted baseline preset snapshot and hash;
- workload/fixture/objective/quality/budget;
- factor catalog and levels actually considered;
- persisted random seed and design rows;
- every typed candidate and bounded command evidence (redacted; no secrets);
- every measurement/status and validity result;
- Morris/main-effect diagnostics if run;
- measured Pareto frontier;
- Fastest/Balanced/Max-context picks and pick-verification samples;
- stop/cancel/failure reason;
- apply history: target preset, before/after fingerprint, timestamp, and validation outcome.

Journal `trial_planned`, `trial_started`, and `trial_finished` with an `fsync` boundary before starting a risky process. On startup/resume, a started-without-finished trial becomes `SuspectedCrash` and is not retried without a separate explicit confirmation.

### 5.6 HTTP routes

Add `src/web/api/calibration.rs`:

| Route | Auth | Purpose |
|---|---|---|
| `POST /api/calibrations/preflight` | api-token | Validate preset/model/binaries/capabilities; return factor set, conflicts, run-count/time range |
| `POST /api/calibrations` | db-admin-token + exact confirmation if stopping/restarting | Start one bounded job; returns 202 and snapshot |
| `GET /api/calibrations` | api-token | List redacted jobs/receipts relevant to the current user/machine |
| `GET /api/calibrations/{id}` | api-token | Poll snapshot/result |
| `POST /api/calibrations/{id}/cancel` | api-token | Cancel child, stop ephemeral server, persist partial result |
| `POST /api/calibrations/{id}/resume` | db-admin-token + confirmation | Resume unfinished safe rows; never retry suspected crash rows by default |
| `POST /api/calibrations/{id}/validate/{candidate}` | db-admin-token | Short real-server validation of one measured candidate |
| `POST /api/calibrations/{id}/apply` | db-admin-token + exact preset fingerprint/confirmation | Create derived preset or explicitly update source preset transactionally |
| `DELETE /api/calibrations/{id}` | db-admin-token + confirmation | Forget a completed receipt/job; reject active deletion |

Do not reuse `{ "tuning": true }` to bypass the existing global benchmark cooldown. Calibration gets its own exclusive lease, rate limits, resource bounds, and lifecycle.

## 6. Search and budget design

### 6.1 Separate safe defaults from experimental factors

Build a declarative catalog containing:

- backend and driver applicability;
- typed preset/config field;
- capability evidence required from the exact binary;
- architecture/memory preconditions from introspection;
- allowed levels/ranges;
- cross-field constraints (`ubatch <= batch`, quantized KV requires compatible flash attention, etc.);
- quality/risk class;
- whether the factor affects load-time grouping;
- whether the factor can be applied to a durable preset;
- product tier: default, advanced, or qualification-only.

Initial default factor set should be conservative:

| Factor | Quick | Balanced | Thorough | Guard |
|---|---|---|---|---|
| `gpu_layers` | narrow around estimator fit | yes | yes | GGUF layers + memory estimate |
| `context_depth` | fixed user minimum | tradeoff axis | ceiling scan | never emit above verified depth |
| `threads` | 2-3 levels | yes | refine | real physical/logical topology |
| `threads_batch` | no | server validation | yes | server capability only |
| `batch_size` | 2 levels | yes | refine | `batch >= ubatch` |
| `ubatch_size` | 2-3 levels | yes | refine | managed binary capability |
| `ctk`/`ctv` | q8/f16 only | yes | explicit floor | no silent lossy cache |
| `n_cpu_moe` | MoE only | yes | refine | introspected MoE layers/exact estimator |
| dense tensor placement | no | qualified platforms | yes | exact `-ot` help capability |
| KV offload/poll/NUMA | no | eligible systems | yes | platform/capability evidence |
| MTP/ngram | no | separate server stage | separate server stage | introspection and correctness fixture |

Multi-GPU `tensor_split`, `split_mode`, and `main_gpu` are excluded until there is hardware-backed qualification on Windows/Linux and per-device memory validation. RoPE/YaRN and arbitrary environment factors are excluded from automatic calibration.

### 6.2 Budget semantics

Pin exact run-count ceilings in code and test them. Suggested initial limits:

- **Quick:** maximum 12 trials, one warmup + one measured repetition, no context ceiling probe, verify only the winning Balanced candidate.
- **Balanced:** Morris-style screen capped at 48 trials, final OA capped at 25 rows, two measured repetitions, verify top two Pareto candidates.
- **Thorough:** screen plus refinement, maximum 125 evaluated rows total per stage and 200 total trials, three-to-five repetitions, context ceiling probe, pick verification.

The preflight must calculate the actual design before consent. If a requested factor space exceeds the budget, stage or prune it; never silently promote a job to a larger array.

### 6.3 Native design implementation

Create pure modules with fixture-driven APIs:

```text
src/calibration/design/grid.rs
src/calibration/design/morris.rs
src/calibration/design/taguchi.rs
src/calibration/design/conditional.rs
src/calibration/analysis/pareto.rs
src/calibration/analysis/effects.rs
src/calibration/analysis/noise.rs
```

Implementation requirements:

- Copy/reference only documented, licensed algorithms and array definitions.
- Add attribution in `THIRD_PARTY_LICENSES.md` or the repository's existing equivalent.
- Generate pinned JSON fixtures from the exact upstream commits for L9, L25, L125, mixed-level mapping, Morris design/analysis, main effects, and recommendations.
- Rust does not need byte-for-byte randomized row order, but must cover every declared level and match array dimensions, balance, analysis, and recommendation semantics.
- Keep `ContextDepth` in the survivor set and refinement tradeoff axis.
- Conditional factors use staged searches; do not place a gate and its mostly inert child knobs in one flat design.
- Measured Pareto rows select winners. Additive predictions can nominate a confirmation candidate but never replace measured results.

## 7. Phased implementation plan

Every phase is a separate implementation context. At phase start, reread this plan and the cited current source. Do not assume line numbers survived prior phases.

### Phase 0 — Documentation discovery, current correctness, and frozen contracts

**Goal:** Establish trustworthy boundaries before adding the optimizer.

#### Tasks

- [ ] Re-clone upstream and record current HEADs, licenses, tags/releases, CLI help, self-test output, and a produced-file manifest under `docs/plans/evidence/20260813-llama-optimize/phase-00/`.
- [ ] Read upstream `README.md`, `docs/DESIGN.md`, `docs/CONDITIONAL-FACTORS.md`, `docs/measurement-validity.md`, `docs/multi-gpu-design.md`, `ROADMAP.md`, and relevant `robust` public headers/binding docs in full.
- [ ] Produce an allowed-contract ledger for pinned upstream CLI, CSV/status values, verification/probe JSON, Morris schema, and Taguchi APIs. Human console output is explicitly forbidden as an integration contract.
- [ ] Inspect exact managed `llama-server`, sibling `llama-bench`, and sibling `llama-fit-params` `--help` on each available platform; store raw stdout/stderr and hashes.
- [ ] Fix the cross-backend recommendation defect:
  - add backend-qualified typed suggestion patches;
  - make llama.cpp patches target llama fields and Rapid patches target `rapid_mlx.*`;
  - implement a Rapid-specific classifier or render unsupported actions informational-only;
  - update Tune panel apply/sync tests.
- [ ] Remove filename/name-derived qualification from the tuning-advice input path:
  - stop deriving preset `param_b`, architecture, or provisional MTP capability from the filename in `static/js/features/presets.js`;
  - make `/api/advise` accept the same authoritative GGUF/HF introspection identity used by `/api/vram-estimate`;
  - return explicit unknown/degraded advice when metadata is unavailable rather than calling a name heuristic;
  - keep model-name text only as a display label, never as evidence.
- [ ] Write the versioned calibration structs/API examples into `docs/reference/inference-tuning.md` before coding later phases.
- [ ] Decide the attribution file location based on current repository convention and add MIT/CC0 notices for any copied fixtures/code.

#### Source patterns to copy

- Typed launch: `src/inference/launch.rs` (`LocalLaunchRequest`, `request_from_preset`, validation).
- Shared tuning card: `static/js/features/tuning-cards.js`.
- Current classifier and routes: `src/llama/spawn_wizard.rs`, `src/web/api/benchmark.rs`.
- Rapid typed command/capabilities: `src/inference/rapid_mlx/command.rs`, `capabilities.rs`, and `escape_hatch.rs`.

#### Verification

- [ ] Unit tests prove a Rapid benchmark can never emit/apply top-level llama.cpp fields.
- [ ] Unit/UI tests prove renamed files do not change advice and missing metadata produces unknown/degraded evidence.
- [ ] Existing llama.cpp suggestion Apply still restarts and synchronizes the correct preset.
- [ ] API parse/auth tests cover all changed structs and backend patch variants.
- [ ] `cargo test` and `tests/auth_routing.rs` pass.
- [ ] UI test covers actionable llama suggestion and informational/typed Rapid suggestion.

#### Hard gate

Do not start the calibration engine while generic flat tuning patches can cross backend boundaries or while upstream evidence lacks pinned commits/licenses/raw contracts.

#### Anti-pattern guards

- Do not import `llama-optimize.py` as a library.
- Do not parse its console output.
- Do not infer llama.cpp flag support from version strings alone.
- Do not use filenames to infer model architecture, MoE, MTP, or context.

### Phase 1 — Calibration model, persistence, staleness, and job lifecycle

**Goal:** Land the backend-neutral contracts and a safe no-op job engine before any GPU work.

#### Tasks

- [ ] Add `src/calibration/mod.rs`, `types.rs`, `fingerprint.rs`, `store.rs`, `job.rs`, and `staleness.rs`.
- [ ] Add application paths for calibration index, job directories, journals, and receipts through `AppPaths`; never concatenate legacy/new product roots.
- [ ] Implement atomic persistence, restrictive permissions, symlink/traversal refusal, bounded diagnostics, maximum retained job count, and pruning rules.
- [ ] Copy the bounded cancellation/poll/forget lifecycle from `src/models/import_lab.rs`, generalized behind a calibration-owned semaphore/lease.
- [ ] Persist a snapshot after each state transition and append/fsync journal events.
- [ ] Implement exact hardware/model/runtime/workload/baseline fingerprints and staleness reasons.
- [ ] Add redacted API serialization separate from protected on-disk serialization.
- [ ] Add the calibration routes with a fake/no-op driver: preflight, start, list, status, cancel, resume, apply rejection, forget.
- [ ] Wire routes through `src/web/api/mod.rs` and route/auth inventory tests.

#### Verification

- [ ] Job state tests: queued -> preflight -> running -> cancelling/cancelled or complete/failed.
- [ ] Restart recovery marks a started-without-finished trial `SuspectedCrash` and refuses automatic retry.
- [ ] Concurrent start returns 409; cancellation is idempotent; active deletion is rejected.
- [ ] Receipt path and input validation reject absolute paths, `..`, symlinks, special files, oversize JSON, and unknown destructive fields.
- [ ] api-token/db-admin-token requirements are explicit in `tests/auth_routing.rs`.
- [ ] Migration-root tests show active `AppPaths.root` controls storage before and after v2.0 migration.

#### Hard gate

No subprocess may be launched until lifecycle, cancellation, crash recovery, auth, and path security pass with a fake driver.

### Phase 2 — Managed binary/capability and typed candidate adapters

**Goal:** Produce validated candidate launch/bench arguments without executing them.

#### Tasks

- [ ] Generalize sibling tool resolution in `src/llama/bench_runner.rs` or a new `src/inference/llama_cpp_tools.rs` for platform names:
  - `llama-bench[.exe]`;
  - `llama-fit-params[.exe]`;
  - future tools only through an explicit enum.
- [ ] Resolve from `AppConfig.llama_server_path`; never from `$PATH`, `HOME`, or a branded default directory.
- [ ] Reuse the exact llama.cpp launch environment and working directory.
- [ ] Extend exact-binary capability probing/hashing so factor eligibility is based on `--help` evidence tied to the fingerprint.
- [ ] Implement the declarative llama.cpp factor catalog and `LlamaCppCalibrationPatch`.
- [ ] Implement baseline preset -> typed patch -> validation -> `LocalLaunchRequest` conversion.
- [ ] Refactor production argv generation only as needed so calibration and real launch share typed emission logic.
- [ ] Implement a separate typed `llama-bench` mapping with relationship validation and no shell/string splitting.
- [ ] Feed each candidate to canonical `/api/vram-estimate` logic internally and prune only when the estimator can prove it unsafe; unknown means “run/degraded,” not “skip.”
- [ ] Return preflight factor levels, excluded factors with reasons, exact run-count bounds, and binary/capability evidence.

#### Verification

- [ ] Golden argv tests for macOS/Linux/Windows executable names and representative dense/MoE/MTP presets.
- [ ] `ubatch <= batch`, layer bounds, MoE layer bounds, KV/flash compatibility, port/host, and context bounds fail closed.
- [ ] Candidate environment and argv contain no secret; paths with spaces stay one argv element.
- [ ] Missing `llama-bench` produces an actionable preflight error, not a panic.
- [ ] Missing/unsupported `llama-fit-params` disables only predictive pruning and records degraded evidence.
- [ ] Config-root migration tests resolve the same active configured binary without any branded path literal.
- [ ] Because this phase touches `#[cfg]`/platform behavior, run the Windows GNU check required by `AGENTS.md`.

#### Hard gate

Review a manifest of every factor and its emitted argv on each supported platform. No factor may reach execution through `extra_args`.

### Phase 3 — Structured `llama-bench` evaluator and measurement validity

**Goal:** Safely evaluate one candidate with exclusive GPU ownership and durable evidence.

#### Tasks

- [ ] Refactor `src/llama/bench_runner.rs` into reusable structured command execution with bounded stdout/stderr drains, timeout, kill-on-drop, and process-tree termination on Windows.
- [ ] Add an exclusive calibration lease that conflicts with local llama.cpp/Rapid launch, binary update, other offline sweeps, and another calibration.
- [ ] Require the active local server to be stopped through an explicit preflight/confirmation path; remember whether it should be restored, but do not restore until the job safely ends.
- [ ] Implement warmup and measured repetitions using `llama-bench -o json`.
- [ ] Parse `avg_ts`, per-test standard deviation when available, prompt/gen/depth fields, and tool version into typed results.
- [ ] Implement independent wall-clock plausibility checks and structured failure classification.
- [ ] Integrate system/GPU memory and temperature snapshots using current cross-platform telemetry; absence lowers confidence.
- [ ] Add deterministic fixture prompts/depth settings without user/private chat content.
- [ ] Write every planned/started/finished transition before proceeding.
- [ ] On cancellation/timeout, kill the exact child/process tree and release the lease; never use broad `pkill`/`taskkill` matching as the primary mechanism.

#### Verification

- [ ] Fake `llama-bench` fixtures cover OK, OOM, nonzero error, timeout, malformed JSON, huge output, NaN/infinite/impossible speed, cancellation, and child cleanup.
- [ ] A real managed `llama-bench --help` preflight receipt is captured.
- [ ] One intentionally tiny/local qualification model completes a bounded real trial on available macOS hardware; store raw evidence outside committed user paths.
- [ ] Cancellation leaves a resumable receipt and no orphan process/port.
- [ ] Server restore occurs only if it was running before calibration and restoration still matches the original configuration fingerprint.

#### Hard gate

A single candidate must be trustworthy and recoverable before implementing a multi-candidate search.

### Phase 4 — Native experiment design, analysis, and bounded calibration funnel

**Goal:** Implement Quick/Balanced/Thorough designs and measured Pareto selection.

**2.0 boundary:** Quick and Balanced plus their measured Pareto/pick-verification path are release-blocking. Thorough, multi-pass refinement beyond Balanced, and automatic context-ceiling probing are post-2.0 extensions and must not delay the bounded v1 once the release gates pass.

#### Tasks

- [ ] Add pure design modules for bounded grid, Morris screening, Taguchi arrays, conditional stages, and deterministic randomization.
- [ ] Generate and commit machine-readable oracle fixtures from pinned upstream L9/L25/L125 and Morris examples with source/license metadata.
- [ ] Implement run-count calculation before job consent.
- [ ] Implement workload-aware factor levels from introspected hardware/model/runtime/baseline, not model name.
- [ ] Implement the budget limits from section 6.2 and refuse/stage oversized designs.
- [ ] Preserve context as a tradeoff axis through screening/refinement.
- [ ] Persist the random seed and group load-affecting candidates to reduce redundant work without biasing execution order.
- [ ] Implement main effects as diagnostics, measured Pareto frontier, and Fastest/Balanced/Max-context selection.
- [ ] Re-measure candidate picks and use robust medians/spread.
- [ ] **Post-2.0:** Add context-ceiling probing only to Thorough, using conservative bounds and never rounding the applied context above verified evidence.
- [ ] Implement conditional stages for MTP/ngram later; the base engine must support them without enabling them in this phase.

#### Verification

- [ ] Rust fixture tests match pinned array dimensions, level coverage/balance, main-effect ordering, and recommendation semantics.
- [ ] Property tests prove deterministic seed behavior, budget ceilings, no invalid combinations, and context survivor invariants.
- [ ] Synthetic response surfaces recover known dominant factors and measured Pareto points.
- [ ] OOM/timeout/implausible rows never become winners.
- [ ] A large additive-prediction error surfaces a warning and still selects measured results.
- [ ] Resume never repeats finished rows and never silently retries suspected crash rows.

#### Hard gate

The verifier must sign off that result selection is measured, bounded, reproducible, and not dependent on parsing upstream human output.

### Phase 5 — Preset Editor calibration UX and transactional apply

**Goal:** Ship the primary user flow with results review and safe preset creation/update.

**Current evidence (2026-08-13):** The dedicated Calibration frontend module, llama.cpp-only Preset Editor action, preflight/start/poll/receipt flow, candidate throughput rendering, authenticated derived-preset apply path, bounded post-apply llama-bench validation with immediate rollback on failure, durable fingerprint-guarded rollback route with private pre-apply snapshots, light-theme/reduced-motion CSS, generated asset registration, and an opt-in real-local-GGUF capture scenario are implemented. `core/calibration.spec.js` now has six passing release-built cases covering results, stale preflight, cancellation, confirmation-before-apply/update conflict, explicit rollback, and Rapid-MLX suppression. The opt-in `calibration` capture reaches the real backend preflight and uses intercepted bounded results so it never starts a benchmark unless a future explicit live mode is added. The full Rust suite (1,249 passed, 13 ignored), clippy, JavaScript validation/lint, release build, formatting, and diff checks are clean. This is not a Phase 5 sign-off: screenshot human acceptance and full native/fake-runtime post-apply receipts remain open.

#### Tasks

- [ ] Add a dedicated frontend module such as `static/js/features/calibration.js`; keep API/state/rendering separate from `presets.js`.
- [ ] Add the Preset Editor **Calibrate this preset** action only for eligible local llama.cpp presets.
- [ ] Build the preflight modal with workload, minimum context, budget, KV floor, runtime/model fingerprint, conflicts, run count, and duration/risk language.
- [ ] Poll the job with bounded backoff and render progress/cancel/resume states.
- [ ] Render baseline, Pareto alternatives, confidence/spread, failure counts, memory headroom, stale warnings, and field-by-field patch review using DOM APIs (`textContent`), never unsafe HTML.
- [ ] Implement **Create derived preset** as the default apply action.
- [ ] Implement explicit **Update this preset** with expected preset fingerprint, server-side merge/validation, before/after receipt, and conflict response if the preset changed.
- [ ] Offer post-apply short validation and rollback to the captured prior preset values.
- [ ] Ensure active-session identity comes from backend session state, not merely the current dropdown selection.
- [ ] Update static asset registration/generated routes and JS module baseline if applicable.

#### Verification

- [ ] UI E2E covers preflight, start, progress, cancel, resume, results, stale state, derived preset, update conflict, validation failure, and rollback using fake runtime fixtures.
- [ ] No preset is mutated before confirmation.
- [ ] Rapid presets do not show the llama.cpp action.
- [ ] New CSS has light-theme and reduced-motion handling; no selector duplication or broken JS/HTML/CSS references.
- [ ] Run release build and capture new sequential scenarios for Preset Editor preflight, running, results, stale, and apply review.
- [ ] Review fresh screenshots with the user before accepting the UX.

#### Hard gate

Do not add wizard or Doctor entry points until the primary Preset Editor flow has human-accepted release-built screenshots and transactional tests.

### Phase 6 — Spawn Wizard reuse and optional queued calibration

**Goal:** Surface measured evidence without turning setup into a mandatory benchmark funnel.

#### Tasks

- [ ] Add receipt lookup by exact fingerprint after local GGUF introspection.
- [ ] Show **Measured on this hardware** only for an exact current receipt; show evidence/staleness details.
- [ ] Map a selected measured candidate through the canonical wizard control registry and existing events; do not maintain a second hidden state object.
- [ ] Add **Calibrate after download** only when the final local model path is known and the user explicitly opts in.
- [ ] If calibration is queued, preserve the ordinary preset/spawn choice and make the job visible outside the modal.
- [ ] Keep Quick/Guided flows concise; place calibration controls in Pro/Advanced and review summary.
- [ ] Maintain backend separation; Rapid gets no translated llama result.

#### Verification

- [ ] Exact receipt is offered; hardware/runtime/model/baseline drift makes it stale and prevents one-click apply.
- [ ] Applying candidate updates the real wizard values and final preset payload.
- [ ] No local model means no run action.
- [ ] Closing/reopening the wizard does not orphan or duplicate a job.
- [ ] Add/reuse sequential `wizard-llamacpp` capture scenarios and run the full CI-equivalent UI suite after release build.

### Phase 7 — Real-server workload validation, MTP, ngram, and concurrency

**Goal:** Extend beyond raw llama-bench only where the real server driver is necessary.

**Release boundary:** This multidimensional search phase is post-2.0. The 2.0 v1 uses the existing live benchmark path only for a bounded, typed post-apply validation; it does not claim that MTP, ngram, or concurrency were jointly optimized.

#### Tasks

- [ ] Add a calibration-owned ephemeral loopback server driver using the production `LlamaCppAdapter`/supervisor, exact typed candidate, isolated port, health check, bounded logs, and cleanup.
- [ ] Use versioned, deterministic, non-private workload fixtures and `/completion` or the canonical supported endpoint based on exact capability evidence.
- [ ] Measure TTFT, prompt/decode tokens, independent wall time, aggregate concurrency, marker/tool correctness, and memory.
- [ ] Reuse a server only for candidates with identical load-time configuration; document and test the grouping key.
- [ ] Add MTP only when introspection proves a NextN/MTP head and exact binary capabilities exist.
- [ ] Add ngram as a conditional staged search: screen variants, retain top K, then tune only that variant's meaningful knobs.
- [ ] Never mix inactive conditional knobs into the base orthogonal array.
- [ ] Validate the final applied candidate in its real workload and record any bench-to-server regression.

#### Verification

- [ ] Fake server covers health failure, load timeout, SSE/JSON fragmentation, wrong token counts, correctness failure, concurrent requests, cancellation, and port cleanup.
- [ ] MTP/ngram fixtures prove unsupported/inert flags cannot enter a design.
- [ ] A repeated or trivial prompt cannot qualify speculative-decoding benefit; constrained-tool and sampled-text fixtures are required.
- [ ] Server-driver winners pass correctness and show measured improvement over the no-spec baseline.
- [ ] No server binds beyond loopback and no secret enters logs/receipts.

#### Hard gate

Keep MTP/ngram/concurrency marked experimental until representative hardware receipts show repeatable nonzero benefit and correctness.

### Phase 8 — Doctor, Tune panel, documentation, and consolidation

**Goal:** Make calibration evidence discoverable and remove fragmented/contradictory advice.

**2.0 boundary:** Accurate documentation, backend-safe Tune behavior, and minimal current/stale/noisy Calibration receipt findings are release-blocking. Broad Doctor endpoint consolidation and cleanup of every legacy tuning surface may continue after 2.0 when it is not required to prevent contradictory or unsafe behavior.

#### Tasks

- [ ] **Post-2.0 unless required for correctness:** Choose `GET /api/doctor/findings` as the canonical aggregate Doctor source and migrate the Dashboard loader away from separately assembled finding arrays, preserving backend-specific evidence.
- [ ] Add typed calibration finding categories/codes and bounded link/open actions.
- [ ] Add post-apply Tune validation that compares against a receipt rather than emitting generic backend-wrong patches.
- [ ] Decide whether narrow batch/depth/MoE/MTP tools become Calibration Quick presets or remain expert tools; remove duplicated code only after parity tests.
- [ ] Update `docs/reference/inference-tuning.md`, `tune-panel.md`, `setup-wizard.md`, `spawn-wizard.md`, `dashboard.md`, and `api.md` as if Calibration always existed.
- [ ] Document scope honestly: llama.cpp first, local GGUF required, long/disruptive runs, receipts are hardware/runtime/workload-specific, no automatic quality guarantee.
- [ ] Document managed binary resolution through config rather than either branded application-home literal.
- [ ] Add attribution and upstream update procedure: fetch pinned upstream, regenerate oracle fixtures, review contract drift, never auto-follow `main`.

#### Verification

- [ ] Doctor shows current/stale/noisy/regressed/resumable/missing-tool states and never starts a job on page load.
- [ ] Old fragmented Doctor calls no longer double-render findings.
- [ ] Narrow tuning actions either share calibration infrastructure or have explicit non-overlapping ownership.
- [ ] `bash scripts/check-unused-screenshots.sh` reports no unreferenced promoted screenshots.
- [ ] Documentation/API examples match serialized test fixtures.

### Phase 9 — Rapid-MLX research and backend-owned adapter (separate decision gate)

**Goal:** Evaluate first-class Rapid calibration without borrowing llama.cpp factors.

**Release boundary:** This phase is post-2.0 and is not part of the initial Calibration v1 promise.

This is not automatic follow-on implementation. First produce a source- and installed-runtime-backed qualification matrix for the exact managed Rapid version.

Potential Rapid-owned factors include prefill step size, retained cache, hybrid cache entries, scheduler sequence/admission limits, prefill/completion batch size, GPU memory utilization, and capability-proven speculative settings. Each must come from `RapidMlxConfig`, requested/effective state, `ServeCapabilities`, and the production command builder.

Do not expose factors currently withheld or inert (including TurboQuant/PFlash policies) without per-model receipts. Use Rapid-specific measurement commands and real workload validation; never reuse llama.cpp `batch_size`, `flash_attn`, KV, or `n_cpu_moe` advice.

#### Decision gate

Proceed only if at least two representative models and workloads show a repeatable improvement over current Rapid defaults without correctness or memory-pressure regressions. Otherwise retain Rapid calibration as informational preflight/validation only.

### Phase 10 — Final cross-platform/security/performance acceptance

**Goal:** Prove the complete feature is safe to release.

For 2.0, “complete feature” means the bounded release-blocking scope defined above. Re-run the applicable acceptance gates when any deferred advanced phase later ships.

#### Mandatory checks

Run in the repository-mandated order:

1. `cargo clippy -- -D warnings`
2. `cargo test`
3. `npm run validate-js`
4. `npm run lint`
5. `git diff --check`
6. `cargo build --release`
7. `cargo fmt` and commit any changes
8. `git status`
9. JS module baseline update if required

Also run:

- [ ] `tests/auth_routing.rs` explicitly.
- [ ] Windows GNU `cargo check` for platform/`#[cfg]` changes.
- [ ] Full CI-equivalent isolated Playwright suite on port 17778 with at least a 600-second timeout.
- [ ] Release-built screenshot scenarios sequentially, never in parallel.
- [ ] Security review covering auth, confirmations, process control, path canonicalization, output bounds, secrets, HTML/DOM safety, rate limits, timeouts, symlink refusal, and journal permissions.
- [ ] Cross-cutting verifier for CSS specificity/light/reduced-motion, JS/HTML/CSS references, API contracts, stale refactor code, and backend leakage.
- [ ] Real qualification on macOS Metal, Linux CUDA, Linux ROCm, and Windows before claiming those platforms. Missing hardware means the claim remains gated, not inferred.
- [ ] Long-run suspend/sleep, cancellation, app restart, binary update, application-home migration, and machine-crash recovery drills.

#### Final acceptance criteria

- A user can calibrate a local llama.cpp preset, cancel/resume safely, compare measured Pareto candidates, create a derived preset, validate it, and roll it back.
- No calibration trial mutates a preset or active session before explicit confirmation.
- Every applied setting is typed, capability-gated, introspection-backed, range-validated, and reproducible from a receipt.
- Results are invalidated on hardware/model/runtime/baseline/workload drift.
- No suspected crash configuration is silently retried.
- Rapid never receives llama.cpp advice.
- Doctor never surprises the user with a heavy run.
- The feature uses the configured managed binary bundle under either current or v2.0 application home with no hardcoded branded path.

## 8. Testing matrix

| Layer | Required coverage |
|---|---|
| Pure design | arrays, Morris, conditional staging, refinement, Pareto, effects, noise, budget properties |
| Candidate mapping | dense, MoE, hybrid, MTP, local paths with spaces, invalid combinations, capability drift |
| Measurement | structured JSON, OOM, timeout, crash, malformed/impossible output, thermal/no-sensor, cancellation |
| Persistence | atomicity, truncated journal, restart recovery, pruning, permissions, symlink/traversal, schema migration |
| API/auth | every verb/route, malformed JSON=400, api vs admin token, confirmation, conflict/409, rate/resource limits |
| Preset apply | derived/update, optimistic fingerprint conflict, active session identity, validation, rollback, secret preservation |
| UI | preflight, progress, cancel/resume, result alternatives, stale/noisy, apply review, mobile/light/reduced-motion |
| Backend separation | llama-only factor catalog; Rapid suggestions/patches never flatten or cross-map |
| Platforms | executable suffix, child-tree kill, filesystem permissions, CPU/GPU telemetry degradation, managed binary bundle |
| Real hardware | representative small/large dense, MoE, long context, MTP, CUDA/ROCm/Metal/Windows where available |

## 9. Security and operational checklist

- [ ] Model is a canonical regular GGUF inside an allowed configured model/library root; reject traversal, symlinks, and special files.
- [ ] Binary paths come only from trusted `AppConfig` and exact sibling resolution.
- [ ] No shell invocation, free-form argv, `extra_args`, or arbitrary environment factor input.
- [ ] Every child has timeout, kill-on-drop, bounded output, exact PID/process-tree ownership, and loopback-only networking.
- [ ] One exclusive GPU calibration lease; no competing local server, update, import, or calibration.
- [ ] Expensive start/resume/apply routes are rate-limited and correctly authenticated.
- [ ] High-impact stop/restart/update/delete actions use db-admin-token and exact confirmation/fingerprint.
- [ ] Receipts/logs redact tokens, API keys, authorization, private prompts, and unnecessary absolute paths.
- [ ] Receipt/journal writes are atomic/durable and permission-hardened; schema fields default safely.
- [ ] Cancellation and app shutdown clean children and ports without broad process killing.
- [ ] Predictive memory pruning fails open to measurement when uncertain; it never asserts fit from filename heuristics.
- [ ] Quality/correctness gates outrank throughput.

## 10. Explicit non-goals for the first release

- Automatic model or quant selection.
- Sampling-parameter quality optimization.
- RoPE/YaRN extrapolation tuning.
- Arbitrary user-defined flags or environment variables.
- Automatic multi-GPU tensor placement.
- Remote-agent calibration unless a later design adds job ownership, binary/model fingerprint transport, and remote process control.
- Rapid-MLX parameter translation from llama.cpp.
- Running calibration automatically from Doctor, startup, download, spawn, or preset save.
- Claiming a universal best preset independent of workload/runtime/hardware.

## 11. Upstream references for implementers

At the pinned `llama-optimize` commit, consult these source regions as reference/oracle inputs, not stable imports:

- `README.md`: methodology, factor table, profiles, output/persistence, knob reference.
- `llama-optimize.py:47-171`: binary discovery and preflight.
- `llama-optimize.py:216-756`: hardware/GGUF introspection.
- `llama-optimize.py:757-1015`: config, factors, array selection.
- `llama-optimize.py:1075-1403`: factor registry and safe command construction.
- `llama-optimize.py:1404-1880`: validity checks and bench/server drivers.
- `llama-optimize.py:1881-2456`: Pareto/picks/reporting.
- `llama-optimize.py:2457-2604`: context probe and pick verification.
- `llama-optimize.py:4070-4270`: Morris screening.
- `llama-optimize.py:4271-5035`: CLI/main, persistence, resume, crash journal.
- `docs/CONDITIONAL-FACTORS.md`: conditional stage invariants.
- `docs/DESIGN.md`: verified-context and thermal-drift invariants.
- `docs/measurement-validity.md`: physical plausibility gates.
- `docs/multi-gpu-design.md`: explicit current multi-GPU gap.
- `robust/optimize/taguchi/include/taguchi.h` and `robust/screen/morris/include/morris.h`: documented C contracts for oracle fixtures.

Before copying any implementation, re-check the exact file and license at the pinned commit, record the copied region in attribution, and add a fixture proving our native result against it.
