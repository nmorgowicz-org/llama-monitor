# Spawn Llama-Server V2 — Architecture & Implementation Plan

Branch: feature/spawn-llama-server-v2
Date: 2026-05-29
Author: Iris (via Hermes)

This document is the single source of truth for the "Spawn Llama-Server V2" feature.
It is written so that a fresh AI agent can read it and execute the implementation without guessing.

══════════════════════════════════════════════
GOALS (MILESTONES)
══════════════════════════════════════════════

These are Nick's requirements, restated as concrete milestones.

M1. Welcome screen: connect or spawn

- On first launch / welcome screen:
  - User can:
    - Select a previous remote endpoint they connected to.
    - Enter a new endpoint.
  - On the right side:
    - Option to spin up a new llama-server.
    - Option to spin up a new llama-server using an existing preset.
- Current state: partially implemented.
- Goal: unify and polish this into a single coherent welcome flow with premium UI.

M2. Modern, premium "Spawn Llama-Server" UX

- New "Spawn Llama-Server" experience must:
  - Match the 2026 UI/UX style already used in the app (glassmorphism, premium modals, micro-interactions).
  - Be intuitive, guided, and visually consistent.
- Current state: basic modals / forms.
- Goal: redesign as a step-based wizard modal with clear sections, inline help, tooltips, and consistent design tokens.

M3. Import existing launch settings (batch file / script)

- Support importing a user's existing launch file:
  - Windows: .cmd / .bat
  - macOS/Linux: .sh / .bash / .zsh
- System must:
  - Parse the script to extract:
    - llama-server binary path.
    - All flags and arguments.
  - Normalize:
    - Handle newline differences (\r\n vs \n vs ^).
    - Clean up environment-specific artifacts.
  - Adapt:
    - Binary path to match the system's llama-server location.
    - OS-specific line endings and quoting.
- Goal: produce a clean, editable preset from an imported script.

M4. Advanced parameter editing (dual mode)

- Provide two modes for editing launch parameters:
  - Raw mode:
    - Show the generated launch script (for the current OS) in an editable code area.
  - Guided mode:
    - Structured UI with:
      - Grouped parameters (model, GPU, context, batch, generation, speculative decoding, etc.).
      - Inline descriptions / tooltips.
      - Validation and suggestions.
- Both modes:
  - Must stay in sync (edits in one update the other).
- Goal: make tuning approachable for both power users and casual users.

M5. Chat template support (.jinja / HF / GitHub)

- Support:
  - Uploading a .jinja chat template file.
  - Specifying a HuggingFace URL.
  - Specifying a GitHub Gist / repo path.
- System must:
  - Fetch or load the template.
  - Map it to --chat-template-file when spawning llama-server.
- Goal: allow flexible, external chat templates without manual filesystem work.

M6. Multiplatform model selection modal

- Provide a modern, multiplatform model selection modal:
  - Point to a local or shared directory containing GGUF models.
  - Allow browsing and selecting models.
  - Integrate with existing scan_models_dir and /api/models.
- Goal: make model selection feel native and polished on all platforms.

M7. HuggingFace model pulling with quant selection

- Allow:
  - Pointing to a HuggingFace repo.
  - Selecting a quantization variant.
  - Downloading directly to the user's configured model directory.
- Requirements:
  - Use HF Hub API to list GGUF files.
  - Respect HUGGING_FACE_HUB_TOKEN for gated models.
  - Show file size, quant label, and basic metadata.
- Goal: seamless "discover → select → download → use" path inside the app.

M8. VRAM estimation and trade-off guidance

- Implement estimation logic that:
  - Uses model size / quantization + context size + KV quantization + speculative decoding settings + batch size to estimate VRAM usage.
  - Compares against available VRAM (from existing GPU monitoring).
  - Advises:
    - If the current config fits.
    - If it risks spilling into system RAM (performance hit).
    - What to adjust:
      - Reduce context size.
      - Use a different quantization.
      - Tune batch/ubatch.
- For multimodal models:
  - Account for mmproj size.
- For MoE models:
  - Account for --n-cpu-moe and its effect on RAM vs VRAM.
- Goal: give users intelligent, quantitative guidance before launch.

M9. MoE expert offload tuning

- For MoE models:
  - Expose --n-cpu-moe as a tunable option.
  - Offer suggestions:
    - How many experts to keep in VRAM vs RAM.
    - Impact on throughput and latency.
- Goal: make MoE tuning accessible without deep internal knowledge.

M10. Health check / benchmark button

- Provide an internal tooling button:
  - "Run Health Check / Benchmark"
- Behavior:
  - Uses the loaded AI model to run a short, controlled benchmark:
    - Prompt throughput.
    - Generation throughput.
    - Latency.
  - Optionally:
    - Interprets results with AI assistance to suggest tuning.
- Constraints:
  - Must be opt-in.
  - Must be safe (bounded tokens, time, no destructive actions).
- Goal: help users validate that their configuration is sane and performant.

══════════════════════════════════════════════
CURRENT STATE (SUMMARY)
══════════════════════════════════════════════

Key existing capabilities:

- Spawning llama-server:
  - src/llama/server.rs:
    - ServerConfig struct: all parameters.
    - start_server(): builds command, sets env, spawns child, streams logs.
  - src/web/api.rs:
    - POST /api/start: direct start from config.
    - POST /api/sessions/spawn: spawn with preset via session.
- Presets:
  - src/presets/mod.rs:
    - ModelPreset struct.
    - load_presets/save_presets, default_presets.
  - Frontend:
    - Preset modal, CRUD via /api/presets.
- Model discovery:
  - src/models/mod.rs:
    - scan_models_dir: scans for .gguf.
  - /api/models, /api/models/refresh.
- File browser:
  - /api/browse?path=...&filter=gguf
  - static/js/features/file-browser.js.
- GPU/VRAM monitoring:
  - src/gpu/*:
    - nvidia-smi, rocm-smi, Apple unified memory, Windows WMI.
- Metrics:
  - src/llama/metrics.rs, src/llama/poller.rs.
- Download infrastructure:
  - src/agent.rs:
    - download_asset_locally, etc. (for app/agent updates).
- UI patterns:
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

══════════════════════════════════════════════
ARCHITECTURE DECISIONS
══════════════════════════════════════════════

A1. Use a dedicated "Spawn Wizard" module

- Rationale:
  - This is complex; a single cohesive module keeps it maintainable.
- Implementation:
  - Rust:
    - New module: src/llama/spawn_wizard.rs
    - Responsibilities:
      - Batch-file parsing.
      - Parameter normalization.
      - VRAM estimation.
      - MoE tuning suggestions.
      - Benchmark coordination.
  - Frontend:
    - New JS: static/js/features/spawn-wizard.js
    - New CSS: static/css/spawn-wizard.css (or extend agent-modal.css).
    - New HTML: spawn wizard modal in index.html.

A2. Extend ServerConfig and ModelPreset

- Rationale:
  - New capabilities (chat-template-file, MoE tuning, benchmark flags) must be first-class.
- Changes:
  - ServerConfig:
    - Add:
      - chat_template_file: Option<String>
      - n_cpu_moe: Option<usize> (already present; ensure exposed).
      - benchmark_mode: bool (internal flag).
  - ModelPreset:
    - Mirror new fields.
  - Ensure serde(default) on all new fields.

A3. HuggingFace integration via HF MCP

- Rationale:
  - We already have HF MCP configured in OpenCode and available in Hermes.
  - For the app backend, we will use direct HF Hub REST API (no MCP dependency).
- Design:
  - New module: src/hf/mod.rs
  - Functions:
    - hf_list_repo_files(repo_id: &str)
    - hf_get_file_info(repo_id: &str, path: &str)
    - hf_get_model_info(repo_id: &str)
  - Auth:
    - Read HUGGING_FACE_HUB_TOKEN from env.
    - Document in README / settings.

A4. Model download: incremental, resumable, safe

- Rationale:
  - Large models; users expect progress, resumability, and safety.
- Design:
  - Extend src/agent.rs or new src/model_download.rs:
    - download_model_file(url, dest, token)
    - Supports:
      - Range requests.
      - Partial file (resume).
      - Integrity check (file size comparison).
  - API:
    - POST /api/models/download:
      - Body: { repo_id, file_path, target_dir }
      - Response: { download_id }
    - GET /api/models/download/:id/status:
      - Response: { progress, bytes_downloaded, total, speed, eta, status }
    - POST /api/models/download/:id/cancel
  - Frontend:
    - Progress modal with:
      - File name, size, ETA.
      - Pause/Cancel.

A5. VRAM estimation: heuristic-based, transparent

- Rationale:
  - Exact VRAM usage depends on llama.cpp internals.
  - We'll use robust heuristics and clearly label as estimates.
- Design:
  - New module: src/llama/vram_estimator.rs
  - Inputs:
    - model_size_bytes (from file or HF metadata).
    - context_size.
    - kv cache quantization (ctk/ctv).
    - batch_size, ubatch_size.
    - speculative decoding flags.
    - multimodal (mmproj) size.
    - MoE expert offload settings.
  - Logic:
    - weights_memory ≈ model_size_bytes (approx).
    - kv_cache_memory ≈ f(context_size, kv_quant, parallel_slots).
    - speculative_overhead ≈ small factor if enabled.
    - mmproj_memory ≈ file size if present.
    - MoE:
      - Experts in VRAM vs RAM:
        - n_cpu_moe experts stored in system memory (slower).
  - Output:
    - estimated_vram_needed.
    - estimated_ram_needed.
    - recommendation:
      - "Fit", "Tight", "Risk", "Won't fit".
  - API:
    - POST /api/estimate-vram:
      - Body: estimation input.
      - Response: structured estimate + human-readable note.

A6. Batch-file import: robust, cross-platform

- Rationale:
  - Users have messy scripts; we must normalize them.
- Design:
  - New module: src/llama/batch_import.rs
  - Steps:
    - Normalize newlines:
      - Replace \r\n and \r with \n.
      - Handle ^ (Windows line continuation) by joining lines.
    - Locate llama-server invocation:
      - First token that matches "llama-server" or "llama-server.exe".
    - Extract arguments:
      - Tokenize respecting quotes.
      - Parse known flags: -m, -ngl, -c, -b, -ub, etc.
      - Keep unrecognized flags in extra_args.
    - Normalize paths:
      - Detect model path from -m.
      - Detect binary path.
    - Return:
      - A ModelPreset struct.
  - API:
    - POST /api/import-launch-file:
      - Body: { content: string, os: "windows" | "macos" | "linux" }
      - Response: { preset: ModelPreset, warnings: [string] }

A7. Parameter editor: dual-mode, consistent

- Rationale:
  - Power users want raw control.
  - Others need guidance.
- Design:
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
      - Validation (e.g., context_size must be power-of-two or multiple-of-256).
  - Raw mode:
    - Generated script:
      - OS-specific:
        - Windows: .cmd with ^ line continuations.
        - Unix: .sh with \ continuations.
      - Editable.
    - On blur/change:
      - Parse and update guided fields where possible.
  - Sync:
    - Source of truth: internal config object.
    - Both views derive from it.

A8. Chat template import

- Rationale:
  - Users may have external templates.
- Design:
  - Supported:
    - File upload.
    - HuggingFace URL (direct file).
    - GitHub Gist / repo raw URL.
  - New API:
    - POST /api/chat-template/fetch:
      - Body: { source_type, source }
      - Response: { template: string, source_url: string }
    - POST /api/chat-template/upload:
      - Multipart with file.
      - Response: { template_id, template }
  - Integration:
    - Stored in app config or preset.
    - Passed as --chat-template-file when spawning.

A9. Benchmark / health check

- Rationale:
  - Users want to know if their config is sane.
- Design:
  - New API:
    - POST /api/benchmark:
      - Body: {
          prompt: string,
          max_tokens: usize,
          temperature: f32
        }
      - Behavior:
        - Sends a prompt to llama-server.
        - Measures:
          - Time to first token.
          - Generation tokens/sec.
          - Total tokens.
        - Uses existing metrics endpoints + timing.
  - Frontend:
    - "Run Health Check" button.
    - Results panel:
      - Tokens/sec (prompt and generation).
      - Latency.
      - Simple verdict: "Good", "Moderate", "Poor".
      - Optional tuning hints.
  - Safety:
    - Hard limits:
      - max_tokens <= 2048.
      - timeout <= 60s.
    - No destructive operations.

A10. MoE tuning assistance

- Rationale:
  - MoE models need expert offload tuning.
- Design:
  - In spawn wizard:
    - If model is detected as MoE (via name pattern or metadata):
      - Show:
        - "Experts to offload to CPU (--n-cpu-moe)"
        - Slider or input.
        - Guidance:
          - "Higher = more VRAM, faster.
           Lower = more CPU, slower."
  - Integrate with VRAM estimator to show impact.

══════════════════════════════════════════════
UI/UX DECISIONS
══════════════════════════════════════════════

U1. Premium modal wizard

- Use existing premium modal patterns:
  - Overlay: radial vignette + backdrop blur(16px).
  - Shell: glassmorphism, blur(24px), breathing border.
  - Width: min(680px, 90vw).
  - Max-height: 85vh, scrollable body.
- Structure:
  - Hero:
    - Icon + title + short description.
  - Steps:
    - Step 1: Connection / Mode
      - Choose:
        - Connect to existing endpoint.
        - Spawn new llama-server (from preset or from scratch).
    - Step 2: Model & Templates
      - Select model (local browse or HF).
      - Upload/point chat template.
    - Step 3: Resources
      - GPU layers, context size, batch sizes.
      - VRAM estimator feedback inline.
    - Step 4: Advanced
      - Parameter editor (guided + raw).
      - MoE tuning.
      - Import launch file.
  - Footer:
    - Primary: "Spawn Server" or "Save Preset".
    - Secondary: "Cancel".

U2. Design tokens and consistency

- Use only existing tokens from static/css/tokens.css:
  - Colors, gradients, radii, shadows.
- Reuse:
  - .widget-card for sections.
  - .btn-primary / .btn-secondary for actions.
  - Existing input/select styles.
- Provide:
  - [data-theme="light"] overrides.
  - @media (prefers-reduced-motion: reduce).

U3. Inline intelligence

- Add small inline feedback:
  - Next to context_size: "Estimated VRAM impact: +X MB".
  - Next to n-cpu-moe: "Using Y experts in CPU may reduce throughput by Z%".
- Style:
  - Subtle info chips with icons.

══════════════════════════════════════════════
API DESIGN (NEW / CHANGED ENDPOINTS)
══════════════════════════════════════════════

- POST /api/import-launch-file
  - Import and parse a launch file into a preset.

- POST /api/chat-template/fetch
  - Fetch chat template from URL (HF/GitHub/Gist).

- POST /api/chat-template/upload
  - Upload a .jinja template file.

- POST /api/models/download
  - Start a model download from HF.

- GET /api/models/download/:id/status
  - Get download progress.

- POST /api/models/download/:id/cancel
  - Cancel a download.

- POST /api/estimate-vram
  - Estimate VRAM usage for a configuration.

- POST /api/benchmark
  - Run a short benchmark on a running llama-server.

- Extend:
  - POST /api/start and POST /api/sessions/spawn:
    - Accept new fields (chat_template_file, n_cpu_moe, etc.).

══════════════════════════════════════════════
DATA MODELS (KEY CHANGES)
══════════════════════════════════════════════

- ServerConfig (src/llama/server.rs):
  - Add:
    - chat_template_file: Option<String>
    - benchmark_mode: bool

- ModelPreset (src/presets/mod.rs):
  - Add:
    - chat_template_file: Option<String>
    - benchmark_mode: bool

- New structs:
  - VramEstimateRequest / VramEstimateResponse
  - ModelDownloadRequest / ModelDownloadStatus
  - BenchmarkRequest / BenchmarkResult
  - ImportLaunchRequest / ImportLaunchResponse

All with #[serde(default)] and safe defaults.

══════════════════════════════════════════════
IMPLEMENTATION PLAN (PHASED)
══════════════════════════════════════════════

Phase 1: Foundation

- Create new modules:
  - src/llama/spawn_wizard.rs
  - src/llama/batch_import.rs
  - src/llama/vram_estimator.rs
  - src/hf/mod.rs
  - src/model_download.rs
- Extend:
  - ServerConfig
  - ModelPreset
- Add new API endpoints (basic, no UI yet):
  - /api/import-launch-file
  - /api/chat-template/fetch
  - /api/chat-template/upload
  - /api/estimate-vram
  - /api/models/download
  - /api/models/download/:id/status
  - /api/models/download/:id/cancel
- Tests:
  - Unit tests for:
    - batch_import parsing (Windows vs Unix).
    - vram_estimator edge cases.
    - hf API wrappers.

Phase 2: Premium Spawn Wizard UI

- Implement:
  - static/css/spawn-wizard.css
  - static/js/features/spawn-wizard.js
  - HTML for spawn wizard modal.
- Integrate:
  - File browser for model selection.
  - VRAM estimation into Step 3.
  - Import launch file into Step 4.
- Ensure:
  - Consistent with existing UI patterns.
  - Accessible (aria-* attributes).

Phase 3: HuggingFace + Downloads

- Integrate:
  - HF model picker in Step 2.
  - Download progress UI.
- Wire:
  - Selected HF model → model dir → preset.

Phase 4: Advanced Parameters + Chat Templates

- Implement:
  - Dual-mode parameter editor.
  - Chat template upload/URL fields.
- Sync:
  - Guided and raw modes.

Phase 5: MoE Tuning + Benchmark

- Implement:
  - MoE expert offload UI.
  - Benchmark endpoint and UI button.
- Integrate AI-assisted tuning hints (optional).

Phase 6: Polish

- Cross-platform checks:
  - Windows, macOS, Linux.
- Edge cases:
  - Long paths.
  - Very large contexts.
  - Very large MoE models.
- Accessibility and theming.
- Documentation updates:
  - docs/reference/chat.md or a new docs/reference/spawn-server.md.

══════════════════════════════════════════════
ADDITIONAL IDEAS (OPTIONAL)
══════════════════════════════════════════════

- Preset versioning:
  - Add a version field to presets.
  - When backend schema changes, allow migration.
- "Quick profiles":
  - Prebuilt profiles: "Max Quality", "Fast Chat", "Agentic/Tooling", "RP Large Context".
- System profile:
  - Auto-detect hardware and suggest a baseline config.
- Export:
  - Allow exporting a preset as a launch script for the current OS.

══════════════════════════════════════════════
END
══════════════════════════════════════════════
