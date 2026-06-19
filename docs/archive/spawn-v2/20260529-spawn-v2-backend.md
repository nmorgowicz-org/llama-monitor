# Spawn Llama-Server V2 — Backend Architecture, APIs, and Implementation

- **Branch:** `feature/spawn-llama-server-v2`
- **Date:** 2026-05-29
- **Author:** Iris (via Hermes)

This is the backend implementation reference for the Spawn Llama-Server V2 feature.
It is written so that a fresh AI agent can read it and execute the implementation without guessing.

For the high-level overview and phased plan, see:
- `docs/plans/20260529-spawn-v2-overview.md`

For frontend architecture and wizard UI, see:
- `docs/plans/20260529-spawn-v2-frontend.md`

For llama.cpp tuning, third-party compatibility, and research, see:
- `docs/plans/20260529-spawn-v2-research.md`

---

## Current State (Summary)

Key existing capabilities:

- **Spawning llama-server:**
  - `src/llama/server.rs`:
    - `ServerConfig` struct: comprehensive parameter set (model_path, context_size, gpu_layers, batch sizes, speculative decoding, MoE, rope scaling, etc.).
    - `start_server()`: builds command from `ServerConfig`, sets environment (NVIDIA/ROCm), spawns child, streams logs.
    - Always sets: `--host 0.0.0.0`, `--jinja`, `--metrics`, `--no-warmup`, `--webui-mcp-proxy`.
    - Auto-applies YaRN rope scaling when context is very large.
  - `src/web/api.rs`:
    - `POST /api/start`: direct start from config; requires api-token.
    - `POST /api/sessions/spawn`: spawn with preset via session; requires db-admin-token; rate-limited (15s cooldown).
- **Presets:**
  - `src/presets/mod.rs`:
    - `ModelPreset` struct mirrors `ServerConfig` almost 1:1.
    - `load_presets`/`save_presets` from `presets.json`; `default_presets` for initial config.
    - Editable via API (currently api-token only).
- **Model discovery:**
  - `src/models/mod.rs`:
    - `scan_models_dir`: scans for `.gguf` in configured models_dir.
  - `/api/models`, `/api/models/refresh`.
- **File browser:**
  - `/api/browse?path=...&filter=gguf`
  - `static/js/features/file-browser.js`: modal file browser for model/executable selection.
- **GPU/VRAM monitoring:**
  - `src/gpu/*`:
    - NVIDIA: `nvidia-smi` (temp, load, power, VRAM used/total, clocks).
    - ROCm: `rocm-smi` (similar).
    - Apple: `mactop` (GPU temp/util/power; system memory for VRAM).
    - Windows WMI: only GPU name + total VRAM; no runtime metrics.
  - Exposed via `/metrics/gpu`, `/metrics`, and WebSocket.
- **Metrics:**
  - `src/llama/metrics.rs`, `src/llama/poller.rs`:
    - Poll llama-server `/health` and `/metrics` for KV cache usage, tokens, TPS, etc.
    - Useful for tuning guidance and VRAM estimation feedback.
- **Download infrastructure:**
  - `src/agent.rs`:
    - `download_asset_locally` and related functions for app/agent updates.
    - Can be extended or used as a pattern for model/binary downloads.

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
- No model introspection (see A17 below).
- No `-hf` (HF repo-based model load) or multimodal (`--mmproj`) support in spawn flow.
- No explicit safety/limits flags (max tokens, etc.) exposed in wizard.

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
  - Details: `docs/plans/20260529-spawn-v2-frontend.md` (A1).

### A2. Extend ServerConfig and ModelPreset

- **Rationale:**
  - New capabilities (HF-based model loading, chat-template-file, MoE tuning, multimodal, KV cache quantization, speculative decoding, safety/limits) must be first-class and wizard-integrated.
- **Current state (for reference):**
  - `ServerConfig` already includes:
    - model_path, context_size, gpu_layers, batch_size, ubatch_size, no_mmap, port, ngram_spec, parallel_slots
    - temperature, top_p, top_k, min_p, repeat_penalty
    - n_cpu_moe, mlock, flash_attn, split_mode, main_gpu
    - threads, threads_batch
    - rope_scaling, rope_freq_base, rope_freq_scale
    - draft_model, draft_min, draft_max, spec_ngram_size
    - seed, system_prompt_file, extra_args
  - `ModelPreset` mirrors this almost 1:1.
- **Required new fields (must be added to both ServerConfig and ModelPreset):**
  - hf_repo: Option<String>
    - For -hf user/repo[:quant]; alternative to model_path for HF-based loading.
  - chat_template_file: Option<String>
    - For --chat-template-file (custom chat template).
  - mmproj: Option<String>
    - For --mmproj (multimodal projector).
  - grammar: Option<String>
    - For grammar-based structured output.
  - json_schema: Option<String>
    - For JSON mode / schema-based output.
  - cache_type_k: Option<String>
    - For -ctk / --cache-type-k (KV cache quantization K).
  - cache_type_v: Option<String>
    - For -ctv / --cache-type-v (KV cache quantization V).
  - max_tokens: Option<u64>
    - For -n / --n-predict; safety/limits.
  - api_key: Option<String>
    - For --api-key; protects llama-server when exposed.
- **Internal-only fields (ServerConfig only; not exposed in presets):**
  - benchmark_mode: bool
    - Internal flag to run a controlled benchmark via /api/benchmark.
- **Implementation rules:**
  - Ensure `#[serde(default)]` on all new fields.
  - All new fields must be wired into `start_server()` with corresponding llama-server flags.
  - For hf_repo:
    - If set and model_path is not explicitly provided, construct -hf flag instead of -m.
    - Do not allow both -m and -hf simultaneously; treat as error.

### A3. HuggingFace integration via hf-hub crate

- **Rationale:**
  - Official, maintained, feature-complete.
  - Handles auth, rate limits, gated models, and repo operations.
- **Design:**
  - Use `hf-hub` crate (by Hugging Face):
    - Prefer `HFClient` (async) for all new code.
    - `HFClientSync` only if needed in non-async contexts.
  - Core operations:
    - `hf_get_model_info(repo_id)`:
      - Use `HFClient::model(repo_id).info()` to get `ModelInfo`.
      - Extract:
        - `gated` (None/false = open; "auto"/"manual" = gated).
        - `tags` (to detect GGUF, MoE, etc.).
        - `gguf` metadata (if present).
    - `hf_list_repo_files(repo_id)`:
      - Use `repo.list_tree().recursive(true).expand(true)` to get `RepoTreeEntry`.
      - Filter for `.gguf` files.
      - Use file size + name to present options in wizard.
    - `hf_get_file_info(repo_id, path)`:
      - Use `repo.get_file_metadata().filepath(path)` for size, ETag, Xet hash.
    - Download:
      - For large GGUF files, use `download_file_stream()` with:
        - Custom file writer.
        - Progress reporting.
        - Manual resume via `.range(current_size..)` on restart.
      - Do not rely solely on `download_file()` for multi-GB models (no built-in resume across restarts).
  - Auth:
    - Read `HUGGING_FACE_HUB_TOKEN` from env.
    - Allow user to input their own token via UI.
    - Store in config file (e.g., `~/.config/llama-monitor/hf-token`).
    - Use a read-only or fine-grained token for security.
    - Never log the full token.
  - Gated models:
    - Detect gating from model metadata (`ModelInfo.gated`).
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
  - Details: `docs/plans/20260529-spawn-v2-research.md` (HuggingFace Integration).

### A4. Model download: incremental, resumable, safe

- **Rationale:**
  - Large models; users expect progress, resumability, and safety.
- **Design:**
  - Extend `src/agent.rs` or new `src/model_download.rs`:
    - `download_model_file(url, dest, token)`
    - Supports:
      - Range requests.
      - Partial file (resume).
      - Integrity check (file size comparison + SHA256 if available).
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
  - Security:
    - Token must be masked in logs and error messages.
    - Download directory must be within an allowed root (e.g., `models_dir`).
    - No path traversal via `target_dir`.

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
  - Details: `docs/plans/20260529-spawn-v2-research.md` (VRAM Estimation).

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
  - Security:
    - No path traversal: model path must be within an allowed root.
    - No execution of the imported script; only parse and normalize.

### A7. Parameter editor: dual-mode, consistent

- **Rationale:**
  - Power users want raw control.
  - Others need guidance.
- **Design:**
  - Guided mode:
    - Sections:
      - Model & paths:
        - model_path or hf_repo
        - mmproj (for multimodal)
        - chat_template_file
      - GPU & memory:
        - gpu_layers (auto / all / manual)
        - split_mode, tensor_split, main_gpu (if multi-GPU)
        - mlock, no_mmap
      - Context & batching:
        - context_size
        - batch_size, ubatch_size
        - parallel_slots
        - cache_type_k, cache_type_v
      - Generation parameters:
        - temperature, top_p, top_k, min_p, repeat_penalty
        - max_tokens (safety/limits)
      - Speculative decoding:
        - speculative_mode (none / ngram-mod / draft-model)
        - draft_model
      - MoE tuning:
        - n_cpu_moe / cpu_moe toggle
      - Advanced / custom args:
        - extra_args (plain text)
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
  - Details: `docs/plans/20260529-spawn-v2-frontend.md` (A7).

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
  - Security:
    - No path traversal: template must be stored in an allowed root.
    - No execution of the template; only parse and normalize.

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
  - Details: `docs/plans/20260529-spawn-v2-research.md` (MoE Tuning).

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
  - Security:
    - Only trust releases from official repo (`ggerganov/llama.cpp`).
    - Verify asset signatures if available.
    - Never auto-execute without user confirmation.
  - Details: `docs/plans/20260529-spawn-v2-research.md` (Release Asset Patterns).

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
  - Details: `docs/plans/20260529-spawn-v2-research.md` (Model-Specific Defaults).

### A17. Model introspection (new)

- **Rationale:**
  - The plan assumes we know model properties (MoE? mmproj? layers? recommended context? quantization?), but we currently have no model introspection.
  - We must add this to make the wizard "premium" instead of guessing.
- **Design:**
  - New module or function in `src/llama/spawn_wizard.rs`:
    - `introspect_model(model_path: &Path) -> ModelMetadata`
  - Behavior:
    - Run llama-server with `--print-model-metadata` (or equivalent) on the selected GGUF.
    - Extract:
      - n_layers, n_ctx_train, n_embd, n_ff, n_exp (MoE), required mmproj, recommended context.
    - Cache result in:
      - `~/.config/llama-monitor/model-cache/<sha256>.json`
  - API:
    - `POST /api/model/introspect`:
      - Body: `{ model_path: string }`
      - Response: `{ metadata: ModelMetadata, cached: bool }`
  - Security:
    - No path traversal: model path must be within an allowed root.
    - No execution of the model; only parse metadata.
  - Details: `docs/plans/20260529-spawn-v2-research.md` (Model Introspection).

---

## API Design (New / Changed Endpoints)

All new endpoints:

- Auth:
  - Use existing auth_guard.
  - Bearer token: Authorization: Bearer <token>.
  - Session cookie: llama_monitor_session (Secure, HttpOnly, SameSite=Lax).
  - Public endpoints: /api/health (and any explicitly marked).
  - All new endpoints must be audited against AGENTS.md security requirements.

Specific endpoints:

- `POST /api/import-launch-file`
  - Import and parse a launch file into a preset.
  - Auth: api-token.
  - Body: `{ content: string, os: "windows" | "macos" | "linux" }`
  - Response: `{ preset: ModelPreset, warnings: [string] }`

- `POST /api/chat-template/fetch`
  - Fetch chat template from URL (HF/GitHub/Gist).
  - Auth: api-token.
  - Body: `{ source_type, source }`
  - Response: `{ template: string, source_url: string }`

- `POST /api/chat-template/upload`
  - Upload a `.jinja` template file.
  - Auth: api-token.
  - Multipart with file.
  - Response: `{ template_id, template }`

- `POST /api/models/download`
  - Start a model download from HF.
  - Auth: api-token.
  - Body: `{ repo_id, file_path, target_dir }`
  - Response: `{ download_id }`

- `GET /api/models/download/:id/status`
  - Get download progress.
  - Auth: api-token.
  - Response: `{ progress, bytes_downloaded, total, speed, eta, status }`

- `POST /api/models/download/:id/cancel`
  - Cancel a download.
  - Auth: api-token.

- `POST /api/estimate-vram`
  - Estimate VRAM usage for a configuration.
  - Auth: api-token.
  - Body: estimation input.
  - Response: structured estimate + human-readable note.

- `POST /api/benchmark`
  - Run a short benchmark on a running llama-server.
  - Auth: api-token.
  - Body: `{ prompt: string, max_tokens: usize, temperature: f32 }`
  - Response: `{ ttft_ms, gen_tps, prompt_tps, total_latency_ms, verdict: "Good" | "Moderate" | "Poor", hints: [string] }`

- `GET /api/llama-cpp/releases`
  - List recent llama.cpp releases.
  - Auth: api-token.
  - Response: `{ releases: [LlamaCppRelease] }`

- `POST /api/llama-cpp/download`
  - Start a llama.cpp binary download.
  - Auth: api-token.
  - Body: `{ release_tag, backend, arch }`
  - Response: `{ download_id }`

- `GET /api/llama-cpp/download/:id/status`
  - Get download progress.
  - Auth: api-token.
  - Response: `{ progress, status, message }`

- `POST /api/llama-cpp/download/:id/cancel`
  - Cancel a download.
  - Auth: api-token.

- `POST /api/model/introspect`
  - Introspect a model (via llama.cpp binary or `--print-model-metadata`).
  - Auth: api-token.
  - Body: `{ model_path: string }`
  - Response: `{ metadata: ModelMetadata, cached: bool }`

- Extend:
  - `POST /api/start` and `POST /api/sessions/spawn`:
    - Accept new fields (`chat_template_file`, `n_cpu_moe`, etc.).

---

## Data Models (Key Changes)

- `ServerConfig` (`src/llama/server.rs`):
  - Add:
    - `chat_template_file: Option<String>`
    - `benchmark_mode: bool`
    - `mmproj: Option<String>`
    - `grammar: Option<String>`
    - `json_schema: Option<String>`
    - `cont_batching: bool`
    - `cache_type_k: Option<String>`
    - `cache_type_v: Option<String>`

- `ModelPreset` (`src/presets/mod.rs`):
  - Add:
    - `chat_template_file: Option<String>`
    - `benchmark_mode: bool`
    - `mmproj: Option<String>`
    - `grammar: Option<String>`
    - `json_schema: Option<String>`
    - `cont_batching: bool`
    - `cache_type_k: Option<String>`
    - `cache_type_v: Option<String>`

- New structs:
  - `VramEstimateRequest` / `VramEstimateResponse`
  - `ModelDownloadRequest` / `ModelDownloadStatus`
  - `BenchmarkRequest` / `BenchmarkResult`
  - `ImportLaunchRequest` / `ImportLaunchResponse`
  - `LlamaCppRelease`, `LlamaCppAsset`
  - `LlamaCppDownloadRequest` / `LlamaCppDownloadStatus`
  - `ModelMetadata` (for introspection)

All with `#[serde(default)]` and safe defaults.

---

## Security Requirements

These are specific to this feature and must be followed. They extend the AGENTS.md security rules.

- **Token handling (HF and API tokens):**
  - HF token:
    - Stored in a config file (e.g., `~/.config/llama-monitor/hf-token`).
    - Masked in logs and error messages; never logged in full.
    - Use a read-only or fine-grained token for security.
  - API tokens:
    - All new endpoints must enforce api-token or db-admin-token via constant-time comparison (use existing helpers).
    - No new == on tokens.

- **Auth levels (concrete rules):**
  - api-token:
    - All new endpoints that read user data (settings, presets, templates, chat, config, models list, browse).
  - db-admin-token:
    - Any endpoint that:
      - Spawns or restarts llama-server (including new spawn-v2 endpoints).
      - Changes llama_server_path or llama_server_cwd.
      - Downloads or replaces llama-server binaries.
      - Downloads models into models_dir.
      - Modifies presets in ways that affect command-line arguments or paths.
      - Performs destructive or high-impact operations (delete backups, restore DB, etc.).
  - Existing behavior:
    - `POST /api/sessions/spawn` already requires db-admin-token; all new spawn-v2 endpoints must do the same.
  - Never rely solely on the global auth_guard (which accepts api-token) for elevated operations.

- **Binary trust:**
  - Only trust releases from official repo (`ggerganov/llama.cpp`).
  - Verify asset signatures if available.
  - Never auto-execute without user confirmation.
  - Binaries must be stored in a standardized directory (e.g., `config_dir/binaries/`); never arbitrary paths.

- **Model download integrity:**
  - Large model downloads must verify SHA256 against HF metadata where available.
  - Partial/corrupted downloads must be detectable and resumable.
  - Target directory must be constrained to `models_dir` (or a designated subdirectory).

- **File browser path traversal:**
  - The browse endpoint must enforce allow-listed roots.
  - Third-party model import (Ollama, LM Studio) must not expose arbitrary filesystem paths.
  - Reject "..", leading "/", and leading "\\" when expecting filenames.

- **Command injection and extra_args:**
  - Never use a shell to execute llama-server.
  - extra_args:
    - Split by whitespace; do not interpret shell metacharacters.
    - Treat as untrusted; do not allow flags that change user, mount, or run arbitrary scripts.
  - If presets are editable via API:
    - Either:
      - Require db-admin-token, or
      - Strictly validate model paths and args to prevent arbitrary executables/commands.

- **Secrets:**
  - No full tokens, passwords, or keys in logs.
  - No plaintext secrets in error messages.
  - Use existing encryption helpers for sensitive config.

---

## Testing Strategy

### Rust Tests

- Unit tests for:
  - batch_import parsing (Windows vs Unix).
  - vram_estimator edge cases.
  - hf API wrappers.
  - llama_cpp_downloader asset selection logic.
  - model_introspect functionality.
  - new API endpoints (auth, error handling).

### Playwright E2E Tests

- Tests for:
  - Wizard steps.
  - Model selection.
  - Error states.
  - Spawn flow end-to-end.

Details: `docs/plans/20260529-spawn-v2-frontend.md` (Testing).

---

## Phased Implementation Plan (Backend)

### Phase 0: Foundations

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

### Phase 1: Premium Spawn Wizard UI

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

### Phase 2: Advanced Tuning and Safety

Deliverables:

- Advanced parameter editor (dual-mode).
- MoE tuning assistance.
- Health check / benchmark button.
- Model-specific generation defaults.
- Error handling and UX flows.
- Tests:
  - Extend Rust tests for new endpoints.
  - Add Playwright tests for wizard steps.

### Phase 3: HuggingFace and Third-Party Integration

Deliverables:

- HuggingFace model search and download.
- Third-party model import (Ollama, LM Studio, etc.).
- Model introspection (via llama.cpp binary or `--print-model-metadata`).
- Tests:
  - Extend Rust tests for HF and import logic.
  - Add Playwright tests for model selection and import.

### Phase 4: Polish and Hardening

Deliverables:

- Final polish of wizard UI.
- Security audit of all new endpoints and data flows.
- Performance optimization (large model downloads, streaming, etc.).
- Comprehensive tests (unit, integration, e2e).
- Documentation updates (AGENTS.md, README.md, docs/reference/).
