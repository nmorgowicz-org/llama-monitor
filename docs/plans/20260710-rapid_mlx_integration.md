# Rapid-MLX Backend Integration

| Field | Value |
|---|---|
| Created | 2026-06-11 |
| Last validated | 2026-07-15 |
| Status | Source-of-truth implementation specification |
| Target base | `main` after PR #215 merges as-is |
| Product scope | Local model launch, lifecycle, chat compatibility, monitoring, presets, updates, and UX |
| Supported host | macOS on Apple Silicon |
| Verified release baseline | Rapid-MLX `v0.10.9` at `3edb3ac69c1d1c5e81836a5d146e5f81048658d9` |
| Provisional upstream signal | Rapid-MLX `main` at `1f710fa5fdd228b168bb896b5e829c8068e52d63` on 2026-07-15; never an install default |
| Compatibility policy | Fast rolling updates through exact-version staged installs, runtime capability detection, atomic activation, and rollback |

## Purpose

Add Rapid-MLX as a first-class inference backend beside llama.cpp. A user must be
able to install, upgrade, roll back, configure, launch, monitor, stop, and chat with
a Rapid-MLX server through the same polished product flow used for `llama-server`.

This document is the implementation contract. Future agents must validate changing
upstream facts before coding, but must not reopen settled product or architecture
decisions without recording a concrete incompatibility and updating this document.
The expected implementers are AI agents working across multiple sessions. Every
milestone therefore produces a tested, named contract for the next milestone instead
of relying on thread-local context or partially wired code.

GGUF-only recovery research has been separated into
`docs/plans/20260715-gguf_to_mlx_conversion_research.md`. That document supersedes the
historical converter recipes retained later in this specification; do not implement
those older recipes as written.

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
4. Rapid-MLX is not permanently pinned to one version. Each managed environment is an
   exact-version install, while the app supports current and historical releases,
   release notes, staged upgrades, rollback, and read-only discovery of external installs.
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
13. Native Rapid-MLX sources are aliases, Hugging Face repositories, and local MLX
    model directories. Rapid-MLX `0.10.9` neither loads GGUF nor ships the previously
    assumed `gguf2mlx` command. GGUF-only finetunes remain a required product use case,
    but conversion must pass a separate research and validation gate for converter
    ownership, architecture coverage, tokenizer/config fidelity, and quantization loss.
14. Rapid-MLX is first implementation target, not permanent shape of backend
    architecture. Shared contracts must remain capable of supporting at least
    one other MLX loader without rewriting session lifecycle, preset schema,
    chat routing, telemetry polling, or dashboard card composition.

## Loader Selection Decision

External survey date: 2026-06-22. This section records current integration
judgment for fresh agents. Re-check upstream before implementation, but do not
change chosen first backend without concrete incompatibility and update to this
document.

### Decision

Implement Rapid-MLX first. Use `vllm-mlx` as reference alternate backend when
designing shared contracts. Do not implement `oMLX`, `vllm-metal`, or `MTPLX`
in this feature branch unless user explicitly expands scope.

Every shared Rust and UI surface added for Rapid-MLX must answer: "could
vllm-mlx be added later by writing a new adapter and compatibility profile,
without changing this shared type?" If answer is no, shared type is too
Rapid-MLX-specific and belongs inside `src/inference/rapid_mlx/`.

### Current project ratings for llama-monitor integration

| Project | Role | Integration rating | Rationale |
|---|---|---:|---|
| Rapid-MLX | First backend | 8.5/10 | Best match for current plan: local single-model serve flow, OpenAI-compatible chat, readiness/status endpoints, runtime install/upgrade paths, tool/reasoning parser focus, and strong Apple Silicon performance claims. |
| vllm-mlx | Reference future adapter | 8/10 | Strongest alternate contract shape: OpenAI and Anthropic APIs, `/health`, `/v1/status`, optional Prometheus `/metrics`, continuous batching, paged/prefix cache, and documented benchmark methodology. |
| oMLX | Watchlist, not first backend | 7/10 | Rich Mac product surface, multi-model serving, menu bar/admin UI, model downloader, tiered SSD KV cache. Overlaps llama-monitor product ownership and appears less clean as headless backend primitive. |
| vllm-metal | Strategic watchlist | 6.5/10 | Important because under `vllm-project` and may become long-term ecosystem path. Current shape is lower-level plugin/runtime path, not first product integration target. |
| MTPLX | Technique watchlist | 5.5/10 backend, 8/10 signal | Native MTP speculative decoding and claimed 1.6x-2.24x speedups are important, but scope is narrower and model-specific. Track as capability inspiration, not initial backend. |

### Performance interpretation

Treat MLX performance gains as plausible but requiring local validation. Public
evidence says MLX-family runtimes can outperform llama.cpp on Apple Silicon,
especially for sustained generation, batching, prefix/paged cache reuse, and
newer model families not equally optimized in llama.cpp. However, published
claims are not directly comparable unless hardware, model weights, quantization,
prompt length, concurrency, thinking mode, sampling, cache state, and tokenizer
accounting match.

Implementation therefore must include an app-owned benchmark/probe path before
claiming speedups in UI or docs. The first release may say Rapid-MLX is an
Apple Silicon native engine and show measured tokens/sec from live telemetry.
It must not promise "2x faster than llama.cpp" unless llama-monitor measured
the same model/task locally and stores enough benchmark metadata to explain the
comparison.

### Required shared contract seams

Design these as backend-neutral contracts in Milestones 1-2. Rapid-MLX and
vllm-mlx should both map cleanly onto them:

- Backend identity: stable enum and persisted preset/session discriminator.
- Runtime discovery: executable, version/source, platform availability,
  compatibility profile, missing extras, and remediation action.
- Launch model: resolved model identifier/path plus backend-specific source
  metadata, not raw wizard inputs.
- Health/readiness: liveness and readiness are separate. Never infer readiness
  from open TCP port or model name alone.
- Chat surface: OpenAI-compatible request/stream path plus capability-filtered
  request controls.
- Anthropic compatibility: optional capability. Do not bake Anthropic-specific
  routing into shared chat contract until backend advertises support.
- Request cancellation: optional capability keyed by backend endpoint shape.
- Status telemetry: normalized optional fields for status, model, uptime,
  running/waiting counts, prompt/generation throughput, token totals, memory,
  cache, and active requests.
- Metrics endpoint: optional capability. Support JSON status and Prometheus
  scrape endpoints as different telemetry sources behind normalized snapshots.
- Dashboard cards: capability-driven registry. Unsupported cards are omitted
  from the DOM, not rendered as empty or permanent `N/A` cards.
- Benchmark evidence: store engine, version, model, quantization/source,
  prompt/output token counts, context length, concurrency, sampling, cache
  state, hardware summary, and measured TTFT/prompt TPS/decode TPS.

### Backend-specific boundaries

Rapid-MLX-specific code owns:

- `rapid-mlx serve` command construction;
- optional independent GGUF converter integration, only after a separate converter
  contract and fidelity gate are accepted;
- Rapid-MLX aliases and model-source rules;
- Rapid-MLX-specific flags, mutual exclusions, extras, and compatibility probes;
- `/health`, `/health/ready`, `/v1/status`, `/v1/cache/stats`, and request
  cancel endpoint parsing;
- Rapid-MLX release/install/rollback details.

Shared code must not mention Rapid-MLX flags, aliases, endpoint field names, or
GGUF conversion mechanics except through adapter-owned types. UI may render
Rapid-MLX labels and controls, but may not reimplement resolver decisions in
JavaScript.

### Future-backend non-goals for this branch

- Do not add a generic plugin ABI.
- Do not add dynamic backend loading.
- Do not implement vllm-mlx, oMLX, vllm-metal, or MTPLX launch paths.
- Do not make UI look like a backend marketplace.
- Do not expose loader-specific experimental flags just because upstream has
  them.

Feature branch succeeds when Rapid-MLX works well and code has one clear
adapter slot where vllm-mlx could be added later.

## Verified Upstream Baseline

The following facts were revalidated against Rapid-MLX release `v0.10.9` at commit
`3edb3ac69c1d1c5e81836a5d146e5f81048658d9`. Revalidate them against every release
selected during implementation. A source install from `main` may still report package
version `0.10.9`, so version text alone is never sufficient evidence.

### Rolling compatibility and safe daily updates

- Start with `minimum_verified = 0.10.9` and `current_verified = 0.10.9`. Pin release
  fixtures to the tag SHA; treat current `main` as informational nightly evidence.
- At discovery and before activation, probe executable path, `--version`, and exact
  `serve --help` tokens. Core syntax, readiness, and configured auth are strict gates;
  optional flags and endpoints are feature-detected.
- Install a new managed release into a versioned staging environment, run bounded
  preflight probes, atomically activate it, and retain the last known-good runtime.
  A failed update never mutates the active runtime.
- Unknown JSON fields are ignored and optional fields are serde-defaulted. Unknown
  enum values degrade the affected capability instead of failing the full snapshot.
  Preserve `Some(0)` versus `None`, reject non-finite/negative values, and bound opaque
  payload sizes.
- External Brew/Pip installs are observed and diagnosed, never automatically changed.
- User-facing compatibility states are `Verified`, `Provisional`, and `Incompatible`,
  with the failed probe and a one-click path back to the tested managed runtime.

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
- `vllm-mlx` is a deprecated CLI alias for `rapid-mlx`; do not hardcode it in command
  construction or probes — detect and use whichever binary is on PATH.
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
| `DELETE /v1/requests/{id}` | API key when configured | Cancellation alias (no `/cancel` suffix) |

Rapid-MLX `0.10.9` does not expose a usable external request-ID contract for those
cancellation routes. Streaming chunks contain a public `chatcmpl-*` response ID, while
the routes expect a separate private scheduler ID held only inside the server's
`request_id_holder`. Therefore the `0.10.9` compatibility profile advertises native
cancellation as unavailable. Stopping chat immediately drops llama-monitor's upstream
response, which triggers Rapid-MLX's disconnect guard and scheduler abort. A future
profile may enable the native route only after an authenticated public scheduler-ID
contract is verified; the endpoint's existence alone is insufficient.

Additional probe-only aliases that bypass authentication (intentional — auth would break
Kubernetes liveness probes): `/healthz` (delegates to `/health`), `/readyz` (delegates to
`/health/ready`), `/livez` (returns `{ status }`).

`/health` includes:

- `status`
- `ready`
- `model_loaded`
- `model_name`
- `model_type`
- `engine_type`
- `mcp`: `{ enabled, servers_connected, servers_total, tools_available }` — omitted or null when MCP is disabled

**`model_loaded` vs `ready`**: `model_loaded` becomes true when the engine object is
created, before warmup, prefix-cache loading, and MCP initialization complete. `ready`
is the authoritative readiness signal. Do not use `model_loaded: true` as a readiness
gate — always use `/health/ready` returning HTTP 200.

`/v1/status` at the research baseline includes:

- `status`: `generating`, `idle`, or `not_loaded`
- `model`: served model name (note: `/health` calls this `model_name`; the names differ)
- `uptime_s`
- `steps_executed`
- `num_running`
- `num_waiting`
- `total_requests_processed`
- `total_prompt_tokens`
- `total_completion_tokens`
- `generation_tps`: always a float; `0.0` when idle, never `null` or absent
- `prompt_tps`: always a float; `0.0` when idle, never `null` or absent
- `metal`: `{ active_memory_gb, peak_memory_gb, cache_memory_gb }` — all values in GB;
  entire object may be absent when Metal telemetry is unavailable
- `cache`: prefix/paged/memory-aware cache object, or `{"enabled": false}` when disabled
- `requests`: array of active request details

**TPS zero vs. absent**: Because `generation_tps` and `prompt_tps` are always present as
`0.0` (never null), a dashboard receiving `generation_tps: 0.0` means the server is idle
and reachable — it does not mean the metric is missing. The throughput card should render
whenever `/v1/status` is reachable, and show zero as zero. The `None` in the normalized
`InferenceMetricsSnapshot` indicates the field was absent from the JSON entirely (i.e.,
polling failed), not that it was zero.

`/v1/cache/stats` response schemas:

- Vision/multimodal model: `{ multimodal_kv_cache, pixel_values_cache, pil_image_cache }`
- Text-only model (graceful degradation when `mlx_vlm` is not loaded):
  `{ message, model_type }` — this shape is a signal that vision cache is unavailable,
  not an error. The cache capability probe must treat the text-only fallback shape as
  "capability absent" and suppress the cache card accordingly.

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
| Identity/network | `--served-model-name`, secure default `--host 127.0.0.1`, `--port 8000`, `--log-level INFO` |
| Capacity | `--max-num-seqs 256`, `--max-concurrent-requests 256` |
| Batching | `--prefill-batch-size 8`, `--completion-batch-size 32`, continuous batching enabled |
| Prefix cache | enabled by default, size `100`, memory MB or percent, memory-aware behavior |
| KV cache | 4/8-bit quantization, default 8-bit, group size 64, minimum quantize tokens 256 |
| TurboQuant | 3/4-bit or auto, group size 32 |
| Streaming | `--stream-interval 1` |
| Limits | max tokens `32768`, `--timeout 1800` seconds |
| Memory | GPU memory utilization `0.90`, paged cache block size 64, `--max-cache-blocks 1000` |
| Prefill | chunked prefill tokens `0`, prefill step size `2048` |
| Speculation | MTP, suffix decoding, and DFlash controls |
| Integrations | MCP config, embedding model, tool parser, auto tool choice, reasoning parser |
| Sampling | default sampling controls |
| Access | API key or `RAPID_MLX_API_KEY`, rate limit |
| Multimodal | MLLM enable/disable behavior |
| Utility | `rapid-mlx doctor`, `rapid-mlx jlens`, `rapid-mlx telemetry`, `rapid-mlx launch` |
| Speculation | MTP, suffix decoding, and DFlash controls; `--mtp-sidecar` |

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

### Active engine indicator

The nav/status bar must always show the active engine while a session is running. A
user switching between SillyTavern, opencode, and the app UI must never have to guess
which engine is serving requests. Display: engine name + model alias + status dot.
Example: `llama.cpp · Qwen3-27B ●` or `Rapid-MLX · gemma-3-4b-it ●`.

When no session is running, the indicator is absent — do not show a placeholder or
"none" state. The indicator appears only when there is something real to show.

For llama.cpp router mode, the indicator shows the model that last received a request
(updated via the router's SSE `model_status: loaded` events), not a static "router"
label.

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
   - Rapid-MLX accepts:
     - an alias
     - Hugging Face repository
     - local MLX model directory
   - A local `.gguf` recommends llama.cpp. If the user explicitly selects Rapid-MLX,
     explain that the runtime cannot load GGUF and offer to switch engines without
     losing the model selection.
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
   - **Memory Safety Warning**: If unified memory allocation exceeds 75%, warn the user that the system may become unstable or crash.
   - **Wired Memory Notice**: Inform users that increasing context window size increases "Wired Memory" (non-pageable), which can lead to abrupt OOMs.

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
~/.config/llama-monitor/runtimes/rapid-mlx/
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
3. Resolve the latest stable release, then use an exact version such as
   `uv tool install rapid-mlx@0.10.9` to create a dedicated, isolated staging runtime.
4. Run `rapid-mlx --version`, capture exact tokens from `rapid-mlx serve --help`, and
   run the core and feature probes against the staged executable.
5. Write metadata including the compatibility profile and probe evidence.
6. Atomically update `current.json` only after all required probes succeed.
7. Retain the previous environment for rollback.
8. Remove a failed staging environment without changing the active runtime.

Do not install a tool named `gguf2mlx` based on this plan. Rapid-MLX does not provide
it. Any future converter is an independent dependency with its own reviewed package,
version, fidelity, security, and cache contract.

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
- status memory telemetry;
- self-diagnostic (doctor);
- interpretability (jlens);
- one-shot launch

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

### Supervisor interface (src/inference/supervisor.rs)

The supervisor is backend-agnostic. It receives a `SupervisedLaunch` bundle and calls back
into a `BackendObserver` for log routing and crash handling. Neither type knows backend
internals.

```rust
/// Everything the supervisor needs to spawn a process. Secrets are safe in `env`
/// — the supervisor never logs env values. `redacted_summary` is the only thing
/// shown in UI diagnostics and logs.
pub struct SupervisedLaunch {
    pub program:           PathBuf,
    pub args:              Vec<OsString>,
    pub env:               Vec<(OsString, OsString)>,
    pub cwd:               Option<PathBuf>,
    pub port:              u16,
    pub redacted_summary:  String,
}

/// Callbacks the supervisor fires back into the backend adapter.
pub trait BackendObserver: Send + Sync + 'static {
    /// Called for every stdout/stderr line from the child process.
    fn on_log_line(&self, line: &str);
    /// Called on unexpected exit (not triggered by stop()).
    fn on_crash(&self, exit_status: std::process::ExitStatus, tail: Vec<String>);
}
```

### Backend adapter interface (src/inference/backend.rs)

The adapter builds the launch bundle and owns all backend-specific behavior.
`BackendAdapter` is an enum so dispatch is exhaustive and zero-overhead.

```rust
pub enum BackendAdapter {
    LlamaCpp(llama_cpp::LlamaCppAdapter),
    RapidMlx(rapid_mlx::RapidMlxAdapter),
}

impl BackendAdapter {
    /// Validate platform, runtime, model source, and flag conflicts before launch.
    pub async fn validate(&self) -> Result<()>;
    /// Build the launch bundle. Called after validate() succeeds.
    pub async fn build_launch(&self) -> Result<SupervisedLaunch>;
    /// Poll until the server is ready to serve requests or the deadline elapses.
    pub async fn await_ready(&self, port: u16, deadline: Instant) -> Result<()>;
    /// Fetch a normalized metrics snapshot. Called by the shared poller loop.
    pub async fn poll_metrics(&self, port: u16) -> Result<InferenceMetricsSnapshot>;
    /// Native request cancellation.
    pub async fn cancel_request(&self, port: u16, request_id: &str) -> Result<()>;
    /// Return the static capability set for the active runtime profile.
    pub fn capabilities(&self) -> &CapabilitySet;
}
```

### llama.cpp launch modes (src/inference/llama_cpp.rs)

llama.cpp has two launch modes that share the same adapter but produce different
`SupervisedLaunch` bundles. Router mode is defined here — not as a separate
`InferenceBackend` variant — because it is still llama-server under the hood.
See `docs/plans/20260618-llama_router_mode.md` for full router mode design.

```rust
pub enum LlamaCppLaunchMode {
    SingleModel(ServerConfig),
    Router(RouterConfig),
}
```

The adapter picks the right arg-builder based on this enum. The supervisor, the
    session layer, and the UI never need to know which mode is active.
    The `llama_cpp` adapter must implement port-prefix log routing and SSE subscription to `/models/sse` for router-mode lifecycle tracking.


**Ownership summary:**

| Concern | Owner |
|---|---|
| Process spawn, PID, kill, wait | supervisor |
| stdout/stderr line streaming | supervisor → `BackendObserver::on_log_line` |
| Crash detection and tail collection | supervisor → `BackendObserver::on_crash` |
| Command/env construction | adapter (`build_launch`) |
| Readiness polling | adapter (`await_ready`) |
| Metrics polling | adapter (`poll_metrics`) |
| Capability set | adapter (`capabilities`) |
| Log line interpretation / filtering | adapter (`on_log_line` implementation) |
| Router port-tag parsing | llama.cpp adapter (`on_log_line`) |
| Secret redaction | adapter (`redacted_summary`, env-not-argv) |

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
    launch_mode       ← "single_model" | "router"
    single_model      ← ServerConfig (present when launch_mode = single_model)
    router            ← RouterConfig (present when launch_mode = router)
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

### Rapid-MLX model source resolver

All Rapid-MLX launch inputs pass through a model-source resolver before command
construction. The base resolver supports native MLX and authoritative safetensors and
rejects GGUF with an actionable llama.cpp handoff. A future Phase 5.5 importer may
produce a validated MLX directory, but its conversion mechanics remain outside this
resolver contract.

```text
RapidMlxModelSource
  -> validate / resolve (or reject with remediation)
  -> ResolvedRapidMlxLaunchModel
  -> rapid-mlx serve <resolved-model>
```

`RapidMlxModelSource` is a tagged enum with these variants:

- `MlxDirectory`: local MLX-format directory.
- `HuggingFaceRepo`: repository ID and optional revision.
- `Alias`: runtime-provided or user-entered alias.
- `GgufFile`: local GGUF file retained as user input so the resolver can recommend
  llama.cpp or an independently available Phase 5.5 import workflow; never launchable
  as native MLX.

`ResolvedRapidMlxLaunchModel` records:

- launch argument passed to `rapid-mlx serve`;
- display name;
- source kind;
- original user input;
- official safetensors-conversion provenance when applicable;
- required environment variables, such as `HF_TOKEN`;
- warnings and remediation hints from validation.

Command construction must consume only `ResolvedRapidMlxLaunchModel`. It must not
inspect raw GGUF paths, run conversions, choose cache directories, or know wizard
selection details.

The resolver owns:

- MLX directory validation;
- HF repo source validation and token handoff metadata;
- alias validation when a known runtime catalog exists, or free-form alias marking
  when it does not;
- actionable rejection of unsupported GGUF input, with a llama.cpp handoff;
- handoff metadata for an independently available Phase 5.5 importer; the base
  resolver never owns reverse-converter mechanics.

The spawn wizard may call resolver validation endpoints to preview states such as
`ready`, `needs_runtime`, or `unsupported_source`. The wizard must not duplicate
resolver rules.

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
3. `/health/ready` succeeds. This is flipped only after the internal sequence: 
   `install_signal_observability()` $\rightarrow$ `GC threshold adjustment` $\rightarrow$ `engine.start()` $\rightarrow$ `Metal shader JIT (generate_warmup)` $\rightarrow$ `_load_prefix_cache_from_disk()` $\rightarrow$ `init_mcp()` $\rightarrow$ `deep_probe_audio_lane()` completes.
4. `/v1/models` resolves the served model when authentication is configured;
5. timeout produces a precise error with redacted recent logs.

Do not equate an open TCP port with readiness.

### Stop and cancellation

- Server stop uses the shared supervised process lifecycle.
- Rapid-MLX stop sends SIGTERM first so its lifespan shutdown can mark draining,
  persist prefix cache state, and stop the engine. Wait up to 10 seconds for process
  exit, then use SIGKILL only as fallback. SIGHUP is diagnostic and must not stop it.
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
| Reasoning content | Existing parser rules | Accept `reasoning` field and `delta.reasoning` |
| Tools | Capability-driven | Capability-driven |
| Structured response | Capability-driven | `response_format` when proven |
| Request seed | Existing behavior | Hidden unless proven |
| Cancellation | Existing behavior | Disconnect abort; native endpoint only with a proven public request ID |
| Reasoning depth | Not supported | `reasoning_effort` parameter |

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

Create a backend-neutral snapshot whose fields are optional. Use `Option<T>` throughout —
`None` means the metric is absent; `Some(0.0)` means it is present and zero. Never
coerce absent to zero.

```rust
// src/inference/metrics.rs
#[derive(Debug, Clone, serde::Serialize)]
pub struct InferenceMetricsSnapshot {
    pub sampled_at:                     std::time::SystemTime,
    pub backend:                        InferenceBackend,
    // Health
    pub health:                         Option<HealthState>,
    pub ready:                          Option<bool>,
    // Identity
    pub model:                          Option<String>,
    pub uptime_seconds:                 Option<f64>,
    // Throughput
    pub generation_tokens_per_second:   Option<f64>,
    pub prompt_tokens_per_second:       Option<f64>,
    // Queue
    pub running_requests:               Option<u64>,
    pub waiting_requests:               Option<u64>,
    // Totals (cumulative)
    pub completed_requests_total:       Option<u64>,
    pub prompt_tokens_total:            Option<u64>,
    pub completion_tokens_total:        Option<u64>,
    pub steps_executed:                 Option<u64>,
    pub global_cache_hit_rate:           Option<f64>,
    pub global_cache_entries:            Option<u64>,
    pub ttft:                            Option<f64>,
    pub speculative_acceptance_rate:     Option<f64>,

    // Memory (always in bytes, regardless of backend source unit)
    pub active_memory_bytes:            Option<u64>,
    pub peak_memory_bytes:              Option<u64>,
    pub cache_memory_bytes:             Option<u64>,
    // Structured opaque payloads — card registry maps these, not raw JSON
    pub cache_metrics:                  Option<serde_json::Value>,
    pub active_requests:                Option<Vec<serde_json::Value>>,
    pub backend_details:                Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum HealthState { Ok, Degraded, NotLoaded, Unreachable }
```

**Unit conversion**: Rapid-MLX `/v1/status` serves `metal.active_memory_gb`,
`metal.peak_memory_gb`, and `metal.cache_memory_gb` in gigabytes (float). The normalized
snapshot stores `active_memory_bytes`, `peak_memory_bytes`, and `cache_memory_bytes`; the
mapper must multiply by `1_073_741_824` (1 GiB) when converting. Do not pass GB values
through as bytes.

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

Note: Metrics like `ttft` and `speculative_acceptance_rate` are provided by `llama.cpp` but omitted for `Rapid-MLX` unless natively supported.

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
| Thinking UI | `delta.reasoning` stream | Render as an expandable accordion |
| Live Progress | `/v1/status` $\rightarrow$ `progress` | Render as a real-time progress bar |

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

Rapid-MLX natively expects MLX-format models:

- weights in safetensors (e.g. weights.00.safetensors / model.safetensors)
- config.json
- tokenizer files

This integration does not claim native or automatic GGUF support. Users can keep
running existing GGUFs with llama.cpp or select a native MLX/Hugging Face source for
Rapid-MLX.

### MLX local directory

Validate:

- path exists and is a directory;
- readable model metadata/config files are present;
- validation errors name the missing requirement;
- launch still defers final architecture support to Rapid-MLX and reports its
  redacted error accurately.

### GGUF local file (for Rapid-MLX)

Rapid-MLX `0.10.9` does not load GGUF. Return an actionable unsupported-source result,
recommend llama.cpp, and preserve the user's model selection during the engine switch.
Do not guess tensor mappings, reconstruct tokenizer/config files, or silently invoke an
unverified third-party converter.

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

## GGUF Conversion Research Gate

Rapid-MLX `0.10.9` does not provide a GGUF load path or reverse converter. GGUF-only
finetunes remain a desired product capability, but they are not part of the initial
Rapid-MLX release gate and must never be implemented from historical recipes in this
document.

The normative research, source-quant guidance, tool audit, fail-closed cache contract,
architecture promotion gates, and future Import Lab UX are in
`docs/plans/20260715-gguf_to_mlx_conversion_research.md`. Until that track passes its
own gates:

- run GGUF directly with llama.cpp;
- preserve the selected model when recommending an engine switch;
- use authoritative original/merged safetensors with pinned official `mlx_lm.convert`
  for supported MLX conversion;
- do not install, discover, or invoke a third-party reverse converter;
- do not create GGUF-derived caches or advertise automatic GGUF conversion.

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

## Codebase Integration Notes

This section resolves the seven implementation gaps identified during plan review using
concrete patterns from the existing codebase. Implementation agents must read these
before touching any of the affected subsystems.

### Gap 1 — Python interpreter selection

The existing app has no Python detection code. All current managed tools are native
Rust or pre-compiled binaries. Rapid-MLX introduces the first Python dependency.

Resolution strategy, evaluated in order at install time:

1. `python3` on `PATH` — resolve to absolute path, then check architecture: run
   `python3 -c "import platform; print(platform.machine())"` without shell
   (`TokioCommand::new`). Accept only `arm64`; reject `x86_64` (Rosetta Python
   silently runs under Rosetta and produces non-native MLX code).
2. `/opt/homebrew/bin/python3` — Homebrew Apple Silicon default. Same arch check.
3. `/usr/bin/python3` — system Python. Same arch check. Minimum version 3.10.

If no candidate passes, surface a **Needs repair** state with:
- which Python was found (path) and why it was rejected (wrong arch, too old);
- a link to `https://brew.sh` as the recommended installation path;
- a manual-path override field in the runtime card (same pattern as
  `llama_server_path` in `AppConfig`).

The selected interpreter path is recorded in `metadata.json` alongside the venv
path so the runtime can be reproduced and probed without re-resolving. Never call
`python` (without the `3` suffix) — it is absent or aliased to Python 2 on many
macOS systems.

### Historical Gap 2 — gguf2mlx installation scope (research pending)

The initial assumption that a reverse converter could share the managed Rapid-MLX
environment was rejected on 2026-07-15. Do not install or probe `gguf2mlx` as part of
Rapid-MLX setup. Any future reverse converter is an independent, exact-version
dependency owned by the Experimental GGUF Import Lab after its R0 tool-selection gate.

### Gap 3 — HF download ownership

Two existing HF download patterns in the codebase set the precedent:

**App-managed download** (`src/model_download.rs`): Used when the user picks a GGUF
file from the model browser. The app streams the file via `hf_hub` crate
(`ApiBuilder::with_token`) and tracks byte-level progress through
`Arc<AtomicU64>` counters. This path owns the file on disk.

**Server-delegated download** (`src/llama/server.rs` line 414): When the user
passes an HF repo to llama.cpp, the app sets `-hf <repo>` on the spawned process
and llama-server handles the download itself. The app only sees download activity
through log lines.

For Rapid-MLX, use **server-delegated download**: `rapid-mlx serve <hf-repo-id>`
initiates its own download via the MLX hub cache (`~/.cache/huggingface`). The app:

- passes the HF token to the subprocess via `cmd.env("HF_TOKEN", token)` (parallel
  to `hf_load_token()` which is already called in `src/web/api.rs:1941`);
- captures download progress from the server's stdout/stderr log stream (same
  `BufReader`/`lines()` path used for llama.cpp log capture);
- displays a "Downloading model..." status in the readiness phase until
  `/health/ready` returns 200.

Do **not** use `model_download.rs` for Rapid-MLX HF model downloads. That path is
for individual GGUF file acquisition and is not designed for repo-level MLX downloads.
In the runtime/model storage settings area, display the
`~/.cache/huggingface/hub/` directory size as informational only; do not manage it.

### Historical Gap 4 — Disk space pre-check before GGUF conversion (deferred)

The historical flat file-size multiplier was rejected because it is unreliable across
quantizations. The research plan now owns parameter-aware FP16 staging, final-output,
temporary-shard, and safety-margin estimates plus cross-platform free-space checks.

### Gap 5 — Incomplete conversion cleanup

Deferred to the Experimental GGUF Import Lab. Its normative contract requires staged
same-filesystem output, manifest validation, atomic promotion, explicit `.converting`
and `.complete` states, bounded diagnostics, and cleanup that can never make an
interrupted output launchable. Sentinel disappearance alone is not success.

### Gap 6 — served-model-name for converted GGUFs

Native MLX/HF/alias sources use the runtime-reported model identity unless the user
explicitly sets `served_model_name`. GGUF-derived naming and provenance labels are
deferred to the Import Lab and must identify both source quantization and output MLX
recipe rather than applying a generic `(MLX)` suffix.

### Gap 7 — Polling interval

The existing llama.cpp poller (`src/llama/poller.rs`) uses `poll_interval` passed
from `app_config.llama_poll_interval`. That value:

- **Default**: 1 second (`default_llama_poll_interval()` in `state.rs:477`, confirmed
  by all six test configs in `main.rs` hardcoding `llama_poll_interval: 1`).
- **Sleep mode**: 15 seconds (`default_sleep_llama_interval_secs()` in `state.rs:309`),
  applied inside the poller loop when `*state.sleep_mode.borrow()` is true.

The Rapid-MLX poller (`src/inference/rapid_mlx/poller.rs`) must use the same
`app_config.llama_poll_interval` field and the same sleep-mode override. No new
configuration field is needed.

Rapid-MLX polling makes up to **three** HTTP calls per cycle (`/health`, `/v1/status`,
optionally `/v1/cache/stats`). To avoid thundering-herd effects, spread them with
a 200 ms gap within the same cycle rather than firing all three simultaneously.
Apply independent per-endpoint timeouts (2 s for `/health`, 3 s for `/v1/status`,
2 s for `/v1/cache/stats`). A timeout on one endpoint must not cancel the others or
mark the session unhealthy; only `/health` failure degrades session state.

### Gap 8 — API key secret handling

For llama.cpp, the API key is currently passed as `--api-key <key>` argv (line 728,
`src/llama/server.rs`), making it visible in process listings. Rapid-MLX supports
the `RAPID_MLX_API_KEY` environment variable as an alternative, which keeps the key
out of argv. Use env for Rapid-MLX:

```rust
if let Some(ref key) = config.api_key {
    cmd.env("RAPID_MLX_API_KEY", key);
    // Do NOT also pass --api-key; env var takes precedence and keeps key out of argv.
}
```

The secret-store mechanism (`encrypt_value` / `decrypt_value` in `src/config.rs`)
is already used for API tokens and ACME DNS credentials. Use the same mechanism to
persist the Rapid-MLX API key in the preset's `rapid_mlx` section. The HF token is
retrieved via `hf::hf_load_token()` (used at `api.rs:1941, 2432`) and passed to
the subprocess via `cmd.env("HF_TOKEN", token)` — follow the same call site pattern
for Rapid-MLX.

### Atomic write conventions

All JSON files in the codebase use a consistent atomic write pattern:

```rust
let tmp = path.with_extension("json.tmp");
serde_json::to_writer_pretty(&std::fs::File::create(&tmp)?, &value)?;
std::fs::rename(&tmp, &path)?;
```

This pattern is in `save_sessions` (`state.rs:607`), `ModelTags::save`
(`state.rs:94`), and `save_tls_config` (`config.rs:368`). All new files written by
the Rapid-MLX runtime manager (`metadata.json`, `current.json`, `releases.json`) must
use this same pattern. `harden_file_permissions` (`config.rs:18`) must be called on
`metadata.json` since it may contain the selected Python interpreter path and
version info (not a secret, but good hygiene matching the encryption-key file).

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

This work is expected to land on one feature branch, but it must be built as
contract-complete milestones. A milestone may contain several local fixups while an
agent is working, but the durable branch history should describe meaningful layers,
not every prompt turn. Do not begin the next milestone with known failing tests from
the previous milestone.

### Milestone 0: Baseline contracts and fixtures

Fixtures are static JSON files under `tests/fixtures/rapid_mlx/`. Each file is named
after the endpoint or CLI command it captures, e.g. `health_loaded.json`,
`status_generating.json`, `cli_help.txt`. Rust unit tests reference these files
directly via `include_str!` — they must never require a live Rapid-MLX install to run.

1. Merge/rebase onto PR #215's final mainline result.
2. Capture Rapid-MLX CLI help and endpoint fixtures for the current stable release.
   - `tests/fixtures/rapid_mlx/cli_help.txt` — `rapid-mlx serve --help` output
   - `tests/fixtures/rapid_mlx/health_loaded.json` — `/health` when model is loaded
   - `tests/fixtures/rapid_mlx/health_unloaded.json` — `/health` before model loads
   - `tests/fixtures/rapid_mlx/status_generating.json` — `/v1/status` during generation
   - `tests/fixtures/rapid_mlx/status_idle.json` — `/v1/status` when idle
   - `tests/fixtures/rapid_mlx/cache_stats_vision.json` — `/v1/cache/stats` (vision model)
   - `tests/fixtures/rapid_mlx/cache_stats_text.json` — `/v1/cache/stats` (text-only fallback)
   - `tests/fixtures/rapid_mlx/models.json` — `/v1/models` response
   - `tests/fixtures/rapid_mlx/stream_chunk.jsonl` — representative streaming chunks
3. Add the first compatibility profile using the reviewed commit plus current stable.
   (`src/inference/rapid_mlx/compatibility.json`)
4. Add fixture tests proving tolerant parsing of every fixture file above.
5. Record any upstream drift in this document before implementation relies on it.

Exit criteria:

- every required upstream claim has a source or fixture;
- no implementation depends on an undocumented guessed field;
- parser tests can run without a live Rapid-MLX install;
- fixture directory exists and is referenced in `tests/README.md`.

### Milestone 1: Backend identity without behavior change

1. Add `InferenceBackend`.
2. Add `backend` to session modes with missing values defaulting to `llama_cpp`.
3. Add preset schema migration that reads legacy flat presets and converts them to
   the new internal shape.
4. Introduce typed spawn configuration while still accepting legacy flat spawn
   payloads.
5. Add fixtures for old sessions, old presets, tagged new presets, unknown future
   fields, and legacy spawn payloads.

Exit criteria:

- all existing llama.cpp tests pass;
- old persisted data loads as llama.cpp;
- old spawn payloads still launch llama.cpp;
- saved presets write the new schema only after the user saves or edits them.

### Milestone 2: Shared supervisor boundary

1. Define `SupervisedLaunch`, `BackendObserver`, and `BackendAdapter` as described in
   the Backend Architecture section. These are the durable contracts; get them right
   before wiring anything to them.
2. Extract shared process lifecycle, stdout/stderr streaming, exit watching, stop
   escalation, and redacted command summaries into `supervisor.rs`.
3. Route llama.cpp through `BackendAdapter::LlamaCpp` and the shared supervisor.
4. Introduce `LlamaCppLaunchMode { SingleModel(ServerConfig), Router(RouterConfig) }`.
   Wire `SingleModel` only; `Router` is the stub for the router mode feature
   (see `docs/plans/20260618-llama_router_mode.md`). The stub must compile and
   serialize/deserialize cleanly.
5. Keep llama.cpp command construction and endpoint polling behavior unchanged.
6. Fix llama.cpp API key: move from `--api-key <argv>` to `LLAMA_SERVER_API_KEY` env
   var at this boundary, consistent with Rapid-MLX's secret handling in Gap 8.
7. Rename newly shared state around "inference process" where practical, but avoid a
   broad state refactor that does not help Rapid-MLX.
8. Add regression tests for PR #215 launch, restore, preset, stop, logs, readiness,
   and polling.

Exit criteria:

- `SupervisedLaunch`, `BackendObserver`, and `BackendAdapter` exist as compilable types;
- `LlamaCppLaunchMode` is defined and round-trips through preset serialization;
- the UI is behaviorally unchanged for llama.cpp single-model mode;
- no Rapid-MLX branches exist inside llama.cpp command construction;
- the shared supervisor can spawn a process without knowing backend-specific flags;
- llama.cpp API key no longer appears in argv.

### Milestone 3: Rapid-MLX runtime and Python capability

1. Add Apple Silicon macOS platform detection and unavailable states for other
   platforms.
2. Add explicit path, managed current, and PATH runtime discovery.
3. Add Python `arm64` and version detection.
4. Add managed venv creation.
5. Install the exact approved Rapid-MLX version into a staged managed venv.
6. Add core compatibility probes and optional runtime capability probes.
7. Add cached probe diagnostics.
8. Add minimal runtime UI/API states: compatible, limited, needs repair,
   unsupported, not installed, probe failed.

Exit criteria:

- a runtime can be resolved and diagnosed without launching a user model;
- runtime capabilities come from the pinned profile plus live probing;
- failed managed install cannot change the selected runtime;
- shared code still compiles on Linux and Windows.

### Milestone 4: Rapid-MLX model-source resolver

1. Add `RapidMlxModelSource`.
2. Add `ResolvedRapidMlxLaunchModel`.
3. Validate MLX local directories.
4. Resolve HF repository sources and token handoff metadata.
5. Resolve runtime aliases without scraping unstable decorative output.
6. Convert authoritative original/merged safetensors with pinned official
   `mlx_lm.convert`, using provenance-keyed staging and atomic promotion.
7. Reject `.gguf` as a native Rapid-MLX source with an actionable llama.cpp fallback
   and a link to the separate experimental Phase 5.5 track.
8. Add resolver preview states for the wizard.

Exit criteria:

- local MLX, HF repo, alias, and authoritative safetensors inputs resolve to a launch
  model or a precise blocked state;
- official safetensors conversion can be tested independently of spawn and wizard UI;
- staged outputs cannot be mistaken for complete models;
- GGUF is rejected without modifying the original selection.

### Milestone 5: Rapid-MLX spawn, readiness, logs, and stop

1. Implement Rapid-MLX command construction from `ResolvedRapidMlxLaunchModel`.
2. Pass `RAPID_MLX_API_KEY` and `HF_TOKEN` through environment variables only.
3. Implement launch validation and mutual-exclusion checks.
4. Implement `/health` and `/health/ready` readiness.
5. Implement `/v1/models` model identity handling.
6. Stream runtime and official safetensors-conversion diagnostics into the existing
   log surface.
7. Implement stop lifecycle through the shared supervisor.
8. Preserve exact runtime identity in the spawned session.

Exit criteria:

- a local MLX model, HF repository, alias, and officially converted authoritative
  safetensors model can launch;
- readiness does not rely on TCP-open checks;
- secrets do not appear in argv, logs, process summaries, API responses, or
  diagnostics;
- failed launch removes the reserved session cleanly.

### Milestone 6: Chat compatibility and cancellation

1. Add backend capability matrix for request fields.
2. Send only Rapid-MLX-supported request controls.
3. Hide or preserve-with-explanation unsupported saved fields, including `seed`
   unless proven supported.
4. Accept streaming `reasoning_content`.
5. Accept usage in final chunks and dedicated usage chunks.
6. Add backend-native cancellation when request IDs and capability exist.
7. Add mocked stream tests.

Exit criteria:

- Rapid-MLX chat streams normally from a spawned or attached compatible endpoint;
- usage and reasoning content do not create empty or duplicate UI;
- unsupported request fields are preserved in presets but not transmitted.

### Milestone 7: Engine-aware wizard and preset editor

1. Add engine cards and deterministic recommendation.
2. Keep recommendation and selected engine as separate state.
3. Add runtime step with install/repair/probe states.
4. Add model step for MLX directory, HF repo, alias, authoritative safetensors, and
   GGUF engine-handoff/import-lab guidance.
5. Add backend-specific controls and capability-gated visibility.
6. Keep shared values when changing engines and preserve backend-local values when
   switching away and back.
7. Update preset save/edit/restore parity in the spawn wizard and welcome-screen
   preset editor.
8. Add review-step summaries for engine, runtime, model source, official conversion or
   import eligibility status, network exposure, and non-default tuning.

Exit criteria:

- explicit engine choice is never overwritten by recommendation changes;
- GGUF is never presented as natively launchable; any Import Lab eligibility state is
  owned by the separate Phase 5.5 contract;
- both preset editors round-trip llama.cpp and Rapid-MLX presets;
- blocked states prevent silent wizard progress.

### Milestone 8: Rapid-MLX polling and dashboard composition

1. Add normalized optional telemetry.
2. Add Rapid-MLX poller using `/health`, `/v1/status`, optional `/v1/cache/stats`,
   and `/v1/models`.
3. Add initial Rapid-MLX cards from real status/cache data.
4. Introduce the smallest card registry abstraction needed to avoid mounting invalid
   llama.cpp cards for Rapid-MLX.
5. Preserve all existing llama.cpp cards.
6. Add first-sample, stale, and sustained-loss removal behavior.
7. Add responsive grid checks for smaller Rapid-MLX layouts.

Exit criteria:

- Rapid-MLX shows only cards backed by real data;
- llama.cpp retains all existing cards;
- numeric zero and absent metric are rendered differently;
- no hidden dead controls or placeholder cards remain in the DOM.

### Milestone 9: Runtime manager polish

1. Add GitHub release metadata and cache.
2. Add install latest and selected historical release.
3. Add switch among installed managed releases.
4. Add rollback.
5. Add inactive managed release deletion.
6. Add repair/re-probe.
7. Add external upgrade where the source supports safe automation.
8. Add release notes with sanitized markdown rendering.

Exit criteria:

- install, upgrade, switch, rollback, repair, and diagnostics work;
- a failed install cannot damage the selected runtime;
- no version is treated as uniquely supported;
- runtime mutation is blocked while a live session uses the affected runtime.

### Milestone 10: Documentation, screenshots, and release validation

1. Update `docs/reference/spawn-wizard.md`.
2. Update `docs/reference/dashboard.md`.
3. Update `docs/reference/api.md`.
4. Update CLI/runtime reference documentation.
5. Extend `tests/ui/capture.mjs` and `tests/ui/README.md`.
6. Add promoted screenshots only after final UI validation.
7. Update README only if the finished multi-engine launcher meets the README's
   high-impact feature bar.
8. Run the repository's mandatory pre-PR checks in order.

Exit criteria:

- reference docs describe the finished behavior as if it always existed;
- screenshot and UI tests match the final selectors and flows;
- mandatory checks pass locally before the branch is pushed or marked ready.

## AI Agent Handoff Rules

This implementation is expected to be shared by Codex, Claude, and local models.
Treat this document, checked-in fixtures, and tests as the handoff surface. Do not
depend on unstated context from a previous agent session.

### Layer boundary cheatsheet

Before writing any code, locate which layer owns the concern. If two layers seem to
share a concern, the boundary is wrong — resolve it here first.

| Concern | File | Never in |
|---|---|---|
| Process spawn / kill / wait | `supervisor.rs` | adapters, API handlers |
| stdout/stderr line forwarding | `supervisor.rs` | adapters |
| Log line interpretation, filtering | adapter `on_log_line` | supervisor |
| Router port-tag parsing (`[PORT]`) | `llama_cpp.rs` `on_log_line` | supervisor |
| Command/env construction | adapter `build_launch` | supervisor, API handlers |
| Readiness polling | adapter `await_ready` | supervisor |
| Metrics polling | adapter `poll_metrics` | supervisor |
| Capability set | adapter `capabilities` | supervisor, UI JS |
| Secret values | env vars in `SupervisedLaunch.env` | argv, logs, API responses |
| Preset migration | `inference/backend.rs` migration fn | session restore path |
| Model source resolution | `rapid_mlx/` resolver | wizard JS, API handlers |
| Card visibility decision | card registry + snapshot fields | CSS, static JS flags |
| Experimental GGUF import | Phase 5.5 importer/profile boundary | base Rapid-MLX resolver, wizard, supervisor, chat layer |

### Hard rules

- Start each session by identifying the active milestone and reading the relevant
  fixture/tests before editing.
- Do not start a later milestone while the previous milestone has known failing
  tests, unresolved migrations, or TODO-only placeholders in the contract surface.
- Do not add UI controls before backend capability detection exists.
- Do not add backend flags before the active compatibility profile or live probe
  proves the flag exists.
- Do not duplicate resolver rules in the wizard. The UI previews resolver state; it
  does not reinterpret model paths.
- Do not add Rapid-MLX branches inside llama.cpp command construction.
- Do not use broad cleanup commits to hide milestone work. Keep unrelated refactors
  out unless they are required by the milestone contract.
- If upstream Rapid-MLX behavior differs from this document, update fixtures and this
  document before wiring product code to the new behavior. Reverse-converter drift is
  handled only in the separate 2026-07-15 research plan.
- If a milestone creates generated files, include them in that milestone's commit.

Preferred durable commit shape:

```text
refactor(spawn): add inference backend identity and migrations
refactor(spawn): route llama.cpp through supervised inference launch
feat(spawn): add Rapid-MLX runtime discovery and probes
feat(spawn): add managed Rapid-MLX Python environment
feat(spawn): add Rapid-MLX model source resolver
feat(spawn): launch Rapid-MLX sessions
feat(chat): support Rapid-MLX streaming and capabilities
feat(wizard): add engine-aware model and runtime flow
feat(settings): manage Rapid-MLX runtimes
feat(dashboard): add Rapid-MLX telemetry cards
docs(spawn): document Rapid-MLX engine support
test(ui): cover Rapid-MLX launch and dashboard states
```

The exact commit list may change, but each kept commit should describe a durable
layer. Use local fixups and squashing while working instead of committing every
agent correction as a permanent branch step.

## AGENTS.md Maintenance

`AGENTS.md` should change as part of this implementation, but only for durable repo
rules that future agents must follow after the feature exists. Do not copy this
entire plan into `AGENTS.md`; keep detailed Rapid-MLX design and milestone tracking
in this document. `AGENTS.md` should stay short enough to be operational guidance
for any future task.

Recommended timing:

- Milestone 1 or 2: add a short "Inference backend architecture" rule once
  `InferenceBackend`, migrated sessions/presets, and the shared supervisor exist.
- Milestone 4: add the model-source resolver rule once `RapidMlxModelSource` and
  `ResolvedRapidMlxLaunchModel` are real contracts.
- Milestone 8 or 10: add dashboard/capability rules after the card registry and
  Rapid-MLX telemetry behavior are implemented.

Do not update `AGENTS.md` before the referenced code contracts exist. Premature
rules make later agents chase aspirational structure that may drift during
implementation.

Recommended `AGENTS.md` additions:

- Backend boundaries:
  - `llama.cpp` and Rapid-MLX are named inference backends selected through
    `InferenceBackend`.
  - Do not add Rapid-MLX conditionals inside llama.cpp command construction.
  - Shared process lifecycle belongs in the inference supervisor; backend adapters
    own command construction, readiness, telemetry parsing, and capability mapping.
- Preset/session compatibility:
  - Missing backend fields in old sessions and presets default to `llama_cpp`.
  - New backend-specific settings must live in the backend section, not in a growing
    flat preset shape.
  - Legacy spawn payloads remain accepted until a documented migration removes them.
- Rapid-MLX model resolution:
  - All Rapid-MLX model inputs flow through `RapidMlxModelSource` and resolve to
    `ResolvedRapidMlxLaunchModel` before command construction.
  - The base resolver rejects GGUF with an engine handoff. The future Phase 5.5
    importer owns conversion, hashing, cache, validation, and cleanup rules.
  - Command builders must never run conversion or inspect raw GGUF paths.
- Capability-gated UI:
  - Only mount controls and dashboard cards whose backend capability and metric data
    exist.
  - Preserve unsupported saved settings, but do not transmit unsupported request
    fields to a backend.
  - A present numeric zero is valid data; an absent metric is not zero.
- Python/runtime safety:
  - Invoke Python, pip, Rapid-MLX, Homebrew, and Git without a shell.
  - Managed Rapid-MLX runtimes use side-by-side environments and atomic activation;
    never mutate the active runtime in place.
  - Do not install an experimental reverse converter into the managed Rapid-MLX venv.
- Validation additions:
  - If `src/inference/**`, preset/session schema, or platform guards change, include
    the Windows cross-compile check before marking the PR ready.
  - If a new JS module is added for engine/runtime UI, update the JS module baseline.
  - Real Rapid-MLX smoke tests remain Apple-Silicon and network/hardware gated;
    generic CI uses fixtures and mocked flows.

When the implementation branch updates `AGENTS.md`, re-check the PR body override
requirements. `AGENTS.md` changes are usually not release-note entries by themselves,
but the Rapid-MLX work will almost certainly need a multi-line
`BEGIN_COMMIT_OVERRIDE` block because it contains multiple user-facing `feat` items.

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
- remote attach detection ambiguity;
- authoritative safetensors → MLX conversion flow with pinned mlx-lm, provenance,
  staging, atomic promotion, and failure handling;
- GGUF resolver rejection preserves the selected source and recommends llama.cpp.

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
- protected API-key values are never reflected into normal DOM/API snapshots;
- GGUF engine-handoff or experimental Import Lab hint shows when appropriate for
  Rapid-MLX without promising conversion support.

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
11. GGUF selection under Rapid-MLX returns an actionable unsupported-source result
    and preserves selection while switching to llama.cpp (mocked).

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
- GGUF input is routed honestly to llama.cpp unless a separately approved converter
  contract is implemented;
- source fixtures and compatibility profiles document upstream behavior;
- reference docs, API docs, tests, screenshot harness, and screenshots are current;
- all mandatory checks pass.

## Explicitly Deferred

The following are not included unless separately designed:

- launching Rapid-MLX on Linux, Windows, or Intel macOS;
- a fake shared abstraction for backend flags whose semantics differ;
- a precise MLX unified-memory estimator without a validated formula;
- exposing unsafe/experimental Rapid-MLX flags in the standard wizard;
- generic third-party inference-engine plugins;
- remote-agent Rapid-MLX spawning before the remote agent gains the same backend and
  platform contracts;
- scraping unstable human-oriented CLI output as a permanent model catalog API.
- automatic GGUF-to-MLX conversion until an independent converter package/version,
  fidelity standard, and corruption-safe cache design are approved.

## Source References

Implementation agents must use primary upstream sources and pin source links to the
commit being validated in their PR notes or fixture metadata.

- Rapid-MLX repository: <https://github.com/raullenchai/Rapid-MLX>
- Verified release baseline:
  <https://github.com/raullenchai/Rapid-MLX/releases/tag/v0.10.9>
- Package metadata:
  <https://github.com/raullenchai/Rapid-MLX/blob/3edb3ac69c1d1c5e81836a5d146e5f81048658d9/pyproject.toml>
- Verified CLI source:
  <https://github.com/raullenchai/Rapid-MLX/blob/3edb3ac69c1d1c5e81836a5d146e5f81048658d9/vllm_mlx/cli.py>
- Verified health/status source:
  <https://github.com/raullenchai/Rapid-MLX/blob/3edb3ac69c1d1c5e81836a5d146e5f81048658d9/vllm_mlx/routes/health.py>

Before coding against a newer release, update the compatibility profile and fixtures.
Update this document only when verified upstream drift changes the product contract or
implementation instructions.
