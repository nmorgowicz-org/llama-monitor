# Spawn Llama-Server V2 — Core Architecture, APIs, and Implementation Plan

- **Branch:** `feature/spawn-llama-server-v2`
- **Date:** 2026-05-29
- **Author:** Iris (via Hermes)

This is the primary architecture doc for the "Spawn Llama-Server V2" feature.
It is written so that a fresh AI agent can read it and execute the implementation without guessing.

For UX designs, flows, and error handling, see:
- `docs/plans/20260529-spawn_llama_server_ux.md`

---

## Goals (Milestones)

These are Nick's requirements, restated as concrete milestones.

### M1. Welcome screen: connect or spawn

On first launch / welcome screen:

- User can:
  - Select a previous remote endpoint they connected to.
  - Enter a new endpoint.
- On the right side:
  - Option to spin up a new llama-server.
  - Option to spin up a new llama-server using an existing preset.

- **Current state:** partially implemented.
- **Goal:** unify and polish this into a single coherent welcome flow with premium UI.

### M2. Modern, premium "Spawn Llama-Server" UX

New "Spawn Llama-Server" experience must:

- Match the 2026 UI/UX style already used in the app (glassmorphism, premium modals, micro-interactions).
- Be intuitive, guided, and visually consistent.

- **Current state:** basic modals / forms.
- **Goal:** redesign as a step-based wizard modal with clear sections, inline help, tooltips, and consistent design tokens.

### M3. Import existing launch settings (batch file / script)

Support importing a user's existing launch file:

- Windows: `.cmd` / `.bat`
- macOS/Linux: `.sh` / `.bash` / `.zsh`

System must:

- Parse the script to extract:
  - llama-server binary path.
  - All flags and arguments.
- Normalize:
  - Handle newline differences (`\r\n` vs `\n` vs `^`).
  - Clean up environment-specific artifacts.
- Adapt:
  - Binary path to match the system's llama-server location.
  - OS-specific line endings and quoting.

- **Goal:** produce a clean, editable preset from an imported script.

### M4. Advanced parameter editing (dual mode)

Provide two modes for editing launch parameters:

- **Raw mode:**
  - Show the generated launch script (for the current OS) in an editable code area.
- **Guided mode:**
  - Structured UI with:
    - Grouped parameters (model, GPU, context, batch, generation, speculative decoding, etc.).
    - Inline descriptions / tooltips.
    - Validation and suggestions.

Both modes:

- Must stay in sync (edits in one update the other).

- **Goal:** make tuning approachable for both power users and casual users.

### M5. Chat template support (.jinja / HF / GitHub)

Support:

- Uploading a `.jinja` chat template file.
- Specifying a HuggingFace URL.
- Specifying a GitHub Gist / repo path.

System must:

- Fetch or load the template.
- Map it to `--chat-template-file` when spawning llama-server.

- **Goal:** allow flexible, external chat templates without manual filesystem work.

### M6. Multiplatform model selection modal

Provide a modern, multiplatform model selection modal:

- Point to a local or shared directory containing GGUF models.
- Allow browsing and selecting models.
- Integrate with existing `scan_models_dir` and `/api/models`.

- **Goal:** make model selection feel native and polished on all platforms.

### M7. HuggingFace model pulling with quant selection

Allow:

- Pointing to a HuggingFace repo.
- Selecting a quantization variant.
- Downloading directly to the user's configured model directory.

Requirements:

- Use HF Hub API (via `hf-hub` Rust crate) to list GGUF files.
- Respect `HUGGING_FACE_HUB_TOKEN` for gated models.
- Show file size, quant label, and basic metadata.

- **Goal:** seamless "discover → select → download → use" path inside the app.

### M8. VRAM estimation and trade-off guidance

Implement estimation logic that:

- Uses model size / quantization + context size + KV quantization + speculative decoding settings + batch size to estimate VRAM usage.
- Compares against available VRAM (from existing GPU monitoring).
- Advises:
  - If the current config fits.
  - If it risks spilling into system RAM (performance hit).
  - What to adjust:
    - Reduce context size.
    - Use a different quantization.
    - Tune batch/ubatch.

For multimodal models:

- Account for mmproj size.

For MoE models:

- Account for `--n-cpu-moe` and its effect on RAM vs VRAM.

- **Goal:** give users intelligent, quantitative guidance before launch.

### M9. MoE expert offload tuning

For MoE models:

- Expose `--n-cpu-moe` as a tunable option.
- Offer suggestions:
  - How many experts to keep in VRAM vs RAM.
  - Impact on throughput and latency.

- **Goal:** make MoE tuning accessible without deep internal knowledge.

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

- **Goal:** help users validate that their configuration is sane and performant.

### M11. Automated llama.cpp binary download

Provide an automated workflow to download llama-server binaries (and required libraries) from llama.cpp GitHub releases.

Requirements:

- Assume the user wants the latest beta build by default.
- Detect platform and offer appropriate assets:
  - macOS:
    - Apple Silicon (arm64): Metal build.
    - Intel (x64): CPU/Metal build.
  - Windows:
    - CPU (x64/arm64).
    - CUDA 12 / CUDA 13 (x64) with companion DLLs.
    - Vulkan (x64).
    - ROCm/HIP (x64).
  - Linux:
    - CPU (x64/arm64).
    - Vulkan (x64/arm64).
    - ROCm (x64).
    - OpenVINO (x64).
- Handle:
  - Selecting release (latest or specific tag).
  - Downloading:
    - Main binary archive.
    - Companion DLLs (e.g., CUDA runtime).
  - Extracting:
    - `.zip` (Windows).
    - `.tar.gz` (macOS/Linux).
  - Placing binaries into a standardized binaries directory.

- **Goal:** user can get a working llama-server with one or two clicks, tailored to their platform and backend.

### M12. Platform-specific backend selection

For Windows (and relevant platforms), provide clear, easy choices:

- CPU-only.
- NVIDIA (CUDA 12 / CUDA 13).
- AMD (ROCm/HIP).
- Vulkan.
- SYCL (Intel) when available.

Behavior:

- Detect:
  - Current GPU (via existing GPU monitoring).
  - Installed drivers (where possible).
- Suggest:
  - Recommended backend.
  - Example: "NVIDIA GPU detected → CUDA 13 recommended."
- Allow override if user prefers a different backend.

- **Goal:** remove guesswork; guide users to the correct backend and binary variant.

### M13. Standardized directory structure (binaries, models, scripts)

Define a clean, cross-platform directory structure:

- **Binaries:**
  - Central location for llama.cpp binaries (per release/backend).
- **Models:**
  - Default models directory.
  - Fully overridable for users who already store models elsewhere (e.g., with llama.cpp, KoboldCPP, vLLM, Ollama, LM Studio, etc.).
- **Scripts / presets:**
  - Consistent location for launch presets/scripts.
  - Avoid duplication per model (one config for "Qwen3.6-27B at 212K context", etc., reuse across models).

Requirements:

- No breaking changes.
- Fully backward compatible with existing config.
- Sensible defaults, but flexible.

- **Goal:** organized, scalable layout that grows with the user.

### M14. Flexible integration with existing tooling

Support users already using other tools:

- Allow:
  - Models stored in external drives, network shares, or existing tool directories.
  - llama-server binaries in custom locations.
- Provide:
  - Easy browsing and selection.
  - Clear guidance, not hard constraints.

- **Goal:** llama-monitor integrates into the user's existing ecosystem, not the other way around.

### M15. Model-specific generation defaults (Unsloth-based)

Provide intelligent, model-family-specific defaults for generation parameters:

- Source:
  - Use Unsloth's published recommendations as the primary reference.
- Behavior:
  - When user selects a model, auto-fill generation parameters (temperature, top_p, top_k, min_p, repetition_penalty, presence_penalty) based on:
    - Model family (Qwen, Gemma, Llama, Mistral, DeepSeek, etc.).
    - Use case profile (general chat, reasoning, tool-calling, coding).
- Storage:
  - Maintain a local JSON/YAML config in `~/.config/llama-monitor/model-defaults.json`.
  - Allow manual updates during development as new models emerge.
  - Optionally: provide a "Refresh Defaults" button to pull latest from a maintained upstream source (if feasible).

- **Goal:** users get sensible, expert-tuned defaults without needing to research each model.

---

## Current State (Summary)

Key existing capabilities:

- **Spawning llama-server:**
  - `src/llama/server.rs`:
    - `ServerConfig` struct: all parameters.
    - `start_server()`: builds command, sets env, spawns child, streams logs.
  - `src/web/api.rs`:
    - `POST /api/start`: direct start from config.
    - `POST /api/sessions/spawn`: spawn with preset via session.
- **Presets:**
  - `src/presets/mod.rs`:
    - `ModelPreset` struct.
    - `load_presets`/`save_presets`, `default_presets`.
  - Frontend:
    - Preset modal, CRUD via `/api/presets`.
- **Model discovery:**
  - `src/models/mod.rs`:
    - `scan_models_dir`: scans for `.gguf`.
  - `/api/models`, `/api/models/refresh`.
- **File browser:**
  - `/api/browse?path=...&filter=gguf`
  - `static/js/features/file-browser.js`.
- **GPU/VRAM monitoring:**
  - `src/gpu/*`:
    - `nvidia-smi`, `rocm-smi`, Apple unified memory, Windows WMI.
- **Metrics:**
  - `src/llama/metrics.rs`, `src/llama/poller.rs`.
- **Download infrastructure:**
  - `src/agent.rs`:
    - `download_asset_locally`, etc. (for app/agent updates).
- **UI patterns:**
  - Premium modals, glassmorphism, design tokens, widget cards.
  - Agent setup modal style as a template for wizards.

What is missing (to be implemented):

- No HuggingFace integration.
- No model downloading from HF.
- No VRAM estimation.
- No batch-file import.
- No advanced parameter editor (raw + guided).
- No chat-template import via URL.
- No benchmark/health-check tool.
- No MoE-specific tuning UI.
- No automated llama.cpp binary download.
- No standardized binaries/models/scripts directory structure.
- No third-party model import (Ollama, LM Studio, etc.).
- No tuning guidance based on llama-server metrics.
- No model-specific generation defaults.

---

## Architecture Decisions

### A1. Use a dedicated "Spawn Wizard" module

- **Rationale:**
  - This is complex; a single cohesive module keeps it maintainable.
- **Implementation:**
  - Rust:
    - New module: `src/llama/spawn_wizard.rs`
    - Responsibilities:
      - Batch-file parsing.
      - Parameter normalization.
      - VRAM estimation.
      - MoE tuning suggestions.
      - Benchmark coordination.
  - Frontend:
    - New JS: `static/js/features/spawn-wizard.js`
    - New CSS: `static/css/spawn-wizard.css` (or extend `agent-modal.css`).
    - New HTML: spawn wizard modal in `index.html`.

### A2. Extend ServerConfig and ModelPreset

- **Rationale:**
  - New capabilities (chat-template-file, MoE tuning, benchmark flags) must be first-class.
- **Changes:**
  - `ServerConfig`:
    - Add:
      - `chat_template_file: Option<String>`
      - `n_cpu_moe: Option<usize>` (already present; ensure exposed).
      - `benchmark_mode: bool` (internal flag).
  - `ModelPreset`:
    - Mirror new fields.
  - Ensure `#[serde(default)]` on all new fields.

### A3. HuggingFace integration via hf-hub crate

- **Rationale:**
  - Official, maintained, feature-complete.
  - Handles auth, rate limits, gated models, and repo operations.
- **Design:**
  - Use `hf-hub` crate (by Hugging Face):
    - `HFClient` for async operations.
    - `HFClientSync` for blocking operations.
  - Functions:
    - `hf_list_repo_files(repo_id: &str)`
    - `hf_get_file_info(repo_id: &str, path: &str)`
    - `hf_get_model_info(repo_id: &str)`
  - Auth:
    - Read `HUGGING_FACE_HUB_TOKEN` from env.
    - Allow user to input their own token via UI.
    - Store in config file (e.g., `~/.config/llama-monitor/hf-token`).
    - Use a read-only or fine-grained token for security.
  - Gated models:
    - Detect gating from model metadata.
    - If 401/403 on metadata or download:
      - Show: "This model is gated. Please visit it on HuggingFace, request access, then provide your HF_TOKEN."
    - Do not attempt to auto-request access.
  - Rate limits:
    - Respect 429 responses and `RateLimit` headers.
    - Implement backoff.
    - Cache results where possible.
  - MoE detection:
    - Check model metadata for "moe" or "mixture_of_experts".
    - If detected:
      - Show: "This is a Mixture-of-Experts (MoE) model."
      - Enable MoE tuning UI.

### A4. Model download: incremental, resumable, safe

- **Rationale:**
  - Large models; users expect progress, resumability, and safety.
- **Design:**
  - Extend `src/agent.rs` or new `src/model_download.rs`:
    - `download_model_file(url, dest, token)`
    - Supports:
      - Range requests.
      - Partial file (resume).
      - Integrity check (file size comparison).
  - API:
    - `POST /api/models/download`:
      - Body: `{ repo_id, file_path, target_dir }`
      - Response: `{ download_id }`
    - `GET /api/models/download/:id/status`:
      - Response: `{ progress, bytes_downloaded, total, speed, eta, status }`
    - `POST /api/models/download/:id/cancel`
  - Frontend:
    - Progress modal with:
      - File name, size, ETA.
      - Pause/Cancel.

### A5. VRAM estimation: heuristic-based, transparent

- **Rationale:**
  - Exact VRAM usage depends on llama.cpp internals.
  - We'll use robust heuristics and clearly label as estimates.
- **Design:**
  - New module: `src/llama/vram_estimator.rs`
  - Inputs:
    - `model_size_bytes` (from file or HF metadata).
    - `context_size`.
    - KV cache quantization (`ctk`/`ctv`).
    - `batch_size`, `ubatch_size`.
    - Speculative decoding flags.
    - Multimodal (mmproj) size.
    - MoE expert offload settings.
    - `available_vram_bytes` (from existing GPU monitoring).
  - Logic:
    - `weights_memory` = `model_size_bytes`.
    - `kv_cache_memory` = 2 * num_layers * num_heads * head_dim * kv_bytes_per_elem * context_size * parallel_slots.
      - kv_bytes_per_elem:
        - F16: 2.0
        - F32: 4.0
        - Q8_0: 1.0
        - Q4_K: 0.5
        - Q3_K: 0.375
        - IQ4_XS: 0.5
        - IQ2_XS: 0.25
    - `speculative_overhead`:
      - If draft model: `draft_model_size_bytes` + `kv_per_token * draft_max * parallel_slots`.
      - If ngram-mod: `kv_per_token * draft_max * parallel_slots`.
    - `mmproj_memory` = `mmproj_size_bytes * 1.02`.
    - MoE:
      - `moe_gpu_memory` = `model_size_bytes * (moe_experts_total - n_cpu_moe) / moe_experts_total`.
      - `moe_cpu_memory` = `model_size_bytes * n_cpu_moe / moe_experts_total`.
      - Use `moe_gpu_memory` in VRAM estimate.
  - Output:
    - `estimated_vram_needed`.
    - `estimated_ram_needed`.
    - Recommendation:
      - "Fit" (>= 1.2x available).
      - "Tight" (1.0-1.2x).
      - "Risk" (0.85-1.0x).
      - "Won't fit" (< 0.85x).
  - API:
    - `POST /api/estimate-vram`:
      - Body: estimation input.
      - Response: structured estimate + human-readable note.
  - Integration:
    - Use existing GPU monitoring (`src/gpu/*`) for `available_vram_bytes`.
    - If GPU query fails:
      - Show: "Could not read GPU details; VRAM estimate is approximate."

### A6. Batch-file import: robust, cross-platform

- **Rationale:**
  - Users have messy scripts; we must normalize them.
- **Design:**
  - New module: `src/llama/batch_import.rs`
  - Steps:
    - Normalize newlines:
      - Replace `\r\n` and `\r` with `\n`.
      - Handle `^` (Windows line continuation) by joining lines.
    - Locate llama-server invocation:
      - First token that matches "llama-server" or "llama-server.exe".
    - Extract arguments:
      - Tokenize respecting quotes.
      - Parse known flags: `-m`, `-ngl`, `-c`, `-b`, `-ub`, etc.
      - Keep unrecognized flags in `extra_args`.
    - Normalize paths:
      - Detect model path from `-m`.
      - Detect binary path.
    - Return:
      - A `ModelPreset` struct.
  - API:
    - `POST /api/import-launch-file`:
      - Body: `{ content: string, os: "windows" | "macos" | "linux" }`
      - Response: `{ preset: ModelPreset, warnings: [string] }`

### A7. Parameter editor: dual-mode, consistent

- **Rationale:**
  - Power users want raw control.
  - Others need guidance.
- **Design:**
  - Guided mode:
    - Sections:
      - Model & paths.
      - GPU & memory.
      - Context & batching.
      - Generation parameters.
      - Speculative decoding.
      - MoE tuning.
      - Advanced / custom args.
    - Each field:
      - Short description.
      - Optional tooltip with deeper info.
      - Validation (e.g., `context_size` must be power-of-two or multiple-of-256).
  - Raw mode:
    - Generated script:
      - OS-specific:
        - Windows: `.cmd` with `^` line continuations.
        - Unix: `.sh` with `\` continuations.
      - Editable.
    - On blur/change:
      - Parse and update guided fields where possible.
  - Sync:
    - Source of truth: internal config object.
    - Both views derive from it.

### A8. Chat template import

- **Rationale:**
  - Users may have external templates.
- **Design:**
  - Supported:
    - File upload.
    - HuggingFace URL (direct file).
    - GitHub Gist / repo raw URL.
  - Validation:
    - Check for common markers: {{system}}, {{user}}, {{assistant}}, {{bos}}, {{eos}}.
    - If present: "Template looks valid."
    - If not: "No common template markers detected. It may still be valid, but check your template before using it."
  - Auth failures:
    - If 401/403:
      - Show: "This file requires authentication. Ensure the repository is public or your HuggingFace/GitHub token is configured."
  - New API:
    - `POST /api/chat-template/fetch`:
      - Body: `{ source_type, source }`
      - Response: `{ template: string, source_url: string }`
    - `POST /api/chat-template/upload`:
      - Multipart with file.
      - Response: `{ template_id, template }`
  - Integration:
    - Stored in app config or preset.
    - Passed as `--chat-template-file` when spawning.

### A9. Benchmark / health check

- **Rationale:**
  - Users want to know if their config is sane.
- **Design:**
  - New API:
    - `POST /api/benchmark`:
      - Body: `{ prompt: string, max_tokens: usize, temperature: f32 }`
      - Behavior:
        - Sends a prompt to llama-server.
        - Measures:
          - Time to first token.
          - Generation tokens/sec.
          - Prompt tokens/sec.
          - Total latency.
        - Uses existing metrics endpoints + timing.
      - Methodology:
        - Use a short, deterministic prompt (e.g., "Explain the difference between VRAM and system RAM in 50 words.").
        - Limit: max_tokens <= 2048, timeout <= 60s.
        - Interpret results:
          - "Good": TTFT < 500ms, gen TPS > 30.
          - "Moderate": TTFT 500-1500ms, gen TPS 10-30.
          - "Poor": TTFT > 1500ms, gen TPS < 10.
  - Frontend:
    - "Run Health Check" button.
    - Results panel:
      - Tokens/sec (prompt and generation).
      - Latency.
      - Simple verdict: "Good", "Moderate", "Poor".
      - Optional tuning hints.
  - Safety:
    - Hard limits:
      - `max_tokens <= 2048`.
      - `timeout <= 60s`.
    - No destructive operations.

### A10. MoE tuning assistance

- **Rationale:**
  - MoE models need expert offload tuning.
- **Design:**
  - MoE detection:
    - Check model metadata for "moe" or "mixture_of_experts".
    - If detected:
      - Show: "This is a Mixture-of-Experts (MoE) model."
      - Enable MoE tuning UI.
  - In spawn wizard:
    - If model is detected as MoE:
      - Show:
        - "Experts to offload to CPU (--n-cpu-moe)"
        - Slider or input.
        - Guidance:
          - "Higher = more VRAM, faster. Lower = more CPU, slower."
  - Integration with VRAM estimator:
    - Show live VRAM vs latency guidance.
    - If n-cpu-moe is too low:
      - Show: "Risk of OOM. Needs ~X GB VRAM; you have Y GB."
    - If n-cpu-moe is too high:
      - Show: "Most experts on CPU; expect slower generation."

### A11. Automated llama.cpp binary download

- **Rationale:**
  - Users should not manually browse GitHub releases.
  - We must automate selection, download, extraction, and placement.
- **Design:**
  - New module: `src/llama/llama_cpp_downloader.rs`
  - Responsibilities:
    - Query GitHub releases for llama.cpp.
    - Select assets based on platform, arch, backend.
    - Download and extract binaries.
  - Backend detection:
    - Use existing GPU monitoring (`src/gpu/*`).
    - If NVIDIA GPU: recommend CUDA 13 (or CUDA 12 if drivers are older).
    - If AMD GPU: recommend ROCm.
    - If Intel GPU: recommend Vulkan or SYCL.
    - If no GPU: recommend CPU.
    - On macOS: recommend Metal.
  - Companion DLLs:
    - For Windows CUDA:
      - Download both:
        - Main binary.
        - Companion `cudart-llama-*.zip`.
      - Extract both to the same directory.
  - Workflow:
    - Detect:
      - OS (macOS/Windows/Linux).
      - Arch (x64/arm64).
      - GPU/backend (from existing GPU detection).
    - Choose release:
      - Default: latest release tagged "Latest".
      - Option: allow user to pick a specific tag.
    - Choose assets:
      - Use naming patterns (see "Release Asset Patterns" below).
    - Extract:
      - `.zip` (Windows).
      - `.tar.gz` (macOS/Linux).
      - Place binaries into standardized directory.
  - API:
    - `GET /api/llama-cpp/releases`:
      - List recent releases.
    - `POST /api/llama-cpp/download`:
      - Body: `{ release_tag, backend, arch }`
      - Response: `{ download_id }`
    - `GET /api/llama-cpp/download/:id/status`:
      - Response: `{ progress, status, message }`
    - `POST /api/llama-cpp/download/:id/cancel`
  - Frontend:
    - "Get llama-server" button in spawn wizard.
    - Platform-aware UI:
      - Show recommended backend.
      - Allow override.

### A12. Release asset patterns (reference)

Based on analysis of recent llama.cpp releases:

- **macOS:**
  - Apple Silicon: `llama-{TAG}-bin-macos-arm64.tar.gz`
  - Intel: `llama-{TAG}-bin-macos-x64.tar.gz`
- **Linux:**
  - CPU (x64): `llama-{TAG}-bin-ubuntu-x64.tar.gz`
  - CPU (arm64): `llama-{TAG}-bin-ubuntu-arm64.tar.gz`
  - Vulkan (x64): `llama-{TAG}-bin-ubuntu-vulkan-x64.tar.gz`
  - Vulkan (arm64): `llama-{TAG}-bin-ubuntu-vulkan-arm64.tar.gz`
  - ROCm: `llama-{TAG}-bin-ubuntu-rocm-7.2-x64.tar.gz`
  - OpenVINO: `llama-{TAG}-bin-ubuntu-openvino-2026.0-x64.tar.gz`
- **Windows:**
  - CPU (x64): `llama-{TAG}-bin-win-cpu-x64.zip`
  - CPU (arm64): `llama-{TAG}-bin-win-cpu-arm64.zip`
  - CUDA 12: `llama-{TAG}-bin-win-cuda-12.4-x64.zip`
    - Companion: `cudart-llama-bin-win-cuda-12.4-x64.zip`
  - CUDA 13: `llama-{TAG}-bin-win-cuda-13.3-x64.zip`
    - Companion: `cudart-llama-bin-win-cuda-13.3-x64.zip`
  - Vulkan: `llama-{TAG}-bin-win-vulkan-x64.zip`
  - ROCm/HIP: `llama-{TAG}-bin-win-hip-radeon-x64.zip`

Notes:

- All tags are "bXXXX" continuous builds; no "stable" releases.
- CUDA minor versions are embedded (e.g., 12.4, 13.3).
- Some builds may be disabled in a release; always verify asset existence.

### A13. Standardized directory structure

- **Rationale:**
  - Provide a clean, cross-platform layout for binaries, models, scripts, and certs.
  - No breaking changes; fully backward compatible.
- **Design:**
  - Base config root (existing):
    - Linux/macOS: `~/.config/llama-monitor`
    - Windows: `%APPDATA%/llama-monitor`
  - New subdirectories (optional, created on-demand):
    - `binaries/`
      - For llama.cpp binaries (per release/backend).
    - `models/`
      - Default models directory (overridable).
    - `scripts/`
      - For launch scripts and imported presets.
    - `certs/`
      - For TLS/ACME/mTLS material.
  - Integration:
    - `AppConfig`:
      - Add optional derived paths:
        - `binaries_dir: PathBuf` (default: `config_dir / "binaries"`)
        - `default_models_dir: PathBuf` (default: `config_dir / "models"`)
        - `scripts_dir: PathBuf` (default: `config_dir / "scripts"`)
        - `certs_dir: PathBuf` (default: `config_dir / "certs"`)
    - UI:
      - Configuration modal:
        - Show:
          - "Binaries folder"
          - "Models folder"
          - "Scripts folder"
          - "Certificates folder"
        - With:
          - "Open" / "Browse" buttons.
          - Editable paths.
    - Spawn wizard:
      - Uses `binaries_dir` for binary selection.
      - Uses `models_dir` for model selection.
      - Uses `scripts_dir` as a helper for imported launch files.

### A14. Flexible integration with existing tooling

- **Rationale:**
  - Users may already use llama.cpp, KoboldCPP, vLLM, Ollama, LM Studio, etc.
  - We must not force them to reorganize.
- **Design:**
  - `models_dir` remains fully configurable (CLI and UI).
  - `llama_server_path` remains fully configurable.
  - File browser:
    - Allow browsing anywhere (with reasonable security constraints).
  - Behavior:
    - If user selects an existing directory (e.g., `D:\ai\models`), respect it.
    - If user selects an external llama-server binary, use it.
    - Provide suggestions, not restrictions.

### A15. Preset reuse across models

- **Rationale:**
  - Avoid duplicating launch scripts per model.
  - One config for "Qwen3.6-27B at 212K context", reused across models.
- **Design:**
  - Presets:
    - Store in `presets.json` (existing).
    - Model-agnostic:
      - `model_path` is filled at launch time.
  - Import:
    - On import, create/update a preset.
    - Optionally store original script in `scripts/` for reference.
  - UI:
    - "Import launch file" button.
    - Preset editor:
      - Guided fields (structured).
      - Raw mode: generated launch script for the current OS.

### A16. Model-specific generation defaults (Unsloth-based)

- **Rationale:**
  - Different model families have different optimal generation parameters.
  - Unsloth publishes well-tested, practical defaults.
- **Design:**
  - Maintain a local JSON config: `~/.config/llama-monitor/model-defaults.json`.
  - Structure:
    - Keyed by model family (qwen, gemma, llama, mistral, deepseek, etc.).
    - Each family has profiles: base, reasoning, tool_calling, coding.
  - Behavior:
    - When user selects a model, auto-fill generation parameters based on:
      - Model family (detected from name/metadata).
      - Use case profile (selected by user or inferred).
  - Updates:
    - During development, proactively update defaults as new models emerge.
    - Optionally: provide a "Refresh Defaults" button to pull latest from a maintained upstream source (if feasible).
  - Example structure:

```json
{
  "model_families": {
    "qwen": {
      "base": {
        "temperature": 0.7,
        "top_p": 0.8,
        "top_k": 20,
        "min_p": 0.0,
        "presence_penalty": 1.5,
        "repetition_penalty": 1.0
      },
      "reasoning": {
        "temperature": 1.0,
        "top_p": 0.95,
        "top_k": 20,
        "min_p": 0.0,
        "presence_penalty": 1.5,
        "repetition_penalty": 1.0
      },
      "coding": {
        "temperature": 0.6,
        "top_p": 0.95,
        "top_k": 20,
        "min_p": 0.0,
        "presence_penalty": 0.0,
        "repetition_penalty": 1.0
      }
    },
    "gemma": {
      "base": {
        "temperature": 1.0,
        "top_p": 0.95,
        "top_k": 64,
        "min_p": 0.0,
        "repetition_penalty": 1.0
      }
    },
    "llama": {
      "base": {
        "temperature": 0.6,
        "top_p": 0.9,
        "min_p": 0.01
      }
    },
    "mistral": {
      "instruct": {
        "temperature": 0.15
      },
      "reasoning": {
        "temperature": 0.7,
        "top_p": 0.95
      }
    },
    "deepseek": {
      "base": {
        "temperature": 0.6,
        "top_p": 0.95,
        "min_p": 0.01
      }
    },
    "granite": {
      "base": {
        "temperature": 0.0,
        "top_p": 1.0,
        "top_k": 0
      }
    }
  }
}
```

---

## Third-Party Research: HuggingFace Integration

### HF Hub API and hf-hub crate

- **Recommended approach:** Use the official `hf-hub` Rust crate.
  - It already provides:
    - Model metadata queries (to detect gated, tags, size).
    - Repo file listing (to find GGUF files).
    - File download helpers (to fetch GGUF).
    - Auth support via `HF_TOKEN`.
  - This is better than:
    - Writing raw HTTP calls: more boilerplate, more edge cases, less future-proof.
    - Using unofficial crates: less stable, less feature-complete.

- **Token handling:**
  - Let users input their HF_TOKEN via:
    - Environment variable: `HF_TOKEN` (standard).
    - Or an interactive prompt on first use: "Enter your HuggingFace token (for gated models):"
  - Store it:
    - In a config file (e.g., `~/.config/llama-monitor/hf-token`).
    - Or in the same directory as llama-monitor.
  - Use a read-only or fine-grained token for security.

- **Gated model handling:**
  - When user selects a gated model:
    - If 401/403 on metadata or download:
      - Show: "This model is gated. Please:
        1) Visit it on HuggingFace and request access in your browser.
        2) Once approved, ensure your HF_TOKEN is set."
    - Do not attempt to auto-request access.
  - Detect gating:
    - From model metadata: "gated" field.
    - Pre-warn: "This model is gated; you may need access."

- **Rate limits:**
  - If scanning many models/files, implement:
    - Rate limit awareness (respect 429 and RateLimit headers).
    - Backoff.
  - Cache results where possible.

- **Risks:**
  - Limits can change over time; anonymous/free tiers are subject to change.
  - If llama-monitor is used in a loop scanning many models, you could hit limits quickly.
  - HF may tighten token policies; ensure llama-monitor uses standard `HF_TOKEN` patterns and fine-grained tokens where possible.

---

## Third-Party Research: Model Import from Other Tools

### Ollama

- **Default storage paths:**
  - macOS: `~/.ollama/models/`
  - Linux: `/usr/share/ollama/.ollama/models/` or `~/.ollama/models/`
  - Windows: `C:\Users\<user>\.ollama\models`
  - Override: `OLLAMA_MODELS` environment variable.

- **Internal structure:**
  - `models/blobs/` — SHA-256 named files (e.g., `sha256-abc123...`)
  - `models/manifests/registry.ollama.ai/library/<model>/<tag>` — small JSON mapping model:tag to blob digests.
  - Inspired by Docker image layers.

- **File format:**
  - The blob for layer with `mediaType: "application/vnd.ollama.image.model"` is a **raw GGUF file**. No proprietary wrapper.
  - The manifest is metadata only (architecture, params, quantization, which blobs belong to which model:tag).

- **llama-server compatibility:**
  - Yes — the blob IS a valid GGUF. You can point `llama-server -m <blob-path>` directly.
  - Edge case: Ollama sometimes ships GGUFs for newer architectures before llama.cpp master supports them, so you may get "key not found" errors with very recent models.

- **Import pattern:**
  - Read the manifest JSON for a model:tag, extract the model layer's digest, map it to the blob path, use that blob as a GGUF.

### LM Studio

- **Default storage paths:**
  - All OS: `~/.lmstudio/models/`
  - Structure: `~/.lmstudio/models/<publisher>/<model>/model-file.gguf`
  - Mirrors Hugging Face repo layout.

- **File format:**
  - Plain GGUF files. No wrapper, no proprietary format.
  - Also supports GGML (older).

- **llama-server compatibility:**
  - Direct. `llama-server -m ~/.lmstudio/models/publisher/model/model.gguf` works.

- **Import pattern:**
  - Place a downloaded GGUF into the expected directory structure, or use `lms import path/to/model.gguf`.

### KoboldCPP

- **Storage:**
  - No fixed internal storage directory. KoboldCPP is a self-contained binary.
  - Models are loaded from user-specified paths via CLI or UI:
    - CLI: `koboldcpp --model /path/to/model.gguf`
    - UI: browse to file.

- **File format:**
  - GGUF (primary), GGML (legacy).
  - No wrapper. No proprietary format.

- **llama-server compatibility:**
  - Direct. Any GGUF KoboldCPP can load is usable with `llama-server -m`.

- **Import pattern:**
  - Just point to the GGUF. No import step needed.

### vLLM

- **Storage:**
  - No proprietary storage. vLLM loads models from:
    - A Hugging Face repo ID (e.g., `Qwen/Qwen2-7B`), or
    - A local directory path containing HF-style files.
  - Uses HF cache by default:
    - Linux/macOS: `~/.cache/huggingface/hub/`
    - Windows: `%USERPROFILE%\.cache\huggingface\hub\`
    - Override: `HF_HOME`, `HF_HUB_CACHE`.

- **File format:**
  - Primary: **Safetensors** (`.safetensors`), preferred and recommended.
  - Fallback: PyTorch `.bin` / `.pt` weights.
  - Also needs: `config.json`, tokenizer files (`tokenizer.json`, `tokenizer_config.json`, etc.).
  - Does NOT natively use GGUF (it is a Python/PyTorch serving library).

- **llama-server compatibility:**
  - Not directly. llama-server expects GGUF; vLLM expects HF-style Safetensors + config.
  - To use a vLLM model with llama-server, you must convert its weights to GGUF (via llama.cpp's `convert_hf_to_gguf.py` or similar).

- **Import pattern:**
  - If you have a vLLM model directory (HF-style with Safetensors), run llama.cpp's conversion tool targeting that directory to produce a GGUF, then use that with llama-server.

### Unsloth Studio

- **Storage:**
  - Uses Hugging Face cache for model storage:
    - Default: `~/.cache/huggingface/hub/` (or wherever `HF_HOME`/`HF_HUB_CACHE` points).
  - GGUF models are stored as individual files in that cache.
  - Training checkpoints and exported models are saved to user-chosen local paths or pushed to Hugging Face Hub.

- **File format:**
  - GGUF (for local inference / llama.cpp / Ollama / LM Studio).
  - Safetensors (16-bit merged models for vLLM/Transformers).
  - LoRA adapter weights.
  - Unsloth can export any of these from trained runs.

- **llama-server compatibility:**
  - GGUF exports: direct. `llama-server -m <path-to-gguf>` works.
  - Safetensors exports: require conversion to GGUF first.

- **Import pattern:**
  - Use Unsloth's export flow to get a GGUF, then point llama-server at it.

### Text Generation WebUI (oobabooga)

- **Storage:**
  - All OS: `<repo-root>/models/` or `user_data/models/` (newer versions).
  - Models are placed as GGUF files or HF-style directories.

- **File format:**
  - GGUF (primary for llama.cpp backend).
  - Also supports HF-style Safetensors/PyTorch for non-GGUF backends.

- **llama-server compatibility:**
  - GGUF models: direct.
  - HF-style models: need conversion.

### Summary: Compatibility with llama-server

- **Directly compatible (GGUF, no conversion needed):**
  - Ollama blobs (model layer blob is raw GGUF).
  - LM Studio models (plain GGUF).
  - KoboldCPP models (plain GGUF).
  - Text Generation WebUI GGUF files.
  - Unsloth GGUF exports.
  - Any GGUF from Hugging Face.

- **Require conversion (HF-style Safetensors/PyTorch):**
  - vLLM models (Safetensors + config.json).
  - Unsloth Safetensors exports.
  - Any HF-native model not yet quantized to GGUF.

- **Key edge cases / notes:**
  - Ollama:
    - Not a single file per model; must resolve manifest -> blob.
    - Newer models may use GGUF extensions not yet supported by your llama.cpp version.
  - vLLM:
    - No GGUF support; always needs conversion for llama-server.
  - All GGUF-based tools are fundamentally compatible with llama-server, as long as the llama.cpp version you're using supports that model's architecture.

---

## Third-Party Research: llama-server Tuning and Metrics

### Key parameters and their impact

- **CONTEXT SIZE (n_ctx / -c):**
  - Controls how many tokens the model can "see" at once (input + output).
  - When n_ctx = 0, llama.cpp uses the model's training context size from metadata.
  - Larger context = more KV cache memory, more VRAM/RAM pressure.
  - Tuning guidance:
    - Chat: 4096-8192 is usually enough.
    - Roleplay / long context: 16384-32768.
    - Agentic / tool-use: 8192-16384 (tool calls + outputs accumulate).
  - Pitfall: Setting n_ctx too high is a common cause of OOM. KV cache scales linearly with context size and number of parallel slots.

- **BATCH SIZE (n_batch / -b) and U-BATCH SIZE (n_ubatch / -ub):**
  - n_batch: Logical maximum batch size for processing tokens.
  - n_ubatch: Physical batch size controlling computation granularity and buffer sizes; cannot exceed n_batch.
  - For pure GPU inference, defaults (2048/512) are fine.
  - For CPU+GPU (especially MoE), larger batches are critical:
    - Recommended: -b 4096 -ub 4096 (or higher).
    - Larger ubatch triggers "GPU offload prompt processing" (op offload), copying CPU weights to GPU for batch processing, which can be much faster.
  - Pitfall: Too small ubatch with MoE/CPU+GPU = slow prompt processing, as the GPU sits idle waiting for small batches.

- **GPU LAYERS (n_gpu_layers / -ngl):**
  - Number of model layers to offload to GPU.
  - -1: Auto-fit to available VRAM.
  - 999 or "all": Try to put as many as possible on GPU.
  - Best practice:
    - Single GPU: Use -ngl 999 or -ngl all.
    - CPU+GPU (MoE): Use -ngl 999 plus tensor overrides to keep MoE experts on CPU.
  - Pitfall: With auto-fit, you may silently get fewer layers than expected. Use -fit off to make OOM explicit when tuning.

- **TENSOR SPLIT (tensor-split / -ts):**
  - For multi-GPU: comma-separated proportions controlling how much of the model each GPU holds.
  - Example: -ts 3,1 gives GPU0 75%, GPU1 25%.
  - Use when GPUs have unequal memory.
  - Works with --split-mode layer (default) and tensor.

- **SPLIT MODE (split-mode / -sm):**
  - layer (default): Pipeline parallelism. Each GPU holds contiguous layers. Best for memory scaling, tolerant of slow interconnect.
  - tensor: Tensor parallelism. Splits each layer across GPUs. Lower latency, but heavily dependent on fast interconnect (NVLink ideal). Experimental. Not supported for all architectures (MoE, Mamba, etc. excluded).
  - none: Use only one GPU.
  - Pitfall: Using tensor mode on an unsupported architecture will fail.

- **KV CACHE QUANTIZATION (cache-type-k / -ctk, cache-type-v / -ctv):**
  - Quantize the KV cache to save memory. Options: f32, f16, bf16, q8_0, q4_0, q4_1, iq4_nl, q5_0, q5_1.
  - Default: f16 for both K and V.
  - Impact:
    - q8_0: Small memory savings, negligible quality loss. Good first step.
    - q4_0/q4_1: Large memory savings (can halve KV cache); can fit much larger context or more slots. Slight quality degradation on very long context.
  - TurboQuant: Newer extreme KV cache quantization (3.5-bit) that enables direct computation on quantized cache, reducing decode overhead at very large contexts.
  - Pitfall:
    - With --split-mode tensor, KV cache must be f32/f16/bf16 (quantized KV not yet supported).
    - Very aggressive quantization (q4_0) on long context can degrade quality on some models.

- **SPECULATIVE DECODING:**
  - Accelerates generation by drafting tokens and verifying in batch.
  - Types:
    - draft-simple / draft-mtp: Use a smaller draft model or MTP heads.
    - ngram-mod: Hash-based, shared across slots, lightweight (~16 MB). Good for reasoning, code iteration, summarization.
    - ngram-simple, ngram-map-k, ngram-map-k4v: Pattern matching on token history. Good for code refactoring and repetitive text.
  - Key params:
    - --spec-type: Comma-separated, e.g. "ngram-mod,ngram-map-k4v"
    - --spec-draft-model: Path to draft model.
    - --spec-draft-n-max: Tokens to draft (default 3).
    - --spec-default: Enables ngram-mod with sane defaults.
  - Stats printed at end show acceptance rate; above ~0.5 is good.
  - Pitfall:
    - Wrong draft model choice or too-high draft-n-max can reduce acceptance and slow things down.
    - For MoE models, longer drafts are needed.

- **MoE TUNING (n-cpu-moe / --cpu-moe):**
  - For mixture-of-experts models (DeepSeek V3, GLM, Kimi, Qwen-3 MoE, etc.).
  - MoE models are huge in total params but only a fraction active per token.
  - Strategy:
    - Put always-active layers (attention, dense FFN, shared expert FFN) on GPU.
    - Put routed expert FFN on CPU (or partially GPU).
  - Commands:
    - --cpu-moe: Keep all MoE experts on CPU.
    - --n-cpu-moe N: Keep MoE weights of the highest N layers on CPU.
    - Or use -ot (tensor override) for fine-grained control:
      - -ot "exps=CPU" puts all routed experts on CPU.
      - -ot "blk.([0-9]|1[0-9]|2[0-9])=CUDA0,exps=CPU" puts layers 0-29 on GPU and experts on CPU.
  - Multi-GPU:
    - Spread layers across GPUs while keeping experts on CPU.
    - Example: -ot "blk.([0-9])=CUDA0,blk.(1[0-9])=CUDA1,exps=CPU"
  - Prompt batch tuning:
    - For MoE with CPU+GPU, increase -b and -ub (e.g., 4096 or higher).
    - Tune GGML_OP_OFFLOAD_MIN_BATCH env var if GPU offload prompt processing is not triggering often enough.
  - Pitfall:
    - Leaving all experts on GPU will OOM for large MoE models.
    - Using -ngl 999 without --cpu-moe on a large MoE model is a common OOM cause.

- **ROPE SCALING:**
  - Used to extend context beyond the model's training length.
  - Key params:
    - --rope-scaling: "none", "linear", "yarn"
    - --rope-freq-base: Base frequency (default model-specific).
    - --rope-freq-scale: Scale factor (e.g., 0.5 for 2x extension).
    - --yarn-orig-ctx: Original context length for YaRN.
    - --yarn-ext-factor, --yarn-attn-factor: YaRN-specific tuning.
  - Guidance:
    - For models trained with YaRN, use --rope-scaling yarn with appropriate parameters.
    - For simple extension: --rope-freq-scale 0.5 for roughly 2x context.
  - Pitfall:
    - Over-scaling (e.g., trying 8x extension) typically degrades quality significantly.
    - Wrong YaRN parameters can cause garbage output even before 1x.

- **GENERATION PARAMETERS (temperature, top_p, etc.):**
  - Set per-request in the API or via defaults in llama-server.
  - temperature: 0 = greedy; higher = more random.
  - top_p: Nucleus sampling; 0.9 is common.
  - top_k: Limit to top K tokens.
  - min_p: Remove tokens below min_p * max_probability.
  - Typical guidance:
    - Chat: temp 0.7-1.0, top_p 0.9.
    - Code / agentic: temp 0.1-0.3, top_p 0.9-1.0 (more deterministic).
    - Roleplay: temp 0.8-1.2, top_p 0.95.

- **FLASH ATTENTION (flash-attn / -fa):**
  - -fa on/off/auto.
  - Required for --split-mode tensor.
  - Reduces memory and improves speed for large context.
  - Use on for most setups unless you see issues.

- **PARALLEL SLOTS (n_parallel / -np):**
  - Number of concurrent inference slots.
  - Each slot reserves its own KV cache (proportional to n_ctx).
  - Tuning:
    - 1 for single-user.
    - 2-4 for small teams.
    - Higher for multi-tenant; watch VRAM.
  - Pitfall: Setting n_parallel too high is a leading cause of OOM.

### Metrics endpoints and interpretation

llama-server exposes several observability endpoints:

- **/health:**
  - Returns server status, slot counts (idle, processing).
  - Useful for health checks and load balancing.
  - Note: Blocks during prompt/image processing; not ideal for sub-millisecond liveness checks.

- **/props:**
  - Returns model properties (architecture, context size, layers, etc.).
  - Useful for validating loaded model.

- **/slots:**
  - Returns per-slot state:
    - slot_id, state (idle, processing, etc.)
    - tokens processed, timing info.
  - Useful for:
    - Monitoring active sessions.
    - Detecting stuck slots.
    - Understanding load distribution.

- **/metrics (Prometheus-compatible):**
  - Must be enabled with --metrics flag.
  - Key metrics:
    - llamacpp:prompt_tokens_total: Total prompt tokens processed.
    - llamacpp:prompt_tokens_seconds: Prompt tokens processed in the current bucket window.
    - llamacpp:predicted_tokens_total: Total generated tokens.
    - llamacpp:predicted_tokens_seconds: Generated tokens in the current window.
    - llamacpp:kv_cache_usage_ratio: KV cache usage as a fraction.
    - llamacpp:kv_cache_tokens_total: Total tokens in KV cache.
    - llamacpp:tokens_inflight: Tokens currently being generated.
    - llamacpp:slot_requests_total: Total requests.
    - llamacpp:speculative_stats_*: Speculative decoding statistics.
  - Note: Metrics are reset on /health calls (shared TASK_TYPE_METRICS). For production monitoring, scrape /metrics only, not /health.

How to interpret metrics:

- **Prompt tokens/sec:**
  - Derived from rate(llamacpp:prompt_tokens_seconds[5m]).
  - Indicates prefill throughput.
  - Low values may indicate:
    - Undersized batch sizes (for MoE/CPU+GPU).
    - GPU underutilization.
    - NUMA issues (cross-socket RAM access).

- **Generation tokens/sec:**
  - Derived from rate(llamacpp:predicted_tokens_seconds[5m]).
  - Indicates decode throughput.
  - Low values may indicate:
    - Too many layers on CPU.
    - KV cache pressure / fragmentation.
    - Memory bandwidth saturation.
    - Too many parallel slots sharing the same GPU.

- **Latency:**
  - Not directly exposed as a histogram, but can be inferred from:
    - Slot timing from /slots.
    - Correlation of tokens_inflight with queue depth.
  - For precise latency, add application-level instrumentation or use a gateway that measures TTFT and per-token latency.

- **Memory / KV cache:**
  - kv_cache_usage_ratio:
    - Above 0.9: high risk of OOM / eviction / latency spikes.
    - Use this to trigger alerts or autoscaling.
  - kv_cache_tokens_total:
    - Compare against n_ctx * n_parallel to see how close you are.

### Best practices by use case

- **Chat (interactive, single-user or small team):**
  - n_ctx: 4096-8192.
  - n_parallel: 1-2.
  - -ngl: all or 999.
  - -fa: on.
  - Speculative: ngram-mod with defaults can help.

- **Roleplay (long context, creative):**
  - n_ctx: 16384-32768.
  - -ngl: all.
  - KV quant: q8_0 if VRAM tight.
  - Speculative: ngram-simple or ngram-mod for repetitive patterns.

- **Agentic / tool-use (API calls, tool outputs, long turns):**
  - n_ctx: 8192-16384.
  - n_parallel: 2-4 (multiple agents).
  - KV quant: q8_0 if needed.
  - Speculative: ngram-mod or draft model if you have repetitive patterns (e.g., JSON responses).
  - Watch KV cache usage: tool call round-trips fill context quickly.

- **MoE models (DeepSeek V3, GLM, Kimi, Qwen-3 MoE):**
  - Use --cpu-moe or -ot to offload experts.
  - -b 4096 -ub 4096 (or higher).
  - -ngl 999 with expert override.
  - Tune GGML_OP_OFFLOAD_MIN_BATCH for CPU+GPU.
  - NUMA tuning: bind to one socket if possible, or use numactl with interleave.

### Best practices by hardware

- **Single GPU (consumer or workstation):**
  - -ngl all.
  - -fa on.
  - n_parallel 1-2.
  - n_ctx 4096-8192 unless you know you need more.
  - Use KV quant (q8_0 or q4_0) if VRAM is tight.
  - Avoid speculative decoding overhead unless it meaningfully helps (check acceptance rate).

- **Multi-GPU (NVIDIA):**
  - Use --split-mode layer as default.
  - Use --split-mode tensor if:
    - You have fast interconnect (NVLink or PCIe Gen4/5 x16+).
    - Your architecture is supported.
    - You want lower latency.
  - Use -ts to balance by memory (e.g., 3,1 for uneven GPUs).
  - Use NCCL (build with -DGGML_CUDA_NCCL=ON) for tensor mode.
  - Consider GGML_CUDA_P2P=1 for direct GPU-to-GPU memory access (test for stability).

- **CPU-heavy (MoE, large models, limited VRAM):**
  - Use --cpu-moe to keep experts on CPU.
  - Pin threads with --cpu-mask or --cpu-range.
  - NUMA:
    - Single NUMA node: bind process to that node.
    - Multi-socket: use numactl, disable NUMA balancing, consider --numa distribute.
  - Increase batch sizes for GPU offload prompt processing.
  - Use --no-mmap for better control; or --mmap with NUMA-aware memory migration.

### Common pitfalls and misconfigurations

- OOM from too-large n_ctx:
  - The #1 cause. Start conservative and increase.

- OOM from too-high n_parallel:
  - Each slot reserves its own KV cache. Doubling n_parallel roughly doubles KV memory.

- MoE OOM:
  - Using -ngl 999 without --cpu-moe or -ot on large MoE models.

- Slow prompt processing with MoE:
  - Default batch sizes (2048/512) are too small. Increase to 4096+.

- Metrics reset on /health:
  - If you scrape /health and /metrics from the same Prometheus job, metrics can be reset unexpectedly. Scrape /metrics only.

- Tensor mode on unsupported architectures:
  - MoE, Mamba, etc. will fail. Use layer mode.

- Speculative decoding hurting performance:
  - If acceptance rate is low, speculative decoding adds overhead. Monitor and disable if not helping.

- NUMA issues on multi-socket:
  - Letting llama.cpp use both sockets freely can degrade performance. Use numactl.

- CUDA_VISIBLE_DEVICES shadowing:
  - Accidentally set to empty or wrong GPUs, causing silent CPU fallback.

- Over-scaling RoPE:
  - Extending context too far degrades quality. Test with your model.

---

## UI/UX Decisions

### U1. Premium modal wizard

Use existing premium modal patterns:

- Overlay: radial vignette + backdrop blur(16px).
- Shell: glassmorphism, blur(24px), breathing border.
- Width: `min(680px, 90vw)`.
- Max-height: `85vh`, scrollable body.

Structure:

- Hero:
  - Icon + title + short description.
- Steps:
  - **Step 1: Connection / Mode**
    - Choose:
      - Connect to existing endpoint.
      - Spawn new llama-server (from preset or from scratch).
  - **Step 2: Model & Templates**
    - Select model (local browse or HF).
    - Upload/point chat template.
  - **Step 3: Resources**
    - GPU layers, context size, batch sizes.
    - VRAM estimator feedback inline.
  - **Step 4: Advanced**
    - Parameter editor (guided + raw).
    - MoE tuning.
    - Import launch file.
    - llama.cpp binary download ("Get llama-server").
- Footer:
  - Primary: "Spawn Server" or "Save Preset".
  - Secondary: "Cancel".

### U2. Design tokens and consistency

- Use only existing tokens from `static/css/tokens.css`:
  - Colors, gradients, radii, shadows.
- Reuse:
  - `.widget-card` for sections.
  - `.btn-primary` / `.btn-secondary` for actions.
  - Existing input/select styles.
- Provide:
  - `[data-theme="light"]` overrides.
  - `@media (prefers-reduced-motion: reduce)`.

### U3. Inline intelligence

Add small inline feedback:

- Next to `context_size`: "Estimated VRAM impact: +X MB".
- Next to `n-cpu-moe`: "Using Y experts in CPU may reduce throughput by Z%".

Style:

- Subtle info chips with icons.

---

## API Design (New / Changed Endpoints)

All new endpoints:

- Auth:
  - Use existing auth_guard.
  - Bearer token: Authorization: Bearer <token>.
  - Session cookie: llama_monitor_session (Secure, HttpOnly, SameSite=Lax).
  - Public endpoints: /api/health (and any explicitly marked).
  - All others: auth-required (api-token or session cookie).
  - api-token: no expiry unless rotated.
  - Session cookie: max-age + invalidation on logout.

- Rate limiting:
  - Global: 200 req/s + burst 500 (token bucket).
  - Per-endpoint limits apply first; global is a ceiling.
  - Scope: per IP and per auth token.
  - On exceed:
    - 429 with { "error": "rate_limited", "message": "..." } and Retry-After header.

- Error envelope:
  - Standard shape:
    - { "error": "short_code", "code": "MACHINE_CODE", "message": "Human-readable.", "request_id": "..." }
  - No stack traces in responses.
  - Always include request_id for debugging.

- SSRF / external fetch:
  - Allowed domains:
    - HuggingFace: huggingface.co, *.hf.sh, huggingface.co/api.
    - GitHub: github.com, raw.githubusercontent.com.
    - Gist: gist.githubusercontent.com.
  - Block internal/metadata IPs: 10/8, 172.16/12, 192.168/16, 169.254/16, 127/8.
  - Limits:
    - Max redirect: 5.
    - Timeout: 15s for fetch, 300s for large downloads.
    - Max response size: 50MB for templates; model downloads via streaming.

- Path traversal / filesystem:
  - Normalize paths (canonicalize).
  - Enforce roots:
    - Models: under models_dir.
    - Scripts: under scripts_dir.
  - Reject:
    - “..” escapes.
    - Absolute paths outside allowed roots.
    - Control characters.

- Prompt injection / content trust:
  - Treat templates as untrusted.
  - Keep templates logically separated from system prompts.
  - Limit size (e.g., 1MB); disallow non-text/binary content.

- Observability:
  - Log request IDs.
  - Log rate limit events.
  - Never log full secrets or full prompt content.

Return structured errors:
  - 400: { "error": "bad_request", "code": "invalid_params", "message": "..." }
  - 401: { "error": "unauthorized", "code": "missing_token", "message": "..." }
  - 403: { "error": "forbidden", "code": "forbidden", "message": "..." }
  - 404: { "error": "not_found", "code": "not_found", "message": "..." }
  - 429: { "error": "rate_limited", "code": "rate_limited", "message": "..." }
  - 500: { "error": "internal_error", "code": "internal_error", "message": "..." }

- `POST /api/import-launch-file`
  - Auth: api-token or session cookie.
  - Rate limit: 20 req/min.
  - Body: `{ content: string, os: "windows" | "macos" | "linux" }`
    - content = full script text.
    - Supported: batch, PowerShell, bash, zsh.
  - Response: `{ preset: ModelPreset, warnings: [string] }`
  - Validation:
    - Max size: 50KB.
    - Sanitize paths, env vars, and commands.
    - Handle conflicting/unsafe flags (e.g., --host 0.0.0.0).
  - Warnings:
    - Non-blocking; free-text.
  - Errors:
    - 400: invalid content or os.
    - 413: content too large.

- `POST /api/chat-template/fetch`
  - Auth: api-token or session cookie.
  - Rate limit: 10 req/min.
  - Body: `{ source_type: "hf" | "github" | "gist", source: string }`
    - For hf: source = "org/model" or full repo URL.
      - Prefer: chat_template.jinja, tokenizer_config.json.
    - For github: source = raw URL or repo+branch+path.
    - For gist: source = gist ID + filename.
  - Auth for gated models:
    - Use HF_TOKEN from config if needed.
  - Response: `{ template: string, source_url: string }`
  - Errors:
    - 400: invalid URL.
    - 401: gated model (no token).
    - 403: forbidden (private repo).
    - 404: not found.
    - 429: rate limited.

- `POST /api/chat-template/upload`
  - Auth: api-token or session cookie.
  - Rate limit: 10 req/min.
  - Multipart: field name = "file".
  - Allowed: .jinja, .txt, .json, .yaml.
  - Max size: 1MB.
  - template_id: generated UUID.
  - Persistence:
    - Stored in app config or templates_dir.
    - Per-user or global (clarify in implementation).
  - Response: `{ template_id, template }`

- `POST /api/models/download`
  - Auth: api-token or session cookie.
  - Rate limit: 5 req/min.
  - Body: `{ repo_id, file_path, target_dir }`
    - repo_id format: "org/model" only.
    - file_path: relative to repo root; only .gguf files allowed.
    - target_dir: must be under allowed models_dir.
  - Auth for gated models:
    - Use HF_TOKEN from config if needed.
  - Concurrency:
    - Max 3 parallel downloads.
    - Dedup by repo_id+file_path.
  - Response: `{ download_id }`
  - Errors:
    - 400: invalid repo_id or file_path.
    - 401: gated model (no token).
    - 403: forbidden.
    - 404: not found.
    - 429: rate limited.

- `GET /api/models/download/:id/status`
  - Auth: api-token or session cookie.
  - Rate limit: 200 req/s.
  - Response:
    - `{ progress, bytes_downloaded, total, speed, eta, status }`
    - eta = seconds (numeric).
    - status values: pending, downloading, completed, failed, cancelled.
    - On failure: include "error" field.
  - Errors:
    - 404: if ID invalid.

- `POST /api/models/download/:id/cancel`
  - Auth: api-token or session cookie.
  - Rate limit: 10 req/min.
  - Behavior:
    - Idempotent: re-cancelling is allowed.
    - Authz: same token/session can cancel its own downloads.
    - If already completed/failed: return 400 with "already_completed" or "already_failed".

- `POST /api/estimate-vram`
  - Auth: api-token or session cookie.
  - Rate limit: 20 req/min.
  - Input:
    - model_path or model_id,
    - context_size,
    - kv_quant,
    - parallel_slots,
    - speculative flags,
    - n_cpu_moe (if MoE),
    - mmproj_size.
  - Output:
    - estimated_vram_bytes,
    - estimated_ram_bytes,
    - verdict (Fit/Tight/Risk/Won't fit),
    - short guidance string.
  - Behavior:
    - Read-only, no side effects.
    - Uses existing GPU monitoring for available_vram.

- `POST /api/benchmark`
  - Auth: api-token or session cookie.
  - Rate limit: 2 req/min.
  - Body: `{ prompt: string, max_tokens: usize, temperature: f32 }`
  - Response: `{ ttft_ms, gen_tps, prompt_tps, verdict, suggestions: [string] }`
  - Errors:
    - 400: invalid prompt or max_tokens.
    - 404: llama-server not running.
    - 500: benchmark failed.

- `GET /api/llama-cpp/releases`
  - Auth: api-token or session cookie.
  - Rate limit: 10 req/min.
  - Response: `[ LlamaCppRelease ]`

- `POST /api/llama-cpp/download`
  - Auth: api-token or session cookie.
  - Rate limit: 2 req/min.
  - Body: `{ release_tag, backend, arch }`
  - Response: `{ download_id }`
  - Errors:
    - 400: invalid release_tag, backend, or arch.
    - 404: not found.
    - 429: rate limited.

- `GET /api/llama-cpp/download/:id/status`
  - Auth: api-token or session cookie.
  - Rate limit: 200 req/s.
  - Response: `{ progress, status, message }`

- `POST /api/llama-cpp/download/:id/cancel`
  - Auth: api-token or session cookie.
  - Rate limit: 10 req/min.

- Extend:
  - `POST /api/start` and `POST /api/sessions/spawn`:
    - Accept new fields (`chat_template_file`, `n_cpu_moe`, etc.).

---

## Data Models (Key Changes)

- `ServerConfig` (`src/llama/server.rs`):
  - Add:
    - `chat_template_file: Option<String>`
    - `benchmark_mode: bool`

- `ModelPreset` (`src/presets/mod.rs`):
  - Add:
    - `chat_template_file: Option<String>`
    - `benchmark_mode: bool`

- New structs:
  - `VramEstimateRequest` / `VramEstimateResponse`
  - `ModelDownloadRequest` / `ModelDownloadStatus`
  - `BenchmarkRequest` / `BenchmarkResult`
  - `ImportLaunchRequest` / `ImportLaunchResponse`
  - `LlamaCppRelease`, `LlamaCppAsset`
  - `LlamaCppDownloadRequest` / `LlamaCppDownloadStatus`

All with `#[serde(default)]` and safe defaults.

---

## Implementation Plan (Phased)

### Phase 1: Foundation

- Create new modules:
  - `src/llama/spawn_wizard.rs`
  - `src/llama/batch_import.rs`
  - `src/llama/vram_estimator.rs`
  - `src/llama/llama_cpp_downloader.rs`
  - `src/hf/mod.rs`
  - `src/model_download.rs`
- Extend:
  - `ServerConfig`
  - `ModelPreset`
  - `AppConfig` (binaries_dir, default_models_dir, scripts_dir, certs_dir)
- Add new API endpoints (basic, no UI yet):
  - `/api/import-launch-file`
  - `/api/chat-template/fetch`
  - `/api/chat-template/upload`
  - `/api/estimate-vram`
  - `/api/models/download`
  - `/api/models/download/:id/status`
  - `/api/models/download/:id/cancel`
  - `/api/llama-cpp/releases`
  - `/api/llama-cpp/download`
  - `/api/llama-cpp/download/:id/status`
  - `/api/llama-cpp/download/:id/cancel`
- Tests:
  - Unit tests for:
    - batch_import parsing (Windows vs Unix).
    - vram_estimator edge cases.
    - hf API wrappers.
    - llama_cpp_downloader asset selection logic.

### Phase 2: Premium Spawn Wizard UI

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
  - Accessible (aria-* attributes).

### Phase 3: HuggingFace + Downloads

- Integrate:
  - HF model picker in Step 2.
  - Download progress UI.
- Wire:
  - Selected HF model → model dir → preset.

### Phase 4: Advanced Parameters + Chat Templates

- Implement:
  - Dual-mode parameter editor.
  - Chat template upload/URL fields.
- Sync:
  - Guided and raw modes.

### Phase 5: MoE Tuning + Benchmark

- Implement:
  - MoE expert offload UI.
  - Benchmark endpoint and UI button.
- Integrate AI-assisted tuning hints (optional).

### Phase 6: Polish

- Cross-platform checks:
  - Windows, macOS, Linux.
- Edge cases:
  - Long paths.
  - Very large contexts.
  - Very large MoE models.
- Accessibility and theming.
- Documentation updates:
  - `docs/reference/chat.md` or a new `docs/reference/spawn-server.md`.

---

## Additional Ideas (Optional)

- Preset versioning:
  - Add a version field to presets.
  - When backend schema changes, allow migration.
- "Quick profiles":
  - Prebuilt profiles: "Max Quality", "Fast Chat", "Agentic/Tooling", "RP Large Context".
- System profile:
  - Auto-detect hardware and suggest a baseline config.
- Export:
  - Allow exporting a preset as a launch script for the current OS.

---

## Security Guidelines

- Token handling:
  - HF token:
    - Store in encrypted config file (AES-256-GCM).
    - Load from env first, then from config.
    - Never log or expose in API responses.
  - API tokens:
    - Use constant-time comparison.
    - Rotate via dedicated endpoints.
- Network requests:
  - Enforce TLS for external endpoints.
  - Limit redirect depth (<= 5).
  - Block private ranges unless explicitly permitted.
- File operations:
  - Normalize all user-supplied paths.
  - Restrict /api/browse to allowed roots.
  - Use temp files for downloads, then move.
- Secrets management:
  - Use existing encrypt_value/decrypt_value for all new secrets.
  - Keep encryption key in env or encrypted file.

## Performance Guidelines

- Large models:
  - Provide VRAM estimates before spawning.
  - Warn when estimated usage > 90% of available VRAM.
- Slow networks:
  - Use resumable downloads for large files.
  - Respect 429 and RateLimit headers.
- Low-end hardware:
  - Limit llama-monitor's own resource usage.
  - Suggest smaller quantizations when VRAM is low.
- Optimization:
  - Lazy-load heavy components.
  - Cache model list in memory and on disk.
  - Use Arc and shared state instead of duplicating large configs.

---

END
