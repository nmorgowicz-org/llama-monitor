# Spawn Llama-Server V2

Spawn Llama-Server V2 is a guided wizard for configuring and launching llama-server with proper hardware tuning, VRAM estimation, and model metadata introspection.

## Features

- Guided profile selection (Quick, Balanced, Advanced).
- Model source selection:
  - Local GGUF file.
  - HuggingFace Hub search and download.
  - Third-party tool imports (Ollama, LM Studio, etc.).
- Hardware tuning with VRAM estimation.
- Summary and health check before launch.
- Rate limiting and security hardening.

## HuggingFace Integration

### Search Models

- Endpoint: POST /api/hf/search
- Body: { "query": "llama", "limit": 20 }
- Requires: api-token
- Rate limit: 10 requests per 60 seconds (per instance).
- Response: { "ok": true, "models": [...] }

### List GGUF Files

- Endpoint: POST /api/hf/files
- Body: { "repo_id": "org/model" }
- Requires: api-token
- Response: { "ok": true, "files": [...] }

### Start Download

- Endpoint: POST /api/hf/download
- Body: { "repo_id": "org/model", "file_path": "model.gguf" }
- Requires: api-token
- Rate limit: 10-second cooldown between starts.
- Path traversal checks: rejects "..", leading "/", leading "\".
- Response: { "ok": true, "download_id": "..." }

## Third-Party Model Import

- Endpoint: POST /api/third-party-models
- Body: { "include_subdirs": true }
- Requires: api-token
- Scans common directories:
  - macOS: Ollama, LM Studio
  - Linux: ~/.ollama, ~/.local/share/lm-studio
  - Windows: LOCALAPPDATA/Ollama, etc.
- Response: { "ok": true, "models": [...] }

## Model Introspection

- Endpoint: POST /api/model/introspect
- Body: { "model_path": "/path/to/model.gguf" }
- Requires: api-token
- Runs: llama-server --print-model-metadata
- Caches results in ~/.config/llama-monitor/model-cache/<sha256>.json
- Response: { "ok": true, "metadata": {...}, "cached": false }

## Security

- All endpoints require api-token.
- Rate limiting on HF search and HF download.
- Path traversal guards on file_path and target_path.
- No full HF token in logs or error messages.

## UI / Accessibility

- Reduced-motion support: animations disabled when prefers-reduced-motion is set.
- Keyboard navigation: Tab, Enter, Escape supported.
- No innerHTML with untrusted data; uses textContent.
