# Spawn Llama-Server V2 — Overview, Goals, and Phased Plan

- **Branch:** `feature/spawn-llama-server-v2`
- **Date:** 2026-05-29
- **Author:** Iris (via Hermes)

This is the top-level overview for the "Spawn Llama-Server V2" overhaul.
It defines goals, architecture, phased rollout, and links to detailed docs.

For implementation details, see:
- Backend: `docs/plans/20260529-spawn-v2-backend.md`
- Frontend: `docs/plans/20260529-spawn-v2-frontend.md`
- Research and tuning reference: `docs/plans/20260529-spawn-v2-research.md`

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

**Goal:** seamless "discover → select → download → use" path inside the app.

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
  - Add: `chat_template_file`, `n_cpu_moe` (ensure exposed), `benchmark_mode`, and other new fields.
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

---

## Phased Implementation Plan

This feature is large; it must be rolled out in phases, each independently shippable.

### Phase 0: Foundations

Goal: set up the structural foundation; no UI required.

Deliverables:

- New modules:
  - `src/llama/spawn_wizard.rs`
  - `src/llama/batch_import.rs`
  - `src/llama/vram_estimator.rs`
  - `src/llama/llama_cpp_downloader.rs`
  - `src/hf/mod.rs`
  - `src/model_download.rs`
- Extend:
  - `ServerConfig` (new fields, `#[serde(default)]`).
  - `ModelPreset` (mirror new fields).
  - `AppConfig` (binaries_dir, default_models_dir, scripts_dir, certs_dir).
- New API endpoints (basic, no UI):
  - `POST /api/import-launch-file`
  - `POST /api/chat-template/fetch`
  - `POST /api/chat-template/upload`
  - `POST /api/estimate-vram`
  - `POST /api/models/download`
  - `GET /api/models/download/:id/status`
  - `POST /api/models/download/:id/cancel`
  - `GET /api/llama-cpp/releases`
  - `POST /api/llama-cpp/download`
  - `GET /api/llama-cpp/download/:id/status`
  - `POST /api/llama-cpp/download/:id/cancel`
- Tests:
  - Unit tests for batch_import parsing (Windows vs Unix).
  - Unit tests for vram_estimator edge cases.
  - Unit tests for hf API wrappers.
  - Unit tests for llama_cpp_downloader asset selection logic.

Details: `docs/plans/20260529-spawn-v2-backend.md` (Phase 0).

### Phase 1: Premium Spawn Wizard UI

Goal: implement the core wizard UI and integrate with Phase 0 APIs.

Deliverables:

- Implement:
  - `static/css/spawn-wizard.css`
  - `static/js/features/spawn-wizard.js`
  - HTML for spawn wizard modal.
- Integrate:
  - File browser for model selection.
  - VRAM estimation into Step 3.
  - Import launch file into Step 4.
  - llama.cpp binary download ("Get llama-server").
- Ensure:
  - Consistent with existing UI patterns.
  - Light theme and reduced-motion support.

Details: `docs/plans/20260529-spawn-v2-frontend.md` (Phase 1).

### Phase 2: Advanced Tuning and Safety

Goal: add advanced tuning features and safety checks.

Deliverables:

- Advanced parameter editor (dual-mode).
- MoE tuning assistance.
- Health check / benchmark button.
- Model-specific generation defaults.
- Error handling and UX flows.
- Tests:
  - Extend Rust tests for new endpoints.
  - Add Playwright tests for wizard steps.

Details: `docs/plans/20260529-spawn-v2-backend.md` (Phase 2), `docs/plans/20260529-spawn-v2-frontend.md` (Phase 2).

### Phase 3: HuggingFace and Third-Party Integration

Goal: integrate HuggingFace and third-party model import.

Deliverables:

- HuggingFace model search and download.
- Third-party model import (Ollama, LM Studio, etc.).
- Model introspection (via llama.cpp binary or `--print-model-metadata`).
- Tests:
  - Extend Rust tests for HF and import logic.
  - Add Playwright tests for model selection and import.

Details: `docs/plans/20260529-spawn-v2-backend.md` (Phase 3), `docs/plans/20260529-spawn-v2-research.md` (Third-Party Integration).

### Phase 4: Polish and Hardening

Goal: finalize the feature; ensure it is production-ready.

Deliverables:

- Final polish of wizard UI.
- Security audit of all new endpoints and data flows.
- Performance optimization (large model downloads, streaming, etc.).
- Comprehensive tests (unit, integration, e2e).
- Documentation updates (AGENTS.md, README.md, docs/reference/).

Details: `docs/plans/20260529-spawn-v2-backend.md` (Phase 4), `docs/plans/20260529-spawn-v2-frontend.md` (Phase 4).

---

## How to Use These Docs

- To understand the overall feature and phased plan: read this doc.
- To implement backend changes: read `docs/plans/20260529-spawn-v2-backend.md`.
- To implement frontend changes: read `docs/plans/20260529-spawn-v2-frontend.md`.
- To consult llama.cpp tuning, third-party compatibility, or research: read `docs/plans/20260529-spawn-v2-research.md`.

When ready to begin implementation, point me to this doc (or the specific backend/frontend doc) and I will:

- Parse the plan.
- Create the branch.
- Implement the first phase step by step.
- Run all required checks (fmt, clippy, tests, lint, etc.).
- Commit with conventional commit format.
