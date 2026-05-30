# Spawn Llama-Server V2 — Overview, Goals, and Phased Plan

- **Branch:** `feature/spawn-llama-server-v2`
- **Date:** 2026-05-29
- **Author:** Iris (via Hermes)

This is the canonical starting point for the "Spawn Llama-Server V2" overhaul.
If you are an AI agent starting work on this feature, start here.

For a concise, self-contained reference (optimized for fresh AI agents), see:
- `docs/plans/20260529-spawn-v2-reference.md`

For implementation details, see:
- Backend: `docs/plans/20260529-spawn-v2-backend.md`
- Frontend: `docs/plans/20260529-spawn-v2-frontend.md`
- Research and tuning reference: `docs/plans/20260529-spawn-v2-research.md`

---

## For AI agents: how to use this doc

- Read this doc first:
  - Understand goals, architecture, and phased plan.
- Then:
  - For backend work: read `20260529-spawn-v2-backend.md`.
  - For frontend/wizard work: read `20260529-spawn-v2-frontend.md` and its “UI/UX Design Guidelines (Critical)”.
  - For llama.cpp tuning, third-party compatibility, VRAM/MoE guidance: read `20260529-spawn-v2-research.md`.
- Always:
  - Follow the security rules in this doc and the backend doc.
  - Follow the UI/UX guidelines in the frontend doc (mandatory, not optional).

Each phase below links directly to the relevant sections in the other docs so you can jump in and implement.

---

## Goals

These are the concrete milestones for this feature.

### M1. Welcome screen: connect or spawn

On first launch / welcome screen:

- User can:
  - Select a previous remote endpoint.
  - Enter a new endpoint.
- On the right side:
  - Option to spin up a new llama-server.
  - Option to spin up using an existing preset.

**Status:** partially implemented.
**Goal:** unify into a single coherent welcome flow with premium UI.

### M2. Modern, premium "Spawn Llama-Server" UX

New "Spawn Llama-Server" experience must:

- Match the 2026 UI/UX style (glassmorphism, premium modals, micro-interactions).
- Be intuitive, guided, and visually consistent.

**Status:** basic modals / forms.
**Goal:** redesign as a step-based wizard modal with clear sections, inline help, tooltips, and consistent design tokens.

### M3. Import existing launch settings (batch file / script)

Support importing a user's existing launch file:

- Windows: `.cmd` / `.bat`
- macOS/Linux: `.sh` / `.bash` / `.zsh`

System must:

- Parse the script to extract llama-server binary path and all flags.
- Normalize (newlines, line continuations, env artifacts).
- Adapt (binary path, OS-specific quoting).

**Goal:** produce a clean, editable preset from an imported script.

### M4. Advanced parameter editing (dual mode)

Provide two modes for editing launch parameters:

- **Raw mode:** show the generated launch script (for the current OS) in an editable code area.
- **Guided mode:** structured UI with grouped parameters, inline descriptions, tooltips, validation, and suggestions.

Both modes must stay in sync (edits in one update the other).

**Goal:** make tuning approachable for both power users and casual users.

### M5. Chat template support (.jinja / HF / GitHub)

Support:

- Uploading a `.jinja` chat template file.
- Specifying a HuggingFace URL.
- Specifying a GitHub Gist / repo path.

System must:

- Fetch or load the template.
- Map it to `--chat-template-file` when spawning llama-server.

**Goal:** allow flexible, external chat templates without manual filesystem work.

### M6. Multiplatform model selection modal

Provide a modern, multiplatform model selection modal:

- Point to a local or shared directory containing GGUF models.
- Allow browsing and selecting models.
- Integrate with existing `scan_models_dir` and `/api/models`.

**Goal:** make model selection feel native and polished on all platforms.

### M7. HuggingFace model pulling with quant selection

Allow:

- Pointing to a HuggingFace repo.
- Selecting a quantization variant.
- Downloading directly to the user's configured model directory.

Requirements:

- Use HF Hub API (via `hf-hub` Rust crate) to list GGUF files.
- Respect `HUGGING_FACE_HUB_TOKEN` for gated models.
- Show file size, quant label, and basic metadata.
- Support large-file streaming with progress and resume.

**Goal:** seamless "discover → select → download → use" path inside the app.

### M7b. HF repo-based model loading (-hf)

Support llama-server’s `-hf` flag:

- Allow specifying a HuggingFace repo ID (e.g., `user/repo[:quant]`) as the model source.
- Let llama-server handle HF model loading and mmproj auto-download when appropriate.
- Integrate with the wizard so users can:
  - Paste a HF repo.
  - Choose a quant.
  - Spawn using `-hf` instead of a local `-m` path.

**Goal:** first-class HF integration without requiring manual downloads in many cases.

### M8. VRAM estimation and trade-off guidance

Implement estimation logic that:

- Uses model size / quantization + context size + KV quantization + speculative decoding settings + batch size to estimate VRAM usage.
- Compares against available VRAM (from existing GPU monitoring).
- Advises:
  - If the current config fits.
  - If it risks spilling into system RAM (performance hit).
  - What to adjust (reduce context size, use a different quantization, tune batch/ubatch).

For multimodal models:

- Account for mmproj size.

For MoE models:

- Account for `--n-cpu-moe` and its effect on RAM vs VRAM.

**Goal:** give users intelligent, quantitative guidance before launch.

### M8b. Multimodal and KV cache quantization support

- Expose:
  - `--mmproj` for multimodal projector (vision/audio).
  - `-ctk` / `-ctv` for KV cache quantization (K/V).
- Integrate into wizard:
  - Auto-detect or prompt for mmproj when model is multimodal.
  - Offer KV cache quantization options when context is large or VRAM is tight.
- Wire into VRAM estimator:
  - Adjust estimates based on mmproj size and KV quantization.

**Goal:** support advanced memory and multimodal configurations safely and intuitively.

### M9. MoE expert offload tuning

For MoE models:

- Expose `--n-cpu-moe` as a tunable option.
- Offer suggestions:
  - How many experts to keep in VRAM vs RAM.
  - Impact on throughput and latency.

**Goal:** make MoE tuning accessible without deep internal knowledge.

### M10. Health check / benchmark button

Provide an internal tooling button:

- "Run Health Check / Benchmark"

Behavior:

- Uses the loaded AI model to run a short, controlled benchmark:
  - Prompt throughput.
  - Generation throughput.
  - Latency.
- Optionally:
  - Interprets results with AI assistance to suggest tuning.

Constraints:

- Must be opt-in.
- Must be safe (bounded tokens, time, no destructive actions).

**Goal:** help users validate that their configuration is sane and performant.

### M10b. Safety/limits and API-key support

- Expose:
  - `-n / --n-predict` as “Max response tokens” (safety/limits).
  - `--api-key` to protect llama-server when exposed on the network.
- Integrate into wizard:
  - Provide a sensible default for max_tokens (e.g., 2048–4096).
  - Provide an optional field for API-key when user chooses to expose server.

**Goal:** reduce accidental runaway generations and simplify secure exposure.

### M11. Automated llama.cpp binary download

Provide an automated workflow to download llama-server binaries (and required libraries) from llama.cpp GitHub releases.

Requirements:

- Assume the user wants the latest beta build by default.
- Detect platform and offer appropriate assets:
  - macOS: Apple Silicon (arm64): Metal build; Intel (x64): CPU/Metal build.
  - Windows: CPU (x64/arm64), CUDA 12 / CUDA 13 (x64) with companion DLLs, Vulkan (x64), ROCm/HIP (x64).
  - Linux: CPU (x64/arm64), Vulkan (x64/arm64), ROCm (x64), OpenVINO (x64).
- Handle:
  - Selecting release (latest or specific tag).
  - Downloading main binary archive and companion DLLs.
  - Extracting (`.zip` on Windows, `.tar.gz` on macOS/Linux).
  - Placing binaries into a standardized binaries directory.

**Goal:** user can get a working llama-server with one or two clicks, tailored to their platform and backend.

### M12. Platform-specific backend selection

For Windows (and relevant platforms), provide clear, easy choices:

- CPU-only.
- NVIDIA (CUDA 12 / CUDA 13).
- AMD (ROCm/HIP).
- Vulkan.
- SYCL (Intel) when available.

Behavior:

- Detect current GPU (via existing GPU monitoring) and installed drivers (where possible).
- Suggest recommended backend (e.g., "NVIDIA GPU detected → CUDA 13 recommended").
- Allow override if user prefers a different backend.

**Goal:** remove guesswork; guide users to the correct backend and binary variant.

### M13. Standardized directory structure (binaries, models, scripts)

Define a clean, cross-platform directory structure:

- **Binaries:** central location for llama.cpp binaries (per release/backend).
- **Models:** default models directory (fully overridable).
- **Scripts / presets:** consistent location for launch presets/scripts.
- **Certificates:** for TLS/ACME/mTLS material.

Requirements:

- No breaking changes.
- Fully backward compatible with existing config.
- Sensible defaults, but flexible.

**Goal:** organized, scalable layout that grows with the user.

### M14. Flexible integration with existing tooling

Support users already using other tools:

- Allow:
  - Models stored in external drives, network shares, or existing tool directories.
  - llama-server binaries in custom locations.
- Provide:
  - Easy browsing and selection.
  - Clear guidance, not hard constraints.

**Goal:** llama-monitor integrates into the user's existing ecosystem, not the other way around.

### M15. Model-specific generation defaults (Unsloth-based)

Provide intelligent, model-family-specific defaults for generation parameters:

- Source: Unsloth's published recommendations as the primary reference.
- Behavior:
  - When user selects a model, auto-fill generation parameters (temperature, top_p, top_k, min_p, repetition_penalty, presence_penalty) based on:
    - Model family (Qwen, Gemma, Llama, Mistral, DeepSeek, etc.).
    - Use case profile (general chat, reasoning, tool-calling, coding).
- Storage:
  - Maintain a local JSON config in `~/.config/llama-monitor/model-defaults.json`.
  - Allow manual updates during development as new models emerge.
  - Optionally: provide a "Refresh Defaults" button to pull latest from a maintained upstream source (if feasible).

**Goal:** users get sensible, expert-tuned defaults without needing to research each model.

---

## Architecture Overview

High-level architecture decisions:

- **A1. Dedicated "Spawn Wizard" module**
  - Rust: `src/llama/spawn_wizard.rs` (batch-file parsing, parameter normalization, VRAM estimation, MoE tuning suggestions, benchmark coordination).
  - Frontend: `static/js/features/spawn-wizard.js`, `static/css/spawn-wizard.css`, spawn wizard modal in `index.html`.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A1), `docs/plans/20260529-spawn-v2-frontend.md` (A1).

- **A2. Extend ServerConfig and ModelPreset**
  - Add: `hf_repo`, `chat_template_file`, `n_cpu_moe` (ensure exposed), `benchmark_mode`, `mmproj`, `grammar`, `json_schema`, `cache_type_k`, `cache_type_v`, `max_tokens`, `api_key`, etc.
  - Ensure `#[serde(default)]` on all new fields.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A2).

- **A3. HuggingFace integration via hf-hub crate**
  - Use `hf-hub` crate for model metadata, repo file listing, file download, and auth.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A3), `docs/plans/20260529-spawn-v2-research.md` (HuggingFace Integration).

- **A4. Model download: incremental, resumable, safe**
  - Extend `src/agent.rs` or new `src/model_download.rs` with streaming, resume, and integrity check.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A4).

- **A5. VRAM estimation: heuristic-based, transparent**
  - New module: `src/llama/vram_estimator.rs`.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A5), `docs/plans/20260529-spawn-v2-research.md` (VRAM Estimation).

- **A6. Batch-file import: robust, cross-platform**
  - New module: `src/llama/batch_import.rs`.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A6).

- **A7. Parameter editor: dual-mode, consistent**
  - Guided mode (structured UI) + raw mode (OS-specific script).
  - Source of truth: internal config object.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A7), `docs/plans/20260529-spawn-v2-frontend.md` (A7).

- **A8. Chat template import**
  - File upload, HuggingFace URL, GitHub Gist / repo raw URL.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A8).

- **A9. Benchmark / health check**
  - New API: `POST /api/benchmark`.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A9).

- **A10. MoE tuning assistance**
  - Expose `--n-cpu-moe` as tunable option; integrate with VRAM estimator.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A10), `docs/plans/20260529-spawn-v2-research.md` (MoE Tuning).

- **A11. Automated llama.cpp binary download**
  - New module: `src/llama/llama_cpp_downloader.rs`.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A11), `docs/plans/20260529-spawn-v2-research.md` (Release Asset Patterns).

- **A12. Release asset patterns (reference)**
  - Naming patterns for macOS, Linux, Windows.
  - Details: `docs/plans/20260529-spawn-v2-research.md` (Release Asset Patterns).

- **A13. Standardized directory structure**
  - Base config root, new subdirectories (`binaries/`, `models/`, `scripts/`, `certs/`).
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A13).

- **A14. Flexible integration with existing tooling**
  - `models_dir` fully configurable; `llama_server_path` fully configurable.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A14).

- **A15. Preset reuse across models**
  - Model-agnostic presets; `model_path` filled at launch time.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A15).

- **A16. Model-specific generation defaults (Unsloth-based)**
  - Local JSON config; keyed by model family and use case profile.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (A16), `docs/plans/20260529-spawn-v2-research.md` (Model-Specific Defaults).

- **A17. Security and auth posture (critical)**
  - All new endpoints must enforce:
    - `api-token` for reading user data.
    - `db-admin-token` for elevated operations (spawn, model/bin downloads, preset changes affecting spawn behavior).
  - Path-safe file operations; no arbitrary execution.
  - Details: `docs/plans/20260529-spawn-v2-backend.md` (Security Requirements), `docs/plans/20260529-spawn-v2-reference.md` (Security).

---

## Phased Implementation Plan

This feature is large; it must be rolled out in phases, each independently shippable.
Each phase lists:
- What must be done.
- Which docs to read for details.

### Phase 0: Foundations

Goal: set up the structural foundation; no UI required.
This phase must be completed before Phase 1.

Step-by-step:

1) Extend data models

- In `src/llama/server.rs` (ServerConfig) and `src/presets/mod.rs` (ModelPreset):
  - Add new fields (all with `#[serde(default)]`):
    - hf_repo: Option<String>
    - chat_template_file: Option<String>
    - mmproj: Option<String>
    - grammar: Option<String>
    - json_schema: Option<String>
    - cache_type_k: Option<String>
    - cache_type_v: Option<String>
    - max_tokens: Option<u64>
    - api_key: Option<String>
  - In ServerConfig only (internal):
    - benchmark_mode: bool
- Update `start_server()` in `src/llama/server.rs`:
  - Wire each new field to its corresponding llama-server flag.
  - Use `-hf` when hf_repo is set and model_path is not.
  - Ensure mutual exclusion: cannot use both -m and -hf.

2) Create backend modules

- Create:
  - `src/llama/spawn_wizard.rs`:
    - Central coordinator for:
      - Batch-file parsing delegation.
      - Parameter normalization helpers.
      - VRAM estimation delegation.
      - MoE tuning suggestions.
      - Benchmark coordination.
  - `src/llama/batch_import.rs`:
    - Implement cross-platform parsing of launch scripts.
    - Normalize newlines, line continuations.
    - Extract llama-server binary path and flags.
    - Return a ModelPreset.
  - `src/llama/vram_estimator.rs`:
    - Implement VRAM estimation using formulas from research doc.
    - Inputs: model size, context size, KV quant, batch sizes, speculative decoding, mmproj, MoE settings, available VRAM.
    - Outputs: estimated VRAM/RAM and recommendation (Fit/Tight/Risk/Won’t fit).
  - `src/llama/llama_cpp_downloader.rs`:
    - List releases from ggerganov/llama.cpp.
    - Select assets by platform/backend.
    - Download and extract into a standardized binaries directory.
  - `src/hf/mod.rs`:
    - Integrate hf-hub crate.
    - Implement:
      - hf_get_model_info(repo_id)
      - hf_list_repo_files(repo_id)
      - hf_get_file_info(repo_id, path)
      - hf_download_file_stream(repo_id, path, token)
  - `src/model_download.rs`:
    - Use hf_download_file_stream to:
      - Stream download into models_dir.
      - Support resume via byte ranges.
      - Report progress.

3) Extend AppConfig

- In `src/config.rs`:
  - Add fields:
    - binaries_dir: PathBuf
    - default_models_dir: PathBuf
    - scripts_dir: PathBuf
    - certs_dir: PathBuf
  - Provide backward-compatible defaults under config_dir.
  - Ensure no breaking changes to existing config.

4) Implement new API endpoints

Add to `src/web/api.rs` (respect auth and security rules):

- POST /api/import-launch-file
  - Auth: api-token.
  - Body: { content: string, os: "windows" | "macos" | "linux" }
  - Response: { preset: ModelPreset, warnings: [string] }

- POST /api/chat-template/fetch
  - Auth: api-token.
  - Body: { source_type, source }
  - Response: { template: string, source_url: string }

- POST /api/chat-template/upload
  - Auth: api-token.
  - Multipart with file.
  - Response: { template_id, template }

- POST /api/estimate-vram
  - Auth: api-token.
  - Body: estimation input.
  - Response: structured estimate + human-readable note.

- POST /api/models/download
  - Auth: db-admin-token.
  - Body: { repo_id, file_path, target_dir }
  - Response: { download_id }

- GET /api/models/download/:id/status
  - Auth: api-token.
  - Response: { progress, bytes_downloaded, total, speed, eta, status }

- POST /api/models/download/:id/cancel
  - Auth: api-token.

- GET /api/llama-cpp/releases
  - Auth: api-token.
  - Response: { releases: [LlamaCppRelease] }

- POST /api/llama-cpp/download
  - Auth: db-admin-token.
  - Body: { release_tag, backend, arch }
  - Response: { download_id }

- GET /api/llama-cpp/download/:id/status
  - Auth: api-token.
  - Response: { progress, status, message }

- POST /api/llama-cpp/download/:id/cancel
  - Auth: api-token.

Security requirements (must be enforced):

- Use api-token for read endpoints.
- Use db-admin-token for:
  - Model download.
  - Binary download.
  - Any endpoint that changes paths, executables, or presets affecting spawn behavior.
- No shell execution; no arbitrary path traversal.

5) Add tests

- Unit tests:
  - batch_import: Windows vs Unix scripts, edge cases.
  - vram_estimator: large models, MoE models, speculative decoding, tight VRAM.
  - hf API wrappers: gated models, rate limits, missing files.
  - llama_cpp_downloader: asset selection by platform/backend.

Read:
- `20260529-spawn-v2-backend.md` → Architecture Decisions A1–A11, Data Models, Security Requirements.
- `20260529-spawn-v2-research.md` → HuggingFace Integration, VRAM Estimation, Release Asset Patterns.

### Phase 1: Premium Spawn Wizard UI

Goal: implement the core wizard UI and integrate with Phase 0 APIs.
This phase depends on Phase 0.

Step-by-step:

1) Create wizard module and styles

- Create:
  - `static/js/features/spawn-wizard.js`
  - `static/css/spawn-wizard.css`
- Requirements:
  - Follow “UI/UX Design Guidelines (Critical)” in the frontend doc.
  - Match existing glassmorphism, gradients, shadows, and micro-interactions.
  - Ensure keyboard navigation and reduced-motion support.

2) Implement wizard HTML

- In `index.html`:
  - Add spawn wizard modal structure with 5 steps:
    1) Profile
    2) Model
    3) Hardware
    4) Summary
    5) Spawn
  - Use two-column layout for complex steps:
    - Left: primary controls.
    - Right: contextual help, VRAM feedback, recommendations.

3) Implement Step 1: Profile

- Provide three compact cards:
  - Quick: auto-tuned.
  - Balanced: guided tuning.
  - Advanced: full control.
- Behavior:
  - Default to Balanced for first-time users.
  - Persist choice in localStorage.
  - Control which fields are visible in later steps based on profile.

4) Implement Step 2: Model

- Provide three options:
  - “Select local model”:
    - Open existing file browser (GGUF filter).
  - “Use HuggingFace repo”:
    - Input for repo ID.
    - On blur: call backend to list GGUF files.
    - Show compact list (filename, size, quant label).
    - Highlight recommended quant based on VRAM.
  - “Import from another tool”:
    - Short hints.
    - Open file browser pointed to common directories.
- Integrate:
  - Use `/api/models`, `/api/browse`, and HF-related endpoints.

5) Implement Step 3: Hardware

- Auto-fill:
  - GPU layers: auto/all.
  - Context size: model-native or safe default.
  - Batch sizes: tuned for platform.
- Integrate VRAM estimation:
  - Call `POST /api/estimate-vram` with current settings.
  - Display status pill (Fit/Tight/Risk/Won’t fit).
- Show:
  - Compact “Hardware summary” card (detected GPU(s), estimated free VRAM).
- Advanced-only:
  - KV cache quantization.
  - MoE tuning.
  - Multi-GPU split controls.

6) Implement Step 4: Summary

- Show:
  - Model.
  - Key settings (context, GPU layers, batch).
  - VRAM estimate.
  - Any active warnings.
- Actions:
  - “Save as Preset” (secondary).
  - “Run Health Check” (tertiary, only if server available).
- UX:
  - Scannable layout.
  - Use icons/tags for MoE, Vision, Gated, etc.

7) Implement Step 5: Spawn

- Primary button: “Spawn Server”
- Behavior:
  - Send final config to `/api/sessions/spawn` or `/api/start` depending on context.
  - Show progress area with live status.
- On success:
  - Short confirmation.
  - Auto-switch to monitor/chat view.
- On failure:
  - Clear, short message with one actionable suggestion.

8) Integrate llama.cpp binary download

- Add “Get llama-server” link/button:
  - Use `/api/llama-cpp/releases` and `/api/llama-cpp/download`.
  - Show progress and status.

Read:
- `20260529-spawn-v2-frontend.md` → SpawnWizard Module Architecture, Steps, Data and API Contract, UI/UX Design Guidelines.
- `20260529-spawn-v2-backend.md` → APIs used by the wizard.

### Phase 2: Advanced Tuning and Safety

Goal: add advanced tuning features and safety checks.
This phase depends on Phase 1.

Step-by-step:

1) Advanced parameter editor (dual-mode)

- Guided mode:
  - Sections:
    - Model & paths (model_path/hf_repo, mmproj, chat_template_file).
    - GPU & memory (gpu_layers, split_mode, tensor_split, main_gpu, mlock, no_mmap).
    - Context & batching (context_size, batch_size, ubatch_size, parallel_slots, cache_type_k, cache_type_v).
    - Generation parameters (temperature, top_p, top_k, min_p, repeat_penalty, max_tokens).
    - Speculative decoding (speculative_mode, draft_model).
    - MoE tuning (n_cpu_moe / cpu_moe toggle).
    - Advanced / custom args (extra_args).
  - Each field: short description, optional tooltip, validation where applicable.
- Raw mode:
  - Show generated OS-specific launch script.
  - On change: parse and update guided fields where possible.
- Sync:
  - Internal config object is source of truth; both views derive from it.

2) MoE tuning assistance

- On MoE detection (via metadata or tags):
  - Show “MoE model” tag.
  - Enable MoE tuning UI in Step 3 (Advanced/Balanced).
- Provide:
  - Slider/input for n_cpu_moe.
  - Guidance:
    - “Higher = more VRAM, faster. Lower = more CPU, slower.”
- Integrate with VRAM estimator:
  - Show live VRAM vs latency guidance.
  - Warn if n_cpu_moe is too low for available VRAM.

3) Health check / benchmark

- Backend:
  - Implement POST /api/benchmark:
    - Short, deterministic prompt.
    - max_tokens <= 2048, timeout <= 60s.
    - Compute TTFT, prompt TPS, gen TPS.
    - Return verdict: Good / Moderate / Poor + hints.
- Frontend:
  - “Run Health Check” button in Step 4 (Summary).
  - Show:
    - Tokens/sec (prompt and generation).
    - Latency.
    - Verdict and 1–2 tuning hints.

4) Model-specific generation defaults

- Implement model-specific defaults:
  - Local JSON config (e.g., model-defaults.json).
  - Keyed by model family and use case profile.
- Behavior:
  - When a model is selected, auto-fill:
    - temperature, top_p, top_k, min_p, repetition_penalty, etc.
  - Allow override in Advanced mode.

5) Error handling and UX flows

- Implement:
  - Toasts, inline alerts, and modal errors per UI/UX guidelines.
  - Specific handling for:
    - Network errors.
    - Auth failures.
    - VRAM issues.
    - Gated models.
    - Corrupted/invalid models.
- Ensure:
  - No raw stack traces.
  - No leaked tokens or internal paths.
  - Short, human-readable messages with one concrete action.

6) Tests

- Extend Rust tests for new endpoints and tuning logic.
- Add Playwright tests for:
  - Wizard steps (especially Hardware and Summary).
  - Health check flow.
  - Error states.

Read:
- `20260529-spawn-v2-backend.md` → A7 (Parameter editor), A9 (Benchmark), A10 (MoE), A16 (Model-specific defaults), Phase 2.
- `20260529-spawn-v2-frontend.md` → Wizard steps 3–5, UI/UX Design Guidelines, Error handling.
- `20260529-spawn-v2-research.md` → MoE Tuning, Best practices by use case.

### Phase 3: HuggingFace and Third-Party Integration

Goal: integrate HuggingFace and third-party model import.
This phase depends on Phase 0–2.

Step-by-step:

1) HuggingFace model search and download

- Backend:
  - Use hf-hub crate to:
    - Fetch model metadata (gated, tags, GGUF metadata).
    - List repo files (filter .gguf).
    - Stream-download files with progress and resume via byte ranges.
  - Integrate with existing model download endpoints.
- Frontend:
  - In Step 2 (Model), “Use HuggingFace repo”:
    - Validate repo ID.
    - Show available GGUF files with size and quant label.
    - Highlight recommended quant based on VRAM.
    - Show gated-model warnings where applicable.
- Behavior:
  - On selection:
    - Start download via /api/models/download.
    - Show progress modal.
    - On completion:
      - Add model to local model list.
      - Pre-fill spawn config.

2) Third-party model import (Ollama, LM Studio, etc.)

- Backend:
  - Implement helpers to:
    - Locate common third-party model directories (configurable).
    - Identify GGUF files.
- Frontend:
  - In Step 2, “Import from another tool”:
    - Provide short hints.
    - Open file browser pointed to common directories.
- Behavior:
  - On selection:
    - Use existing file browser to choose GGUF.
    - Pre-fill spawn config.

3) Model introspection

- Backend:
  - Implement POST /api/model/introspect:
    - Run llama.cpp binary with --print-model-metadata.
    - Extract:
      - n_layers, n_ctx_train, n_embd, n_ff, n_exp (MoE), mmproj requirements.
    - Cache results in model-cache/<sha256>.json.
- Frontend:
  - Use introspection data to:
    - Set context size.
    - Enable MoE tuning UI when MoE is detected.
    - Suggest mmproj when multimodal is detected.

4) Tests

- Extend Rust tests for HF and import logic.
- Add Playwright tests for:
  - Model selection from HF.
  - Import from third-party tools.
  - Introspection-driven config.

Read:
- `20260529-spawn-v2-backend.md` → A3 (HF integration), A4 (Model download), Phase 3.
- `20260529-spawn-v2-frontend.md` → Step 2 (Model) and Data and API Contract.
- `20260529-spawn-v2-research.md` → HuggingFace Integration, Third-Party Integration, Model Introspection.

### Phase 4: Polish and Hardening

Goal: finalize the feature; ensure it is production-ready.
This phase depends on Phase 3.

Step-by-step:

1) UI polish

- Ensure:
  - Consistent glassmorphism, gradients, shadows, and micro-interactions across all wizard steps.
  - No visual regressions vs existing app.
- Actions:
  - Refine spacing, typography, and button hierarchy.
  - Ensure reduced-motion support and keyboard navigation.
  - Validate error states and edge-case UI (e.g., no GPU, no models, no internet).

2) Security audit

- Review all new endpoints:
  - Verify correct use of api-token vs db-admin-token.
  - Verify path safety:
    - No traversal outside allowed roots.
    - No arbitrary command execution.
- Review:
  - Token handling (no full tokens in logs or error messages).
  - extra_args handling (no shell metacharacters; treat as untrusted).
  - Preset editing rules (db-admin-token or strict validation).

3) Performance optimization

- Optimize:
  - Large model downloads (streaming, resume, memory usage).
  - Binary downloads and extraction.
  - VRAM estimation and model introspection (caching).
- Ensure:
  - No blocking on hot paths.
  - Reasonable timeouts and backpressure.

4) Comprehensive tests

- Unit tests:
  - All new backend modules and endpoints.
- Integration tests:
  - End-to-end flows for:
    - Model import (local, HF, third-party).
    - VRAM estimation.
    - Health check.
- E2E tests (Playwright):
  - Wizard steps.
  - Model selection.
  - Spawn flow.
  - Error states.

5) Documentation updates

- Update:
  - AGENTS.md (if needed).
  - README.md (for high-impact features).
  - docs/reference/ (API docs, chat features, etc.).
- Ensure:
  - All new endpoints documented.
  - All new fields in ServerConfig/ModelPreset documented.
  - Security and auth rules documented.

Read:
- `20260529-spawn-v2-backend.md` → Security Requirements, Phase 4.
- `20260529-spawn-v2-frontend.md` → UI/UX Design Guidelines, Testing Strategy.
- `20260529-spawn-v2-research.md` → Best practices and pitfalls.

---

## How to Use These Docs

- This doc is the starting point:
  - Understand goals, architecture, and phased plan.
  - Each phase lists exactly which other docs to read.
- For backend work:
  - Read `docs/plans/20260529-spawn-v2-backend.md`.
- For frontend/wizard work:
  - Read `docs/plans/20260529-spawn-v2-frontend.md`.
  - You MUST follow the “UI/UX Design Guidelines (Critical)” section.
- For llama.cpp tuning, third-party compatibility, VRAM/MoE guidance:
  - Read `docs/plans/20260529-spawn-v2-research.md`.
- For a concise, self-contained reference:
  - Use `docs/plans/20260529-spawn-v2-reference.md`.

When ready to begin implementation:

- Parse this plan.
- Create the branch.
- Implement the first phase step by step.
- Run all required checks (fmt, clippy, tests, lint, etc.).
- Commit with conventional commit format.
