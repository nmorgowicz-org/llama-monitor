# Rapid-MLX Backend Integration

| Field | Value |
|---|---|
| Created | 2026-06-11 |
| Last validated | 2026-06-13 |
| Status | Source-of-truth implementation specification |
| Target base | `main` after PR #215 merges as-is |
| Product scope | Local model launch, lifecycle, chat compatibility, monitoring, presets, updates, and UX |
| Supported host | macOS on Apple Silicon |
| Research baseline | Rapid-MLX `main` at `be5d8bd65d293a3c667510156f62c6f593b54bf6`, package version `0.7.5` |
| Compatibility policy | Rolling releases with runtime capability detection; no application-level Rapid-MLX version pin |

## Purpose

Add Rapid-MLX as a first-class inference backend beside llama.cpp. A user must be
able to install, upgrade, roll back, configure, launch, monitor, stop, and chat with
a Rapid-MLX server through the same polished product flow used for `llama-server`.

This document is the implementation contract. Future agents must validate changing
upstream facts before coding, but must not reopen settled product or architecture
decisions without recording a concrete incompatibility and updating this document.

The integration is not a command substitution inside the current llama.cpp spawn
path. PR #215 establishes a llama.cpp-specific implementation. Rapid-MLX requires a
backend-neutral orchestration layer so that backend-specific command construction,
health checks, metrics, controls, model inputs, and release management do not leak
into shared session and UI code.

## Non-Negotiable Decisions

1. `llama.cpp` and Rapid-MLX are equal, named inference backends.
2. Existing llama.cpp behavior from PR #215 must remain unchanged.
3. Rapid-MLX is supported only on Apple Silicon macOS. It is visible but explicitly
   unavailable on Linux, Windows, and Intel macOS.
4. Rapid-MLX is not pinned to one version. The app supports current and historical
   releases, release notes, upgrades, rollback, and external installations.
5. A reviewed upstream commit is an evidence baseline, not a runtime requirement.
6. Backend compatibility is established by probes and capabilities, not only by a
   version string.
7. Unsupported or unavailable monitoring cards are not rendered. The UI must never
   display empty, fake, permanently `N/A`, or semantically incorrect cards.
8. Backend-specific configuration remains backend-specific. Do not emulate Rapid-MLX
   flags with llama.cpp names or pass unsupported llama.cpp flags through.
9. Secrets are passed through environment variables or protected persisted fields,
   never command-line arguments, logs, diagnostics, or browser-visible payloads.
10. Presets, restored sessions, and API data remain backward compatible with PR #215.
11. Spawn, attach, stop, logs, readiness, chat, and dashboard behavior must all route
    through the selected backend.
12. The final UX must feel like one launcher with multiple engines, not two unrelated
    tools bolted together.

## Verified Upstream Baseline

The following facts were verified against Rapid-MLX source at commit
`be5d8bd65d293a3c667510156f62c6f593b54bf6`. Revalidate them against the release
selected during implementation.

### Platform and installation

- Repository: <https://github.com/raullenchai/Rapid-MLX>
- License: Apache-2.0.
- Python requirement: Python 3.10 or newer.
- Supported hardware: Apple Silicon macOS.
- Homebrew installation:
  `brew install raullenchai/rapid-mlx/rapid-mlx`
- Python installation: `pip install rapid-mlx`
- Primary launch command: `rapid-mlx serve <model-or-path>`
- `<model-or-path>` can be a Rapid-MLX alias, a Hugging Face repository ID, or a
  local model directory.
- Optional package extras include vision, DFlash, embeddings, guided generation,
  audio, chat, and an aggregate `all` extra.

### HTTP behavior

Rapid-MLX presents an OpenAI-compatible API, but its operational endpoints and
telemetry are not llama.cpp-compatible.

| Endpoint | Authentication | Required integration behavior |
|---|---|---|
| `GET /health` | Unauthenticated | Liveness and model-load state |
| `GET /health/ready` | Unauthenticated | Readiness; returns non-success until warm-up is complete |
| `GET /v1/models` | API key when configured | OpenAI model discovery |
| `POST /v1/chat/completions` | API key when configured | Chat and streaming |
| `GET /v1/status` | API key when configured | Runtime, queue, throughput, memory, and request telemetry |
| `GET /v1/cache/stats` | API key when configured | Optional cache telemetry; schema may vary |
| `POST /v1/requests/{id}/cancel` | API key when configured | Request cancellation |
| `DELETE /v1/requests/{id}/cancel` | API key when configured | Cancellation alias |

`/health` includes:

- `status`
- `ready`
- `model_loaded`
- `model_name`
- `model_type`
- `engine_type`
- MCP state

`/v1/status` at the research baseline includes:

- `status`: `generating`, `idle`, or `not_loaded`
- `model`
- `uptime_s`
- `steps_executed`
- `num_running`
- `num_waiting`
- `total_requests_processed`
- `total_prompt_tokens`
- `total_completion_tokens`
- `generation_tps`
- `prompt_tps`
- optional Metal memory values such as active, peak, and cache gigabytes
- a variable cache object
- a variable requests array

The integration must parse status payloads tolerantly. Unknown fields are ignored;
missing optional fields remain absent. Do not fail an otherwise healthy session
because a release adds, removes, or renames optional telemetry.

### Streaming behavior

- Streaming chat can include `reasoning_content`.
- Usage can arrive in the finishing chunk or in a dedicated usage chunk when
  `stream_options.include_usage` is enabled.
- The existing chat stream parser must accept both shapes.
- Absence of reasoning content is normal and must not create empty reasoning UI.

### Request controls

The reviewed baseline supports the following relevant request fields:

- `temperature`
- `top_p`
- `max_tokens`
- `max_completion_tokens`
- `stream`
- `stream_options`
- `stop`
- `top_k`
- `min_p`
- `repetition_penalty`
- `presence_penalty`
- `frequency_penalty`
- `tools`
- `tool_choice`
- `parallel_tool_calls`
- `response_format`
- `logprobs`
- `timeout`
- `enable_thinking`
- `chat_template_kwargs`
- video-related inputs for multimodal requests

The baseline does not establish request-level `seed` support. The UI and request
builder must not expose or send `seed` for Rapid-MLX unless the active compatibility
profile proves support.

### CLI configuration baseline

The reviewed baseline exposes the following launch controls. Agents must obtain the
active runtime's `rapid-mlx serve --help` output and reconcile changes before mapping
flags.

| Area | Baseline flags and defaults |
|---|---|
| Identity/network | `--served-model-name`, `--host 0.0.0.0`, `--port 8000`, `--log-level INFO` |
| Capacity | `--max-num-seqs 256`, `--max-concurrent-requests 256` |
| Batching | `--prefill-batch-size 8`, `--completion-batch-size 32`, continuous batching enabled |
| Prefix cache | enabled by default, size `100`, memory MB or percent, memory-aware behavior |
| KV cache | 4/8-bit quantization, default 8-bit, group size 64, minimum quantize tokens 256 |
| TurboQuant | 3/4-bit or auto, group size 32 |
| Streaming | `--stream-interval 1` |
| Limits | max tokens `32768`, request timeout `1800` seconds |
| Memory | GPU memory utilization `0.90`, paged cache block size 64, max blocks 1000 |
| Prefill | chunked prefill tokens `0`, prefill step size `2048` |
| Speculation | MTP, suffix decoding, and DFlash controls |
| Integrations | MCP config, embedding model, tool parser, auto tool choice, reasoning parser |
| Sampling | default sampling controls |
| Access | API key or `RAPID_MLX_API_KEY`, rate limit |
| Multimodal | MLLM enable/disable behavior |

Mutual-exclusion rules at the baseline:

- DFlash cannot be combined with MTP or suffix decoding.
- MTP and suffix decoding cannot both be active.
- TurboQuant cannot be combined with standard KV-cache quantization.
- Unsafe or explicitly experimental flags, including optimistic MTP behavior, are
  not exposed in the normal UI.

### Upstream version behavior

- `rapid-mlx upgrade --yes` exists and detects supported installation methods.
- `rapid-mlx models` and `rapid-mlx info` do not provide a stable JSON contract at
  the baseline.
- `/v1/models` does not report the installed Rapid-MLX package version.
- A remote attached server may not reveal an exact package version. Capability and
  schema probing therefore remains authoritative for remote sessions.

## Existing Codebase Baseline

Implement against `main` after PR #215 merges as-is. The relevant current design is:

- `src/llama/server.rs` owns llama.cpp configuration, command construction, process
  supervision, log capture, and exit watching.
- `src/llama/poller.rs` assumes llama.cpp endpoints including `/metrics`, `/slots`,
  `/health`, and `/v1/models`.
- `LlamaMetrics` and the dashboard assume slot, context, speculative decoding, and
  llama.cpp KV-cache concepts.
- spawned sessions do not currently persist a backend discriminator.
- model presets are flat and llama.cpp-oriented.
- `/api/sessions/spawn` manually copies llama.cpp fields.
- setup, spawn wizard, preset editing, model discovery, and updater surfaces assume
  GGUF plus `llama-server`.
- the existing llama.cpp updater already establishes the expected quality bar:
  current version, available versions, release notes, installation, upgrades, and
  rollback.
- remote-agent spawning and discovery are llama.cpp-oriented.

Do not extend this by adding Rapid-MLX conditionals throughout `src/llama`. Create a
backend-neutral layer and preserve `src/llama` as the llama.cpp adapter.

## Product Experience

### User language

Use these product names consistently:

- **Engine**: the inference backend selected by the user.
- **llama.cpp**: the GGUF engine.
- **Rapid-MLX**: the MLX engine.
- **Runtime**: the installed executable environment for an engine.
- **Model source**: local path, Hugging Face repository, or supported alias.

Avoid exposing implementation terms such as adapter, provider enum, Python venv,
poller, or capability manifest in primary flows.

### Unified launch flow

The spawn wizard and welcome-screen preset editor must remain round-trippable and use
the same components and validation. The flow becomes:

1. **Engine and goal**
   - Engine cards for llama.cpp and Rapid-MLX.
   - A concise description of model format and platform support.
   - A `Recommended` badge when deterministic selection rules identify a clear fit.
   - Unavailable engines remain visible with the exact reason and remediation.
2. **Model**
   - llama.cpp retains GGUF, Hugging Face file selection, multimodal projection, and
     quantization flows from PR #215.
   - Rapid-MLX accepts an alias, Hugging Face repository, or local MLX model
     directory.
   - The UI must not ask a Rapid-MLX user to select a GGUF file or `mmproj`.
3. **Runtime**
   - Display detected runtime, source, version, compatibility, and required extras.
   - Offer install, repair, version selection, and upgrade actions inline.
   - Advanced engine controls are grouped and progressively disclosed.
4. **Generation and access**
   - Show only controls supported by the selected engine and compatibility profile.
   - Shared controls retain the same labels where semantics match.
   - API key entry uses a protected input and never echoes the saved value.
5. **Review and launch**
   - Summarize engine, runtime version/source, model, network exposure, capabilities,
     and non-default tuning.
   - Surface blocking validation before the user presses Launch.

Changing engines preserves shared values where semantics match, including host, port,
API-key intent, temperature, top-p, and token limit. Backend-only values remain stored
in their backend section so switching away and back does not erase configuration.
Never silently translate a GGUF model path into a Rapid-MLX model source or vice versa.

### Deterministic engine recommendation

Use the following order:

1. A restored session or existing preset retains its stored engine.
2. A selected `.gguf` file or GGUF-specific Hugging Face artifact selects llama.cpp.
3. A validated MLX directory, Rapid-MLX alias, or MLX Hugging Face repository selects
   Rapid-MLX.
4. With no model selected, Apple Silicon macOS recommends Rapid-MLX only when its core
   runtime probe passes; otherwise recommend llama.cpp.
5. An explicit user choice is never changed automatically.

Recommendation and selection are separate state. A recommendation may update as
inputs change; the selected engine may not change after user interaction.

### Setup and settings

The setup screen shows one runtime card per engine:

- installed state
- current version
- installation source
- compatibility state
- update availability
- `Install`, `Manage`, `Repair`, or `Unavailable` primary action

Settings gains an **Inference engines** section. Rapid-MLX management must provide the
same level of polish as the llama.cpp version manager rather than delegating the
entire experience to a terminal command.

## Rapid-MLX Runtime Lifecycle

### No version pinning

Do not hard-code one supported Rapid-MLX version in application logic or dependency
installation. Rapid-MLX and llama.cpp both evolve quickly. A pin would turn routine
upstream releases into artificial incompatibilities and prevent the app from offering
the same upgrade experience already provided for llama.cpp.

The reviewed commit remains:

- a source citation;
- a fixture/schema baseline;
- the minimum body of behavior verified during design.

It is not:

- the only installable version;
- an automatic downgrade target;
- a condition for launching a runtime that passes compatibility probes.

### Runtime sources and precedence

Resolve the active Rapid-MLX runtime in this order:

1. explicit executable path configured by the user;
2. selected llama-monitor-managed environment;
3. `rapid-mlx` found on `PATH`.

Report the selected source. Also report shadowed installations in diagnostics so a
user can understand why a particular executable is active.

Supported source labels:

- `managed`
- `homebrew`
- `pip`
- `pipx`
- `custom`
- `path-unknown`

Do not infer source solely from `rapid-mlx --version`. Inspect the resolved executable
path and, where safe, package metadata.

### Managed environments

Managed installations are side-by-side and independently selectable:

```text
<config-dir>/runtimes/rapid-mlx/
  current.json
  releases.json
  <version>/
    venv/
    metadata.json
```

`metadata.json` records:

- requested version
- resolved package version
- Python executable and version
- Rapid-MLX executable path
- installed extras
- installation timestamp
- package source
- compatibility probe result and timestamp

Managed installation procedure:

1. Validate Apple Silicon macOS and a supported Python interpreter.
2. Fetch available release metadata.
3. Create a new version directory and virtual environment.
4. Install the exact user-selected Rapid-MLX release and selected extras into that
   environment. Exact version selection here is an installation action, not an app
   compatibility pin.
5. Run core and feature probes against the new executable.
6. Write metadata.
7. Atomically update `current.json` only after all required probes succeed.
8. Retain the previous environment for rollback.
9. Remove a failed staging environment without changing the active runtime.

Never mutate an active managed environment in place.

Use a temp/staging directory under the same filesystem and an atomic rename. One
process-wide installation mutex prevents concurrent install, switch, delete, or
repair operations.

### Releases, notes, upgrades, and rollback

The version manager must support:

- installed version and source;
- upstream latest stable release;
- update availability;
- release history;
- release notes;
- install latest;
- install a selected historical release;
- switch among installed managed releases;
- roll back to the previously selected managed release;
- delete an inactive managed release;
- repair/re-probe an installation;
- upgrade an externally managed runtime when its source supports it.

Release metadata is fetched from GitHub releases, cached for 30 minutes, and includes
tag, normalized version, publication date, URL, prerelease state, and notes. Stable
releases are the default list. Prereleases require an explicit user setting.

Render release notes with the same sanitized markdown pipeline used by the existing
llama.cpp updater. Never inject upstream HTML directly.

For externally managed installations:

- Homebrew can offer the appropriate Homebrew upgrade command through the existing
  subprocess management pattern.
- supported Rapid-MLX installations can offer `rapid-mlx upgrade --yes`.
- custom paths display instructions and a re-check action if no safe automated
  upgrade method is known.

Never auto-upgrade on launch. Checking for updates is allowed; changing the runtime
always requires an explicit user action.

An installation may download while a server is running. Do not switch or remove the
runtime backing a live process. Defer activation until no session uses that runtime,
or require the user to stop affected sessions.

### Compatibility state

Use these user-visible states:

- **Compatible**: all core probes pass.
- **Compatible with limited features**: core passes; one or more optional capabilities
  are unavailable.
- **Needs repair**: executable exists but core probes fail.
- **Unsupported platform**: host cannot run Rapid-MLX.
- **Not installed**: no runtime resolved.
- **Probe failed**: a transient diagnostic error prevented a determination.

Do not label every newer unrecognized release unsupported. An unknown version that
passes core probes is compatible, with optional features determined independently.

## Capability Detection

### Compatibility profiles

Add the checked-in compatibility manifest at:

```text
src/inference/rapid_mlx/compatibility.json
```

Each known profile records:

- version range or exact known version;
- source commit used to verify it;
- expected core CLI flags;
- known endpoint schemas;
- optional feature flags;
- known incompatibilities;
- fixture identifiers.

Profiles optimize detection and regression tests. They do not replace live probes.

Profile classifications:

- `verified`: explicitly tested by the project;
- `provisional`: newer/unknown release that passes live probes;
- `legacy`: known older release with limited support;
- `incompatible`: core contract cannot be satisfied.

### Core probes

A runtime is launch-compatible only when all applicable probes pass:

1. executable resolves and can start;
2. version command returns successfully and can be normalized when available;
3. `serve --help` confirms required model, host, and port behavior;
4. Python/package imports required by the base installation succeed for managed
   environments;
5. host is Apple Silicon macOS;
6. an ephemeral server can pass `/health`, `/health/ready`, `/v1/models`, and
   `/v1/status` fixture or smoke validation before a profile is marked `verified`.

Normal startup need not launch an extra smoke server every time. Cache successful
probes using executable path, file metadata, version, extras, and probe-schema version
as the cache key.

### Optional capabilities

Detect independently:

- vision/multimodal support;
- DFlash;
- embeddings;
- guided generation;
- audio;
- chat extras;
- MTP;
- suffix decoding;
- standard KV quantization;
- TurboQuant;
- tool parsing and automatic tool choice;
- reasoning parser/thinking controls;
- MCP;
- cache telemetry;
- cancellation;
- status memory telemetry.

The UI only exposes a control when the active runtime proves the corresponding
capability. Missing extras receive a specific installation/remediation action.

### Probe safety

- Invoke executables without a shell.
- Apply timeouts to every probe.
- Cap captured stdout/stderr.
- Redact secrets.
- Store a concise diagnostic result, not unbounded raw output.
- Do not run arbitrary commands parsed from upstream release notes or model metadata.

## Backend Architecture

### Module boundary

Create:

```text
src/inference/
  mod.rs
  backend.rs
  capabilities.rs
  metrics.rs
  supervisor.rs
  llama_cpp.rs
  rapid_mlx/
    mod.rs
    command.rs
    compatibility.rs
    discovery.rs
    poller.rs
    runtime.rs
    updater.rs
```

Keep existing llama.cpp modules where practical and adapt them behind
`inference::llama_cpp`. Do not perform a broad rewrite of proven PR #215 code merely
to make naming perfectly generic.

Use enum dispatch. There are two known in-process backends, so a plugin ABI or
`async_trait` hierarchy adds complexity without product value.

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InferenceBackend {
    LlamaCpp,
    RapidMlx,
}
```

The backend adapter owns:

- runtime discovery and diagnosis;
- launch validation;
- command/environment construction;
- readiness behavior;
- telemetry polling;
- model identity;
- supported request controls;
- cancellation behavior;
- log interpretation;
- runtime version management.

The shared supervisor owns:

- process lifecycle;
- stdout/stderr streaming;
- exit watching;
- session-state transitions;
- stop escalation;
- port reservation;
- secret-safe diagnostics;
- common persistence.

### Session persistence

Add `backend` to spawned and attached session modes. Deserialization defaults missing
values to `llama_cpp`, preserving all PR #215 sessions.

Persist the exact runtime identity used by a spawned process:

- backend;
- runtime source;
- executable path;
- resolved version;
- managed environment ID when applicable;
- capability-profile result;
- launch-time model identity.

This prevents a settings change from rewriting the history of a running or restored
session.

### Presets

Replace the indefinitely growing flat preset with:

```text
ModelPreset
  schema_version
  name
  backend
  shared
  llama_cpp
  rapid_mlx
```

`shared` contains values with identical semantics:

- host/bind policy;
- port policy;
- API authentication intent;
- temperature;
- top-p;
- maximum completion tokens;
- stop sequences where supported.

The backend sections contain only native controls. Existing presets migrate to
`backend = llama_cpp`; no existing field may be dropped during migration. Saving and
reopening either backend must reproduce all visible controls.

Use an explicit schema version and migration function. Add JSON fixtures for:

- pre-backend PR #215 preset;
- migrated llama.cpp preset;
- Rapid-MLX preset;
- unknown future fields;
- missing optional backend section.

### Rapid-MLX configuration

The persisted Rapid-MLX section must represent at least:

- model source and served model name;
- runtime selection (`managed current`, managed version, or explicit executable);
- installed/required extras;
- host and port;
- API-key reference, never plaintext in general preset exports;
- log level;
- sequence and concurrent-request limits;
- prefill/completion batch sizes;
- prefix-cache enable, size, MB/percent limit, and memory-aware mode;
- KV-cache quantization bits, group size, and minimum token threshold;
- TurboQuant mode and group size;
- stream interval;
- maximum tokens;
- continuous batching;
- GPU memory utilization;
- paged-cache block size and maximum blocks;
- chunked prefill and prefill step size;
- one speculation mode: off, DFlash, MTP, or suffix decoding;
- mode-specific safe controls;
- MCP config;
- rate limit and request timeout;
- tool parser and automatic tool choice;
- reasoning parser and thinking default;
- pinned system prompt;
- multimodal enablement;
- embedding model;
- default sampling values.

Defaults come from the active compatibility profile. Do not serialize every upstream
default as an explicit override. Store user choices and profile identity so future
defaults can evolve without silently changing a saved preset's intended behavior.

Validation must reject mutually exclusive options before spawning.

## Launch and Process Flow

### Spawn request

Replace manual field copying in `/api/sessions/spawn` with a typed backend request:

```text
SpawnSessionRequest
  backend
  shared
  config: tagged backend configuration
```

Flow:

1. authenticate and authorize;
2. deserialize and migrate request shape if necessary;
3. select backend adapter;
4. resolve runtime;
5. load/cache compatibility result;
6. validate platform, model source, flags, extras, port, and secret references;
7. reserve session identity and port;
8. build redacted command description plus actual process command/environment;
9. spawn under the shared supervisor;
10. stream logs;
11. poll liveness and readiness using backend-native endpoints;
12. mark ready only after the backend readiness contract succeeds;
13. begin telemetry polling;
14. expose chat once the model endpoint is ready.

Rapid-MLX API keys are set with `RAPID_MLX_API_KEY`. They must not appear in argv,
process summaries, logs, API responses, or persisted diagnostic bundles.

### Readiness

Rapid-MLX:

1. process remains alive;
2. `/health` reports a loaded/healthy model;
3. `/health/ready` succeeds;
4. `/v1/models` resolves the served model when authentication is configured;
5. timeout produces a precise error with redacted recent logs.

Do not equate an open TCP port with readiness.

### Stop and cancellation

- Server stop uses the shared supervised process lifecycle.
- In-flight generation cancellation uses the backend-native request-cancel endpoint
  when a request ID exists and the capability is present.
- Client disconnect still cancels local forwarding work.
- Absence of request cancellation is a capability limitation, not a server failure.

## Attach Flow

Attach asks for or detects the backend:

1. If the user explicitly selects an engine, probe only that contract.
2. In automatic mode, use endpoint evidence without sending state-changing requests.
3. Store the detected backend and capability snapshot with the attached session.
4. Never claim an exact remote Rapid-MLX package version unless the remote API
   provides trustworthy evidence.

Ambiguous endpoints require user selection. Detection must not classify every
OpenAI-compatible server as Rapid-MLX.

Remote-agent spawning is not part of the first implementation unless the remote agent
also receives the backend abstraction and explicit platform validation. Remote attach
to an already-running Rapid-MLX endpoint is allowed. The UI must not imply that Linux
or Windows remote agents can launch Rapid-MLX.

## Chat Compatibility

The shared chat layer remains OpenAI-compatible, with a backend capability matrix.

| Behavior | llama.cpp | Rapid-MLX |
|---|---:|---:|
| Streaming chat | Yes | Yes |
| Usage chunks | Existing behavior | Finish or dedicated usage chunk |
| Reasoning content | Existing parser rules | Accept `reasoning_content` |
| Tools | Capability-driven | Capability-driven |
| Structured response | Capability-driven | `response_format` when proven |
| Request seed | Existing behavior | Hidden unless proven |
| Cancellation | Existing behavior | Native endpoint when proven |

Request construction sends only fields supported by the active backend profile.
Unsupported saved fields remain preserved in the preset but are not transmitted.
The UI explains why a preserved control is inactive if the user views a preset under
a runtime that lacks the capability.

## Monitoring and Dashboard

### Core rule

The dashboard renders cards from data contracts, not from a static llama.cpp layout.
A card exists only when its required metrics are present and semantically valid for
the active backend.

Never:

- mount a card containing permanent `N/A`;
- substitute zero for a missing metric;
- show llama.cpp labels for a vaguely similar Rapid-MLX value;
- show slot, context, speculative-acceptance, sampler, or KV-cache cards without the
  underlying native metric;
- retain a nonfunctional card merely to keep a symmetrical grid.

The grid must reflow cleanly with fewer cards. A concise, accurate dashboard is more
premium than a full grid of placeholders.

### Normalized snapshot

Create a backend-neutral snapshot whose fields are optional:

```text
InferenceMetricsSnapshot
  sampled_at
  backend
  health
  ready
  model
  uptime_seconds
  generation_tokens_per_second
  prompt_tokens_per_second
  running_requests
  waiting_requests
  completed_requests_total
  prompt_tokens_total
  completion_tokens_total
  active_memory_bytes
  peak_memory_bytes
  cache_memory_bytes
  cache_metrics
  active_requests
  backend_details
```

Absence remains `None`; it is not converted to `0`.

### Card registry

Each card definition declares:

- stable card ID;
- supported backend(s);
- required metric paths;
- optional metric paths;
- formatter;
- stale-data policy;
- empty-state policy;
- ordering group;
- detail-panel behavior.

The renderer evaluates the registry against each snapshot. Unsupported cards are not
created in the DOM. Do not merely hide them with CSS, because hidden dead controls
remain reachable through selectors, accessibility trees, and stale event handlers.

### Initial Rapid-MLX cards

Render these only when their required data exists:

| Card | Required data | Notes |
|---|---|---|
| Model runtime | model plus health/readiness or uptime | Engine, model, state, uptime |
| Inference throughput | `generation_tps` and/or `prompt_tps` | Show only available series |
| Request queue | `num_running` and/or `num_waiting` | Present zero is valid |
| Request activity | non-empty parseable request collection | Omit when collection/schema is unavailable |
| Runtime memory | any valid Metal active/peak/cache value | Label as Metal/runtime memory, not VRAM |
| Prefix/cache state | recognized, semantically mapped cache statistic | No generic cache card from unknown fields |
| Totals | any valid request/token total | Clearly cumulative |

Do not render these llama.cpp cards for Rapid-MLX unless a future native metric is
explicitly mapped:

- context-window utilization;
- slot occupancy or per-slot progress;
- tokens per decode;
- speculative acceptance rate;
- sampler state;
- llama.cpp KV-cache utilization;
- llama.cpp prompt/decode timing decomposition.

### First sample, loss, and staleness

- Before the first successful status sample, show a compact loading state for the
  dashboard region, not empty cards.
- A numeric zero is displayed only when the field is present with value zero.
- When a previously valid metric is temporarily absent, the card may retain its last
  value with a visible stale timestamp for up to three normal poll intervals.
- After three consecutive missing samples, remove an optional card.
- Health/readiness cards may remain to explain a disconnected or failed server.
- When the session returns, card availability is recalculated from fresh data.

### Polling

Rapid-MLX polling uses:

- `/health` for liveness and model state;
- `/health/ready` during startup and readiness transitions;
- `/v1/status` for operational telemetry;
- `/v1/cache/stats` only when the capability exists;
- `/v1/models` for served-model identity.

Use endpoint-specific timeouts and independent error handling. A malformed optional
cache response must not mark the session unhealthy. Apply backoff when disconnected
and avoid overlapping polls.

Preserve backend-native details in a bounded diagnostic payload, but do not make the
main UI depend on arbitrary JSON.

## Model Discovery and Validation

Rapid-MLX model sources are not GGUF files.

### Local directory

Validate:

- path exists and is a directory;
- readable model metadata/config files are present;
- no GGUF-only validation is applied;
- validation errors name the missing requirement;
- launch still defers final architecture support to Rapid-MLX and reports its
  redacted error accurately.

### Hugging Face

Reuse repository search and authentication infrastructure, but use a backend-specific
selection flow:

- choose a repository, not a GGUF file;
- identify MLX-compatible metadata/tags conservatively;
- do not guarantee compatibility from a tag alone;
- display download/cache implications;
- retain the exact repository revision when the user explicitly selects one.

### Aliases

Treat aliases as runtime-provided capabilities. Because baseline `models`/`info`
commands lack a stable JSON contract:

- do not scrape decorative terminal output as a permanent API;
- use a checked compatibility parser only for known versions;
- otherwise allow manual alias entry with launch-time validation;
- label dynamically verified aliases separately from free-form input.

### Memory estimation

Do not reuse the GGUF VRAM estimator for MLX models. Initial Rapid-MLX UX uses:

- model repository/download size when known;
- current system unified memory;
- selected cache and concurrency controls;
- a clearly labeled advisory, not a false precise fit estimate.

A future MLX estimator requires a separate documented formula and fixtures.

## API Surface

Implement these exact backend-neutral operations:

| Method and route | Required response/action |
|---|---|
| `GET /api/inference/backends` | platform availability, installation, active version/source, compatibility, capabilities |
| `GET /api/inference/backends/{backend}/diagnostics` | redacted probe steps and remediation |
| `POST /api/inference/backends/{backend}/probe` | invalidate the cached probe and run diagnosis again |
| `GET /api/inference/backends/rapid_mlx/releases` | cached stable/prerelease metadata and installed state |
| `POST /api/inference/backends/rapid_mlx/releases/install` | asynchronous managed install with structured progress |
| `POST /api/inference/backends/rapid_mlx/releases/select` | atomic switch after live-session checks |
| `DELETE /api/inference/backends/rapid_mlx/releases/{version}` | inactive managed version only |
| `POST /api/inference/backends/rapid_mlx/repair` | re-probe or rebuild a managed environment |
| `POST /api/inference/backends/rapid_mlx/upgrade-external` | source-aware explicit upgrade action |
| `POST /api/sessions/spawn` | existing route with the new tagged backend config |
| `POST /api/sessions/attach` | existing route with explicit or automatic backend detection |

Use `rapid_mlx` in serialized route/backend identifiers and `Rapid-MLX` in user-facing
copy. Do not add parallel llama-specific spawn routes.

Runtime mutation endpoints require the same administrator authorization used for
binary/database maintenance. Validate all versions against fetched release metadata;
do not interpolate arbitrary user strings into package or shell commands.

Progress events must be structured:

- phase;
- human-readable message;
- bounded percentage when known;
- terminal success/failure;
- redacted diagnostic code.

## Security

- Never invoke Rapid-MLX, Python, pip, Homebrew, or Git through a shell.
- Validate executable and model paths.
- Canonicalize managed paths before deletion.
- Deletion is restricted to inactive children of the managed runtime root.
- Pass API secrets through environment variables.
- Store secret values using the existing protected secret mechanism.
- Redact authorization headers, API keys, environment values, and signed URLs.
- Bind defaults must follow existing llama.cpp safety policy. Because upstream
  defaults to `0.0.0.0`, the app must not inherit that exposure accidentally.
- Reuse existing warnings/confirmation for non-loopback bind addresses.
- Release notes and upstream text are untrusted and sanitized.
- Downloads and package operations have timeouts, bounded output, and cancellation.
- Managed runtime files receive the same permission hardening available elsewhere;
  retain the documented Windows no-op, although Rapid-MLX itself is unavailable there.

## Platform Behavior

| Platform | UI | Launch behavior |
|---|---|---|
| Apple Silicon macOS | Full Rapid-MLX card and management | Supported after probes |
| Intel macOS | Visible unavailable card | Block with architecture explanation |
| Linux | Visible unavailable card | Block; llama.cpp remains available |
| Windows | Visible unavailable card | Block; llama.cpp remains available |

All shared Rust and frontend code must compile on macOS, Linux, and Windows. Gate only
the subprocess/runtime-specific implementation. New `#[cfg]` guards require comments
describing the unsupported path and fallback. Run the mandatory Windows cross-check
when affected files include platform guards or shared dependencies.

## Implementation Sequence

### Phase 0: Baseline and fixtures

1. Merge/rebase onto PR #215's final mainline result.
2. Capture Rapid-MLX CLI help and endpoint fixtures for the current stable release.
3. Add the first compatibility profile using the reviewed commit plus current stable.
4. Add fixture tests proving tolerant status, health, readiness, cache, models, and
   streaming parsing.
5. Record any upstream drift in this document before implementation relies on it.

Exit criteria:

- every required upstream claim has a source or fixture;
- no implementation depends on an undocumented guessed field.

### Phase 1: Backend-neutral domain

1. Add `InferenceBackend`.
2. Add session and preset schema migration.
3. Introduce typed spawn configuration.
4. Extract shared supervision without changing llama.cpp behavior.
5. Route llama.cpp through enum dispatch.
6. Add regression tests for PR #215 launch, restore, preset, stop, logs, and polling.

Exit criteria:

- all existing llama.cpp tests pass;
- old persisted data loads as llama.cpp;
- the UI is behaviorally unchanged for llama.cpp.

### Phase 2: Rapid-MLX runtime management

1. Add platform detection.
2. Add explicit, managed, and PATH runtime discovery.
3. Add live compatibility and feature probes.
4. Add GitHub release metadata and cache.
5. Add side-by-side managed installation, selection, rollback, deletion, and repair.
6. Add external upgrade support.
7. Add setup and Settings version-manager UI.

Exit criteria:

- install, upgrade, switch, rollback, repair, and diagnostics work;
- a failed install cannot damage the selected runtime;
- no version is treated as uniquely supported.

### Phase 3: Spawn and chat

1. Implement Rapid-MLX command construction and validation.
2. Implement readiness and lifecycle handling.
3. Add backend-native cancellation.
4. Extend chat request and stream parsing.
5. Add attach detection.
6. Add backend-specific model selection and preset editing.

Exit criteria:

- a local MLX model, HF repository, and supported alias can be launched;
- chat streams, usage, stop, logs, restore, and preset round trips work;
- secrets do not appear in diagnostics or process display.

### Phase 4: Monitoring and UX

1. Add normalized optional telemetry.
2. Replace the fixed dashboard with the card registry.
3. Map Rapid-MLX status/cache fields conservatively.
4. Add first-sample, stale, and removal behavior.
5. Complete responsive wizard/setup/dashboard polish.
6. Add accessibility states and keyboard behavior.

Exit criteria:

- Rapid-MLX shows only cards backed by real data;
- llama.cpp retains all existing cards;
- grid layouts remain intentional at all breakpoints;
- no empty card or dead control is present.

### Phase 5: Documentation and release validation

1. Update `docs/reference/spawn-wizard.md`.
2. Update `docs/reference/dashboard.md`.
3. Update `docs/reference/api.md`.
4. Update CLI/runtime reference documentation.
5. Extend `tests/ui/capture.mjs` and `tests/ui/README.md`.
6. Add promoted screenshots only after final UI validation.
7. Update README only if the finished multi-engine launcher meets the README's
   high-impact feature bar.

## File-Level Worklist

Expected areas, adjusted to the final PR #215 tree:

| Area | Work |
|---|---|
| `src/inference/**` | New backend domain, capability, metrics, supervisor, adapters |
| `src/llama/**` | Adapt existing implementation without changing native behavior |
| session/config persistence | backend discriminator, runtime identity, migrations |
| API routes/types | engine status, lifecycle, releases, spawn/attach tagged payloads |
| chat forwarding | capability-aware requests, reasoning/usage stream shapes |
| `static/**` setup/wizard | engine selection, model/runtime steps, validation |
| `static/**` settings | Rapid-MLX version manager and diagnostics |
| `static/**` dashboard | card registry and capability-driven composition |
| preset editors | shared components and backend-specific sections |
| tests/fixtures | upstream version, endpoint, stream, and migration fixtures |
| `tests/ui/**` | engine flow, runtime manager, dashboard-card behavior |
| `docs/reference/**` | comprehensive user and API documentation |

Search the final tree before creating files. Reuse established modules and component
patterns where ownership matches; do not duplicate updater, modal, progress, markdown,
secret-input, or preset-form infrastructure.

## Test Contract

### Rust unit/integration tests

- legacy session and preset migration defaults to llama.cpp;
- tagged config serialization round trips;
- Rapid-MLX command construction for every supported control;
- mutual-exclusion validation;
- secret redaction;
- platform availability;
- runtime precedence and source detection;
- release normalization and ordering;
- atomic managed-runtime switch and rollback;
- active-runtime deletion prevention;
- known, unknown-newer, legacy, and incompatible profile behavior;
- tolerant health/status/cache/model parsing;
- absent metric remains absent;
- present zero remains zero;
- readiness transitions and timeout;
- process exit and stop escalation;
- request cancellation capability;
- remote attach detection ambiguity.

### Frontend tests

- recommendation does not override explicit engine choice;
- changing engine preserves shared and backend-local form state;
- GGUF and MLX model selectors never appear together incorrectly;
- unavailable platforms show exact remediation;
- unsupported controls are not mounted;
- Rapid-MLX dashboard cards appear only with required metrics;
- missing metrics do not create zero/`N/A` cards;
- temporary metric loss marks a previously valid card stale;
- sustained metric loss removes optional cards;
- grid reflows after card removal;
- release manager supports notes, install, switch, rollback, and repair states;
- saved presets round-trip through both wizard editors;
- protected API-key values are never reflected into normal DOM/API snapshots.

### UI end-to-end tests

Add deterministic mocked/fixture-backed coverage for:

1. Apple Silicon compatible engine selection.
2. unsupported platform card.
3. install/version-manager flow without performing a real network install.
4. Rapid-MLX preset create, edit, save, restore.
5. spawn validation and readiness transition.
6. dashboard full supported metric set.
7. dashboard partial metric set with omitted cards.
8. live zero queue values.
9. stale then removed optional telemetry.
10. llama.cpp regression flow.

Real Rapid-MLX smoke tests are opt-in and hardware-gated. They must not make generic CI
depend on Apple Silicon or network model downloads.

### Mandatory repository checks

Run the repository's required pre-PR sequence from `AGENTS.md`. For this feature, also
run:

```bash
rustup target add x86_64-pc-windows-gnu
cargo check --target x86_64-pc-windows-gnu
```

When UI changes are implemented, rebuild release and run the isolated Playwright suite
on `LLAMA_MONITOR_TEST_PORT=17778`. Screenshot scenarios run sequentially only.

## Definition of Done

The feature is complete only when:

- Rapid-MLX has full install/version/release-note/upgrade/rollback management;
- current and newer compatible releases are accepted through capability probes;
- llama.cpp behavior from PR #215 is preserved;
- old sessions and presets migrate without user action;
- engine selection, model selection, runtime management, generation settings, and
  review form one coherent wizard;
- spawned and attached Rapid-MLX sessions support readiness, logs, stop, chat, usage,
  reasoning, and capability-aware cancellation;
- the dashboard contains only valid cards with real metrics;
- partial Rapid-MLX telemetry produces a deliberate smaller layout, not placeholders;
- platform limitations are explicit and shared code compiles on all targets;
- secrets are absent from argv, logs, diagnostics, and browser payloads;
- source fixtures and compatibility profiles document upstream behavior;
- reference docs, API docs, tests, screenshot harness, and screenshots are current;
- all mandatory checks pass.

## Explicitly Deferred

The following are not included unless separately designed:

- launching Rapid-MLX on Linux, Windows, or Intel macOS;
- converting GGUF models to MLX;
- a fake shared abstraction for backend flags whose semantics differ;
- a precise MLX unified-memory estimator without a validated formula;
- exposing unsafe/experimental Rapid-MLX flags in the standard wizard;
- generic third-party inference-engine plugins;
- remote-agent Rapid-MLX spawning before the remote agent gains the same backend and
  platform contracts;
- scraping unstable human-oriented CLI output as a permanent model catalog API.

## Source References

Implementation agents must use primary upstream sources and pin source links to the
commit being validated in their PR notes or fixture metadata.

- Rapid-MLX repository: <https://github.com/raullenchai/Rapid-MLX>
- Research baseline commit:
  <https://github.com/raullenchai/Rapid-MLX/tree/be5d8bd65d293a3c667510156f62c6f593b54bf6>
- Package metadata:
  <https://github.com/raullenchai/Rapid-MLX/blob/be5d8bd65d293a3c667510156f62c6f593b54bf6/pyproject.toml>

Before coding against a newer release, update the compatibility profile and fixtures.
Update this document only when verified upstream drift changes the product contract or
implementation instructions.
