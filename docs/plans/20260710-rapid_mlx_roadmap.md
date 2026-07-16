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
- Compatibility profile: probe `rapid-mlx --version` and exact tokens from
  `rapid-mlx serve --help` before constructing optional arguments. The first verified
  profile is release `0.10.9` (tag SHA `3edb3ac69c1d1c5e81836a5d146e5f81048658d9`).

### Implementation Steps
1. **Implement Runtime Discovery**:
    - In `discovery.rs`, implement logic to resolve the `rapid-mlx` binary via explicit path, managed env, or `PATH`.
    - Implement a core probe (`rapid-mlx --version`) to verify the binary is usable.
2. **Implement Command Construction**:
    - In `command.rs`, implement the builder for `rapid-mlx serve`.
    - Ensure all core flags (host, port, etc.) from the active compatibility profile are supported.
    - For `0.10.9`, use `--timeout` and `--max-cache-blocks`; the older
      `--request-timeout` and `--max-blocks` names are not valid.
    - Default to loopback (`127.0.0.1`). LAN exposure must be explicit and must pass
      the same authentication and warning policy as llama.cpp.
    - Pass API secrets through `RAPID_MLX_API_KEY`, never argv.
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
- **Capability Probe**: Verify required core tokens from `serve --help`; omit an
  optional flag only when its desired value is the upstream default, and reject an
  explicitly configured unsupported flag with an actionable pre-spawn error.
- **Readiness Accuracy**: Verify that the session is marked "Ready" only after `/health/ready` returns 200, not when the process first spawns.
- **Lifecycle Integration**: A fixture runtime must prove discovery -> validation -> spawn -> loading -> ready -> stop and an early-exit fixture must prove actionable failure propagation.
- **Graceful Stop**: Send SIGTERM, wait up to 10 seconds for Rapid-MLX to drain and
  persist cache state, then use a hard-kill fallback. SIGHUP is diagnostic and must
  never be used as a stop signal.
- **Real Runtime Smoke Test**: On Apple Silicon, probe the installed Rapid-MLX CLI and launch one compatible small model or explicitly record the external runtime/model blocker. Mock-only evidence cannot mark this phase complete.
- **Checkpoint**: Verifier sign-off followed by `feat(spawn): add Rapid-MLX launch and readiness`.

### Known Pitfalls & Constraints
- **Binary Aliases**: Do not hardcode `vllm-mlx`; detect and use whichever binary is on `PATH` (preferring `rapid-mlx`).
- **Timeout Handling**: Rapid-MLX warmup (JIT/Cache) can be slow; ensure `await_ready` has a generous timeout (e.g., 300s).
- **Daily Upstream Movement**: A version string alone is not a compatibility contract.
  The managed stable channel accepts official stable releases at or above its minimum
  version only after required live capability probes pass; it does not require a
  llama-monitor source allowlist update per Rapid-MLX release. Current `main` and other
  prerelease/local builds are provisional user-owned runtimes, never the default
  managed install target.

---

## Phase 3 Checkpoint and Handoff — 2026-07-15

**Status: Complete. Stop point requested before Phase 3.5.**

Phase 3 passed the Builder → Verifier gate. The implementation now provides:

- explicit path, managed-path, `rapid-mlx`, and deprecated `vllm-mlx` discovery with
  source classification;
- exact stable managed `0.10.9` verification and live `--version` plus
  `serve --help` capability probing;
- Provisional handling for compatible external/newer/nightly/local runtimes, while
  older versions and missing core flags fail closed;
- five-second probe deadlines, child cleanup, concurrent stdout/stderr draining, and
  256 KiB per-stream output limits;
- secure loopback defaults and an API-key requirement for non-loopback binds;
- current `--timeout` and `--max-cache-blocks` mappings, with configured unsupported
  options rejected before spawn;
- `RAPID_MLX_API_KEY` secret transport, authenticated operational polling, and no
  secret persistence, argv exposure, or diagnostic leakage;
- `RAPID_MLX_TELEMETRY=0` on app-managed launches so first-run consent cannot block an
  unattended spawn;
- `/health/ready` HTTP-200 readiness and backend-neutral early-exit propagation;
- Unix SIGTERM with a ten-second drain window and SIGKILL fallback, while preserving
  the Windows `taskkill /F` implementation.

### Verification evidence

- Independent verifier sign-off with `cargo test`: 780 passed, 6 ignored.
- `cargo clippy -- -D warnings`, `cargo check`, formatting/whitespace checks, and
  `cargo check --target x86_64-pc-windows-gnu` passed.
- Fixture coverage includes bounded/hung probes, stable/prerelease/local version
  classification, discovery → probe → spawn → loading → ready → stop, actionable
  exit-before-ready logs, correct/wrong/missing API keys, SIGTERM, and forced SIGKILL.
- Real Apple Silicon runtime:
  - Python `3.12.13`;
  - Rapid-MLX `0.10.9` at
    `~/.config/llama-monitor/runtimes/rapid-mlx/0.10.9/venv/bin/rapid-mlx`;
  - `mlx-community/Qwen3-0.6B-4bit`, downloaded to the normal Hugging Face cache.
- Rebuilt llama-monitor app smoke on isolated ports `17779`/`18081`:
  - direct `backend: rapid_mlx` spawn succeeded;
  - the recorded command contained only `serve`, model, `--host 127.0.0.1`, and
    `--port 18081`;
  - `/health/ready` returned HTTP 200 with `ready: true`;
  - `/v1/status` returned the expected model identity;
  - the authenticated app stop route triggered the Rapid-MLX prefix-cache save path
    and a clean intentional supervisor exit.

An initial app smoke accidentally used a release binary that finished rebuilding after
the smoke process had already started and therefore reproduced the stale
`--request-timeout`/`--max-blocks` failure. The process start time and binary mtime
identified the stale executable. Re-running with the completed build produced the
successful evidence above; current source and final binary contain only the verified
flag names.

### Resume point

Do not redo Phase 3. Start at **Phase 3.5: Chat Routing, Streaming & Cancellation**.
That phase must route the real llama-monitor chat path through the active backend,
authenticate `/v1/*` requests, filter backend-specific request fields, validate SSE
reasoning/usage shapes, and implement capability-aware cancellation. Do not fold the
later runtime installer/version-manager UI (Phase 6), telemetry-card completion
(Phase 4), model resolver (Phase 5), or Experimental GGUF Import Lab (Phase 5.5) into
Phase 3.5.

---

## Phase 3.5: Chat Routing, Streaming & Cancellation

### Phase Objective

Prove that a ready Rapid-MLX session is usable through llama-monitor's real chat path,
including authenticated requests, streaming, errors, disconnects, and capability-aware
cancellation. Launch success alone is not backend integration.

### Precise Scope

- Route chat, model discovery, and diagnostics from the active session's persisted
  backend/model identity instead of assuming llama.cpp.
- Map the shared request contract only to Rapid-MLX controls advertised by the active
  compatibility profile. Do not send llama.cpp-only fields.
- Support OpenAI `/v1/chat/completions` streaming and bounded non-streaming probes.
- Pass `RAPID_MLX_API_KEY` to the process and authenticate app-to-runtime `/v1/*`
  requests without logging, echoing, or persisting plaintext secrets in session JSON.
- Implement backend-native request cancellation only when the runtime advertises a
  compatible endpoint/request-ID contract. Client disconnect must always stop local
  forwarding work.

### Hard Gates (Verification)

- **Fixture Chat**: A CI-safe fixture proves request mapping, SSE chunk parsing,
  terminal usage, reasoning/tool fields when advertised, malformed chunks, upstream
  non-2xx errors, and disconnect behavior.
- **Auth**: No-key, correct-key, and wrong-key fixtures prove readiness remains usable
  while protected `/v1/*` calls use the configured secret correctly.
- **No Cross-Backend Leakage**: llama.cpp requests remain byte-for-byte compatible and
  Rapid-MLX never receives llama.cpp-only generation or model-load controls.
- **Cancellation**: Supported cancellation is tested end to end; unsupported runtimes
  degrade explicitly without treating the server as failed.
- **Rapid-MLX 0.10.9 note**: its public SSE `chatcmpl-*` ID is not the private scheduler
  ID required by the advertised cancellation routes. Keep native cancellation disabled
  for this profile and prove immediate disconnect-driven scheduler abort instead.
- **Real Chat Smoke**: On Apple Silicon, launch a small verified model, wait for ready,
  obtain an exact deterministic response through llama-monitor's chat route, then stop.
- **Checkpoint**: Verifier sign-off followed by
  `feat(chat): route Rapid-MLX conversations and cancellation`.

---

## Phase 3.5 Checkpoint and Handoff — 2026-07-15

**Status: Complete. Stop point requested before Phase 4.**

Phase 3.5 passed the Builder → Verifier gate. The implementation now routes main chat,
guided chat, notes, suggestions, and benchmarks through the active backend while
preserving llama.cpp request bodies byte-for-byte. Rapid-MLX requests are filtered by
the active compatibility profile, translate `repeat_penalty` to
`repetition_penalty`, inject the persisted model identity, and use transient bearer
authentication without persisting or logging runtime secrets.

The proxy accepts bounded streaming and non-streaming responses, preserves reasoning,
tool, and usage fields, handles malformed or unterminated SSE input, and stops local
forwarding immediately when the browser disconnects. Attach and restored-session flows
now retain explicit backend and model identity, with legacy attach payloads continuing
to default to llama.cpp.

### Verification evidence

- Independent verifier sign-off covered 42 Rapid-focused tests, 7 upstream contract
  tests, 6 stream tests, and authenticated Rapid attach/model-discovery fixtures.
- Full repository gates passed: `cargo clippy -- -D warnings`; `cargo test` with 817
  passed and 6 ignored; JavaScript syntax and lint checks; release build; whitespace
  checks; and `cargo check --target x86_64-pc-windows-gnu`.
- Isolated Playwright completed with 201 passed, 1 skipped, and 1 transient module-count
  flake; the focused rerun passed at the unchanged 55-module baseline. The welcome
  screenshot harness also passed with the Rapid-MLX attach state.
- Real Apple Silicon app smoke used llama-monitor on `127.0.0.1:17779`, verified
  managed Rapid-MLX `0.10.9` on `127.0.0.1:18081`, and
  `mlx-community/Qwen3-0.6B-4bit`.
- `/api/sessions/spawn` reached ready state with persisted `backend: rapid_mlx` and the
  expected model identity. A deterministic streaming request sent through
  llama-monitor's `/api/chat` returned exactly `PHASE35_OK`.
- The terminal dedicated usage chunk reported 26 prompt tokens, 6 completion tokens,
  and 32 total tokens. `/api/capabilities` reported the verified Rapid feature profile
  with native cancellation disabled.
- The authenticated stop path saved Rapid-MLX's prefix cache and produced a clean
  intentional supervisor exit. Both isolated processes were stopped after the smoke.

### Cancellation compatibility decision

Rapid-MLX `0.10.9` exposes cancellation routes, but its public SSE `chatcmpl-*` ID is
not the private scheduler ID those routes require. Native cancellation therefore
remains disabled for the verified profile. Stop-generation closes the upstream
response immediately, and Rapid-MLX's disconnect guard aborts scheduler work. Do not
enable native cancellation until a future runtime exposes and documents a public ID
contract that is proven end to end.

### Security follow-up

This checkpoint corrected the attach allowlist's RFC1918 `172.16.0.0/12` range. Attach
URLs that use DNS hostnames still need a separate DNS-resolution and address-pinning
design to prevent rebinding while preserving friendly LAN hostnames; track that as a
security hardening item rather than silently rejecting existing hostname-based setups.

### Resume point

Do not redo Phase 3 or Phase 3.5. Commit this checkpoint as
`feat(chat): route Rapid-MLX conversations and cancellation`, then begin **Phase 4:
Telemetry Normalization & Dashboard Cards** only when explicitly requested. Keep the
Phase 5 model resolver, Phase 5.5 Experimental GGUF Import Lab, and Phase 6 runtime
installer/version manager out of Phase 4.

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
    - Send the configured API key to authenticated `/v1/*` endpoints and require a
      successful HTTP status before parsing JSON. Keep readiness on `/health/ready`.
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

## Phase 4 Checkpoint and Handoff — 2026-07-15

**Status: Complete. Stop point requested before Phase 5.**

Phase 4 passed the Builder → Verifier gate after a corrective audit. The shared poller
now produces backend-neutral inference snapshots for spawned and attached Rapid-MLX
sessions. It authenticates `/health`, `/v1/status`, and optional `/v1/cache/stats`
requests, applies endpoint-specific deadlines and body limits, converts GiB values
exactly, and tolerates missing or future fields without converting absence into zero.
Only semantically recognized cache, request-identity, and progress fields cross the
WebSocket boundary; opaque request content and unknown cache payloads are discarded.

The dashboard now mounts cards from available capabilities instead of preserving a
llama.cpp-shaped grid. Rapid-MLX can render runtime, throughput, queue, Metal runtime
memory, prefix/cache, totals, recognized request activity, and accessible live progress.
Unsupported llama.cpp cards are removed from the DOM and restored on backend switch.
Cards preserve real zero values, show a last-seen age while stale, disappear after the
third missing poll, and reset between sessions. Failed polling leaves runtime identity
visible as degraded and cannot repeatedly flip a disconnected session back online.

### Verification evidence

- Independent verification first rejected eight contract gaps. Corrections added
  stable failure hysteresis, per-session failure reset, explicit bind-host/IPv6 polling,
  degraded runtime state, strict runtime-card requirements, an ARIA progress bar,
  semantic cache normalization, a generic-only WebSocket contract, and last-seen age.
- Rapid poller fixtures cover authentication, unknown/missing fields, malformed required
  status, optional cache failure, invalid cache shapes, stripped opaque request fields,
  exact GiB conversion, zero-versus-missing values, and recognized multimodal cache.
- Focused dashboard Playwright covers capability mounting, XSS-safe text, true zero
  rendering, three-poll staleness, session reset, DOM restoration, degraded telemetry,
  and progress-bar semantics.
- The screenshot harness produced and visually verified dark, light, and partial
  `dashboard-rapid-mlx` compositions. The cards use theme tokens, responsive auto-fit,
  and reduced-motion behavior.
- Real Apple Silicon smoke used llama-monitor on `127.0.0.1:17779`, Rapid-MLX `0.10.9`
  on `127.0.0.1:18081`, and `mlx-community/Qwen3-0.6B-4bit`. Live DOM values included
  zero throughput/queue, Metal allocation, cache entries, and zero cumulative totals;
  llama.cpp generation, context, and slot cards were absent. Both processes stopped
  cleanly with Rapid-MLX prefix-cache persistence.
- Full repository clippy, Rust tests, JavaScript validation/lint, whitespace check,
  release build, formatting, isolated Playwright, and Windows cross-check gates passed.

### Resume point

Do not redo Phase 4. Commit this checkpoint as
`feat(ui): render backend-capability telemetry`, then begin **Phase 5: Native MLX and
Authoritative Safetensors Resolution** only when explicitly requested. Phase 5.5 GGUF
import exploration and Phase 6 runtime management remain separate later gates.

---

## Phase 5: Native MLX and Authoritative Safetensors Resolution

### Phase Objective
Implement the Rapid-MLX model source resolver for native aliases, native MLX Hugging
Face repositories, local MLX directories, and authoritative original/merged HF
safetensors. Rapid-MLX `0.10.9` does not load GGUF. GGUF-only recovery is handled by
the separate experimental Phase 5.5 gate.

### Precise Scope
**Files to Create**:
- `src/inference/rapid_mlx/model_resolver.rs`: The full `RapidMlxModelSource` resolution pipeline.

**Structured model library**:

The configured `models_dir` is the single user-visible model-library root. New installs
use this backend-neutral layout:

```text
models/
├── gguf/                       # llama.cpp models and mmproj companions
├── mlx/
│   ├── native/                 # imported/user-managed native MLX directories
│   └── converted/              # immutable, manifest-backed official conversions
├── transformers/              # complete local HF/safetensors source models
├── cache/
│   └── huggingface/
│       ├── hub/                # server-delegated HF snapshots/blobs
│       └── xet/                # Xet-backed content when used
└── .staging/                   # incomplete downloads/conversions; never launchable
```

- Existing root-level model files remain discoverable for backward compatibility.
- Migration is explicit, same-filesystem, restartable, and journaled. It rewrites
  persisted preset, session, tag, draft-model, and mmproj paths atomically only after
  each move succeeds. It never silently drops an unknown reference or moves a file
  outside the configured model-library/config roots.
- Root-level complete `.gguf`/mmproj files move to `gguf/`; incomplete `.part` files
  move to `.staging/downloads/`. Existing large libraries are not copied.
- App-launched Rapid-MLX and official HF download tools receive `HF_HUB_CACHE` and
  `HF_XET_CACHE` pointing inside this library. `HF_TOKEN` remains transient and the
  Hugging Face cache remains content-addressed rather than duplicated into `mlx/`.
- Migration may explicitly import selected app-referenced repositories from the user's
  previous default Hugging Face cache. It must not relocate the entire shared cache or
  unrelated repositories owned by other applications.
- The model library and selectors give every discovered item first-class badges for
  runtime format, source, lifecycle state, and compatibility. GGUF, mmproj, native
  MLX, converted MLX, Transformers/safetensors sources, HF-cached snapshots, and
  incomplete/staged items must not fall back to filename-only or second-class rows.

**Logic to Implement**:
- Resolver pipeline: `Input` $\rightarrow$ `Validate` $\rightarrow$ `ResolvedLaunchModel`.
- Official `mlx_lm.convert` orchestration for authoritative original/merged HF
  safetensors, with optional one-time MLX quantization.
- Provenance and cache management keyed by source revision/hash, mlx-lm version, and
  quantization recipe.
- The first verified converter profile is official `mlx-lm==0.31.3`. Conversion fails
  closed when the selected runtime environment does not prove that exact package and
  the required `python -m mlx_lm convert` flag contract.

### Implementation Steps
1. **Define Model Sources**: Implement the `RapidMlxModelSource` enum
   (`MlxDirectory`, `HuggingFaceRepo`, `Alias`, `AuthoritativeSafetensors`,
   `GgufFile`). `GgufFile` preserves the input; `unsupported_source` is the resolver
   outcome and never a launchable model kind.
2. **Implement the Resolution Pipeline**:
    - Validate local MLX directories and HF repo IDs.
    - Resolve aliases via the runtime catalog.
3. **Convert Authoritative Safetensors**:
    - Use pinned official `mlx_lm.convert` for a local full HF model directory or
      revision-pinned HF repository. LoRA input requires a known base plus adapter,
      an authoritative merge step, and a saved full safetensors model first.
    - Support FP16/BF16 output and optional MLX 4/6/8-bit recipes exposed by the pinned
      mlx-lm version. Never claim that one quant recipe preserves another format's
      block/imatrix semantics.
4. **Handle GGUF Explicitly**:
    - Keep llama.cpp as the default recommendation for `.gguf` and route opt-in import
      to Phase 5.5. Never silently treat GGUF as native MLX input.
5. **Implement the Resolved Output**:
    - Create `ResolvedRapidMlxLaunchModel` which contains the final path to pass to `rapid-mlx serve`.
6. **Wire to Adapter**: Ensure `RapidMlxAdapter::build_launch` consumes only the `ResolvedRapidMlxLaunchModel`.
7. **Unify Storage Discovery**:
    - Discover legacy root models and every structured directory through one typed
      inventory contract.
    - Keep physical cache ownership distinct from user-visible model identity and
      deduplicate HF blob/snapshot aliases without copying weights.
8. **Migrate Safely**:
    - Plan and preview every filesystem move and persisted-path rewrite.
    - Journal completed moves, use atomic JSON rewrites, resume after interruption,
      and provide rollback metadata without attempting cross-filesystem renames.
9. **Render First-Class Badges**:
    - Use the shared inventory metadata in model cards, selectors, and details.
    - Badge text, color, iconography, and tooltips must work in dark/light themes,
      reduced motion, keyboard navigation, and narrow layouts.

### Hard Gates (Verification)
- **Clippy Compliance**: Run `cargo clippy -- -D warnings`. Zero warnings are allowed.
- **GGUF Honesty**: Verify `.gguf` recommends llama.cpp and Rapid-MLX returns an
  actionable Phase 5.5 experimental-import choice rather than pretending it is native.
- **Official Conversion**: Convert and launch one authoritative local/HF safetensors
  model through pinned `mlx_lm.convert`; verify source revision, tokenizer/config, and
  quantization recipe are recorded.
- **Resolver Isolation**: Verify `RapidMlxAdapter` sees only a
  `ResolvedRapidMlxLaunchModel`, never wizard inputs or converter mechanics.
- **Official Conversion Safety**: Prove authoritative safetensors conversion records
  its pinned source/tool identity and that interruption or invalid staged output cannot
  yield a launchable cache entry.
- **End-to-End Resolution**: Resolve and launch at least one local MLX directory and
  one supported HF identifier.
- **Storage Migration**: Fixture and live-library evidence proves GGUF/mmproj moves,
  `.part` staging, persisted-path rewrites, interruption resume, collision refusal,
  and legacy discovery without copying or losing data.
- **App-Scoped HF Cache**: Spawned native HF, alias, and conversion-download commands
  use the configured library's HF hub/Xet cache paths while tokens remain absent from
  argv, manifests, persisted data, logs, and diagnostics.
- **Selective HF Import**: The live migration moves the known Rapid-MLX smoke model into
  the app-scoped cache without moving unrelated Hugging Face repositories or breaking
  the snapshot's blob links.
- **Badge Parity**: Screenshot and Playwright evidence covers every inventory kind in
  dark/light and narrow layouts. Each item exposes format, source, status, and
  compatibility labels with accessible names; no backend is represented only by a
  filename or generic fallback badge.
- **Checkpoint**: Verifier sign-off followed by `feat(models): resolve Rapid-MLX model sources`.

### Known Pitfalls & Constraints
- **Source provenance**: Do not treat arbitrary safetensors as authoritative without a
  complete config, tokenizer, model class, and revision/hash identity.
- **Staged output**: Official conversion output becomes launchable only after load
  validation and atomic promotion.
- **Apple Silicon local gate**: Local Rapid-MLX configuration and launch are available
  only on macOS/aarch64. Rust rejects unsupported hosts before discovery, download, or
  conversion side effects; other platforms retain read/manage/copy inventory access and
  may still attach to a remote compatible endpoint.
- **No destructive implicit migration**: Startup may discover and recommend migration,
  but moving an existing library requires an explicit migration operation and a durable
  journal. A collision, symlink escape, or failed metadata rewrite stops the operation.

### Phase 5 checkpoint and handoff (2026-07-15)

Phase 5 is complete. Stop here; Phase 5.5 and Phase 6 require separate explicit starts.

Delivered:

- Typed native MLX, revision-pinned HF, alias, authoritative safetensors, and explicit
  unsupported GGUF sources resolve through `ResolvedRapidMlxLaunchModel`.
- Official conversion is pinned to `mlx-lm==0.31.3`, verifies the real CLI flag
  contract, uses bounded/redacted subprocesses, validates a real MLX load, records
  immutable source/tool/recipe/content hashes, and atomically promotes only complete
  outputs. Cache reuse independently revalidates schemas, pinned tool identity,
  immutable HF provenance, derived cache/directory identity, and every manifest hash.
- The backend-neutral model library, authenticated inventory/resolver/migration APIs,
  explicit Unknown/No-backend treatment, typed preset seeding, and first-class badge
  rails are implemented. Dark, light, narrow, reduced-motion, and non-Apple platform
  presentation are covered by the screenshot harness and Playwright.
- Local Rapid-MLX is gated to Apple Silicon before any discovery/download/conversion
  side effect. Other platforms retain model inventory/management and remote attach.
- Live migration plan `874b49dc8f5c3c6174a016f503d25f2293cf89253e41e8d3f9cf889919bb49b3`
  completed: 57 GGUFs (730,246,897,792 bytes), two partial downloads, and only the
  selected `mlx-community/Qwen3-0.6B-4bit` repo/locks moved; two preset and six tag
  references rewrote; 19 unrelated shared HF repositories and the root `.jinja` stayed
  untouched. One pre-existing tag for an already absent GGUF was intentionally retained.
- Real smoke evidence passed for a local MLX directory, a typed HF source, an official
  Qwen/Qwen3-0.6B FP16 conversion at immutable revision
  `c1899de289a04d12100db370d81485cdf75e47ca`, conversion cache reuse, Rapid-MLX chat and
  status telemetry, and the migrated llama.cpp preset readiness/chat flow.

Validation at the checkpoint:

- Independent Phase 5 verifier sign-off, including the Apple Silicon follow-up gate.
- Rust: full suite 870 passed / 6 ignored; focused resolver, library, Rapid-MLX, and
  auth-routing suites passed; clippy with warnings denied passed.
- UI: focused inventory 5/5. An earlier isolated full Playwright run passed 209 / 2
  skipped. The final rerun passed 206 / 2 skipped with one transient module-count case
  and three live Hugging Face search timeouts; the module-count spec then passed 2/2
  standalone. On 2026-07-16 the user confirmed a Hugging Face service incident and
  directed that those three external-service cases be skipped for this checkpoint.
- JavaScript validation/lint, release build, formatting, diff check, and Windows GNU
  cross-check passed.

Next-session entry point:

1. Confirm this checkpoint commit is present and the worktree is clean.
2. Archive/fold the completed Rapid-MLX planning docs into reference/developer notes as
   part of the later documentation-consolidation task.
3. Begin Phase 5.5 only when explicitly requested, using
   `docs/plans/20260715-gguf_to_mlx_conversion_research.md`; otherwise proceed to Phase 6
   only when explicitly requested.

---

## Phase 5.5: Experimental GGUF Import Lab

Detailed research, local probe evidence, and the architecture-promotion program live in
`docs/plans/20260715-gguf_to_mlx_conversion_research.md`.

This phase also qualifies Rapid-MLX `v0.10.10` as the first concrete daily-update
exercise. The release is pinned to
`5ca536275e89ddf0de3b49bd6f55fad80e42656e`; its notes describe release-artifact
acceptance hardening and a version bump, but the 15-commit tag comparison also includes
substantive inference/runtime work: model onboarding, Gemma 4 routing/KV-share changes,
KV-cache export/import, model-profile refactoring, and DFlash security changes.
Compatibility must therefore be proven through a staged `v0.10.9` -> `v0.10.10`
install, runtime smoke, atomic activation, and rollback rather than inferred from the
narrow release notes.

### Phase Objective

Provide a fail-closed path for valuable GGUF-only finetunes without presenting reverse
conversion as lossless or universally supported. Promote support one architecture and
quantization family at a time based on real parity evidence.

### Precise Scope

- Pin the candidate reverse converter to an audited source SHA; do not install
  unpinned `main`, discover arbitrary PATH copies, or imply Rapid-MLX owns the tool.
- Dequantize GGUF tensors into a staged FP16 representation, validate it, then
  optionally use pinned official mlx-lm tooling for a new MLX quantization.
- Classify imports as `Verified`, `Experimental`, or `Unsupported` by architecture,
  asset set, source quantization, and validated converter profile.
- Preserve source GGUF, metadata, tensor inventory, quant types, converter/runtime
  versions, config/tokenizer hashes, and output recipe in a manifest.
- Run the `v0.10.10` qualification in a new app-scoped environment. Do not mutate the
  active `v0.10.9` runtime or any external Brew/Pip installation. A manual isolated
  harness is acceptable in Phase 5.5 if the production updater is not yet implemented;
  Phase 6 remains responsible for the user-facing runtime manager.
- Complete the R0.75 runtime/transport hardening defined in the detailed research plan:
  offload blocking model hashing/scans, add backend-aware bounded concurrency, bound SSE
  backpressure, reuse HTTP/poller clients, and preserve a raw Rapid-MLX SSE fast path.
- Fold inventory/platform caching and incremental Rapid-MLX telemetry rendering into the
  R4 Import Lab UX rather than shipping another full-refetch/full-DOM-rebuild flow.

### Hard Gates (Verification)

- **Fail Closed**: Any skipped/unmapped tensor, unexpected count/shape, non-finite
  value, unknown architecture, incomplete config/tokenizer, or missing auxiliary asset
  aborts conversion and cannot create a launchable cache entry.
- **Tokenizer Parity**: Prompt token IDs, BOS/EOS/PAD, special tokens, and chat-template
  rendering match the source metadata/reference tokenizer.
- **Runtime Load**: Pinned mlx-lm and Rapid-MLX both load the staged result before it is
  atomically published from `.converting` to a manifest-backed `.complete` cache.
- **Behavioral Parity**: Fixed greedy prompts compare first-token/top-k/logit tolerance
  and deterministic output against llama.cpp. A re-quantized result is compared both
  to the recovered FP16 model and the original GGUF.
- **Quantization UX**: F16/BF16 and Q8 sources are preferred; Q6 is cautionary;
  Q5/Q4 require a compounded-loss warning; IQ3/Q3/Q2 remain high-risk until separately
  verified. Never describe dequantization as restoring lost precision or imatrix data.
- **Initial Rejection Fixtures**: Qwen3.5/3.6 hybrid MoE/MTP/mmproj and Gemma4
  MoE/MTP/mmproj inputs from the user's library must be rejected until their full
  architecture and auxiliary-asset mappings have dedicated passing fixtures.
- **Resource Safety**: Estimate source + roughly two bytes per parameter for FP16
  staging + final MLX output + temp/shard margin before starting. Cancellation,
  insufficient disk, crash recovery, duplicate imports, and stale sentinels are tested.
- **Release Independence**: Phase 5.5 may ship as an Experimental/Lab capability after
  its own sign-off, but a missing architecture profile does not block native Rapid-MLX
  release readiness.
- **Upgrade Qualification**: Verify tag/package provenance, live version, capability
  profile, readiness, deterministic chat, telemetry, stop, atomic activation, retained
  last-known-good `v0.10.9`, and rollback for Rapid-MLX `v0.10.10`. Any failure leaves
  the prior runtime active with bounded, redacted diagnostics.
- **Transport Hardening**: Large hashing/scans do not block Tokio workers; llama.cpp
  remains serialized; Rapid-MLX gains bounded concurrent requests without stale-session
  routing; slow SSE consumers remain memory-bounded and clean up upstream work.
- **UI Efficiency**: Search/filter/sort/view changes reuse the in-memory inventory and
  shared platform state, while telemetry updates preserve stable accessible card DOM.
- **Checkpoint**: Verifier sign-off followed by
  `feat(models): add verified experimental GGUF import`.

### Known Pitfalls & Constraints

- The audited `barrontang/gguf2mlx` SHA
  `6a0da6529f233df79362cbf62dd96221c895351f` is a research candidate, not an
  approved production dependency: it has no tagged release or real end-to-end model
  suite and can currently skip failed tensors while still reporting completion.
- MTP/NextN heads, multimodal projectors, tied-weight semantics, hybrid attention/SSM,
  MoE routing, and custom architectures require explicit mappings and runtime proof.
- GGUF -> FP16 -> MLX quantization is approximation on approximation. It cannot recover
  precision or the original quantizer's K-block/imatrix decisions.

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
    - In `updater.rs`, implement versioned `install`, `upgrade`, `repair`, and rollback.
      Install an exact selected release in staging (for example
      `uv tool install rapid-mlx@0.10.9`), probe it, then atomically activate it.
      Never mutate an active environment in place.
    - Implement a version manager that fetches releases from GitHub.
    - Treat external Brew/Pip installations as user-owned: discover and report them,
      but never upgrade or rewrite them automatically.
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
- **Safe Daily Updates**: Verify latest-stable discovery, staged capability preflight,
  atomic activation, retention of the last known-good runtime, and one-click rollback.
  An incompatible new release must leave the active runtime untouched and explain the
  failed probe.
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
- Fold durable runtime, API, model-source, telemetry, updater, and UX contracts into
  `docs/reference/` and developer notes. Add completion/supersession banners and archive
  the dated Rapid-MLX plans so they do not remain competing sources of truth after release.
- Create the final checkpoint only after Verifier sign-off; do not add the
  `ready-to-test` label.
