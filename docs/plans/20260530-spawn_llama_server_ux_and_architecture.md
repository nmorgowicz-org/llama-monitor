# Spawn Llama-Server V2 — UX, Architecture, and Implementation Reference

- **Branch:** `feature/spawn-llama-server-v2`
- **Date:** 2026-05-30
- **Author:** Iris (via Hermes)

This is the single, self-contained reference for the Spawn Llama-Server V2 feature.
It includes:
- UX mockups (ASCII)
- Flows
- Error handling
- Security and performance guidelines
- VRAM estimation
- MoE tuning
- Benchmark methodology

Use this as the primary doc for future implementation agents.

---

## 1. WELCOME SCREEN

- Left card:
  - Attach to existing endpoint.
  - Recent endpoints (2-3).
  - Manual attach.
- Right card:
  - Spawn new llama-server (from preset or scratch).
  - "Get llama-server" if no binary detected.

First-time user:
- No recent endpoints.
- "Open Spawn Wizard" as primary CTA.

Returning user:
- Fast path: select preset + "Start Server".
- Secondary: "Open Spawn Wizard" for new config.

---

## 2. SPAWN WIZARD (STEPS 1-4)

- Step 1: Connection / Mode.
- Step 2: Model & Templates.
- Step 3: Resources.
- Step 4: Advanced.

Each step uses premium glassmorphism, clear CTAs, inline help.

---

## 3. PARAMETER EDITOR

- Dual-mode: guided + raw.
- Guided:
  - Sections: Model & paths, GPU & memory, Context & batching, Generation, Speculative decoding, MoE tuning, Advanced.
  - Each field: label, input, short description, tooltip.
- Raw:
  - OS-specific script.
  - Editable.
- Sync:
  - Guided -> raw: immediate.
  - Raw -> guided: on-demand ("Sync to Guided").
  - Conflict: last-write-wins with warning.

---

## 4. VRAM ESTIMATION

- Formulas:
  - weights_memory = model_size_bytes.
  - kv_cache_memory = f(context_size, kv_quant, parallel_slots).
  - speculative_overhead = f(draft_model_size, draft_max).
  - mmproj_memory = mmproj_size_bytes.
  - MoE expert memory = f(n_cpu_moe, moe_experts_total).
- Verdict:
  - Fit (>= 1.2x available).
  - Tight (1.0-1.2x).
  - Risk (0.85-1.0x).
  - Won't fit (< 0.85x).

Examples:
- Qwen3.6-27B at 212K: ~121.6 GiB (won't fit on single 48 GB GPU).
- DeepSeek V3 (MoE) with --cpu-moe: ~15.7 GiB VRAM, 27.8 GiB RAM.
- Gemma 4 with speculative decoding: ~11.3 GiB.

---

## 5. MODEL SELECTION

- Local:
  - Directory picker.
  - GGUF list with size, quant, last-modified.
  - "Use this model" button.
- HF:
  - Repo input.
  - Variant list (GGUF files with size/quant).
  - Download progress.
- Import:
  - Ollama, LM Studio, KoboldCPP, vLLM, Unsloth.
  - Auto-detect + scan.

---

## 6. LLAMA.CPP BINARY DOWNLOAD

- Backend selection:
  - CPU, CUDA 12, CUDA 13, Vulkan, ROCm.
  - Platform detection + recommendation.
- Download progress:
  - Release tag.
  - Asset name.
  - Progress bar.
  - ETA.
  - Cancel button.

Flows:
- First-time user (no llama-server).
- Returning user (updating binary).
- Error cases (network failure, disk full).

---

## 7. CHAT TEMPLATE IMPORT

- File upload.
- URL input (HF/GitHub/Gist).
- Paste area.
- Preview.
- "Use this template" button.

Flows:
- Uploading a .jinja file.
- Fetching from HF/GitHub.
- Error cases (invalid template, auth failure).

---

## 8. BENCHMARK / HEALTH CHECK

- Methodology:
  - Short prompt.
  - Measure TTFT, gen TPS, prompt TPS.
  - Interpret results.
- UI:
  - "Run Health Check" button.
  - Results panel.
  - Tuning suggestions.

Flows:
- Running a benchmark.
- Interpreting results.
- Error cases (slow model, network failure).

---

## 9. MoE TUNING

- Slider + input for n-cpu-moe.
- --cpu-moe toggle.
- Live VRAM vs latency guidance.
- Multi-GPU options.

Flows:
- Tuning MoE settings.
- Error cases (OOM, slow performance).

---

## 10. ERROR HANDLING

- Classification:
  - FATAL, CRITICAL, WARNING, INFO.
- UI patterns:
  - Toasts.
  - Inline alerts.
  - Modal errors.

Specifics:
- Network failures.
- Auth failures.
- Model download failures.
- Binary download failures.
- VRAM estimation failures.
- Disk full.
- GPU already in use.
- Corrupted models.

---

## 11. SECURITY AND PERFORMANCE

Security:
- Token handling (HF, API, etc.).
- Network requests.
- File operations.
- Secrets management.

Performance:
- Large models.
- Slow networks.
- Low-end hardware.
- Optimization strategies.

---

END
