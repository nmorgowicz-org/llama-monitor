# Rapid-MLX Implementation Manual

This document transforms the high-level roadmap into a comprehensive technical implementation manual. It serves as the primary guide for sub-agents implementing the Rapid-MLX backend.

## Execution and Checkpoint Protocol

Every phase is an independently reviewable checkpoint. A phase is not complete because
its code compiles or its unit tests pass; it is complete only when every hard gate has
named evidence and the Verifier has compared behavior with the source-of-truth
specification and, where applicable, the pre-refactor llama.cpp behavior.

For every phase:

1. **Builder** implements only the phase scope and records files changed, tests run,
   assumptions, and remaining risks.
2. **Verifier** reviews the exact diff against this roadmap and the integration
   specification. The Verifier adds or requests missing regression, contract, security,
   and platform tests and must explicitly sign off or reject the phase.
3. **Coordinator** runs the phase hard gates, removes accidental or stale artifacts,
   updates the roadmap if reality invalidated an assumption, and creates one
   Conventional Commit checkpoint only after sign-off.
4. No later phase may be used to excuse a failed earlier gate. Restore the last healthy
   checkpoint before continuing if a phase regresses an established backend.

Required evidence at each checkpoint:

- targeted tests for the changed contracts;
- `cargo clippy -- -D warnings`, `cargo test`, and `git diff --check`;
- command/HTTP fixtures that prove backend-specific behavior rather than mocks that only
  mirror the implementation;
- a real runtime smoke test whenever the phase changes discovery, launch, readiness,
  process lifecycle, chat routing, or telemetry;
- the screenshot harness and focused Playwright coverage whenever the phase changes UI;
- a concise gate record in the commit message body or implementation handoff.

## Recovery Gate 0: Establish a Trustworthy llama.cpp Baseline

This gate is mandatory when starting from a branch where the backend-neutral refactor
already exists. It must complete before further Rapid-MLX feature work.

- Compare the generated llama.cpp program, argument order, environment, working
  directory, logging, state transitions, readiness, crash handling, and stop behavior
  with the last known-good implementation on `main`.
- Remove placeholder implementations, empty lifecycle callbacks, accidental generated
  files, transcript artifacts, and tests that assert only that code compiles.
- Launch the configured welcome-screen preset against a real local GGUF, wait for
  `/health`, send one chat completion, stop it, and prove the child process exited.
- Verify that launch failure is visible in both application logs and the user flow.
- Commit only after the llama.cpp command-parity tests, lifecycle tests, auth routing
  tests, and real-model smoke test pass.

## Phase 1: Infrastructure & Backend Neutrality

### Phase Objective
Establish a backend-agnostic orchestration layer in `src/inference` to support multiple inference engines (llama.cpp and Rapid-MLX) without leaking backend-specific logic into session or UI code.

### Precise Scope
**Files to Create**:
- `src/inference/mod.rs`: Module root and public API exports.
- `src/inference/backend.rs`: Core `BackendAdapter` enum and interface definitions.
- `src/inference/capabilities.rs`: `CapabilitySet` and `CapabilityProfile` definitions.
- `src/inference/metrics.rs`: Normalized `InferenceMetricsSnapshot` and telemetry types.
- `src/inference/supervisor.rs`: Process supervision logic, `SupervisedLaunch`, and `BackendObserver`.

**Logic to Port/Implement**:
- Move the concept of "server running" and "server stopping" from `src/llama/server.rs` to the `supervisor.rs`.
- Replace the `LlamaMetrics` struct with a backend-neutral `InferenceMetricsSnapshot`.

### Implementation Steps
1. **Define the Backend Identity**: Implement `InferenceBackend` enum with `LlamaCpp` and `RapidMlx` variants.
2. **Implement the Supervisor**:
    - Create `SupervisedLaunch` struct to hold `program`, `args`, `env`, `cwd`, `port`, and `redacted_summary`.
    - Create `BackendObserver` trait with `on_log_line` and `on_crash` methods.
    - Implement the supervisor loop that handles `tokio::process::Child` lifecycle.
3. **Define the Adapter Interface**:
    - Implement `BackendAdapter` enum dispatching to `LlamaCppAdapter` and `RapidMlxAdapter`.
    - Define the required methods: `validate()`, `build_launch()`, `await_ready()`, `poll_metrics()`, `cancel_request()`, and `capabilities()`.
4. **Implement Normalized Telemetry**:
    - In `src/inference/metrics.rs`, create `InferenceMetricsSnapshot` where all telemetry fields (Tps, Memory, etc.) are `Option<T>`.
    - Implement `HealthState` enum (`Ok`, `Degraded`, `NotLoaded`, `Unreachable`).
5. **Establish Capability Mapping**:
    - Implement `CapabilitySet` to track supported features (e.g., `vision`, `mtp`, `cancellation`).

### Hard Gates (Verification)
- **Clippy Compliance**: Run `cargo clippy -- -D warnings`. Zero warnings are allowed.
- **Telemetry Neutrality**: Verify that `InferenceMetricsSnapshot` contains no llama.cpp-specific fields (like `slots_idle`) as top-level requirements; these must be in `backend_details` or omitted.
- **Supervisor Isolation**: Verify that `SupervisedLaunch` is the only way a process is started and that it never logs the `env` vector.
- **Interface Exhaustiveness**: Verify that `BackendAdapter` is an enum and that all match arms are handled in the session layer.
- **Lifecycle Contract**: Tests must prove spawned-child ownership, graceful/forced stop, unexpected-exit reporting, log forwarding, and state cleanup. A compile-only test is insufficient.
- **Checkpoint**: Verifier sign-off followed by `refactor(spawn): establish backend-neutral supervision` (or a corrective `fix(spawn): ...` commit on an already-refactored branch).

### Known Pitfalls & Constraints
- **Async Trait Complexity**: Avoid `async_trait` if possible; use enum dispatch for zero-overhead and better compile times.
- **Observer Lifecycle**: `BackendObserver` must be `Send + Sync + 'static` to be safely moved into the supervisor's log-streaming tasks.

---

## Phase 2: Llama.cpp Adapter Port

### Phase Objective
Migrate existing llama.cpp logic from `src/llama` into the new adapter architecture to ensure no regressions in behavior.

### Precise Scope
**Files to Create**:
- `src/inference/llama_cpp.rs`: The full implementation of `LlamaCppAdapter`.

**Files to Modify**:
- `src/llama/server.rs`: Replace `start_server` and `stop_server` with `BackendAdapter` calls.
- `src/llama/poller.rs`: Replace `llama_metrics_poller` with `BackendAdapter::poll_metrics` calls.
- `src/llama/metrics.rs`: Port `LlamaMetrics` and parsing logic to `src/inference/metrics.rs` and `llama_cpp.rs`.

**Logic to Port**:
- **Command Construction**: Port `append_fit_args`, `append_kv_cache_args`, and the main `TokioCommand` builder from `src/llama/server.rs` to `LlamaCppAdapter::build_launch`.
- **Readiness Polling**: Port the `/health` check logic from `src/llama/poller.rs` to `LlamaCppAdapter::await_ready`.
- **Metrics Polling**: Port the Prometheus (`/metrics`) and Slot (`/slots`) parsing logic to `LlamaCppAdapter::poll_metrics`.

### Implementation Steps
1. **Implement `LlamaCppAdapter`**: Create the struct and implement the `BackendAdapter` interface.
2. **Port Command Builder**:
    - Implement `build_launch` to replicate the exact argument sequence in `src/llama/server.rs:413-802`.
    - Ensure `redacted_summary` accurately describes the model and port.
3. **Port Readiness Logic**:
    - Implement `await_ready` to poll `/health` and verify a success response.
4. **Port Metrics Logic**:
    - Implement `poll_metrics` to fetch `/metrics` and `/slots`.
    - Map `LlamaMetrics` fields to the normalized `InferenceMetricsSnapshot`.
5. **Refactor the Session Layer**:
    - Update `src/llama/server.rs` and `src/llama/poller.rs` to use the `BackendAdapter::LlamaCpp` variant.

### Hard Gates (Verification)
- **Clippy Compliance**: Run `cargo clippy -- -D warnings`. Zero warnings are allowed.
- **Command Parity**: Verify that `LlamaCppAdapter::build_launch` produces a command identical to the one currently generated in `src/llama/server.rs`.
- **Telemetry Parity**: Verify that `LlamaCppAdapter::poll_metrics` returns a snapshot that results in the same dashboard values as the current `poller.rs`.
- **Behavioral Integrity**: Verify that llama.cpp servers still spawn, reach readiness, and respond to chat requests without changes in timing or output.
- **State Parity**: Verify `server_running`, `local_server_running`, active session status, persisted server config, PID/process ownership, logs, crash reason, and stop cleanup match the pre-refactor behavior.
- **Environment Parity**: Assert NVIDIA, ROCm, macOS quarantine/working-directory behavior, and Windows no-window handling remain intact where applicable.
- **Real Preset Smoke Test**: Start the existing welcome-screen preset with a real GGUF, wait for health, make one chat request, stop it, and assert the port and process are gone.
- **Checkpoint**: Verifier sign-off followed by a focused `fix(spawn): restore llama.cpp lifecycle parity` or `refactor(spawn): port llama.cpp backend adapter` commit.

### Known Pitfalls & Constraints
- **SillyTavern Integration**: Ensure that the shift to the supervisor does not interrupt prompt forwarding or the existing SSE `model_status` updates.
- **Path Lookups**: Maintain the existing `app_config.llama_server_path` validation logic during the port.

---

## Phase 2.5: Persisted Backend Routing & Shared Launch Entry Point

### Phase Objective

Make backend identity a backward-compatible persisted product contract and route every
local launch surface through one backend-neutral entry point before implementing more
Rapid-MLX-specific behavior.

### Precise Scope

- Add a serde-defaulted backend discriminator to presets, spawned sessions, API payloads,
  and restored-session state. Missing values from existing files must resolve to
  `llama_cpp` without rewriting or dropping user data.
- Define one backend-neutral launch request/resolved-launch contract. Welcome preset
  cards, preset editor, spawn wizard, direct spawn API, restart/update, benchmark, and
  restored sessions must not independently reconstruct backend-specific configuration.
- Construct `BackendAdapter::LlamaCpp` or `BackendAdapter::RapidMlx` only inside the
  shared launch service. HTTP handlers and JavaScript must not instantiate or emulate
  adapter-specific decisions.
- Persist selected backend and resolved model identity on the session so polling, chat,
  diagnostics, stop, and restore can route through the same engine.

### Hard Gates (Verification)

- **Backward Compatibility**: Load existing presets/sessions with no backend field and
  prove they launch as llama.cpp with no destructive migration.
- **Round Trip**: Save, reload, edit, duplicate, and launch both backend variants without
  losing backend-specific configuration.
- **Surface Parity**: Contract tests cover welcome card, wizard, preset modal, direct API,
  restart/update, benchmark, and restored-session routing through the shared service.
- **No Dead Adapter**: A production test must prove that selecting Rapid-MLX constructs
  `BackendAdapter::RapidMlx`; selecting llama.cpp constructs `BackendAdapter::LlamaCpp`.
- **Auth and Errors**: Invalid backend/model combinations return authenticated 400-class
  errors, never 404, silent fallback, or user-data deletion.
- **Checkpoint**: Verifier sign-off followed by
  `refactor(spawn): unify backend launch routing`.

### Known Pitfalls & Constraints

- The backend default belongs in Rust serde/domain logic, not duplicated JavaScript.
- Do not flatten every backend flag into one growing shared struct. Persist a small shared
  envelope plus backend-owned configuration.
- An explicit user engine choice always wins over recommendation logic.

---

## Phase 3: Rapid-MLX Basic Launch & Readiness

### Phase Objective
Implement the ability to discover the Rapid-MLX runtime, construct its launch command, and verify its readiness.

### Precise Scope
**Files to Create**:
- `src/inference/rapid_mlx/mod.rs`: `RapidMlxAdapter` implementation.
- `src/inference/rapid_mlx/command.rs`: Command construction for `rapid-mlx serve`.
- `src/inference/rapid_mlx/runtime.rs`: Runtime identification and metadata.
- `src/inference/rapid_mlx/discovery.rs`: Logic to find `rapid-mlx` on `PATH` or in managed envs.

**Logic to Implement**:
- Platform validation (Apple Silicon macOS only).
- Command construction: `rapid-mlx serve <model-or-path>`.
- Readiness check: Poll `/health/ready` for HTTP 200.

### Implementation Steps
1. **Implement Runtime Discovery**:
    - In `discovery.rs`, implement logic to resolve the `rapid-mlx` binary via explicit path, managed env, or `PATH`.
    - Implement a core probe (`rapid-mlx --version`) to verify the binary is usable.
2. **Implement Command Construction**:
    - In `command.rs`, implement the builder for `rapid-mlx serve`.
    - Ensure all baseline flags (host, port, etc.) from the specification are supported.
3. **Implement the Adapter**:
    - Create `RapidMlxAdapter` and implement `validate()` (check macOS + Silicon).
    - Implement `build_launch()` using the command builder.
4. **Implement Readiness Polling**:
    - Implement `await_ready()` to poll `/health/ready`.
    - **Critical**: Do not mark ready based on TCP port open; only on HTTP 200 from `/health/ready`.

### Hard Gates (Verification)
- **Clippy Compliance**: Run `cargo clippy -- -D warnings`. Zero warnings are allowed.
- **Platform Gate**: Verify that `RapidMlxAdapter::validate()` returns an error on Linux or Windows.
- **Binary Resolution**: Verify that the adapter correctly identifies `rapid-mlx` installations from Homebrew and Pip.
- **Readiness Accuracy**: Verify that the session is marked "Ready" only after `/health/ready` returns 200, not when the process first spawns.
- **Lifecycle Integration**: A fixture runtime must prove discovery -> validation -> spawn -> loading -> ready -> stop and an early-exit fixture must prove actionable failure propagation.
- **Real Runtime Smoke Test**: On Apple Silicon, probe the installed Rapid-MLX CLI and launch one compatible small model or explicitly record the external runtime/model blocker. Mock-only evidence cannot mark this phase complete.
- **Checkpoint**: Verifier sign-off followed by `feat(spawn): add Rapid-MLX launch and readiness`.

### Known Pitfalls & Constraints
- **Binary Aliases**: Do not hardcode `vllm-mlx`; detect and use whichever binary is on `PATH` (preferring `rapid-mlx`).
- **Timeout Handling**: Rapid-MLX warmup (JIT/Cache) can be slow; ensure `await_ready` has a generous timeout (e.g., 300s).

---

## Phase 4: Telemetry Normalization & Dashboard Cards

### Phase Objective
Implement normalized metrics polling for Rapid-MLX and update the UI to render cards dynamically based on available telemetry.

### Precise Scope
**Files to Create**:
- `src/inference/rapid_mlx/poller.rs`: Rapid-MLX specific telemetry fetching logic.

**Files to Modify**:
- `src/inference/metrics.rs`: Update normalization logic.
- `static/js/` (Dashboard components): Update card rendering logic.

**Logic to Implement**:
- Polling of `/v1/status` and `/v1/cache/stats`.
- Conversion of Rapid-MLX GB values to bytes for the normalized snapshot.
- Dynamic UI card mounting based on `Option<T>` presence.

### Implementation Steps
1. **Implement the Rapid-MLX Poller**:
    - In `rapid_mlx/poller.rs`, implement logic to fetch JSON from `/v1/status` and `/v1/cache/stats`.
    - Map `generation_tps` and `prompt_tps` directly to the normalized snapshot.
2. **Perform Unit Conversion**:
    - Convert `metal.active_memory_gb`, `peak_memory_gb`, and `cache_memory_gb` to bytes by multiplying by `1_073_741_824`.
3. **Update the Frontend Card Registry**:
    - Modify the JS dashboard to check for the presence of required metrics before mounting a card.
    - Implement the "Stale Data" policy: keep a card for 3 poll intervals after the metric disappears, then remove it.
4. **Integrate Polling**:
    - Wire `RapidMlxAdapter::poll_metrics` into the shared poller loop.

### Hard Gates (Verification)
- **Clippy Compliance**: Run `cargo clippy -- -D warnings`. Zero warnings are allowed.
- **Unit Accuracy**: Verify that a `metal.active_memory_gb` value of `1.0` in the API results in `1073741824` bytes in the normalized snapshot.
- **UI Dynamism**: Verify that when switching from llama.cpp to Rapid-MLX, llama.cpp-specific cards (e.g., Slot occupancy) are removed from the DOM, not just hidden.
- **Zero vs None**: Verify that `generation_tps: 0.0` renders as "0", while a missing field results in no throughput card.
- **Schema Drift Fixtures**: Cover unknown fields, missing optional objects, authenticated status, text-only cache fallback, malformed payloads, and transient endpoint failure.
- **Rendered UI Evidence**: Run `cargo build --release` and the matching dashboard screenshot scenario; verify dark/light themes and reduced motion, and ensure unsupported cards are removed from the DOM.
- **Checkpoint**: Verifier sign-off followed by `feat(ui): render backend-capability telemetry`.

### Known Pitfalls & Constraints
- **Telemetry Schema**: Rapid-MLX telemetry may vary by release; use tolerant JSON parsing that ignores unknown fields.
- **Card Symmetry**: Do not maintain a "grid" of cards; the UI must reflow cleanly when cards are added or removed.

---

## Phase 5: Model Resolution & GGUF Conversion

### Phase Objective
Implement the Rapid-MLX model source resolver to support MLX directories, Hugging Face repos, and automated GGUF conversion.

### Precise Scope
**Files to Create**:
- `src/inference/rapid_mlx/model_resolver.rs`: The full `RapidMlxModelSource` resolution pipeline.

**Logic to Implement**:
- Resolver pipeline: `Input` $\rightarrow$ `Validate` $\rightarrow$ `Convert (if GGUF)` $\rightarrow$ `ResolvedLaunchModel`.
- Orchestration of the `gguf2mlx` tool.
- Conversion cache management.

### Implementation Steps
1. **Define Model Sources**: Implement the `RapidMlxModelSource` enum (`MlxDirectory`, `HuggingFaceRepo`, `Alias`, `GgufFile`).
2. **Implement the Resolution Pipeline**:
    - Validate local MLX directories and HF repo IDs.
    - Resolve aliases via the runtime catalog.
3. **Implement GGUF Conversion**:
    - When a `.gguf` file is provided, check the conversion cache.
    - If not cached, invoke `gguf2mlx` in a subprocess.
    - Store the resulting MLX directory in the cache.
4. **Implement the Resolved Output**:
    - Create `ResolvedRapidMlxLaunchModel` which contains the final path to pass to `rapid-mlx serve`.
5. **Wire to Adapter**: Ensure `RapidMlxAdapter::build_launch` consumes only the `ResolvedRapidMlxLaunchModel`.

### Hard Gates (Verification)
- **Clippy Compliance**: Run `cargo clippy -- -D warnings`. Zero warnings are allowed.
- **Conversion Flow**: Verify that selecting a `.gguf` file for Rapid-MLX triggers the `gguf2mlx` process and launches the server using the converted directory.
- **Cache Efficiency**: Verify that a second launch of the same GGUF file skips the conversion step and uses the cache.
- **Resolver Isolation**: Verify that the `RapidMlxAdapter` does not know about `gguf2mlx`; it only sees the `ResolvedRapidMlxLaunchModel`.
- **Failure Safety**: Prove insufficient disk, interrupted conversion, duplicate conversion, stale sentinel, invalid output, and cache-key changes cannot yield a launchable corrupt cache entry.
- **End-to-End Resolution**: Resolve and launch at least one local MLX directory and one supported HF identifier; exercise GGUF conversion when the installed runtime exposes `gguf2mlx`.
- **Checkpoint**: Verifier sign-off followed by `feat(models): resolve Rapid-MLX model sources`.

### Known Pitfalls & Constraints
- **Disk Space**: Implement a pre-check for disk space before starting a GGUF conversion.
- **Sentinel Files**: Use `.converting` sentinel files to prevent concurrent conversions of the same model.

---

## Phase 6: UI/UX Polish & Management

### Phase Objective
Integrate backend selection into the Spawn Wizard and implement full Rapid-MLX runtime management in Settings.

### Precise Scope
**Files to Modify**:
- `static/js/` (Spawn Wizard, Settings, Nav Bar).
- `src/inference/backend.rs`: Deterministic recommendation logic.

**Files to Create**:
- `src/inference/rapid_mlx/updater.rs`: Installation and upgrade logic.

**Logic to Implement**:
- Engine selection cards in the wizard.
- Active engine indicator in the nav bar.
- Runtime management via `uv tool install`.
- Deterministic engine recommendation.

### Implementation Steps
1. **Implement the Runtime Manager**:
    - In `updater.rs`, implement `install`, `upgrade`, and `repair` using `uv tool install rapid-mlx`.
    - Implement a version manager that fetches releases from GitHub.
2. **Update the Spawn Wizard**:
    - Add "Engine" selection cards.
    - Implement shared value preservation (temperature, port) when switching engines.
    - Add the "Memory Safety Warning" for >75% unified memory usage.
3. **Implement the Nav Bar Indicator**:
    - Create the indicator: `Engine · Model ●`.
    - Wire it to update based on the active session's backend and model.
4. **Implement Recommendation Logic**:
    - GGUF file $\rightarrow$ recommend llama.cpp.
    - MLX dir/HF repo $\rightarrow$ recommend Rapid-MLX.
    - No selection on macOS $\rightarrow$ recommend Rapid-MLX (if probe passes).

### Hard Gates (Verification)
- **Clippy Compliance**: Run `cargo clippy -- -D warnings`. Zero warnings are allowed.
- **Round-trip Presets**: Verify that a preset saved under Rapid-MLX can be opened, edited, and launched without losing backend-specific flags.
- **Visual Indicator**: Verify that the nav bar correctly displays `Rapid-MLX · <model> ●` only when a Rapid-MLX session is active.
- **Runtime Isolation**: Verify that installing a new Rapid-MLX version via the manager does not mutate the environment of a currently running server.
- **Unified Flow**: Welcome preset cards, preset editor, spawn wizard, restored sessions, and direct spawn API must all preserve and route the backend discriminator through one shared validation path.
- **Action Feedback**: Launch, conversion, installation, loading, ready, stopping, and failure states must be visible and actionable; no native `alert`, `confirm`, or `prompt` dialogs are permitted.
- **Visual Verification**: Run `cargo build --release`, the relevant screenshot scenarios, dark/light theme checks, reduced-motion checks, and the isolated CI-equivalent Playwright suite.
- **Checkpoint**: Verifier sign-off followed by `feat(ui): add unified inference engine experience`.

### Known Pitfalls & Constraints
- **User Overrides**: An explicit user choice of engine must never be overridden by the recommendation logic.
- **Managed Environments**: Ensure that `uv` managed environments are stored in the dedicated `~/.config/llama-monitor/runtimes/rapid-mlx/` directory.

---

## Phase 7: Cross-Backend Release Gate

### Phase Objective

Prove that the feature is releasable as one coherent multi-engine product and that
llama.cpp remains a first-class, regression-free backend.

### Hard Gates (Verification)

- Run the mandatory pre-PR checks in `AGENTS.md` in their exact order.
- Run the isolated full Playwright suite and all auth routing tests.
- Run real llama.cpp preset launch -> ready -> chat -> telemetry -> stop.
- Run real Rapid-MLX discovery -> launch -> ready -> chat -> telemetry -> stop on a
  supported Apple Silicon host, or keep the PR draft with the missing evidence named.
- Verify preset round trips for both backends and backward compatibility for presets and
  sessions that have no backend discriminator.
- Complete the security checklist: authentication, secret handling, path validation,
  subprocess arguments/environment, timeouts/rate limits, and diagnostics redaction.
- Inspect the repository tree and diff for transcript files, generated junk, placeholder
  comments, dead adapters, stale compatibility branches, or unrelated changes.
- Capture final welcome, spawn-wizard, running dashboard, settings/runtime-management,
  and failure-state screenshots. Record any UX follow-ups that are intentionally deferred.
- Create the final checkpoint only after Verifier sign-off; do not add the
  `ready-to-test` label.
