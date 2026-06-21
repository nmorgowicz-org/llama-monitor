// ── Imports from sibling modules ──────────────────────────────────────────────

use super::arch_heuristics::*;
#[allow(unused_imports)]
use super::quant_table::*;

// ── KV cache formula ──────────────────────────────────────────────────────────

/// Bytes per KV element for the given quantization name.
pub fn kv_elem_bytes(quant: &str) -> f64 {
    find_quant(quant).map(|q| q.kv_bpe).unwrap_or(1.0) // default q8_0
}

/// Compute total KV cache memory in bytes.
///
/// For Gemma-style hybrid attention:
/// - Global layers store the full context.
/// - Local layers use a sliding window.
pub fn kv_cache_bytes(
    arch: &ModelArch,
    context_size: u64,
    parallel_slots: u32,
    ctk: &str,
    ctv: &str,
) -> u64 {
    let slots = parallel_slots.max(1) as f64;
    let ctx = context_size as f64;
    let k_bpe = kv_elem_bytes(ctk);
    let v_bpe = kv_elem_bytes(ctv);

    // Hybrid linear-attention: only n_attn_layers of n_layers use a KV cache.
    // The remaining layers use a fixed recurrent state (counted separately in full_estimate).
    let effective_layers = if arch.is_hybrid_attn() {
        arch.n_attn_layers
    } else {
        arch.n_layers
    };

    if arch.has_local_attn() {
        let global_layers = arch.n_global_attn_layers.min(effective_layers) as f64;
        let local_layers = (effective_layers.saturating_sub(arch.n_global_attn_layers)) as f64;
        let g_kv = arch.n_kv_heads.max(1) as f64;
        let l_kv = arch.local_kv_heads.max(1) as f64;
        // Gemma 4 uses wider heads for global layers (global_head_dim=512 vs head_dim=256 local).
        let g_hd = if arch.global_head_dim > 0 {
            arch.global_head_dim
        } else {
            arch.head_dim
        };
        let g_hd = g_hd.max(1) as f64;
        let l_hd = arch.head_dim.max(1) as f64;
        let window = arch.local_attn_window as f64;

        // Global: full context × all slots
        let global_k = global_layers * g_kv * g_hd * ctx * slots * k_bpe;
        let global_v = global_layers * g_kv * g_hd * ctx * slots * v_bpe;

        // Local: sliding window (at most window tokens, regardless of ctx)
        let effective_local_ctx = ctx.min(window) * slots;
        let local_k = local_layers * l_kv * l_hd * effective_local_ctx * k_bpe;
        let local_v = local_layers * l_kv * l_hd * effective_local_ctx * v_bpe;

        (global_k + global_v + local_k + local_v) as u64
    } else {
        let n_layers = effective_layers.max(1) as f64;
        let n_kv = arch.n_kv_heads.max(1) as f64;
        let hd = arch.head_dim.max(1) as f64;

        let k = n_layers * n_kv * hd * ctx * slots * k_bpe;
        let v = n_layers * n_kv * hd * ctx * slots * v_bpe;
        (k + v) as u64
    }
}

// ── Weight distribution (MoE) ─────────────────────────────────────────────────

/// Split model weights between VRAM and RAM for a given `--n-cpu-moe` value.
/// Returns `(vram_bytes, ram_bytes)`.
pub fn moe_weight_split(model_size_bytes: u64, arch: &ModelArch, n_cpu_moe: i32) -> (u64, u64) {
    if !arch.is_moe() || n_cpu_moe <= 0 {
        return (model_size_bytes, 0);
    }
    // `--n-cpu-moe N` keeps the experts of the first N transformer layers on the
    // CPU, so the offloaded fraction is N / (MoE layer count) — NOT N / (experts
    // per layer). n_layers is the right denominator (≈ the MoE layer count; a few
    // models have a handful of dense layers, which this slightly over-counts).
    let moe_layers = arch.n_layers.max(1) as f64;
    let cpu_layers = (n_cpu_moe as f64).min(moe_layers);
    let cpu_ratio = cpu_layers / moe_layers;
    let expert_frac = arch.expert_fraction.clamp(0.3, 0.99);

    let cpu_bytes = (model_size_bytes as f64 * expert_frac * cpu_ratio) as u64;
    let vram_bytes = model_size_bytes.saturating_sub(cpu_bytes);
    (vram_bytes, cpu_bytes)
}

/// MTP prediction-head VRAM overhead.
/// Each depth level adds approximately 1.5% of model weights.
pub fn mtp_overhead_bytes(model_size_bytes: u64, mtp_depth: u32) -> u64 {
    if mtp_depth == 0 {
        return 0;
    }
    (model_size_bytes as f64 * 0.015 * mtp_depth as f64) as u64
}

/// Metal (unified-memory) GPU context + transient compute-buffer overhead.
/// Scales slightly with ubatch_size. Used only on Apple Silicon; discrete GPUs use
/// [`discrete_overhead_bytes`] (which is calibrated separately against real VRAM).
pub fn gpu_overhead_bytes(ubatch_size: u32) -> u64 {
    // 300 MB base (Metal context, allocator metadata, etc.)
    // + approx 0.15 MB per ubatch unit above 512
    let base = 300 * 1024 * 1024;
    let ubatch_extra = ((ubatch_size.saturating_sub(512)) as u64) * 150 * 1024;
    base + ubatch_extra
}

/// Per-head K/V dimension that drives the context-scaling compute buffer. Gemma's global
/// (full-context) layers use a wider dimension than local layers, so take the max. All
/// fields are read from the GGUF (`attention.key_length` / `key_length_swa`).
fn overhead_head_dim(arch: &ModelArch) -> u64 {
    (arch.head_dim.max(arch.global_head_dim)).max(1) as u64
}

/// Context-INDEPENDENT part of the discrete-GPU (CUDA/ROCm) overhead, in bytes.
///
/// Covers the graph compute scratch (∝ ubatch × model width), MoE expert gather/scatter
/// buffers, and — for Gemma — the per-layer-input embedding tables that are resident beyond
/// the reported weights. All inputs are GGUF-derived (`embedding_length`, `expert_count`,
/// `block_count`, sliding-window pattern), never name-parsed.
pub fn discrete_overhead_base_bytes(arch: &ModelArch, ubatch_size: u32) -> u64 {
    if arch.n_embd == 0 || arch.n_layers == 0 {
        return 256 * 1024 * 1024; // unknown arch: flat 256 MB safety reserve
    }
    let mib = 1024.0 * 1024.0;
    // Graph compute scratch: ~0.22 MiB per ubatch unit at n_embd=5120, scaled by width.
    let scratch = 0.22 * ubatch_size as f64 * (arch.n_embd as f64 / 5120.0);
    // MoE expert gather/scatter working buffers (~260 MiB measured on 35B-A3B / Gemma-MoE).
    let moe = if arch.is_moe() { 260.0 } else { 0.0 };
    // Gemma alternating-attention models carry sizeable per-layer-input embeddings (~20 MiB/layer).
    let gemma = if arch.has_local_attn() {
        20.0 * arch.n_layers as f64
    } else {
        0.0
    };
    let total_mib = (scratch + moe + gemma).max(200.0); // 200 MiB floor (CUDA context)
    (total_mib * mib) as u64
}

/// Context-SCALING part of the discrete overhead: bytes of compute buffer per token of
/// context. The attention mask plus per-layer prefill scratch grow linearly with context
/// and with `n_layers × per-head-dim`; a mild ubatch factor captures the larger mask at
/// higher ubatch. ≈ 0.46 bytes per (token × layer × head-dim-unit).
pub fn discrete_overhead_ctx_bytes_per_token(arch: &ModelArch, ubatch_size: u32) -> f64 {
    if arch.n_layers == 0 {
        return 0.0;
    }
    let ub_factor = 0.8 + 0.2 * (ubatch_size as f64 / 1024.0);
    0.46 * arch.n_layers as f64 * overhead_head_dim(arch) as f64 * ub_factor
}

/// Total discrete-GPU (CUDA/ROCm) overhead beyond weights + KV + mmproj + MTP, in bytes.
///
/// **Calibrated against direct RTX 5090 32 GB VRAM measurements** (llama.cpp b9728,
/// `--parallel 1 --kv-unified -fa on`, q8_0 KV, full GPU offload), across Qwen3.6-27B
/// (dense-hybrid), Qwen3.6-35B-A3B (MoE-hybrid), Gemma-4-31B (dense SWA) and Gemma-4-26B-A4B
/// (MoE SWA), at 4k–213k context and ubatch 1024/2048. Predictions land within tens of MiB
/// for the Qwen family and over-reserve Gemma modestly (the safe direction). The overhead is
/// roughly **independent of model depth's KV footprint** — it grows with ubatch (scratch) and
/// context (attention mask), so the old `n_layers × n_embd × ubatch` model (context-independent)
/// was wrong in both directions. Returns the Metal estimate's analogue only for discrete GPUs;
/// unified memory uses [`gpu_overhead_bytes`] in `full_estimate`.
pub fn discrete_overhead_bytes(arch: &ModelArch, ubatch_size: u32, context_size: u64) -> u64 {
    let base = discrete_overhead_base_bytes(arch, ubatch_size);
    let ctx =
        (discrete_overhead_ctx_bytes_per_token(arch, ubatch_size) * context_size as f64) as u64;
    base + ctx
}

/// Compute the headroom fraction to reserve when sizing context or evaluating fit.
///
/// Both platforms use a sliding-window approach: a percentage base rate that is capped
/// at an absolute maximum so large-memory systems don't waste capacity on oversized reserves.
///
/// - **Unified memory (Apple Silicon)**: 10% base, capped at 2 GB.
///   Covers Metal burst compute buffers. The caller has already applied the Metal GPU
///   wired cap (~66–75% of RAM) and subtracted OS/kernel reserve, so this headroom is
///   only for transient Metal allocations during inference.
///   - 24 GB effective budget: 10% = 2.4 GB → capped to 2 GB
///   - 42 GB effective budget: 10% = 4.2 GB → capped to 2 GB
///   - 10 GB effective budget: 10% = 1 GB → not capped (appropriate for small systems)
///
/// - **Discrete GPU**: 5% base, capped at 1.5 GB.
///   Display-driver + CUDA context overhead is roughly flat regardless of VRAM size.
///   Capping at 1.5 GB lets 5% apply up to a ~30 GB card; above that the cap locks in.
pub fn compute_headroom(available_vram_bytes: u64, is_unified_memory: bool) -> f64 {
    if available_vram_bytes == 0 {
        return if is_unified_memory { 0.10 } else { 0.05 };
    }
    let (base_fraction, max_bytes) = if is_unified_memory {
        (0.10f64, 2_000_000_000u64) // 2 GB cap for Metal burst buffers
    } else {
        (0.05f64, 1_500_000_000u64) // 1.5 GB cap for driver overhead
    };
    let cap_fraction = max_bytes as f64 / available_vram_bytes as f64;
    f64::min(base_fraction, cap_fraction)
}

// ── Estimate model file size from param count ─────────────────────────────────

/// Estimate model file size in bytes from parameter count and quantization.
pub fn estimate_model_size_bytes(param_b: f64, quant: &str) -> u64 {
    let bpw = find_quant(quant).map(|q| q.bpw).unwrap_or(4.85);
    (param_b * 1e9 * bpw / 8.0) as u64
}

// ── Full VRAM estimate with breakdown ─────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct VramBreakdown {
    pub weights_bytes: u64,
    pub kv_cache_bytes: u64,
    /// Fixed recurrent state for hybrid linear-attention layers (DeltaNet / SSM).
    /// Zero for standard transformers. Does not grow with context length.
    pub linear_attn_state_bytes: u64,
    pub mmproj_bytes: u64,
    pub mtp_bytes: u64,
    pub overhead_bytes: u64,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub headroom_bytes: i64, // can be negative (over budget)
    pub ram_bytes: u64,      // weights offloaded to CPU RAM (MoE only)
    pub recommendation: VramRecommendation,
    pub note: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VramRecommendation {
    Fit,
    Tight,
    Risk,
    WontFit,
}

/// Full VRAM estimate for a configured setup.
///
/// `is_unified_memory`: true for Apple Silicon and other unified-memory architectures where
/// GPU and system RAM share the same pool. On unified memory there is no CPU spill path —
/// exceeding available memory causes OS compression/paging, not a graceful fallback.
#[allow(clippy::too_many_arguments)]
pub fn full_estimate(
    model_size_bytes: u64,
    arch: &ModelArch,
    context_size: u64,
    ctk: &str,
    ctv: &str,
    parallel_slots: u32,
    ubatch_size: u32,
    n_cpu_moe: i32,
    available_vram_bytes: u64,
    is_unified_memory: bool,
) -> VramBreakdown {
    let (weight_vram, ram) = moe_weight_split(model_size_bytes, arch, n_cpu_moe);
    let kv = kv_cache_bytes(arch, context_size, parallel_slots, ctk, ctv);
    // For hybrid linear-attention models (e.g. Qwen3-Coder-Next / DeltaNet):
    // add the fixed recurrent state. This is constant — it does NOT grow with context.
    let linear_state = arch.linear_attn_state_bytes;
    let mmproj = arch.mmproj_bytes;
    let mtp = mtp_overhead_bytes(model_size_bytes, arch.mtp_depth);
    // Platform-specific overhead. Discrete GPUs use the RTX-5090-calibrated model (scratch +
    // MoE/Gemma base + context-scaling attention buffers). Unified memory (Metal) keeps the
    // lighter, context-independent estimate — transient Metal buffers are covered there plus
    // by the headroom reserve in `compute_headroom`.
    let overhead = if is_unified_memory {
        gpu_overhead_bytes(ubatch_size)
    } else {
        discrete_overhead_bytes(arch, ubatch_size, context_size)
    };
    let total = weight_vram + kv + linear_state + mmproj + mtp + overhead;
    let headroom = available_vram_bytes as i64 - total as i64;

    let (recommendation, note) = if available_vram_bytes == 0 {
        (
            VramRecommendation::Risk,
            "Memory size unknown; estimate is best-effort.".into(),
        )
    } else if total <= (available_vram_bytes * 82 / 100) {
        (
            VramRecommendation::Fit,
            if is_unified_memory {
                "Fits with good headroom within the available unified memory budget."
            } else {
                "Fits comfortably with >18% headroom."
            }
            .into(),
        )
    } else if total <= available_vram_bytes {
        (
            VramRecommendation::Tight,
            if is_unified_memory {
                "Near the memory budget — macOS may compress memory under load. Reduce context or KV quant if you notice slowdowns."
            } else {
                "Fits, but VRAM is nearly full. Reduce context or KV quant if you hit OOM."
            }
            .into(),
        )
    } else if !is_unified_memory && total <= (available_vram_bytes * 120 / 100) {
        (
            VramRecommendation::Risk,
            "Exceeds VRAM; expect CPU spill and slower generation. Lower context or use KV quantization.".into(),
        )
    } else {
        (
            VramRecommendation::WontFit,
            if is_unified_memory {
                "Exceeds unified memory budget. On Apple Silicon there is no CPU spill path — this causes memory pressure and paging. Lower context, KV quant, or model quantization."
            } else {
                "Significantly over VRAM budget. Lower the model quant, context, or offload more MoE experts to CPU."
            }
            .into(),
        )
    };

    VramBreakdown {
        weights_bytes: weight_vram,
        kv_cache_bytes: kv,
        linear_attn_state_bytes: linear_state,
        mmproj_bytes: mmproj,
        mtp_bytes: mtp,
        overhead_bytes: overhead,
        total_bytes: total,
        available_bytes: available_vram_bytes,
        headroom_bytes: headroom,
        ram_bytes: ram,
        recommendation,
        note,
    }
}

// ── Maximum context for a given config ────────────────────────────────────────

/// Find the largest context that fits in available VRAM.
///
/// `fit_granularity` rounds the result down to a multiple (e.g. 1024 for --fit-ctx 1024).
/// `headroom_fraction` reserves a fraction of VRAM as a safety buffer (default 0.05 = 5%).
/// `n_ctx_train` is an optional hard cap: even if more context fits in VRAM, we should not
/// silently exceed the model's training context length (unless the user extends it via
/// RoPE/YaRN scaling or a manual override).
#[allow(clippy::too_many_arguments)]
pub fn max_context(
    model_size_bytes: u64,
    arch: &ModelArch,
    ctk: &str,
    ctv: &str,
    parallel_slots: u32,
    ubatch_size: u32,
    n_cpu_moe: i32,
    available_vram_bytes: u64,
    fit_granularity: u64,
    headroom_fraction: f64,
    n_ctx_train: Option<u64>,
    is_unified_memory: bool,
) -> u64 {
    if available_vram_bytes == 0 {
        return 0;
    }
    let (weight_vram, _) = moe_weight_split(model_size_bytes, arch, n_cpu_moe);
    let mmproj = arch.mmproj_bytes;
    let mtp = mtp_overhead_bytes(model_size_bytes, arch.mtp_depth);
    let linear_state = arch.linear_attn_state_bytes; // constant; doesn't scale with context
    // Context-INDEPENDENT overhead goes into the fixed budget; the context-SCALING part
    // (discrete attention buffers) is charged per-token alongside the KV cache below.
    let (base_overhead, overhead_slope) = if is_unified_memory {
        (gpu_overhead_bytes(ubatch_size), 0.0)
    } else {
        (
            discrete_overhead_base_bytes(arch, ubatch_size),
            discrete_overhead_ctx_bytes_per_token(arch, ubatch_size),
        )
    };
    let fixed = weight_vram + mmproj + mtp + linear_state + base_overhead;

    let usable = (available_vram_bytes as f64 * (1.0 - headroom_fraction)) as u64;
    if fixed >= usable {
        return 0;
    }
    let kv_budget = usable - fixed;

    // Binary search for context such that kv_cache_bytes(ctx) + overhead(ctx) ≤ kv_budget.
    // For non-sliding-window models we can solve directly; for Gemma we binary-search.
    let mut ctx = if arch.has_local_attn() {
        binary_search_context(arch, ctk, ctv, parallel_slots, kv_budget, overhead_slope)
    } else {
        direct_max_context(arch, ctk, ctv, parallel_slots, kv_budget, overhead_slope)
    };

    // Cap at training context (unless user has extended via RoPE/YaRN).
    if let Some(cap) = n_ctx_train
        && ctx > cap
    {
        ctx = cap;
    }

    // Round down to fit_granularity
    let g = fit_granularity.max(1);
    (ctx / g) * g
}

fn direct_max_context(
    arch: &ModelArch,
    ctk: &str,
    ctv: &str,
    parallel_slots: u32,
    kv_budget: u64,
    overhead_slope: f64,
) -> u64 {
    let slots = parallel_slots.max(1) as f64;
    // Hybrid DeltaNet models (Qwen3.6, Qwen3.5): only n_attn_layers grow the KV cache.
    // Using n_layers here would underestimate max context by 4× for those models.
    let n_layers = if arch.is_hybrid_attn() {
        arch.n_attn_layers.max(1)
    } else {
        arch.n_layers.max(1)
    } as f64;
    let n_kv = arch.n_kv_heads.max(1) as f64;
    let hd = arch.head_dim.max(1) as f64;
    let k_bpe = kv_elem_bytes(ctk);
    let v_bpe = kv_elem_bytes(ctv);
    let mut bytes_per_token = n_layers * n_kv * hd * slots * (k_bpe + v_bpe);
    // Add the discrete-GPU per-token overhead (attention mask + context-scaling buffers).
    bytes_per_token += overhead_slope;
    if bytes_per_token <= 0.0 {
        return 0;
    }
    (kv_budget as f64 / bytes_per_token) as u64
}

fn binary_search_context(
    arch: &ModelArch,
    ctk: &str,
    ctv: &str,
    parallel_slots: u32,
    kv_budget: u64,
    overhead_slope: f64,
) -> u64 {
    let mut lo = 512u64;
    // If even the minimum context doesn't fit, report zero rather than returning 512 and OOMing.
    let min_cost =
        kv_cache_bytes(arch, lo, parallel_slots, ctk, ctv) + (overhead_slope * lo as f64) as u64;
    if min_cost > kv_budget {
        return 0;
    }
    let mut hi = 2_097_152u64; // 2M upper bound
    while lo + 1 < hi {
        let mid = lo + (hi - lo) / 2;
        let cost = kv_cache_bytes(arch, mid, parallel_slots, ctk, ctv)
            + (overhead_slope * mid as f64) as u64;
        if cost <= kv_budget {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

// ── Use-case type ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UseCase {
    /// Tool-calling agents, RAG pipelines: needs coherent long-context output.
    /// Minimum q8_0 KV recommended.
    Agentic,
    /// Everyday chat, summarization, coding assistance.
    #[default]
    General,
    /// Creative writing, roleplay: context beats coherence precision.
    /// q4_0 KV acceptable.
    Roleplay,
}

impl UseCase {
    /// Minimum recommended KV cache key quantization for this use case.
    pub fn min_kv_quant(self) -> &'static str {
        match self {
            UseCase::Agentic => "q8_0",
            UseCase::General => "q8_0",
            UseCase::Roleplay => "q4_0",
        }
    }

    /// Whether to warn about KV quant degradation.
    pub fn should_warn_kv(self, kv_quant: &str) -> bool {
        let kv_bpe = kv_elem_bytes(kv_quant);
        let min_bpe = kv_elem_bytes(self.min_kv_quant());
        kv_bpe < min_bpe
    }
}

// ── Auto-size recommendation ──────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct AutoSizeResult {
    /// Recommended context size (rounded to fit_granularity).
    pub context_size: u64,
    /// Recommended KV key quantization.
    pub kv_quant_k: String,
    /// Recommended KV value quantization.
    pub kv_quant_v: String,
    /// Recommended --fit-ctx value.
    pub fit_ctx: u32,
    /// Recommended ubatch_size.
    pub ubatch_size: u32,
    /// Recommended n_cpu_moe (None for dense models or when all fits in VRAM).
    pub n_cpu_moe: Option<i32>,
    /// Full VRAM breakdown for the recommended config.
    pub breakdown: VramBreakdown,
    /// Alternative scenarios for the wizard scenario cards.
    pub scenarios: Vec<ContextScenario>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextScenario {
    pub label: String,
    pub kv_quant_k: String,
    pub kv_quant_v: String,
    pub context_size: u64,
    pub n_cpu_moe: Option<i32>,
    pub vram_total_gb: f64,
    pub recommended: bool,
    pub warning: Option<String>,
    pub note: String,
}

/// Compute optimal settings for a given model, hardware, and use case.
///
/// `is_unified_memory`: true for Apple Silicon — tightens headroom reservation and
/// removes the CPU-spill budget zone (paging on unified memory is not a graceful fallback).
/// `n_ctx_train`: optional training context length to cap the recommended context.
#[allow(clippy::too_many_arguments)]
pub fn auto_size(
    model_size_bytes: u64,
    arch: &ModelArch,
    available_vram_bytes: u64,
    use_case: UseCase,
    requested_parallel_slots: u32,
    preferred_fit_granularity: u64,
    is_unified_memory: bool,
    n_ctx_train: Option<u64>,
) -> AutoSizeResult {
    let fit_gran = preferred_fit_granularity.max(512);
    let parallel_slots = requested_parallel_slots.max(1);

    // Ubatch: 1024 for agentic/general (faster prefill), 512 for roleplay (save VRAM)
    let ubatch = match use_case {
        UseCase::Agentic | UseCase::General => 1024,
        UseCase::Roleplay => 512,
    };

    // ── Step 1: Find optimal n_cpu_moe for MoE models ────────────────────────
    // Start with all experts in VRAM. If the weights alone don't fit, increment
    // n_cpu_moe until they do (or until fully offloaded).
    let n_cpu_moe = if arch.is_moe() {
        find_min_cpu_moe_to_fit_weights(model_size_bytes, arch, available_vram_bytes, ubatch)
    } else {
        0
    };
    let n_cpu_moe_opt = if n_cpu_moe > 0 { Some(n_cpu_moe) } else { None };

    // ── Step 2: Determine KV quant based on use case ─────────────────────────
    let (kv_k, kv_v) = best_kv_quant_for_use_case(use_case);

    let headroom = compute_headroom(available_vram_bytes, is_unified_memory);

    // ── Step 3: Compute max context for recommended KV quant ─────────────────
    let ctx = max_context(
        model_size_bytes,
        arch,
        &kv_k,
        &kv_v,
        parallel_slots,
        ubatch,
        n_cpu_moe,
        available_vram_bytes,
        fit_gran,
        headroom,
        n_ctx_train,
        is_unified_memory,
    );

    let breakdown = full_estimate(
        model_size_bytes,
        arch,
        ctx,
        &kv_k,
        &kv_v,
        parallel_slots,
        ubatch,
        n_cpu_moe,
        available_vram_bytes,
        is_unified_memory,
    );

    // ── Step 4: Warnings ──────────────────────────────────────────────────────
    let mut warnings = Vec::new();
    let mut notes = Vec::new();

    if ctx == 0 {
        warnings.push("Model weights alone exceed available VRAM. Try a smaller quantization or offload more experts to CPU.".into());
    }

    if use_case.should_warn_kv(&kv_k) {
        warnings.push(format!(
            "{} KV cache quantization can reduce coherence for {} workflows.",
            kv_k.to_uppercase(),
            format!("{use_case:?}").to_lowercase()
        ));
    }
    if use_case == UseCase::Agentic && kv_elem_bytes(&kv_k) < 1.0 {
        warnings.push("⚠ Agentic workflows should use q8_0 KV minimum. Tool-calling accuracy degrades below this.".into());
    }

    if arch.is_moe() && n_cpu_moe > 0 {
        let cpu_layers = (n_cpu_moe as u32).min(arch.n_layers);
        let gpu_layers = arch.n_layers.saturating_sub(cpu_layers);
        let cpu_fraction = cpu_layers as f64 / arch.n_layers.max(1) as f64;
        notes.push(format!(
            "MoE: expert tensors for {gpu_layers} of {} layers in VRAM, {cpu_layers} layers on CPU (~{:.0}% generation speed reduction).",
            arch.n_layers,
            cpu_fraction * 60.0
        ));
    }
    if arch.mtp_depth > 0 {
        notes.push(format!(
            "MTP depth {} adds {:.1} GB overhead for speculative prediction heads.",
            arch.mtp_depth,
            mtp_overhead_bytes(model_size_bytes, arch.mtp_depth) as f64 / 1e9
        ));
    }
    if arch.mmproj_bytes > 0 {
        notes.push(format!(
            "Vision projector ({:.1} GB) stays resident in VRAM.",
            arch.mmproj_bytes as f64 / 1e9
        ));
    }

    // ── Step 5: Alternative scenarios for the scenario cards ─────────────────
    let scenarios = build_scenarios(
        model_size_bytes,
        arch,
        available_vram_bytes,
        parallel_slots,
        ubatch,
        n_cpu_moe,
        fit_gran,
        use_case,
        &kv_k,
        is_unified_memory,
        n_ctx_train,
    );

    AutoSizeResult {
        context_size: ctx,
        kv_quant_k: kv_k,
        kv_quant_v: kv_v,
        fit_ctx: fit_gran as u32,
        ubatch_size: ubatch,
        n_cpu_moe: n_cpu_moe_opt,
        breakdown,
        scenarios,
        warnings,
        notes,
    }
}

fn best_kv_quant_for_use_case(use_case: UseCase) -> (String, String) {
    match use_case {
        UseCase::Agentic | UseCase::General => ("q8_0".into(), "q8_0".into()),
        UseCase::Roleplay => ("q4_0".into(), "q4_0".into()),
    }
}

/// Find the smallest `--n-cpu-moe` value whose weight footprint fits in VRAM.
/// Reused by the auto-size flow and the Spawn Wizard / Preset Editor auto-tuner
/// so the instant estimate always agrees with the animated VRAM bar.
pub fn find_min_cpu_moe_to_fit_weights(
    model_size_bytes: u64,
    arch: &ModelArch,
    available_vram_bytes: u64,
    ubatch_size: u32,
) -> i32 {
    let target = (available_vram_bytes * 80 / 100).saturating_sub(
        gpu_overhead_bytes(ubatch_size)
            + arch.mmproj_bytes
            + mtp_overhead_bytes(model_size_bytes, arch.mtp_depth),
    );

    // `--n-cpu-moe` counts layers, so the maximum is the layer count.
    let max_cpu = arch.n_layers as i32;

    // Binary search: find the smallest n_cpu_moe where vram fits.
    let mut lo: i32 = 0;
    let mut hi: i32 = max_cpu;

    // If even with all experts on CPU it doesn't fit, return max.
    let (vram_all_cpu, _) = moe_weight_split(model_size_bytes, arch, hi);
    if vram_all_cpu > target {
        return hi;
    }

    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (vram, _) = moe_weight_split(model_size_bytes, arch, mid);
        if vram <= target {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }

    lo
}

#[allow(clippy::too_many_arguments)]
fn build_scenarios(
    model_size_bytes: u64,
    arch: &ModelArch,
    available_vram_bytes: u64,
    parallel_slots: u32,
    ubatch: u32,
    n_cpu_moe: i32,
    fit_gran: u64,
    _use_case: UseCase,
    recommended_kv: &str,
    is_unified_memory: bool,
    n_ctx_train: Option<u64>,
) -> Vec<ContextScenario> {
    let mut scenarios = Vec::new();
    let headroom = compute_headroom(available_vram_bytes, is_unified_memory);

    let kv_options: &[(&str, &str, &str)] = &[
        ("q8_0", "q8_0", "Max coherence (q8_0 KV)"),
        ("q4_0", "q4_0", "Max context (q4_0 KV)"),
        ("f16", "f16", "Reference quality (f16 KV)"),
    ];

    for (kk, kv, label) in kv_options {
        let ctx = max_context(
            model_size_bytes,
            arch,
            kk,
            kv,
            parallel_slots,
            ubatch,
            n_cpu_moe,
            available_vram_bytes,
            fit_gran,
            headroom,
            n_ctx_train,
            is_unified_memory,
        );
        let bd = full_estimate(
            model_size_bytes,
            arch,
            ctx,
            kk,
            kv,
            parallel_slots,
            ubatch,
            n_cpu_moe,
            available_vram_bytes,
            is_unified_memory,
        );
        let warn = if _use_case == UseCase::Agentic && kv_elem_bytes(kk) < 1.0 {
            Some("⚠ Below q8_0 — not recommended for agents".into())
        } else {
            None
        };
        let rec = *kk == recommended_kv;
        scenarios.push(ContextScenario {
            label: label.to_string(),
            kv_quant_k: kk.to_string(),
            kv_quant_v: kv.to_string(),
            context_size: ctx,
            n_cpu_moe: if n_cpu_moe > 0 { Some(n_cpu_moe) } else { None },
            vram_total_gb: bd.total_bytes as f64 / 1e9,
            recommended: rec,
            warning: warn,
            note: format!("{} tokens", format_ctx(ctx)),
        });
    }

    // For MoE: add a "tight-fit" scenario with more CPU offload for extended context
    if arch.is_moe() && arch.n_layers > 1 {
        let aggressive_cpu = ((arch.n_layers as f64 * 0.75) as i32).min(arch.n_layers as i32 - 1);
        let ctx = max_context(
            model_size_bytes,
            arch,
            "q8_0",
            "q8_0",
            parallel_slots,
            ubatch,
            aggressive_cpu,
            available_vram_bytes,
            fit_gran,
            headroom,
            n_ctx_train,
            is_unified_memory,
        );
        let bd = full_estimate(
            model_size_bytes,
            arch,
            ctx,
            "q8_0",
            "q8_0",
            parallel_slots,
            ubatch,
            aggressive_cpu,
            available_vram_bytes,
            is_unified_memory,
        );
        scenarios.push(ContextScenario {
            label: format!("Extended ({}× CPU offload)", aggressive_cpu),
            kv_quant_k: "q8_0".into(),
            kv_quant_v: "q8_0".into(),
            context_size: ctx,
            n_cpu_moe: Some(aggressive_cpu),
            vram_total_gb: bd.total_bytes as f64 / 1e9,
            recommended: false,
            warning: Some(format!(
                "Expert tensors from {aggressive_cpu} layers on CPU — slower generation"
            )),
            note: format!("{} tokens", format_ctx(ctx)),
        });
    }

    scenarios
}

fn format_ctx(ctx: u64) -> String {
    if ctx >= 1_000_000 {
        return format!("{:.1}M", ctx as f64 / 1e6);
    }
    if ctx >= 1_000 {
        return format!("{}K", ctx / 1000);
    }
    ctx.to_string()
}

// ── Pre-download quant comparison table ───────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct QuantOption {
    pub quant: String,
    pub label: String,
    pub model_size_gb: f64,
    pub fits_vram: bool,
    /// Max context at q8_0 KV (agentic quality).
    pub max_ctx_q8: u64,
    /// Max context at q4_0 KV (maximum context).
    pub max_ctx_q4: u64,
    pub quality: QuantQuality,
    pub is_imatrix: bool,
    pub large_moe_only: bool,
    pub recommended: bool,
    pub quality_label: String,
    pub notes: Vec<String>,
}

/// Build a quant comparison table for the wizard's pre-download advisor.
///
/// `param_b`: approximate parameter count (from HF metadata)
/// `arch`: architecture (from introspection or `ModelArch::from_name_and_params`)
/// `available_vram_bytes`: effective available memory (caller must subtract OS overhead on unified)
/// `use_case`: affects the recommended-quant choice
/// `is_unified_memory`: true for Apple Silicon — tightens headroom and fits check
pub fn quant_comparison_table(
    param_b: f64,
    arch: &ModelArch,
    model_name: &str,
    available_vram_bytes: u64,
    _use_case: UseCase,
    parallel_slots: u32,
    is_unified_memory: bool,
) -> Vec<QuantOption> {
    // Quants we show in the advisor (sorted from highest to lowest quality)
    let show_quants = [
        "f16", "q8_0", "q6_k", "q5_k_m", "q5_k_s", "q4_k_m", "q4_k_s", "iq4_xs", "q4_0", "q3_k_m",
        "iq3_m", "iq3_xs", "q2_k", "iq2_xxs", "iq2_xs", "iq1_m",
    ];

    let mut options: Vec<QuantOption> = Vec::new();
    let mut best_quant: Option<String> = None;
    let mut best_score = 0u64;
    let headroom = compute_headroom(available_vram_bytes, is_unified_memory);
    let lower_name = model_name.to_ascii_lowercase();
    let is_gemma4_qat = (lower_name.contains("gemma-4") || lower_name.contains("gemma4"))
        && lower_name.contains("qat");

    for &q_name in &show_quants {
        let qi = match find_quant(q_name) {
            Some(qi) => qi,
            None => continue,
        };
        let quality = if is_gemma4_qat && q_name == "q4_0" {
            QuantQuality::Excellent
        } else {
            qi.quality
        };

        // Skip large-MoE-only quants for dense or small models
        if qi.large_moe_only && param_b < 70.0 && !arch.is_moe() {
            continue;
        }

        let model_bytes = estimate_model_size_bytes(param_b, q_name);
        let model_gb = model_bytes as f64 / 1e9;
        // A quant "fits" only if there's also room for a minimal useful KV cache (8 K tokens at q8_0).
        // Without this check, a model that fills all available memory shows as fitting even though
        // there's no budget left for inference context.
        let min_kv = kv_cache_bytes(arch, 8192, parallel_slots, "q8_0", "q8_0");
        let fits = model_bytes + gpu_overhead_bytes(512) + min_kv < available_vram_bytes;

        let max_q8 = max_context(
            model_bytes,
            arch,
            "q8_0",
            "q8_0",
            parallel_slots,
            512,
            0,
            available_vram_bytes,
            1024,
            headroom,
            None, // pre-download advisor: VRAM-limited maxes only
            is_unified_memory,
        );
        let max_q4 = max_context(
            model_bytes,
            arch,
            "q4_0",
            "q4_0",
            parallel_slots,
            512,
            0,
            available_vram_bytes,
            1024,
            headroom,
            None, // pre-download advisor: VRAM-limited maxes only
            is_unified_memory,
        );

        let mut notes = Vec::new();
        if qi.is_imatrix {
            notes.push("Requires imatrix calibration for best quality".into());
        }
        if qi.large_moe_only {
            notes.push("Designed for large MoE models; poor for dense".into());
        }
        if is_gemma4_qat && q_name == "q4_0" {
            notes.push(
                "Official Gemma 4 QAT target; preserves near-BF16 quality at 4-bit weights".into(),
            );
        }
        match quality {
            QuantQuality::Reference => notes.push("Bit-accurate reference quality".into()),
            QuantQuality::Excellent => {
                if !(is_gemma4_qat && q_name == "q4_0") {
                    notes
                        .push("Near-lossless; essentially equivalent to F16 for most tasks".into());
                }
            }
            QuantQuality::VeryGood => {}
            QuantQuality::Good => {}
            QuantQuality::Acceptable => {
                notes.push("Noticeable quality reduction on complex tasks".into())
            }
            QuantQuality::Fair => {
                notes.push("Significant quality loss; only for maximum context or large MoE".into())
            }
            QuantQuality::Reduced | QuantQuality::VeryLow => {
                notes.push(
                    "Heavy quality reduction; avoid for production use on dense models".into(),
                );
            }
        }

        // Score for recommendation: balance of quality × context × fits
        let score = if fits {
            max_q8.min(128_000) * quality_weight(quality)
        } else {
            0
        };
        if score > best_score {
            best_score = score;
            best_quant = Some(q_name.to_string());
        }

        options.push(QuantOption {
            quant: q_name.to_string(),
            label: qi.label.to_string(),
            model_size_gb: model_gb,
            fits_vram: fits,
            max_ctx_q8: max_q8,
            max_ctx_q4: max_q4,
            quality,
            is_imatrix: qi.is_imatrix,
            large_moe_only: qi.large_moe_only,
            recommended: false, // filled in below
            quality_label: quality_label(quality),
            notes,
        });
    }

    // Gemma 4 QAT is explicitly trained for Q4_0. Prefer that target whenever it
    // fits instead of allowing a generic higher-bit option to win a tied score.
    if is_gemma4_qat
        && options
            .iter()
            .any(|opt| opt.quant == "q4_0" && opt.fits_vram)
    {
        best_quant = Some("q4_0".into());
    }

    // Mark recommended
    if let Some(ref best) = best_quant {
        for opt in &mut options {
            if &opt.quant == best {
                opt.recommended = true;
            }
        }
    }

    options
}

fn quality_weight(q: QuantQuality) -> u64 {
    match q {
        QuantQuality::Reference => 10,
        QuantQuality::Excellent => 9,
        QuantQuality::VeryGood => 8,
        QuantQuality::Good => 7,
        QuantQuality::Acceptable => 5,
        QuantQuality::Fair => 3,
        QuantQuality::Reduced => 1,
        QuantQuality::VeryLow => 0,
    }
}

fn quality_label(q: QuantQuality) -> String {
    match q {
        QuantQuality::Reference => "Reference",
        QuantQuality::Excellent => "Excellent",
        QuantQuality::VeryGood => "Very Good",
        QuantQuality::Good => "Good",
        QuantQuality::Acceptable => "Acceptable",
        QuantQuality::Fair => "Fair",
        QuantQuality::Reduced => "Reduced",
        QuantQuality::VeryLow => "Very Low",
    }
    .into()
}

// ── Backward-compat wrapper kept for existing callers ─────────────────────────

/// Simple estimate used by the existing `/api/vram/estimate` endpoint.
/// Kept for backward compatibility; prefer `full_estimate` for new callers.
#[allow(clippy::too_many_arguments)]
pub fn estimate_vram(
    model_size_bytes: u64,
    context_size: u64,
    kv_quant: &str,
    batch_size: u32,
    ubatch_size: u32,
    speculative_decoding: bool,
    mmproj_size_bytes: u64,
    n_cpu_moe: Option<i32>,
    available_vram_bytes: u64,
) -> VramEstimate {
    // Build a minimal arch from params we have.
    let arch = ModelArch {
        // Without introspection we don't know n_layers/n_kv_heads; use zero so
        // kv_cache_bytes falls back to 0 and we keep backward-compat behaviour
        // (caller supplies model_size + rough context instead of per-token formula).
        mmproj_bytes: mmproj_size_bytes,
        ..Default::default()
    };

    // Legacy KV heuristic: context × (batch or ubatch) × bytes_per_tok
    // Keep this so existing presets with no arch info still get a number.
    let kv_bytes_per_tok = {
        let bpe = kv_elem_bytes(kv_quant);
        (bpe * 64.0) as u64 // 64 = rough bytes/token for an "average" 7-30B model
    };
    let effective_batch = batch_size.max(ubatch_size).max(1);
    let kv_est = context_size
        .saturating_mul(effective_batch as u64)
        .saturating_mul(kv_bytes_per_tok);

    let cpu_moe = n_cpu_moe.unwrap_or(0);
    let (weight_vram, ram) = if arch.is_moe() {
        moe_weight_split(model_size_bytes, &arch, cpu_moe)
    } else {
        (model_size_bytes, 0)
    };

    let mut total = weight_vram
        .saturating_add(kv_est)
        .saturating_add(mmproj_size_bytes);

    if speculative_decoding {
        total = total.saturating_add(model_size_bytes / 8);
    }
    total = total.saturating_add(gpu_overhead_bytes(ubatch_size));

    let (recommendation, note) = if available_vram_bytes == 0 {
        (VramRecommendation::Risk, "No VRAM info available.".into())
    } else if total <= available_vram_bytes * 82 / 100 {
        (
            VramRecommendation::Fit,
            "Configuration fits with headroom.".into(),
        )
    } else if total <= available_vram_bytes {
        (
            VramRecommendation::Tight,
            "Fits, but near the VRAM limit.".into(),
        )
    } else if total <= available_vram_bytes * 150 / 100 {
        (VramRecommendation::Risk, "Likely exceeds VRAM.".into())
    } else {
        (
            VramRecommendation::WontFit,
            "Significantly over VRAM budget.".into(),
        )
    };

    VramEstimate {
        estimated_vram_bytes: total,
        estimated_ram_bytes: ram,
        available_vram_bytes,
        recommendation,
        note,
    }
}

/// Legacy struct returned by `estimate_vram` for backward compatibility.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VramEstimate {
    pub estimated_vram_bytes: u64,
    pub estimated_ram_bytes: u64,
    pub available_vram_bytes: u64,
    pub recommendation: VramRecommendation,
    pub note: String,
}
