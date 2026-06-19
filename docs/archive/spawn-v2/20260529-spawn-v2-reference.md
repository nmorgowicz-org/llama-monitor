# Spawn Llama-Server V2 — Agent Reference (Self-Contained)

Purpose:
- This doc is the single, self-contained reference for AI agents implementing the Spawn Llama-Server V2 feature.
- It summarizes architecture, APIs, UI flows, security, and external dependencies so that a fresh agent can work with minimal extra context.

For full details, see:
- Overview and phased plan: `docs/plans/20260529-spawn-v2-overview.md`
- Backend architecture and APIs: `docs/plans/20260529-spawn-v2-backend.md`
- Frontend architecture and wizard UI: `docs/plans/20260529-spawn-v2-frontend.md`
- Research and tuning reference: `docs/plans/20260529-spawn-v2-research.md`

---

## 1. Goals (High-Level)

We are building a premium, secure, multiplatform “Spawn Llama-Server” experience that:

- Guides users (first-time and advanced) through:
  - Selecting or importing a model (local, HF, third-party).
  - Configuring hardware, context, batching, MoE, speculative decoding.
  - Estimating VRAM and validating that the config is sane.
- Integrates with:
  - HuggingFace for model discovery and download.
  - llama.cpp official releases for binary download.
- Provides:
  - A step-by-step spawn wizard.
  - Advanced parameter editing (guided + raw).
  - Model-specific generation defaults and tuning guidance.
  - Health check / benchmark for performance validation.

Key principles:
- No breaking changes; fully backward compatible.
- Security-first: auth on all endpoints; path-safe; no arbitrary exec.
- Self-contained docs: future agents should rely on these docs, not chat history.

---

## 2. Existing System (What We Have Today)

- Spawning:
  - `POST /api/start`: start llama-server from config (api-token).
  - `POST /api/sessions/spawn`: spawn with preset via session (db-admin-token, 15s cooldown).
  - `ServerConfig` in `src/llama/server.rs` already supports many flags (GPU layers, batching, speculative decoding, MoE, rope scaling, etc.).
- Presets:
  - `ModelPreset` mirrors `ServerConfig`; stored in `presets.json`.
  - Editable via API (currently api-token only).
- Models:
  - `scan_models_dir` scans for `.gguf`; exposed via `/api/models`.
- GPU/VRAM:
  - NVIDIA/ROCm: accurate VRAM and metrics.
  - Apple: system memory used as proxy.
  - Windows WMI: limited (total VRAM only).
- Metrics:
  - Poll llama-server `/health` and `/metrics` for tuning feedback.
- Download:
  - `download_asset_locally` in `src/agent.rs` can be reused as a pattern.

What is missing (to be implemented):
- HuggingFace integration.
- Model downloading from HF.
- VRAM estimation.
- Batch-file import.
- Advanced parameter editor (raw + guided).
- Chat-template import via URL.
- Benchmark/health-check tool.
- MoE-specific tuning UI.
- Automated llama.cpp binary download.
- Standardized binaries/models/scripts directory structure.
- Third-party model import (Ollama, LM Studio, etc.).
- Tuning guidance based on llama-server metrics.
- Model-specific generation defaults.
- Model introspection.
- `-hf` support and multimodal (`--mmproj`) support in spawn flow.
- Explicit safety/limits flags (max tokens, etc.) exposed in wizard.

---

## 3. Architecture (Key Decisions)

- A1: Dedicated Spawn Wizard module:
  - Backend: `src/llama/spawn_wizard.rs` (batch import, normalization, VRAM estimation, MoE tuning, benchmark coordination).
  - Frontend: `static/js/features/spawn-wizard.js`, `static/css/spawn-wizard.css`, spawn wizard modal in `index.html`.

- A2: Extend ServerConfig and ModelPreset:
  - New fields (both structs):
    - hf_repo, chat_template_file, mmproj, grammar, json_schema,
      cache_type_k, cache_type_v, max_tokens, api_key.
  - Internal-only:
    - benchmark_mode (ServerConfig).
  - All with `#[serde(default)]`.
  - Wire into `start_server()` with corresponding llama-server flags.
  - Use `-hf` when hf_repo is set; do not combine with `-m`.

- A3: HuggingFace integration via hf-hub crate:
  - Use `HFClient` (async) for:
    - Model metadata (gated, tags, GGUF metadata).
    - Listing repo files (filter `.gguf`).
    - File metadata (size, ETag, Xet hash).
    - Download via `download_file_stream()` for large models; implement resume via `.range(current_size..)`.
  - Auth:
    - Read `HUGGING_FACE_HUB_TOKEN`; allow UI input; store in `~/.config/llama-monitor/hf-token`.
  - Gated:
    - Detect gating; show guidance; do not auto-request access.
  - Rate limits:
    - Respect 429 and headers; backoff; cache where possible.

- A4: Model download:
  - Incremental, resumable (via byte ranges).
  - Confined to `models_dir` or a designated subdirectory.
  - Integrity check (SHA256 when available).

- A5: VRAM estimation:
  - Heuristic-based, transparent.
  - Inputs: model size, context size, KV quant, batch sizes, speculative decoding, mmproj, MoE tuning, available VRAM.
  - Outputs: estimated VRAM/RAM, recommendation (Fit/Tight/Risk/Won’t fit).

- A6: Batch-file import:
  - Cross-platform; robust parsing.
  - Returns a `ModelPreset`.

- A7: Parameter editor:
  - Dual-mode (guided + raw).
  - Guided sections: model/paths, GPU/memory, context/batching, generation, speculative decoding, MoE, advanced/custom args.

- A8: Chat template import:
  - File upload, HF URL, GitHub/Gist URL.
  - Stored in allowed root; passed as `--chat-template-file`.

- A9: Benchmark/health check:
  - Short, bounded benchmark via `/api/benchmark`.
  - Measures TTFT, prompt/ generation TPS; provides verdict and hints.

- A10: MoE tuning assistance:
  - Expose `--n-cpu-moe` and related options.
  - Integrate with VRAM estimator.

- A11: Automated llama.cpp binary download:
  - From official releases only.
  - Platform-aware asset selection.
  - Standardized binaries directory.

- A12: Release asset patterns:
  - See `docs/plans/20260529-spawn-v2-research.md` (Release Asset Patterns).

- A13: Standardized directory structure:
  - Use config root with:
    - `binaries/` for llama.cpp binaries.
    - `models/` for models (or user-configured).
    - `scripts/` for launch presets/scripts.
    - `certs/` for TLS/ACME/mTLS material.

- A14: Flexible integration:
  - Allow external drives, network shares, existing tool directories.
  - No hard constraints; provide guidance.

- A15: Preset reuse across models:
  - Model-agnostic presets; `model_path` filled at launch time.

- A16: Model-specific generation defaults:
  - Local JSON config; keyed by model family and use case profile.
  - Based on Unsloth recommendations and community best practices.

---

## 4. APIs (Summary)

All new endpoints must be audited against AGENTS.md security rules.
- Read user data → api-token.
- Destructive/elevated → db-admin-token.

Key endpoints:

- POST /api/import-launch-file
  - Auth: api-token.
  - Parse launch file into a preset.

- POST /api/chat-template/fetch
  - Auth: api-token.
  - Fetch chat template from URL (HF/GitHub/Gist).

- POST /api/chat-template/upload
  - Auth: api-token.
  - Upload a .jinja template file.

- POST /api/models/download
  - Auth: db-admin-token.
  - Start model download from HF.

- GET /api/models/download/:id/status
  - Auth: api-token.

- POST /api/models/download/:id/cancel
  - Auth: api-token.

- POST /api/estimate-vram
  - Auth: api-token.
  - Estimate VRAM usage for a configuration.

- POST /api/benchmark
  - Auth: api-token.
  - Run a short benchmark on a running llama-server.

- GET /api/llama-cpp/releases
  - Auth: api-token.
  - List recent llama.cpp releases.

- POST /api/llama-cpp/download
  - Auth: db-admin-token.
  - Start llama.cpp binary download.

- GET /api/llama-cpp/download/:id/status
  - Auth: api-token.

- POST /api/llama-cpp/download/:id/cancel
  - Auth: api-token.

- POST /api/model/introspect
  - Auth: api-token.
  - Introspect a model (via llama.cpp binary or --print-model-metadata).

Extend:
- POST /api/start and POST /api/sessions/spawn to accept new fields (chat_template_file, n_cpu_moe, hf_repo, mmproj, etc.).
- POST /api/sessions/spawn already uses db-admin-token; all new spawn-v2 endpoints must do the same.

---

## 5. Frontend / Wizard UX (Summary)

- SpawnWizard:
  - 5 steps:
    1) Profile: Quick / Balanced / Advanced.
    2) Model: local, HF, third-party.
    3) Hardware: GPU layers, context, batch sizes, VRAM feedback, MoE tuning.
    4) Summary: review, save as preset, run health check.
    5) Spawn: spawn server, progress bar, status.
  - Integrates with:
    - File browser for model selection.
    - Preset selection.
    - Session modal and attach-detach flows.
  - Uses existing API endpoints and utilities (authHeaders, toast, showConnectingState).

- Error handling:
  - Classify: FATAL / CRITICAL / WARNING / INFO.
  - Use toasts, inline alerts, modal errors.
  - Provide clear, safe guidance (no silent failures).

---

## 6. Security (Key Rules)

- Tokens:
  - Use constant-time comparison for all token checks.
  - No == on tokens.

- Auth levels:
  - api-token:
    - All new endpoints that read user data.
  - db-admin-token:
    - Any endpoint that:
      - Spawns or restarts llama-server (including new spawn-v2 endpoints).
      - Changes llama_server_path or llama_server_cwd.
      - Downloads or replaces llama-server binaries.
      - Downloads models into models_dir.
      - Modifies presets in ways that affect command-line arguments or paths.
      - Performs destructive or high-impact operations.

- Command and path safety:
  - Never use a shell to execute llama-server.
  - extra_args: split by whitespace; treat as untrusted.
  - Paths:
    - Reject "..", leading "/", leading "\\" when expecting filenames.
    - Confine model downloads and binary downloads to allowed roots.

- Secrets:
  - No full tokens, passwords, or keys in logs.
  - Use existing encryption helpers for sensitive config.

---

## 7. External Dependencies

- llama.cpp:
  - Use latest stable/beta builds from ggerganov/llama.cpp.
  - Align with current flags and best practices:
    - -hf, --mmproj, --chat-template-file, -ctk/-ctv, -n, --api-key, --split-mode, -ts, -mg, --n-cpu-moe, speculative decoding, rope scaling, etc.
  - See `docs/plans/20260529-spawn-v2-research.md` for detailed flag usage and wizard mapping.

- HuggingFace:
  - Use hf-hub crate for:
    - Model metadata (gated, tags, GGUF metadata).
    - Listing repo files.
    - Download with streaming and resume via byte ranges.
  - See `docs/plans/20260529-spawn-v2-research.md` for concrete API usage.

---

## 8. How to Use These Docs

- To understand the overall feature and phased plan:
  - Read `docs/plans/20260529-spawn-v2-overview.md`.
- To implement backend changes:
  - Read `docs/plans/20260529-spawn-v2-backend.md`.
- To implement frontend changes:
  - Read `docs/plans/20260529-spawn-v2-frontend.md`.
  - You MUST follow the “UI/UX Design Guidelines (Critical)” section in that doc.
- To consult llama.cpp tuning, third-party compatibility, or research:
  - Read `docs/plans/20260529-spawn-v2-research.md`.
- For a quick, self-contained reference:
  - Use this doc.

When ready to implement:
- Parse the plan.
- Create the branch.
- Implement the first phase step by step.
- Run all required checks (fmt, clippy, tests, lint, etc.).
- Commit with conventional commit format.
