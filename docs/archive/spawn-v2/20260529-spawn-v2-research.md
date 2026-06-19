# Spawn Llama-Server V2 — Research and Tuning Reference

- **Branch:** `feature/spawn-llama-server-v2`
- **Date:** 2026-05-29
- **Author:** Iris (via Hermes)

This is the research and tuning reference for the Spawn Llama-Server V2 feature.
It is NOT for linear reading; it is for agents and developers to consult as needed.

For a concise, self-contained reference (optimized for fresh AI agents), see:
- `docs/plans/20260529-spawn-v2-reference.md`

For the high-level overview and phased plan, see:
- `docs/plans/20260529-spawn-v2-overview.md`

For backend architecture, APIs, and data models, see:
- `docs/plans/20260529-spawn-v2-backend.md`

For frontend architecture and wizard UI, see:
- `docs/plans/20260529-spawn-v2-frontend.md`

How to use this doc:
- Use this as:
  - A lookup for llama.cpp tuning decisions.
  - A reference for third-party compatibility (Ollama, LM Studio, etc.).
  - A source for VRAM estimation formulas and MoE tuning guidance.
- Do NOT:
  - Use this as the primary spec for implementation.
  - Implement features based solely on this doc without reading backend/frontend docs.

---

## HuggingFace Integration

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

## Third-Party Integration: Model Import from Other Tools

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

## llama-server Tuning and Metrics

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

## VRAM Estimation

### Formulas

- weights_memory = model_size_bytes.
- kv_cache_memory = 2 * num_layers * num_heads * head_dim * kv_bytes_per_elem * context_size * parallel_slots.
  - kv_bytes_per_elem:
    - F16: 2.0
    - F32: 4.0
    - Q8_0: 1.0
    - Q4_K: 0.5
    - Q3_K: 0.375
    - IQ4_XS: 0.5
    - IQ2_XS: 0.25
- speculative_overhead:
  - If draft model: draft_model_size_bytes + kv_per_token * draft_max * parallel_slots.
  - If ngram-mod: kv_per_token * draft_max * parallel_slots.
- mmproj_memory = mmproj_size_bytes * 1.02.
- MoE:
  - moe_gpu_memory = model_size_bytes * (moe_experts_total - n_cpu_moe) / moe_experts_total.
  - moe_cpu_memory = model_size_bytes * n_cpu_moe / moe_experts_total.
  - Use moe_gpu_memory in VRAM estimate.

### Verdict

- Fit (>= 1.2x available).
- Tight (1.0-1.2x).
- Risk (0.85-1.0x).
- Won't fit (< 0.85x).

### Examples

- Qwen3.6-27B at 212K: ~121.6 GiB (won't fit on single 48 GB GPU).
- DeepSeek V3 (MoE) with --cpu-moe: ~15.7 GiB VRAM, 27.8 GiB RAM.
- Gemma 4 with speculative decoding: ~11.3 GiB.

---

## Release Asset Patterns (Reference)

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

---

## Model-Specific Generation Defaults (Unsloth-Based)

Different model families have different optimal generation parameters.
Unsloth publishes well-tested, practical defaults.

### Example structure

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

## Model Introspection

### Approach

- Use llama.cpp binary (e.g., `llama-cli` or `llama-server`) with `--print-model-metadata` (or equivalent) on the selected GGUF.
- Extract:
  - n_layers, n_ctx_train, n_embd, n_ff, n_exp (MoE), required mmproj, recommended context.
- Cache result in:
  - `~/.config/llama-monitor/model-cache/<sha256>.json`

### Notes

- This is foundational for:
  - Accurate VRAM estimation.
  - MoE tuning.
  - Model-specific defaults.
  - Guided parameter recommendations.
