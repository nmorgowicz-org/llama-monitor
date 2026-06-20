# VRAM Estimator Reference

The VRAM estimator (`src/llama/vram_estimator/`) predicts GPU memory usage for a given model, quantization, context size, and hardware configuration. It powers the animated VRAM breakdown bar, the auto-size wizard, and the pre-download quant advisor.

![VRAM breakdown bar in the Hardware step](../screenshots/spawn-wizard-step3-vram.png)

---

## ModelArch

`ModelArch` is the central struct. Every estimation function takes a `&ModelArch`. It is populated either from GGUF metadata (ground truth) or from `ModelArch::from_name_and_params()` (heuristic, used pre-download).

```rust
pub struct ModelArch {
    // Standard attention
    pub n_layers: u32,          // Total transformer layers
    pub n_kv_heads: u32,        // KV heads (GQA/MQA)
    pub head_dim: u32,          // Per-head key/value dimension

    // Sliding-window / alternating attention (Gemma 3/4)
    pub n_global_attn_layers: u32,  // Layers that attend full context (0 = all)
    pub local_attn_window: u32,     // Sliding window size in tokens (0 = N/A)
    pub local_kv_heads: u32,        // KV heads for local layers
    pub global_head_dim: u32,       // head_dim override for global layers (Gemma 4 = 512)

    // MoE
    pub n_experts: u32,         // Total experts per layer (0 = dense)
    pub n_experts_used: u32,    // Experts activated per token
    pub expert_fraction: f64,   // Fraction of params in expert FFNs (default 0.65)

    // Hybrid linear attention (Qwen3.5 / Qwen3.6 / DeltaNet)
    pub n_attn_layers: u32,             // Layers with a KV cache (0 = all layers)
    pub linear_attn_state_bytes: u64,   // Fixed recurrent state size (context-independent)

    // MTP
    pub mtp_depth: u32,         // MTP prediction head count (0 = none)

    // Multimodal
    pub mmproj_bytes: u64,      // Vision projector size in bytes (0 = none)

    // Sizing / overhead
    pub param_b: f64,           // Approximate param count in billions
    pub n_embd: u32,            // Hidden/embedding dimension (for CUDA overhead estimate)
                                // 0 = unknown; set from GGUF embedding_length when available
}
```

### Helper predicates

| Method | True when |
|--------|-----------|
| `is_moe()` | `n_experts > 0` |
| `is_hybrid_attn()` | `n_attn_layers > 0` (some layers are DeltaNet/SSM) |
| `has_local_attn()` | `n_global_attn_layers > 0` (some layers are sliding-window) |

---

## Architecture Heuristics

`ModelArch::from_name_and_params(name, param_b)` returns a best-effort arch from the model filename and parameter count. This is used when the GGUF file has not yet been introspected. Ground-truth values from GGUF metadata override these when available.

### Priority order (first match wins)

1. `"coder-next"` / `"qwen3-coder-next"` → `qwen3_coder_next_arch()`
2. `"qwen3.5"` → `qwen35_heuristic(param_b)`
3. `"qwen3.6"` + `"35b-a3b"` → `qwen36_35b_a3b_arch()`
4. `"qwen3.6"` → `qwen36_heuristic(param_b)`
5. GGUF arch string `"qwen35"` or `"qwen3_6"` → `qwen36_heuristic(param_b)` (catches renamed finetunes)
6. `"gemma-4"` / `"gemma4"` → `gemma4_heuristic(name, param_b)` then MoE suffix
7. `"exaone-4.5"` / `"exaone45"` → `exaone45_heuristic(param_b)`
8. `"qwen3"` → `standard_heuristic(param_b)` with MoE suffix parsing
9. Any name with `"NB-AMB"` MoE suffix → generic heuristic + MoE suffix
10. Fallback → `standard_heuristic(param_b)`

### Per-family heuristics

#### Dense / Qwen3 standard (`standard_heuristic`)

| param_b | n_layers | n_kv_heads | head_dim |
|---------|----------|------------|----------|
| < 1 | 16 | 8 | 64 |
| 1–3 | 28 | 8 | 128 |
| 3–8 | 32 | 8 | 128 |
| 8–14 | 40 | 8 | 128 |
| 14–25 | 40 | 8 | 128 |
| 25–35 | 48 | 4 | 128 — tuned for Qwen3-30B-A3B |
| 35–75 | 64 | 8 | 128 |
| 75+ | 94 | 4 | 128 — tuned for Qwen3-235B |

#### Qwen3.6 (hybrid DeltaNet + dense)

3:1 DeltaNet:Attention ratio — exactly 1/4 of layers are standard softmax attention with a KV cache. The remaining 3/4 use a fixed-size recurrent state regardless of context length.

KV cache only grows for `n_attn_layers` — the DeltaNet layers contribute nothing to context scaling.

| param_b | n_layers | n_attn_layers | n_kv_heads | head_dim | n_embd | DeltaNet state |
|---------|----------|--------------|------------|----------|--------|----------------|
| ≤ 35 (27B) | 64 | 16 | 4 | 256 | 5120 | 48 × 48 × 128² × 2 B ≈ 720 MB |
| > 35 (davidau 40B) | 96 | 24 | 4 | 256 | 5120 | 48 × 72 × 128² × 2 B ≈ 1.1 GB |

**Calibrated on**: Qwen3.6-27B-NEO-CODE Q4_K_M GGUF (GGUF arch tag: `qwen35`). n_embd=5120 confirmed from `embedding_length` in GGUF metadata.

#### Qwen3.6-35B-A3B (exact — confirmed from model card)

40 total layers: 10 Attention + 30 DeltaNet. "A3B" = 3B **active parameters**, not 3 active experts.

| Field | Value |
|-------|-------|
| n_layers | 40 |
| n_attn_layers | 10 |
| n_kv_heads | 2 |
| head_dim | 256 |
| n_embd | 4096 (estimated; overridden by GGUF) |
| n_experts | 256 |
| n_experts_used | 9 (8 routed + 1 shared) |
| expert_fraction | 0.85 |
| DeltaNet state | 32 × 30 × 128² × 2 B ≈ 300 MB |

#### Qwen3.5 (hybrid DeltaNet + MoE)

Same 3:1 ratio. Only confirmed for 122B-A10B; heuristics applied to smaller sizes.

| param_b | n_layers | n_attn_layers | n_kv_heads | n_experts | n_experts_used | n_embd |
|---------|----------|--------------|------------|-----------|----------------|--------|
| ≤ 80 | 40 | 10 | 2 | 256 | 9 | 4096 |
| > 80 (122B) | 48 | 12 | 2 | 256 | 9 | 7168 |

DeltaNet V-heads: 64 for 122B (confirmed), 32 assumed for smaller.

#### Qwen3-Coder-Next (exact — confirmed)

48 layers: 12 standard attention + 36 DeltaNet.

| Field | Value |
|-------|-------|
| n_layers | 48 |
| n_attn_layers | 12 |
| n_kv_heads | 2 |
| head_dim | 256 |
| n_embd | 7168 (235B-class architecture) |
| n_experts | 512 |
| n_experts_used | 11 (10 routed + 1 shared) |
| expert_fraction | 0.92 |
| DeltaNet state | 36 × 32 × 128² × 2 B ≈ 1.2 GB |

#### Gemma 4 (sliding-window alternating attention)

5:1 local:global pattern — every 6th layer attends the full context; the rest use a sliding window.

| Tier | n_layers | global_layers | n_kv_heads (global) | local_kv_heads | n_experts | window | n_embd |
|------|----------|---------------|---------------------|----------------|-----------|--------|--------|
| E2B | 35 | 7 | 1 | 1 | 0 | 512 | 1152 |
| E4B | 42 | 7 | 2 | 2 | 0 | 512 | 2048 |
| 12B dense | 48 | 8 | 1 | 8 | 0 | 1024 | 3072 |
| 26B-A4B MoE | 30 | 5 | 2 | 8 | 128 | 1024 | 2048 |
| 31B dense | 60 | 10 | 4 | 16 | 0 | 1024 | 5120 |

- Global layers use `global_head_dim = 512`, local layers use `head_dim = 256`
- "A4B" = 4B active parameters, **not** 4 active experts (9 active: 8 routed + 1 shared)
- n_embd values for E2B/E4B/12B/26B-A4B are estimated; GGUF embedding_length overrides when present

#### EXAONE 4.5

| param_b | n_layers | global_layers | n_kv_heads | head_dim | window | mtp_depth | mmproj |
|---------|----------|---------------|------------|----------|--------|-----------|--------|
| ≤ 15 | 32 | 8 | 8 | 128 | 4096 | 0 | 0 |
| > 20 (33B) | 64 | 16 | 8 | 128 | 4096 | 1 | 2.58 GB |

### Generic MoE suffix parsing

For names not matched by the above, `parse_moe_suffix()` extracts patterns like `"35B-A3B"` or `"122B-A10B"`:
- Total experts inferred from sparsity ratio: < 5% → 512, < 100B → 128, < 50B → 64, < 20B → 32, else → 8
- `expert_fraction` left at default 0.65
- `n_experts_used` set to `active_b.round()` — this is **active parameters in billions**, not expert count; a known limitation that only affects the generation-speed display note

---

## Estimation Functions

### `kv_cache_bytes`

```
kv_cache_bytes(arch, context_size, parallel_slots, ctk, ctv) → u64
```

Standard (dense / hybrid, no local attention):
```
effective_layers = n_attn_layers if n_attn_layers > 0 else n_layers
K = effective_layers × n_kv_heads × head_dim × context × slots × k_bpe
V = effective_layers × n_kv_heads × head_dim × context × slots × v_bpe
total = K + V
```

For DeltaNet hybrid models (Qwen3.5/3.6): `effective_layers = n_attn_layers` (e.g. 16 out of 64 for 27B). KV grows at 1/4 the rate of a standard dense model with the same layer count.

Sliding-window (Gemma 3/4):
```
global_K = global_layers × n_kv_heads × global_head_dim × context × slots × k_bpe
global_V = (same with v_bpe)
effective_local_ctx = min(context, window) × slots
local_K  = local_layers  × local_kv_heads × head_dim × effective_local_ctx × k_bpe
local_V  = (same with v_bpe)
total = global_K + global_V + local_K + local_V
```

### `moe_weight_split`

```
moe_weight_split(model_size_bytes, arch, n_cpu_moe) → (vram_bytes, ram_bytes)
```

For `n_cpu_moe > 0` on a MoE model:
```
expert_frac = arch.expert_fraction.clamp(0.3, 0.99)
cpu_ratio   = min(n_cpu_moe, n_experts) / n_experts
cpu_bytes   = model_size_bytes × expert_frac × cpu_ratio
vram_bytes  = model_size_bytes − cpu_bytes
```

For dense models or `n_cpu_moe ≤ 0`: all weights in VRAM.

### `mtp_overhead_bytes`

```
mtp_overhead_bytes(model_size_bytes, mtp_depth) → u64
  = model_size_bytes × 0.015 × mtp_depth
```

1.5% of model weights per MTP depth level. Heuristic for DeepSeek-V3 / Qwen3-MTP style heads.

### `gpu_overhead_bytes`

```
gpu_overhead_bytes(ubatch_size) → u64
  = 300 MB + max(0, ubatch_size − 512) × 150 KB
```

Covers Metal/CUDA context, KV allocator metadata. **This alone is sufficient for Metal (Apple Silicon unified memory)** — Metal compute buffers are small and transient. For CUDA, also call `cuda_compute_buffer_bytes`.

### `cuda_compute_buffer_bytes`

```
cuda_compute_buffer_bytes(arch, ubatch_size) → u64
  = n_layers × ubatch_size × n_embd × 2 bytes × 4 passes
  = 0 if n_embd == 0 or n_layers == 0
```

CUDA/ROCm persistent compute buffer for Q/K/V projections, FFN intermediates, and normalization scratch tensors. These are allocated at model load and do not change with context length.

**Calibrated on RTX 5090 32 GB (WDDM), Qwen3.6-27B-NEO-CODE Q4_K_M, 212,992-token context, ubatch=1024, flash_attn=on:**

| Component | Measured |
|-----------|----------|
| Model weights (Q4_K_M, 16,511 MB) | 16.12 GiB |
| mmproj BF16 (888 MB) | 0.87 GiB |
| KV q8_0 @ 212k (16 attn layers × 4 heads × 256 dim) | 6.50 GiB |
| WDDM desktop apps (DWM, Discord, NVIDIA App, etc.) | ~1.50 GiB |
| **CUDA compute buffers (residual)** | **~2.45 GiB** |
| **Total nvidia-smi** | **27.44 GiB (28,101 MiB)** |

Formula check: 65 × 1024 × 5120 × 2 × 4 = 2.54 GiB → matches within 4%.

Note: WDDM display apps are **already accounted for** in `available_vram_bytes` (the caller subtracts current VRAM used before calling the estimator). The compute buffer formula only covers llama.cpp's persistent scratch allocations.

Returns 0 for `is_unified_memory = true` (Metal) or when `arch.n_embd == 0` (unknown model). When `n_embd = 0`, the estimate falls back to `gpu_overhead_bytes` alone — still accurate for Metal, but will underestimate by ~2–3 GiB for CUDA at large context.

### `full_estimate`

Sums all components:

```
weight_vram, ram = moe_weight_split(model_size, arch, n_cpu_moe)
kv              = kv_cache_bytes(arch, context, slots, ctk, ctv)
linear_state    = arch.linear_attn_state_bytes    (fixed; not context-dependent)
mmproj          = arch.mmproj_bytes               (set from mmproj_path stat in API)
mtp             = mtp_overhead_bytes(model_size, arch.mtp_depth)
cuda_buf        = 0 if is_unified else cuda_compute_buffer_bytes(arch, ubatch)
overhead        = gpu_overhead_bytes(ubatch) + cuda_buf
total           = weight_vram + kv + linear_state + mmproj + mtp + overhead
```

Recommendation thresholds:

| Result | Condition |
|--------|-----------|
| `Fit` | total ≤ 82% of available VRAM |
| `Tight` | total ≤ 100% of available VRAM |
| `Risk` | total ≤ 120% of available VRAM |
| `WontFit` | total > 120% of available VRAM |

On unified-memory Macs: the preset editor and spawn wizard show an mlock warning when result is Tight or Risk. mlock pins model memory so macOS cannot reclaim it; with an already tight estimate, this can push the OS into memory compression or swap.

### `max_context`

Binary search (sliding-window models) or direct formula (standard):

```
max_context(model_size, arch, ctk, ctv, parallel_slots, ubatch,
            n_cpu_moe, available_vram, fit_granularity,
            headroom_fraction, n_ctx_train, is_unified_memory) → u64

usable   = available_vram × (1 − headroom_fraction)
cuda_buf = 0 if is_unified else cuda_compute_buffer_bytes(arch, ubatch)
overhead = gpu_overhead_bytes(ubatch) + cuda_buf
fixed    = weight_vram + mmproj + mtp + linear_state + overhead
kv_budget = usable − fixed
```

Direct (standard / hybrid):
```
max_ctx = kv_budget / (effective_layers × n_kv_heads × head_dim × slots × (k_bpe + v_bpe))
```

Binary search bounds: lo = 512, hi = 2,097,152. Returns 0 if `kv_cache_bytes(512) > kv_budget`.

**Important**: `is_unified_memory` is threaded from the caller. For Mac/Metal: always true. For Windows/Linux CUDA: always false. The auto_size and quant advisor both pass this flag correctly.

### `auto_size`

Orchestrates all of the above to produce `AutoSizeResult`:

1. Determine ubatch: 1024 for Agentic/General, 512 for Roleplay
2. Find minimum `n_cpu_moe` to fit model weights (`find_min_cpu_moe_to_fit_weights`)
3. For each KV quant × context combination (q8_0, q4_0, f16):
   - Call `max_context(…, is_unified_memory)` to find the largest context that fits
   - Call `full_estimate` at that context to get the full breakdown
4. Pick the standard scenario (q8_0) as the recommended result
5. Emit warnings for: agentic + low KV quant, context > n_ctx_train, MoE offload speed penalty
6. Emit notes for: MoE offload ratio, MTP overhead, mmproj presence

### `estimate_model_size_bytes`

```
size_bytes = param_b × 1e9 × bpw / 8
```

Used for pre-download estimates where the actual file size is not yet known.

---

## API: mmproj Path Support

The `/api/vram-estimate` (breakdown) endpoint accepts either:
- `mmproj_bytes` (u64): explicit byte count
- `mmproj_path` (string): filesystem path; the server stats the file to get the size

If both are provided, `mmproj_bytes` takes precedence. The path is resolved relative to the server's working directory. This allows the preset editor to pass the configured mmproj path directly without needing a separate stat request.

---

## Backend-Specific Accuracy

| Backend | Model size | KV cache | CUDA compute buf | Total accuracy |
|---------|-----------|----------|-----------------|----------------|
| Metal (Apple Silicon) | ✓ exact from file | ✓ formula | N/A (0) | ±0.3 GiB |
| CUDA (Windows/Linux) | ✓ exact from file | ✓ formula | ✓ when n_embd known | ±0.5 GiB |
| CUDA, n_embd unknown | ✓ exact from file | ✓ formula | 0 (fallback) | ~2–3 GiB low |

**Mac M5 Max calibration** (Q5_K_S, 262k ctx, q8_0 KV):
- Estimated: 18.11 + 0.87 + 8.00 + 0.30 = 27.27 GiB
- Actual: ~27 GiB observed (model loaded, memory pressure spike to ~60 GB = 33 GB baseline + 27 GB model)

**Windows RTX 5090 calibration** (Q4_K_M, 212k ctx, q8_0 KV, ubatch=1024, flash_attn=on):
- Estimated: 16.12 + 0.87 + 6.50 + 0.38 (base) + 2.54 (CUDA buf) = 26.41 GiB
- Actual nvidia-smi: 28.10 GiB (includes ~1.5 GiB WDDM display apps already in available_vram)
- Net llama-only: ~26.6 GiB → within ~0.2 GiB of estimate

---

## Quantization Table

All quantizations recognized by the estimator, with bits-per-weight and KV bytes-per-element:

| Quant | BPW | KV BPE | Quality | imatrix | Large MoE only |
|-------|-----|--------|---------|---------|----------------|
| F32 | 32.0 | 4.0 | Reference | — | — |
| F16 / BF16 | 16.0 | 2.0 | Reference | — | — |
| Q8_0 | 8.5 | 1.0 | Excellent | — | — |
| Q6_K | 6.5625 | 0.75 | Very Good | — | — |
| Q5_K_M | 5.69 | 0.625 | Very Good | — | — |
| Q5_K_S | 5.52 | 0.625 | Very Good | — | — |
| Q5_0 | 5.5 | 0.625 | Very Good | — | — |
| Q4_K_M | 4.85 | 0.5 | Good | — | — |
| Q4_K_S | 4.58 | 0.5 | Good | — | — |
| Q4_0 | 4.55 | 0.5 | Acceptable | — | — |
| Q4_1 | 4.7 | 0.5 | Acceptable | — | — |
| IQ4_XS | 4.25 | 0.5 | Very Good | ✓ | — |
| IQ4_NL | 4.5 | 0.5 | Good | ✓ | — |
| Q3_K_M | 3.875 | 0.375 | Acceptable | — | — |
| Q3_K_S | 3.4375 | 0.375 | Fair | — | — |
| Q3_K_L | 4.0 | 0.375 | Acceptable | — | — |
| IQ3_M | 3.6875 | 0.375 | Acceptable | ✓ | — |
| IQ3_S | 3.5 | 0.375 | Fair | ✓ | — |
| IQ3_XS | 3.3125 | 0.375 | Fair | ✓ | — |
| IQ3_XXS | 3.0625 | 0.375 | Fair | ✓ | — |
| Q2_K | 2.625 | 0.25 | Reduced | — | — |
| IQ2_M | 2.6875 | 0.25 | Reduced | ✓ | ✓ |
| IQ2_S | 2.5 | 0.25 | Reduced | ✓ | ✓ |
| IQ2_XS | 2.3125 | 0.25 | Reduced | ✓ | ✓ |
| IQ2_XXS | 2.0625 | 0.25 | Reduced | ✓ | ✓ |
| IQ1_M | 1.75 | 0.125 | Very Low | ✓ | ✓ |
| IQ1_S | 1.5625 | 0.125 | Very Low | ✓ | ✓ |
| TQ1_0 | 1.69 | 0.125 | Very Low | ✓ | ✓ |
| TQ2_0 | 2.06 | 0.25 | Reduced | ✓ | ✓ |

`Large MoE only`: the quant advisor hides these for models that are not large MoE.

KV BPE is used only for KV cache estimation (`kv_cache_bytes`). The `ctk` / `ctv` names map to these values directly.

---

## GGUF Metadata Integration

When a GGUF file is introspected via `POST /api/model/introspect`, the following metadata keys map to `ModelArch` fields. These override heuristic values.

| GGUF key | ModelArch field | Notes |
|----------|----------------|-------|
| `{arch}.block_count` | `n_layers` | |
| `{arch}.attention.head_count_kv` | `n_kv_heads` | |
| `{arch}.attention.key_length` | `head_dim` | Ignored for Gemma4 (uses heuristic) |
| `{arch}.embedding_length` | `n_embd` | Used for CUDA compute buffer formula |
| `{arch}.expert_count` | `n_experts` | |
| `{arch}.expert_used_count` | `n_experts_used` | |
| `{arch}.attention.sliding_window` | `local_attn_window` | |
| `{arch}.context_length` | `n_ctx_train` | Hard cap warning in auto_size |
| `general.architecture` | (selects family heuristic) | Overrides filename matching; catches renamed finetunes |

The GGUF arch string `"qwen35"` is mapped to the Qwen3.6 DeltaNet heuristic via `gguf_arch_to_heuristic_name()`. This is llama.cpp's internal tag for both Qwen3.5 and Qwen3.6 hybrid DeltaNet models.

---

## Known Limitations and Calibration Notes

| Issue | Scope | Status |
|-------|-------|--------|
| `mtp_overhead_bytes` uses a 1.5% heuristic | All MTP models | Estimate; actual varies by architecture |
| Gemma 4 n_embd values for E2B/E4B/12B/26B-A4B are estimated | Gemma 4 | Overridden by GGUF embedding_length when file is local |
| Qwen3.6-35B-A3B n_embd=4096 is estimated | Qwen3.6-35B-A3B | Overridden by GGUF when file is local |
| Qwen3.5-122B n_embd=7168 is estimated | Qwen3.5 > 80B | Overridden by GGUF when file is local |
| Qwen3.5 expert counts (256/9) only confirmed for 122B-A10B | Qwen3.5 < 122B | Applied to smaller sizes; update when those release |
| Generic MoE suffix `n_experts_used` is in active-param-billions, not expert count | Non-Qwen/Gemma MoE | Only affects speed-penalty display note, not VRAM |
| `expert_fraction` default 0.65 is a rough average | All MoE models | Overridden per-family; calibrate when architecture is public |
| CUDA compute buffer formula uses `n_embd=0` fallback for unknown models | Any model without GGUF introspection | Underestimates by ~2–3 GiB on discrete GPU at large context |

---

## Adding a New Model Family

When a new architecture is released:

1. Add a named constructor to `ModelArch` (e.g. `fn new_family_heuristic(param_b: f64) -> Self`)
2. Add a match arm in `from_name_and_params()` before the generic MoE suffix fallback
3. Set `n_attn_layers` + `linear_attn_state_bytes` if hybrid (DeltaNet/SSM)
4. Set `n_global_attn_layers` + `local_attn_window` + `local_kv_heads` + `global_head_dim` if sliding-window
5. Set `n_embd` from the architecture spec (GGUF `embedding_length` or model card `hidden_size`). This is required for accurate CUDA overhead estimates on Windows/Linux.
6. Set `expert_fraction` from published parameter breakdown; leave at 0.65 if unknown
7. Add `"arch-tag"` → `"family-heuristic-name"` mapping to `gguf_arch_to_heuristic_name()` if the GGUF uses a non-obvious arch string
8. Add a unit test with exact arithmetic for at least one known context size
9. Update this file with the new family's heuristic table row

---

## Related Files

| File | Purpose |
|------|---------|
| `src/llama/vram_estimator/estimate.rs` | Estimation logic (`full_estimate`, `max_context`, `kv_cache_bytes`, overhead functions) |
| `src/llama/vram_estimator/arch_heuristics.rs` | `ModelArch` struct + per-family heuristics |
| `src/llama/vram_estimator/quant_table.rs` | BPW and KV BPE table |
| `src/llama/vram_estimator/tests.rs` | Unit tests including calibration assertions |
| `src/llama/spawn_wizard.rs` | `auto_size` orchestration wrapper; `ModelMetadata::to_arch()` (GGUF → ModelArch) |
| `src/llama/gguf_meta.rs` | GGUF metadata reader (feeds ground-truth arch values) |
| `src/web/api/vram.rs` | `/api/vram/*` route handlers; `mmproj_path` → `mmproj_bytes` stat |
| `docs/reference/setup-wizard.md` | Wizard UI and API reference; links here for estimation details |
