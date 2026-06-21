# VRAM Estimator Reference

The VRAM estimator (`src/llama/vram_estimator/`) predicts GPU memory usage for a given model, quantization, context size, and hardware configuration. It powers:

- The auto-size wizard
- The pre-download quant advisor
- The preset-editor VRAM strip
- Launch-card VRAM estimates

The animated VRAM breakdown bar in the UI is a client-side replica that uses the same architecture data (from GGUF introspection) but applies a simplified overhead model for speed.

Estimation is primarily based on GGUF introspection, not name matching. Filename-based heuristics are used only when GGUF metadata is unavailable (e.g., pre-download estimates or incomplete headers).

![VRAM breakdown bar in the Hardware step](../screenshots/spawn-wizard-step3-vram.png)

---

## ModelArch

`ModelArch` is the central struct. Every estimation function takes a `&ModelArch`.

**All fields are concrete integers or floats (0 = unknown).** Upstream `ModelMetadata` uses `Option<T>` fields; `ModelMetadata::to_arch()` (in `src/llama/spawn_wizard.rs`) resolves or falls back for each field, and uses 0 when unknown.

It is populated via `to_arch()` in two primary ways:

- **From a local GGUF file (primary)**:
  `GgufMetadata::to_model_metadata()` reads the file, producing a `ModelMetadata`, which is then converted via `ModelMetadata::to_arch()` into `ModelArch`. Structural fields (layer counts, attention config, hybrid attention interval, MTP depth, etc.) come from the GGUF header and override any name-based guess. This is authoritative for all downloaded models, including finetunes with unusual names.

- **From name + param count (fallback only)**:
  `ModelArch::from_name_and_params(name, param_b)` builds a coarse heuristic when GGUF introspection is not possible (e.g., before download). Even when a GGUF file exists, `to_arch()` always runs `from_name_and_params()` first as an initial scaffold; then overrides fields from GGUF data; and — if the name heuristic produced only weak defaults and a known GGUF architecture is present — may re-run the heuristic using the GGUF-derived family name to get the correct shape. This is intentionally minimal and should not be relied on for correctness when GGUF metadata is available.

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
| `is_moe()` | `n_experts > 1` |
| `is_hybrid_attn()` | `n_attn_layers > 0 && n_attn_layers < n_layers` |
| `has_local_attn()` | `local_attn_window > 0 && n_global_attn_layers < n_layers` |

---

## Architecture Heuristics (Name-Based Fallback)

Used only when GGUF introspection is not available, or as an initial scaffold that is then overridden by GGUF values.

`ModelArch::from_name_and_params(name, param_b)` returns a best-effort arch from the model filename and parameter count. For any GGUF file on disk, the per-family tables below are NOT directly used for VRAM estimation — the GGUF is authoritative. These heuristics shape only pre-download or missing-field estimates.

### Priority order (first match wins)

1. `"exaone-4.5"` / `"exaone4.5"` → `exaone45_heuristic(param_b)` (checked first)
2. `"coder-next"` / `"qwen3-coder-next"` → `qwen3_coder_next_arch()`
3. `"qwen3.6"` / `"qwen3-6"` / `"qwopus3.6"` / `"qwopus3-6"` / `"qwopus36"` →
   - `"35b-a3b"` → `qwen36_35b_a3b_arch()`
   - else → `qwen36_heuristic(param_b)`
   - MTP detection: if filename contains `"mtp"` or `"multi-token"`, set `mtp_depth = 1`
4. `"qwen3.5"` / `"qwen3-5"` → `qwen35_heuristic(param_b)` (plus MTP detection if present)
5. `"gemma-4"` / `"gemma4"` → `gemma4_heuristic(name, param_b)` (plus MTP detection if present)
6. `"gemma-3"` / `"gemma3"` → `gemma3_heuristic(param_b)` (plus MTP detection if present)
7. Fallback → `standard_heuristic(param_b)` with MoE suffix parsing and MTP detection if present

### Per-family heuristics

These are initial defaults for `from_name_and_params()`; GGUF introspection overrides them when present.

#### Dense / Qwen3 standard (`standard_heuristic`)

| param_b | n_layers | n_kv_heads | head_dim |
|---------|----------|------------|----------|
| < 2 | 22 | 4 | 64 |
| 2–5 | 28 | 4 | 128 — Qwen2.5-3B / Phi-3 style |
| 5–10 | 32 | 8 | 128 — Llama-3.1-8B, Mistral-7B, Qwen2.5-7B |
| 10–25 | 40 | 8 | 128 — Llama-2-13B, Qwen2.5-14B, Mistral-22B |
| 25–35 | 48 | 4 | 128 — tuned for Qwen3-30B-A3B |
| 35–75 | 80 | 8 | 128 — Llama-70B, Qwen2.5-72B |
| 75+ | 94 | 4 | 128 — Qwen3-235B |

#### Qwen3.6 (hybrid DeltaNet + dense)

3:1 DeltaNet:Attention ratio — exactly 1/4 of layers are standard softmax attention with a KV cache. The remaining 3/4 use a fixed-size recurrent state regardless of context length.

KV cache only grows for `n_attn_layers` — the DeltaNet layers contribute nothing to context scaling.

| param_b | n_layers | n_attn_layers | n_kv_heads | head_dim | n_embd | DeltaNet state |
|---------|----------|--------------|------------|----------|--------|----------------|
| ≤ 35 (27B) | 64 | 16 | 4 | 256 | 5120 | 48 layers × 48 V-heads × 128² × 2 B ≈ 75 MB |
| > 35 (davidau 40B) | 96 | 24 | 4 | 256 | 5120 | 72 layers × 48 V-heads × 128² × 2 B ≈ 113 MB |

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
| DeltaNet state | 30 layers × 32 V-heads × 128² × 2 B ≈ 31 MB |

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
| DeltaNet state | 36 layers × 32 V-heads × 128² × 2 B ≈ 38 MB |

#### Gemma 3 (alternating local/global attention)

1-in-6 layers use full global attention; remaining layers use a 512-token sliding window with MQA (`local_kv_heads = 1`).

The `sliding_window_pattern` bool array in the GGUF determines each layer's role: `false` = global (full context), `true` = local (sliding window). `n_global_attn_layers` is the count of global layers.

| param_b | n_layers | global_layers | n_kv_heads (global) | head_dim | local_kv_heads | window |
|---------|----------|---------------|---------------------|----------|----------------|--------|
| < 5 (4B) | 34 | 6 | 4 | 256 | 1 | 512 |
| 5–14 (12B) | 52 | 9 | 8 | 256 | 1 | 512 |
| > 14 (27B) | 62 | 10 | 16 | 256 | 1 | 512 |

`global_layers` is computed as `round(n_layers / 6)`.

#### Gemma 4 (sliding-window alternating attention)

5:1 local:global pattern — every 6th layer attends the full context; the rest use a sliding window.

Global layers use `global_head_dim = 512`, local layers use `head_dim = 256`. Gemma4-26B-A4B has `n_experts_used = 9` (8 routed + 1 always-loaded shared expert).

| Tier | n_layers | global_layers | n_kv_heads (global) | local_kv_heads | n_experts | window | n_embd |
|------|----------|---------------|---------------------|----------------|-----------|--------|--------|
| E2B | 35 | 7 | 1 | 1 | 0 | 512 | 1152 |
| E4B | 42 | 7 | 2 | 2 | 0 | 512 | 2048 |
| 12B dense | 48 | 8 | 1 | 8 | 0 | 1024 | 3072 |
| 26B-A4B MoE | 30 | 5 | 2 | 8 | 128 | 1024 | 2048 |
| 31B dense | 60 | 10 | 4 | 16 | 0 | 1024 | 5120 |

- n_embd values for E2B/E4B/12B/26B-A4B are estimated; GGUF `embedding_length` overrides when present
- Auto-size uses `block_count ≥ 75` → Qwen3.5; `< 75` → Qwen3.6 to disambiguate the shared `"qwen35"` arch tag when the GGUF is present
- Gemma4-31B is identified via `n_layers=60`; `param_b` is overridden to 31B to ensure the correct heuristic tier

#### EXAONE 4.5

All known EXAONE 4.5 sizes unconditionally set `mtp_depth = 1`.

| param_b | n_layers | global_layers | n_kv_heads | head_dim | window | mtp_depth | mmproj |
|---------|----------|---------------|------------|----------|--------|-----------|--------|
| ≤ 15 | 32 | 8 | 8 | 128 | 4096 | 1 | 0 |
| > 20 (33B) | 64 | 16 | 8 | 128 | 4096 | 1 | 2.58 GB |

### Generic MoE suffix parsing

For names not matched by the above, `parse_moe_suffix()` scans for `"NB-AMB"` or `"NB_AMB"` patterns (e.g. `"26B-A4B"`, `"122B-A10B"`). It:

- Uses the last valid pattern in the name (rightmost match) to avoid false positives.
- Enforces `total_b >= 7.0` to reject tokens like `"llama-3-a4b"`.
- Enforces `active_b <= total_b`.

For matched suffixes:

- `n_experts` inferred from sparsity (`active_b / total_b`):
  - < 5% → 512 (extremely sparse, Coder-Next style)
  - total > 100B → 128
  - total > 50B → 64
  - total > 20B → 32
  - else → 8 (Mixtral style)
- `n_experts_used` derived from sparsity:
  - < 5% → 11
  - ≤ 15% → 9
  - else → 8
- `expert_fraction` defaults to 0.65; family-specific constructors override this (0.85 for Qwen3.5/3.6, 0.92 for Coder-Next)

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

Sliding-window (for any `has_local_attn()` model, e.g. Gemma 3/4, EXAONE 4.5):

```
global_layers = min(n_global_attn_layers, effective_layers)
local_layers  = max(effective_layers - n_global_attn_layers, 0)
g_hd          = global_head_dim if > 0 else head_dim   // Gemma 4 uses 512 for global; others fall back to head_dim
effective_local_ctx = min(context, local_attn_window) × slots

global_K = global_layers × n_kv_heads × g_hd × context × slots × k_bpe
global_V = (same with v_bpe)
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
moe_layers   = max(n_layers, 1)
cpu_layers   = min(n_cpu_moe, moe_layers)
cpu_ratio    = cpu_layers / moe_layers
expert_frac  = expert_fraction.clamp(0.3, 0.99)
cpu_bytes    = model_size_bytes × expert_frac × cpu_ratio
vram_bytes   = model_size_bytes − cpu_bytes
```

`--n-cpu-moe N` treats N as the number of transformer layers whose experts are kept on CPU — not as a count of individual experts.

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

Covers Metal (Apple Silicon) context, KV allocator metadata, and transient Metal compute buffers. On unified memory, this is the only overhead function used; discrete GPUs use `discrete_overhead_bytes` instead.

### Discrete-GPU overhead (CUDA/ROCm)

The discrete overhead model has three parts. All inputs are GGUF-derived (`embedding_length`, `expert_count`, `block_count`, sliding-window pattern), never from name parsing.

- `discrete_overhead_base_bytes(arch, ubatch_size)`:
  - Context-independent: graph compute scratch (∝ ubatch × model width), MoE expert gather/scatter buffers, and (for Gemma with sliding-window) per-layer-input embedding tables.
  - If `n_embd == 0` or `n_layers == 0` (unknown architecture): flat 256 MB fallback.
  - 200 MiB floor in `base_bytes` (CUDA context minimum).

- `discrete_overhead_ctx_bytes_per_token(arch, ubatch_size)`:
  - Context-dependent: attention mask and per-layer prefill scratch that grow linearly with context, `n_layers`, and per-head dimension.
  - Uses `max(head_dim, global_head_dim)` for Gemma-style models with wider global heads.
  - Formula: `0.46 × n_layers × max(head_dim, global_head_dim) × (0.8 + 0.2 × ubatch/1024)`.

- `discrete_overhead_bytes(arch, ubatch_size, context_size)`:
  - Total discrete overhead = base + (per_token × context_size).

**Calibrated on RTX 5090 32 GB (WDDM), llama.cpp b9728, `--parallel 1 --kv-unified -fa on`, q8_0 KV, full GPU offload**, across:

- Qwen3.6-27B (dense-hybrid)
- Qwen3.6-35B-A3B (MoE-hybrid)
- Gemma-4-31B (dense SWA)
- Gemma-4-26B-A4B (MoE SWA)

at 4k–213k context, ubatch 1024/2048. For Qwen-family models, predictions land within tens of MiB; Gemma models are over-estimated modestly (the safe direction). The overhead is roughly **independent of model depth's KV footprint** — it grows with ubatch (scratch) and context (attention mask), so the prior `n_layers × n_embd × ubatch` formula (context-independent) was wrong in both directions.

On unified memory (Metal), none of these functions are used — `gpu_overhead_bytes` plus the headroom reserve is sufficient.

### `full_estimate`

Sums all components:

```
weight_vram, ram = moe_weight_split(model_size, arch, n_cpu_moe)
kv              = kv_cache_bytes(arch, context, slots, ctk, ctv)
linear_state    = arch.linear_attn_state_bytes             (fixed; not context-dependent)
mmproj          = arch.mmproj_bytes                        (set from mmproj_path stat in API)
mtp             = mtp_overhead_bytes(model_size, arch.mtp_depth)

overhead:
  if is_unified_memory:
      gpu_overhead_bytes(ubatch)
  else:
      discrete_overhead_bytes(arch, ubatch, context)
        (base + per_token × context)

total = weight_vram + kv + linear_state + mmproj + mtp + overhead
```

When `available_vram_bytes == 0`, recommendation is always `Risk` with note "Memory size unknown; estimate is best-effort."

Recommendation thresholds:

| Result | Discrete GPU | Unified Memory |
|--------|-------------|----------------|
| `Fit` | total ≤ 82% of available VRAM | same |
| `Tight` | total ≤ 100% | same |
| `Risk` | 100–120% (CPU spill possible) | **never** — unified memory skips Risk and jumps to WontFit |
| `WontFit` | > 120% | > 100% |

Rationale: on unified memory there is no graceful CPU-spill path — once you exceed available memory, macOS begins compression and paging. So Risk is only offered on non-unified-memory systems where the OS can spill to system RAM without thrashing.

On unified-memory Macs: the preset editor and spawn wizard show an mlock warning when result is Tight. mlock pins model memory so macOS cannot reclaim it; with an already tight estimate, this can push the OS into memory compression or swap.

### `max_context`

Binary search (sliding-window models) or direct formula (standard):

```
max_context(model_size, arch, ctk, ctv, parallel_slots, ubatch,
            n_cpu_moe, available_vram, fit_granularity,
            headroom_fraction, n_ctx_train, is_unified_memory) → u64

usable = available_vram × (1 − headroom_fraction)

# base overhead (context-independent) and slope (per-token)
if is_unified_memory:
    base_overhead = gpu_overhead_bytes(ubatch)
    overhead_slope = 0
else:
    base_overhead = discrete_overhead_base_bytes(arch, ubatch)
    overhead_slope = discrete_overhead_ctx_bytes_per_token(arch, ubatch)

fixed       = weight_vram + mmproj + mtp + linear_state + base_overhead
kv_budget   = usable − fixed

# Binary search (sliding-window) or direct solve (standard/hybrid).
# The per-token cost is:
#   (kv_bytes_per_token) + overhead_slope
# so the overhead slope is charged alongside the KV cache.
```

Direct (standard / hybrid):

```
max_ctx = kv_budget / (kv_bytes_per_token + overhead_slope)
```

Binary search bounds: lo = 512, hi = 2,097,152. For sliding-window models, binary-search is used to find the largest context; for standard/hybrid, the direct formula is used.

Zero-guard: returns 0 when:

- `available_vram_bytes == 0`, or
- fixed costs alone (weights in VRAM + mmproj + MTP + linear-state + overhead) exceed `usable` (available VRAM after headroom is reserved), or
- for sliding-window models, `kv_cache_bytes(512) > kv_budget` (minimum context doesn't fit).

**Important**: `is_unified_memory` is threaded from the caller. For Mac/Metal: always true. For Windows/Linux CUDA: always false. The auto_size and quant advisor both pass this flag correctly.

### MTP detection

Primary: GGUF fields (`nextn_predict_layers`, `next_n_token_count`, `num_nextn_predict_layers`, `multi_token_prediction_depth`). When present, these set `mtp_depth` directly via introspection.

Fallback: When any heuristic path (Qwen3.6, Qwen3.5, Gemma 3/4, or generic) encounters `"mtp"` or `"multi-token"` in the filename, it sets `mtp_depth = 1` via `from_name_and_params()`. This value is then used by `full_estimate` (for MTP overhead in the breakdown) and `max_context` (for budgeting fixed costs). EXAONE 4.5 unconditionally uses MTP (all sizes).

### `find_min_cpu_moe_to_fit_weights`

Binary search over `[0, n_layers]` to find the smallest `n_cpu_moe` whose weight footprint fits in VRAM.

Uses the unified-memory overhead function `gpu_overhead_bytes(ubatch)` (not discrete overhead) as its overhead estimate, then checks whether `moe_weight_split` yields a VRAM footprint that fits in 80% of available VRAM minus overhead, mmproj, and MTP overhead.

If even with all experts on CPU it still doesn't fit, returns `n_layers`.

### `auto_size`

Orchestrates all of the above to produce `AutoSizeResult`:

1. Determine ubatch: 1024 for Agentic/General, 512 for Roleplay
2. Compute headroom via `compute_headroom(available_vram_bytes, is_unified_memory)`:
   - **Unified memory**: 10% base, capped at 2 GB.
   - **Discrete GPU**: 5% base, capped at 1.5 GB.
   On large budgets (>>30 GB) the cap takes effect; on smaller systems the percentage applies.
3. Find minimum `n_cpu_moe` to fit model weights via `find_min_cpu_moe_to_fit_weights`
4. For each KV quant × context combination (q8_0, q4_0, f16):
   - Call `max_context(…, is_unified_memory)` to find the largest context that fits
   - Call `full_estimate` at that context to get the full breakdown
5. Pick the standard scenario (q8_0) as the recommended result
6. Emit warnings for: agentic + low KV quant, context > n_ctx_train, MoE offload speed penalty
7. Emit notes for: MoE offload ratio, MTP overhead, mmproj presence

`n_ctx_train` is a hard cap: even if more context fits in VRAM, the auto-size function will not recommend exceeding the model's training context length.

Auto-size ignores client-provided `n_ctx`, `ctk`, `ctv`, `n_cpu_moe` and chooses its own values.

For MoE models, `build_scenarios` adds an "extended" scenario with aggressive CPU offload (~75% of layers on CPU) to show a higher-context, slower option.

When `gguf_arch == "qwen35"`, auto-size disambiguates: `block_count ≥ 75` → Qwen3.5; `< 75` → Qwen3.6.

### `estimate_model_size_bytes`

```
size_bytes = param_b × 1e9 × bpw / 8
```

Used for pre-download estimates where the actual file size is not yet known.

### Quant advisor (`quant_comparison_table`)

For each candidate quantization, it:

- Estimates file size from `param_b` and the quant's BPW.
- Checks "fits" only if the model plus a minimal KV cache (8 K tokens at q8_0) is under available VRAM.
- Scores each quant as `min(max_ctx_q8, 128K) × quality_weight` if it fits.

Gemma 4 QAT:

- If the model name contains both `"gemma-4"` (or `"gemma4"`) and `"qat"`, then Q4_0 is treated as Excellent quality.
- When Q4_0 fits, it is chosen as the recommended quant instead of higher-bit defaults.

When called by the UI after introspection, the quant-compare endpoint can receive explicit arch fields (`n_layers`, `n_kv_heads`, `head_dim`, `global_head_dim`, `n_experts`, `mtp_depth`, etc.) that override the name heuristic, improving accuracy for renamed finetunes.

### Legacy `estimate_vram` wrapper

Backward-compat wrapper kept for existing `/api/vram/estimate` callers. Uses a legacy KV heuristic (`context × effective_batch × 64 × kv_bpe`) and no per-layer formula. Always builds a default `ModelArch` with all zeros; `new` code should use `full_estimate`.

If `speculative_decoding` is true, adds `model_size_bytes / 8` to the total.

---

## API: mmproj Path Support

The `/api/vram-estimate` (breakdown) endpoint accepts either:

- `mmproj_bytes` (u64): explicit byte count
- `mmproj_path` (string): filesystem path; the server stats the file to get the size

If both are provided, `mmproj_bytes` takes precedence. The path is resolved relative to the server's working directory. This allows the preset editor to pass the configured mmproj path directly without needing a separate stat request.

---

## API Endpoints

### POST /api/vram-estimate

Architecture-aware VRAM breakdown endpoint.

- Requires: `api-token` (Authorization header)
- Requires (body): `model_path`
- Optional (body): `n_ctx`, `gpu_layers`, `parallel_slots`, `ubatch_size`, `ctk`, `ctv`, `n_cpu_moe`, `available_vram_bytes`, `is_unified_memory`, `mmproj_path`, `mmproj_bytes`
- Behavior: reads GGUF metadata from `model_path` as primary source; falls back to `ModelArch::from_name_and_params()` on parse failure
- Output fields: `weights_bytes`, `kv_cache_bytes`, `linear_attn_state_bytes`, `mmproj_bytes`, `mtp_bytes`, `overhead_bytes`, `total_bytes`, `available_bytes`, `headroom_bytes`, `ram_bytes`, `recommendation`, `note`

### API helpers (`build_arch_from_body`)

Used by `quant-compare` and `auto-size` endpoints when GGUF is not present or for fields not in the GGUF.

- Role: merges a name-based heuristic `ModelArch` with explicit architecture fields from the request body.
- Priority: explicit body fields take precedence; fallback to heuristic defaults.
- When `gguf_arch` is present, uses `gguf_arch_to_heuristic_name()` to pick the correct family.

### VRAM bar (UI)

The animated VRAM breakdown bar in the UI is architecture-aware via GGUF introspection, but uses a simplified overhead model (not the full discrete-overhead pipeline) for performance. It consumes the same arch from GGUF but with a lighter-weight overhead formula, so its numbers may differ slightly from `/api/vram-estimate`.

---

## Backend-Specific Accuracy

| Backend | Model size | KV cache | Discrete overhead | Total accuracy |
|---------|-----------|----------|-----------------|----------------|
| Metal (Apple Silicon) | ✓ exact from file | ✓ formula | N/A (uses gpu_overhead_bytes) | ±0.3 GiB |
| CUDA (Windows/Linux) | ✓ exact from file | ✓ formula | ✓ calibrated when n_embd known | ±0.5 GiB |
| CUDA, n_embd unknown | ✓ exact from file | ✓ formula | 256 MB fallback | ~2–3 GiB low |

Discrete overhead (CUDA/ROCm) is calibrated on RTX 5090 32 GB using measurement-grounded formulas. When `n_embd` is unknown (no GGUF or missing `embedding_length`), it falls back to a 256 MB flat reserve and underestimates overhead.

**Mac M5 Max calibration** (Q5_K_S, 262k ctx, q8_0 KV):

- Estimated: 18.11 + 0.87 + 8.00 + 0.30 = 27.27 GiB
- Actual: ~27 GiB observed (model loaded, memory pressure spike to ~60 GB = 33 GB baseline + 27 GB model)

**Windows RTX 5090 calibration** (Q4_K_M, 212k ctx, q8_0 KV, ubatch=1024, flash_attn=on):

- Estimated: 16.12 + 0.87 + 6.50 + 0.38 (base) + 2.54 (ctx overhead) = 26.41 GiB
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
| TQ2_0 | 2.0 | 0.25 | Reduced | ✓ | ✓ |

`Large MoE only`: the quant advisor hides these for models that are not large MoE.

KV BPE is used only for KV cache estimation (`kv_cache_bytes`). The `ctk` / `ctv` names map to these values directly.

---

## GGUF Metadata Integration

When a GGUF file is present, `gguf_meta.rs` reads the model's real KV header and `ModelMetadata::to_model_metadata()` (in `gguf_meta.rs`) builds the metadata struct. `ModelMetadata::to_arch()` (in `spawn_wizard.rs`) then converts it into `ModelArch`.

**Structural fields come from the file, not from name guesses** — the name heuristic is always run first as a scaffold, but is then overridden field-by-field with GGUF data. For "weak" heuristic results (no MoE, no hybrid, no sliding-window) with a known GGUF architecture, it may re-run the heuristic using the GGUF-derived family name. The breakdown endpoint (`/api/vram-estimate`) and `auto_size` (`/api/vram/auto-size`) both build their arch through this real-data path when `model_path` points at an on-disk GGUF.

### Key mapping

| GGUF key | ModelArch field | Notes |
|----------|----------------|-------|
| `{arch}.block_count` | `n_layers` | |
| `{arch}.attention.head_count` | (used as n_head) | Used to derive head_dim = n_embd / n_head when key_length missing |
| `{arch}.attention.head_count_kv` | `n_kv_heads` | Scalar, or per-layer array on Gemma 3/4 (see below) |
| `{arch}.attention.key_length` | `head_dim` / `global_head_dim` | Global K/V dim; on Gemma it's the wide (512) global value |
| `{arch}.attention.key_length_swa` | `head_dim` (local) | Narrow (256) local sliding-window dim on Gemma 4 |
| `{arch}.attention.sliding_window` | `local_attn_window` | e.g. Gemma 4 = 1024 |
| `{arch}.attention.sliding_window_pattern` | `n_global_attn_layers` | Per-layer bool array; count of `false` = global layers |
| `{arch}.full_attention_interval` | `n_attn_layers` | Hybrid DeltaNet: `n_attn_layers = block_count / interval` |
| `{arch}.ssm.{inner_size,state_size,conv_kernel}` | `linear_attn_state_bytes` | DeltaNet recurrent-state size |
| `{arch}.embedding_length` | `n_embd` | Used for discrete overhead formula |
| `{arch}.expert_count` | `n_experts` | |
| `{arch}.expert_used_count` | `n_experts_used` | Routed experts (shared expert counted separately) |
| `{arch}.feed_forward_length` | `n_ff` | Available for downstream; not used directly in VRAM formulas |
| `{arch}.nextn_predict_layers` (and `next_n_token_count`, `num_nextn_predict_layers`) | `mtp_depth` | MTP head count |
| `{arch}.context_length` | `n_ctx_train` | Hard cap passed to `auto_size`/`max_context` |
| `general.architecture` | (selects family heuristic) | Central for choosing correct family when filename is ambiguous (e.g. Pantheon-27B as qwen35). See `gguf_arch_to_heuristic_name()` |
| `general.parameter_count` | (used for param_b) | Sanity checks and param_b(); not in VRAM formulas but important for ID/MoE |

`gguf_arch_to_heuristic_name()` maps arch tags to heuristic names (e.g., `qwen35`/`qwen35moe` → Qwen3.6 by default; `qwen3_coder_next` → Coder-Next; `gemma4` → Gemma 4).

### Hybrid attention (Qwen3-Next / DeltaNet)

Only `block_count / full_attention_interval` layers carry a KV cache; the rest hold a fixed (context-independent) DeltaNet state sized from SSM fields: `ssm_inner_size × (ssm_state_size + ssm_conv_kernel) × 2 B` per linear layer, multiplied by the linear layers count. Both are read from the file, so a finetune with a non-standard layer count (e.g. the 35B-A3B's real `block_count` is **41**, not the 40 the name heuristic assumes) is handled correctly.

### Gemma 3/4 alternating attention

`attention.head_count_kv` is a per-layer array; the global-layer KV head count (smaller, e.g. 2/4) and local-layer count (larger, e.g. 8/16) are read at the positions marked global/local by `sliding_window_pattern`. The global layers use `key_length` (512) over the full context; the local layers use `key_length_swa` (256) capped at `sliding_window` (1024).

The GGUF arch string `"qwen35"` is mapped via `gguf_arch_to_heuristic_name()` (llama.cpp's shared tag for Qwen3.5/3.6); since `n_layers` and `n_attn_layers` now come straight from the file, the Qwen3.5-vs-3.6 distinction no longer affects the KV math.

### Implementation details

- **Introspection cache**: results are cached in `~/.config/llama-monitor/model-cache/<sha256>.json`, keyed by file path + size + mtime.
- **Fallback**: if the direct GGUF read fails (corrupt/partial file), the system falls back to running `llama-server --print-model-metadata` and parsing its output.

---

## Known Limitations and Calibration Notes

| Issue | Scope | Status |
|-------|-------|--------|
| `mtp_overhead_bytes` uses a 1.5% heuristic | All MTP models | Estimate; actual varies by architecture |
| Gemma 4 n_embd values for E2B/E4B/12B/26B-A4B are estimated | Gemma 4 | Overridden by GGUF embedding_length when file is local |
| Qwen3.6-35B-A3B n_embd=4096 is estimated | Qwen3.6-35B-A3B | Overridden by GGUF when file is local |
| Qwen3.5-122B n_embd=7168 is estimated | Qwen3.5 > 80B | Overridden by GGUF when file is local |
| Qwen3.5 expert counts (256/9) only confirmed for 122B-A10B | Qwen3.5 < 122B | Applied to smaller sizes; update when those release |
| Generic MoE suffix `n_experts_used` is heuristic (11/9/8) based on sparsity | Non-Qwen/Gemma MoE | Approximation; exact values from GGUF introspection take precedence |
| `expert_fraction` default 0.65 is a rough average | All MoE models | Overridden per-family; calibrate when architecture is public |
| `discrete_overhead_base_bytes` uses 256 MB fallback when n_embd unknown | Any model without GGUF `embedding_length` | Conservative; may over-reserve vs actual CUDA usage |
| Name heuristics are heuristic-only fallback | All pre-download / no-GGUF estimates | Do not rely on them when a GGUF file is present — GGUF is authoritative |

---

## Adding a New Model Family

When a new architecture is released:

1. Add a named constructor to `ModelArch` (e.g. `fn new_family_heuristic(param_b: f64) -> Self`)
2. Add a match arm in `from_name_and_params()` before the generic MoE suffix fallback to use as a heuristic for missing metadata
3. Set `n_attn_layers` + `linear_attn_state_bytes` if hybrid (DeltaNet/SSM)
4. Set `n_global_attn_layers` + `local_attn_window` + `local_kv_heads` + `global_head_dim` if sliding-window
5. Set `n_embd` from the architecture spec (GGUF `embedding_length` or model card `hidden_size`). This is required for accurate discrete overhead estimates on Windows/Linux.
6. Set `expert_fraction` from published parameter breakdown; leave at 0.65 if unknown
7. Add `"arch-tag"` → `"family-heuristic-name"` mapping to `gguf_arch_to_heuristic_name()` if the GGUF uses a non-obvious arch string
8. Add a unit test with exact arithmetic for at least one known context size
9. Update this file with the new family's heuristic table row

---

## Related Files

| File | Purpose |
|------|---------|
| `src/llama/vram_estimator/estimate.rs` | Estimation logic (`full_estimate`, `max_context`, `kv_cache_bytes`, overhead functions) |
| `src/llama/vram_estimator/arch_heuristics.rs` | `ModelArch` struct + per-family heuristics + `gguf_arch_to_heuristic_name()` |
| `src/llama/vram_estimator/quant_table.rs` | BPW and KV BPE table |
| `src/llama/vram_estimator/tests.rs` | Unit tests including calibration assertions |
| `src/llama/spawn_wizard.rs` | `ModelMetadata::to_arch()` (GGUF → ModelArch); auto_size orchestration wrapper |
| `src/llama/gguf_meta.rs` | GGUF metadata reader; `GgufMetadata::to_model_metadata()` (feeds ground-truth arch values) |
| `src/web/api/vram.rs` | `/api/vram/*` route handlers; `mmproj_path` → `mmproj_bytes` stat; `build_arch_from_body()` |
| `docs/reference/setup-wizard.md` | Wizard UI and API reference; links here for estimation details |
