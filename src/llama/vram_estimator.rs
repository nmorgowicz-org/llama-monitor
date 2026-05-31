/// Architecture-aware VRAM estimator for llama-server configurations.
///
/// Handles:
/// - Standard full-attention (Llama, Mistral, Qwen, …)
/// - Sliding-window / alternating-attention (Gemma 3/4)
/// - MoE expert offloading (Mixtral, Qwen-MoE, DeepSeek, …)
/// - Multi-Token Prediction heads (DeepSeek-R1 style)
/// - Vision projector (mmproj) VRAM
/// - Pre-download quant comparison table
/// - Auto-size recommendation for a given use case

// ── Quant table ───────────────────────────────────────────────────────────────

/// Quantization descriptor: weight file size + KV cache element size.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QuantInfo {
    /// Canonical lowercase name used in API requests / flag values.
    pub name: &'static str,
    /// Display name shown in the wizard.
    pub label: &'static str,
    /// Average bits-per-weight — used to estimate model file size.
    pub bpw: f64,
    /// Bytes per KV element when this quant is used as `-ctk`/`-ctv`.
    pub kv_bpe: f64,
    pub quality: QuantQuality,
    /// True = importance-matrix calibration recommended (imatrix flag at conversion).
    pub is_imatrix: bool,
    /// Suitable for large (80B+) MoE models; poor for 7B dense.
    pub large_moe_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantQuality {
    Reference,   // F16 / F32 — bit-exact
    Excellent,   // Q8_0
    VeryGood,    // Q6_K, Q5_K_M/S, IQ4_XS
    Good,        // Q4_K_M/S, IQ4_NL
    Acceptable,  // Q4_0, Q3_K_L/M, IQ3_M/S
    Fair,        // Q3_K_S, IQ3_XS/XXS (MoE-class)
    Reduced,     // Q2_K, IQ2_*
    VeryLow,     // IQ1_*
}

/// All supported quantization levels, from best to most compressed.
pub fn all_quants() -> &'static [QuantInfo] {
    &QUANT_TABLE
}

/// Look up a quant by name (case-insensitive).
pub fn find_quant(name: &str) -> Option<&'static QuantInfo> {
    let lower = name.to_ascii_lowercase();
    QUANT_TABLE.iter().find(|q| q.name == lower.as_str())
}

static QUANT_TABLE: &[QuantInfo] = &[
    // Reference
    QuantInfo { name:"f32",      label:"F32",       bpw:32.0,  kv_bpe:4.0,   quality:QuantQuality::Reference, is_imatrix:false, large_moe_only:false },
    QuantInfo { name:"f16",      label:"F16",       bpw:16.0,  kv_bpe:2.0,   quality:QuantQuality::Reference, is_imatrix:false, large_moe_only:false },
    QuantInfo { name:"bf16",     label:"BF16",      bpw:16.0,  kv_bpe:2.0,   quality:QuantQuality::Reference, is_imatrix:false, large_moe_only:false },
    // Lossless / near-lossless
    QuantInfo { name:"q8_0",     label:"Q8_0",      bpw:8.5,   kv_bpe:1.0,   quality:QuantQuality::Excellent, is_imatrix:false, large_moe_only:false },
    // High quality
    QuantInfo { name:"q6_k",     label:"Q6_K",      bpw:6.5625,kv_bpe:0.75,  quality:QuantQuality::VeryGood,  is_imatrix:false, large_moe_only:false },
    QuantInfo { name:"q5_k_m",   label:"Q5_K_M",    bpw:5.69,  kv_bpe:0.625, quality:QuantQuality::VeryGood,  is_imatrix:false, large_moe_only:false },
    QuantInfo { name:"q5_k_s",   label:"Q5_K_S",    bpw:5.52,  kv_bpe:0.625, quality:QuantQuality::VeryGood,  is_imatrix:false, large_moe_only:false },
    QuantInfo { name:"q5_0",     label:"Q5_0",      bpw:5.5,   kv_bpe:0.625, quality:QuantQuality::VeryGood,  is_imatrix:false, large_moe_only:false },
    // Good quality
    QuantInfo { name:"q4_k_m",   label:"Q4_K_M",    bpw:4.85,  kv_bpe:0.5,   quality:QuantQuality::Good,      is_imatrix:false, large_moe_only:false },
    QuantInfo { name:"q4_k_s",   label:"Q4_K_S",    bpw:4.58,  kv_bpe:0.5,   quality:QuantQuality::Good,      is_imatrix:false, large_moe_only:false },
    QuantInfo { name:"q4_0",     label:"Q4_0",      bpw:4.55,  kv_bpe:0.5,   quality:QuantQuality::Acceptable,is_imatrix:false, large_moe_only:false },
    QuantInfo { name:"q4_1",     label:"Q4_1",      bpw:4.7,   kv_bpe:0.5,   quality:QuantQuality::Acceptable,is_imatrix:false, large_moe_only:false },
    // imatrix high
    QuantInfo { name:"iq4_xs",   label:"IQ4_XS",    bpw:4.25,  kv_bpe:0.5,   quality:QuantQuality::VeryGood,  is_imatrix:true,  large_moe_only:false },
    QuantInfo { name:"iq4_nl",   label:"IQ4_NL",    bpw:4.5,   kv_bpe:0.5,   quality:QuantQuality::Good,      is_imatrix:true,  large_moe_only:false },
    // 3-bit range
    QuantInfo { name:"q3_k_m",   label:"Q3_K_M",    bpw:3.875, kv_bpe:0.375, quality:QuantQuality::Acceptable,is_imatrix:false, large_moe_only:false },
    QuantInfo { name:"q3_k_s",   label:"Q3_K_S",    bpw:3.4375,kv_bpe:0.375, quality:QuantQuality::Fair,      is_imatrix:false, large_moe_only:false },
    QuantInfo { name:"q3_k_l",   label:"Q3_K_L",    bpw:4.0,   kv_bpe:0.375, quality:QuantQuality::Acceptable,is_imatrix:false, large_moe_only:false },
    // imatrix 3-bit
    QuantInfo { name:"iq3_m",    label:"IQ3_M",     bpw:3.6875,kv_bpe:0.375, quality:QuantQuality::Acceptable,is_imatrix:true,  large_moe_only:false },
    QuantInfo { name:"iq3_s",    label:"IQ3_S",     bpw:3.5,   kv_bpe:0.375, quality:QuantQuality::Fair,      is_imatrix:true,  large_moe_only:false },
    QuantInfo { name:"iq3_xs",   label:"IQ3_XS",    bpw:3.3125,kv_bpe:0.375, quality:QuantQuality::Fair,      is_imatrix:true,  large_moe_only:false },
    QuantInfo { name:"iq3_xxs",  label:"IQ3_XXS",   bpw:3.0625,kv_bpe:0.375, quality:QuantQuality::Fair,      is_imatrix:true,  large_moe_only:false },
    // 2-bit range — meaningful mainly for very large MoE with heavy CPU offload
    QuantInfo { name:"q2_k",     label:"Q2_K",      bpw:2.625, kv_bpe:0.25,  quality:QuantQuality::Reduced,   is_imatrix:false, large_moe_only:false },
    QuantInfo { name:"iq2_m",    label:"IQ2_M",     bpw:2.6875,kv_bpe:0.25,  quality:QuantQuality::Reduced,   is_imatrix:true,  large_moe_only:true  },
    QuantInfo { name:"iq2_s",    label:"IQ2_S",     bpw:2.5,   kv_bpe:0.25,  quality:QuantQuality::Reduced,   is_imatrix:true,  large_moe_only:true  },
    QuantInfo { name:"iq2_xs",   label:"IQ2_XS",    bpw:2.3125,kv_bpe:0.25,  quality:QuantQuality::Reduced,   is_imatrix:true,  large_moe_only:true  },
    QuantInfo { name:"iq2_xxs",  label:"IQ2_XXS",   bpw:2.0625,kv_bpe:0.25,  quality:QuantQuality::Reduced,   is_imatrix:true,  large_moe_only:true  },
    // 1-bit — experimental
    QuantInfo { name:"iq1_m",    label:"IQ1_M",     bpw:1.75,  kv_bpe:0.125, quality:QuantQuality::VeryLow,   is_imatrix:true,  large_moe_only:true  },
    QuantInfo { name:"iq1_s",    label:"IQ1_S",     bpw:1.5625,kv_bpe:0.125, quality:QuantQuality::VeryLow,   is_imatrix:true,  large_moe_only:true  },
];

// ── Architecture descriptor ───────────────────────────────────────────────────

/// Transformer architecture parameters needed for accurate VRAM estimation.
/// All fields are `u32` / `u64` / `f64` so they can be compiled from
/// introspection output *or* from the heuristic fallbacks.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ModelArch {
    // ── Attention ────────────────────────────────────────────────────────────
    /// Total transformer layers (including any MoE layers).
    pub n_layers: u32,
    /// KV heads (GQA/MQA compressed; may differ from query head count).
    pub n_kv_heads: u32,
    /// Per-head dimension = n_embd / n_heads.
    pub head_dim: u32,

    // ── Sliding-window / alternating attention (Gemma 3/4 style) ────────────
    /// Layers that attend over the full context (0 = all layers).
    /// Non-global layers use local_attn_window.
    pub n_global_attn_layers: u32,
    /// Sliding window size in tokens (0 = not applicable).
    pub local_attn_window: u32,
    /// KV heads for local-attention layers (often 1 for MQA).
    pub local_kv_heads: u32,

    // ── MoE ─────────────────────────────────────────────────────────────────
    /// Total experts per layer (0 = dense model).
    pub n_experts: u32,
    /// Experts activated per token (for generation-speed estimation).
    pub n_experts_used: u32,
    /// Fraction of total model params that are expert FFN blocks.
    /// Typical range 0.55–0.75; default 0.65 when unknown.
    #[serde(default = "default_expert_fraction")]
    pub expert_fraction: f64,

    // ── MTP (Multi-Token Prediction) ─────────────────────────────────────────
    /// Number of MTP prediction heads (0 = none).
    pub mtp_depth: u32,

    // ── Multimodal ───────────────────────────────────────────────────────────
    /// Vision projector size in bytes (0 = no mmproj).
    pub mmproj_bytes: u64,

    // ── Sizing metadata ──────────────────────────────────────────────────────
    /// Approximate parameter count in billions (used for quant-size estimation
    /// when the exact file size is not yet known, e.g. pre-download advisor).
    pub param_b: f64,
}

fn default_expert_fraction() -> f64 {
    0.65
}

impl ModelArch {
    /// Return a reasonable architecture heuristic from model name + parameter count.
    /// Used when the model hasn't been introspected yet (pre-download advisor).
    pub fn from_name_and_params(name: &str, param_b: f64) -> Self {
        let lower = name.to_ascii_lowercase();
        if lower.contains("gemma-3") || lower.contains("gemma3")
            || lower.contains("gemma-4") || lower.contains("gemma4")
        {
            return Self::gemma3_heuristic(param_b);
        }
        Self::standard_heuristic(param_b)
    }

    /// Standard full-attention architecture heuristic.
    fn standard_heuristic(param_b: f64) -> Self {
        // These are rough central estimates; actual models vary.
        let (n_layers, n_kv_heads, head_dim) = if param_b < 2.0 {
            (22, 4, 64)
        } else if param_b < 5.0 {
            (28, 4, 128)   // Qwen2.5-3B, Phi-3-mini style
        } else if param_b < 10.0 {
            (32, 8, 128)   // Llama-3.1-8B, Mistral-7B, Qwen2.5-7B
        } else if param_b < 18.0 {
            (40, 8, 128)   // Llama-2-13B, Qwen2.5-14B (48 layers but 8 kv)
        } else if param_b < 25.0 {
            (32, 8, 128)   // Mistral-22B-range; Qwen2.5-14B is 48 layers so slightly off
        } else if param_b < 35.0 {
            (40, 8, 128)   // Qwen2.5-32B (actually 64 layers but similar KV)
        } else if param_b < 55.0 {
            (60, 8, 128)   // Models in the 40B range
        } else {
            (80, 8, 128)   // Llama-3.1-70B, Qwen2.5-72B
        };
        Self {
            n_layers,
            n_kv_heads,
            head_dim,
            expert_fraction: 0.65,
            ..Default::default()
        }
    }

    /// Gemma-3/4 alternating local/global attention heuristic.
    /// 1-in-6 layers use full global attention; rest use a 512-token sliding window
    /// with a single KV head (MQA).
    fn gemma3_heuristic(param_b: f64) -> Self {
        let (n_layers, global_kv_heads, head_dim) = if param_b < 5.0 {
            (34u32, 4u32, 256u32)   // Gemma-3-4B
        } else if param_b < 14.0 {
            (52, 8, 256)            // Gemma-3-12B
        } else {
            (62, 16, 256)           // Gemma-3-27B / Gemma-4-27B
        };
        let global_layers = (n_layers as f64 / 6.0).round() as u32;
        Self {
            n_layers,
            n_kv_heads: global_kv_heads,
            head_dim,
            n_global_attn_layers: global_layers,
            local_attn_window: 512,
            local_kv_heads: 1,
            expert_fraction: 0.65,
            // Gemma4 MoE variants (26B-A4B) need to be handled by introspection;
            // the heuristic assumes dense.
            ..Default::default()
        }
    }

    pub fn is_moe(&self) -> bool {
        self.n_experts > 1
    }

    pub fn has_local_attn(&self) -> bool {
        self.local_attn_window > 0 && self.n_global_attn_layers < self.n_layers
    }
}

// ── KV cache formula ──────────────────────────────────────────────────────────

/// Bytes per KV element for the given quantization name.
pub fn kv_elem_bytes(quant: &str) -> f64 {
    find_quant(quant).map(|q| q.kv_bpe).unwrap_or(1.0) // default q8_0
}

/// Compute total KV cache memory in bytes.
///
/// Accounts for Gemma alternating local/global attention:
/// - Global layers store the full context.
/// - Local layers store only the sliding window (fixed size for context > window).
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

    if arch.has_local_attn() {
        let global_layers = arch.n_global_attn_layers as f64;
        let local_layers = (arch.n_layers - arch.n_global_attn_layers) as f64;
        let g_kv = arch.n_kv_heads.max(1) as f64;
        let l_kv = arch.local_kv_heads.max(1) as f64;
        let hd = arch.head_dim.max(1) as f64;
        let window = arch.local_attn_window as f64;

        // Global: full context × all slots
        let global_k = global_layers * g_kv * hd * ctx * slots * k_bpe;
        let global_v = global_layers * g_kv * hd * ctx * slots * v_bpe;
        // Local: sliding window (at most `window` tokens, regardless of ctx)
        let effective_local_ctx = ctx.min(window) * slots;
        let local_k = local_layers * l_kv * hd * effective_local_ctx * k_bpe;
        let local_v = local_layers * l_kv * hd * effective_local_ctx * v_bpe;

        (global_k + global_v + local_k + local_v) as u64
    } else {
        let n_layers = arch.n_layers.max(1) as f64;
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
pub fn moe_weight_split(
    model_size_bytes: u64,
    arch: &ModelArch,
    n_cpu_moe: i32,
) -> (u64, u64) {
    if !arch.is_moe() || n_cpu_moe <= 0 {
        return (model_size_bytes, 0);
    }
    let total_experts = arch.n_experts as f64;
    let cpu_experts = (n_cpu_moe as f64).min(total_experts);
    let cpu_ratio = cpu_experts / total_experts;
    let expert_frac = arch.expert_fraction.clamp(0.3, 0.85);

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

/// Fixed GPU context + compute-buffer overhead (CUDA/Metal/ROCm).
/// Scales slightly with ubatch_size.
pub fn gpu_overhead_bytes(ubatch_size: u32) -> u64 {
    // 300 MB base (CUDA context, KV allocator metadata, etc.)
    // + approx 0.15 MB per ubatch unit above 512
    let base = 300 * 1024 * 1024;
    let ubatch_extra = ((ubatch_size.saturating_sub(512)) as u64) * 150 * 1024;
    base + ubatch_extra
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
    pub mmproj_bytes: u64,
    pub mtp_bytes: u64,
    pub overhead_bytes: u64,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub headroom_bytes: i64, // can be negative (over budget)
    pub ram_bytes: u64,       // weights offloaded to CPU RAM (MoE only)
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
) -> VramBreakdown {
    let (weight_vram, ram) = moe_weight_split(model_size_bytes, arch, n_cpu_moe);
    let kv = kv_cache_bytes(arch, context_size, parallel_slots, ctk, ctv);
    let mmproj = arch.mmproj_bytes;
    let mtp = mtp_overhead_bytes(model_size_bytes, arch.mtp_depth);
    let overhead = gpu_overhead_bytes(ubatch_size);
    let total = weight_vram + kv + mmproj + mtp + overhead;
    let headroom = available_vram_bytes as i64 - total as i64;

    let (recommendation, note) = if available_vram_bytes == 0 {
        (VramRecommendation::Risk, "GPU VRAM unknown; estimate is best-effort.".into())
    } else if total <= (available_vram_bytes * 82 / 100) {
        (VramRecommendation::Fit, "Fits comfortably with >18% headroom.".into())
    } else if total <= available_vram_bytes {
        (VramRecommendation::Tight, "Fits, but VRAM is nearly full. Reduce context or KV quant if you hit OOM.".into())
    } else if total <= (available_vram_bytes * 120 / 100) {
        (VramRecommendation::Risk, "Exceeds VRAM; expect CPU spill and slower generation. Lower context or use KV quantization.".into())
    } else {
        (VramRecommendation::WontFit, "Significantly over VRAM budget. Lower the model quant, context, or offload more MoE experts to CPU.".into())
    };

    VramBreakdown {
        weights_bytes: weight_vram,
        kv_cache_bytes: kv,
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
) -> u64 {
    if available_vram_bytes == 0 {
        return 0;
    }
    let (weight_vram, _) = moe_weight_split(model_size_bytes, arch, n_cpu_moe);
    let mmproj = arch.mmproj_bytes;
    let mtp = mtp_overhead_bytes(model_size_bytes, arch.mtp_depth);
    let overhead = gpu_overhead_bytes(ubatch_size);
    let fixed = weight_vram + mmproj + mtp + overhead;

    let usable = (available_vram_bytes as f64 * (1.0 - headroom_fraction)) as u64;
    if fixed >= usable {
        return 0;
    }
    let kv_budget = usable - fixed;

    // Binary search for context such that kv_cache_bytes(ctx) ≤ kv_budget
    // For non-sliding-window models we can solve directly; for Gemma we binary-search.
    let ctx = if arch.has_local_attn() {
        binary_search_context(arch, ctk, ctv, parallel_slots, kv_budget)
    } else {
        direct_max_context(arch, ctk, ctv, parallel_slots, kv_budget)
    };

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
) -> u64 {
    let slots = parallel_slots.max(1) as f64;
    let n_layers = arch.n_layers.max(1) as f64;
    let n_kv = arch.n_kv_heads.max(1) as f64;
    let hd = arch.head_dim.max(1) as f64;
    let k_bpe = kv_elem_bytes(ctk);
    let v_bpe = kv_elem_bytes(ctv);
    let bytes_per_token = n_layers * n_kv * hd * slots * (k_bpe + v_bpe);
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
) -> u64 {
    let mut lo = 512u64;
    let mut hi = 2_097_152u64; // 2M upper bound
    while lo + 1 < hi {
        let mid = lo + (hi - lo) / 2;
        let cost = kv_cache_bytes(arch, mid, parallel_slots, ctk, ctv);
        if cost <= kv_budget { lo = mid; } else { hi = mid; }
    }
    lo
}

// ── Use-case type ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UseCase {
    /// Tool-calling agents, RAG pipelines: needs coherent long-context output.
    /// Minimum q8_0 KV recommended.
    Agentic,
    /// Everyday chat, summarization, coding assistance.
    General,
    /// Creative writing, roleplay: context beats coherence precision.
    /// q4_0 KV acceptable.
    Roleplay,
}

impl Default for UseCase { fn default() -> Self { UseCase::General } }

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
#[allow(clippy::too_many_arguments)]
pub fn auto_size(
    model_size_bytes: u64,
    arch: &ModelArch,
    available_vram_bytes: u64,
    use_case: UseCase,
    requested_parallel_slots: u32,
    preferred_fit_granularity: u64,
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

    // ── Step 3: Compute max context for recommended KV quant ─────────────────
    let ctx = max_context(
        model_size_bytes, arch, &kv_k, &kv_v,
        parallel_slots, ubatch, n_cpu_moe, available_vram_bytes,
        fit_gran, 0.05,
    );

    let breakdown = full_estimate(
        model_size_bytes, arch, ctx, &kv_k, &kv_v,
        parallel_slots, ubatch, n_cpu_moe, available_vram_bytes,
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
        let expert_fraction = (arch.n_experts - n_cpu_moe as u32) as f64 / arch.n_experts as f64;
        let speed_penalty = 1.0 - expert_fraction;
        notes.push(format!(
            "MoE: {} of {} experts in VRAM, {} on CPU (~{:.0}% generation speed reduction).",
            arch.n_experts - n_cpu_moe as u32,
            arch.n_experts,
            n_cpu_moe,
            speed_penalty * 60.0
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
        model_size_bytes, arch, available_vram_bytes,
        parallel_slots, ubatch, n_cpu_moe, fit_gran, use_case,
        &kv_k,
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

fn find_min_cpu_moe_to_fit_weights(
    model_size_bytes: u64,
    arch: &ModelArch,
    available_vram_bytes: u64,
    ubatch_size: u32,
) -> i32 {
    // We need at least 20% headroom after weights for KV cache.
    let target = (available_vram_bytes * 80 / 100).saturating_sub(
        gpu_overhead_bytes(ubatch_size) + arch.mmproj_bytes + mtp_overhead_bytes(model_size_bytes, arch.mtp_depth)
    );
    let mut n_cpu = 0i32;
    while n_cpu < arch.n_experts as i32 {
        let (vram, _) = moe_weight_split(model_size_bytes, arch, n_cpu);
        if vram <= target { break; }
        n_cpu += 1;
    }
    n_cpu
}

fn build_scenarios(
    model_size_bytes: u64,
    arch: &ModelArch,
    available_vram_bytes: u64,
    parallel_slots: u32,
    ubatch: u32,
    n_cpu_moe: i32,
    fit_gran: u64,
    use_case: UseCase,
    recommended_kv: &str,
) -> Vec<ContextScenario> {
    let mut scenarios = Vec::new();

    let kv_options: &[(&str, &str, &str)] = &[
        ("q8_0", "q8_0", "Max coherence (q8_0 KV)"),
        ("q4_0", "q4_0", "Max context (q4_0 KV)"),
        ("f16",  "f16",  "Reference quality (f16 KV)"),
    ];

    for (kk, kv, label) in kv_options {
        let ctx = max_context(
            model_size_bytes, arch, kk, kv,
            parallel_slots, ubatch, n_cpu_moe, available_vram_bytes,
            fit_gran, 0.05,
        );
        let bd = full_estimate(
            model_size_bytes, arch, ctx, kk, kv,
            parallel_slots, ubatch, n_cpu_moe, available_vram_bytes,
        );
        let warn = if use_case == UseCase::Agentic && kv_elem_bytes(kk) < 1.0 {
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
    if arch.is_moe() && arch.n_experts > 0 {
        let aggressive_cpu = ((arch.n_experts as f64 * 0.75) as i32).min(arch.n_experts as i32 - 1);
        let ctx = max_context(
            model_size_bytes, arch, "q8_0", "q8_0",
            parallel_slots, ubatch, aggressive_cpu, available_vram_bytes,
            fit_gran, 0.05,
        );
        let bd = full_estimate(
            model_size_bytes, arch, ctx, "q8_0", "q8_0",
            parallel_slots, ubatch, aggressive_cpu, available_vram_bytes,
        );
        scenarios.push(ContextScenario {
            label: format!("Extended ({}× CPU offload)", aggressive_cpu),
            kv_quant_k: "q8_0".into(),
            kv_quant_v: "q8_0".into(),
            context_size: ctx,
            n_cpu_moe: Some(aggressive_cpu),
            vram_total_gb: bd.total_bytes as f64 / 1e9,
            recommended: false,
            warning: Some(format!("{aggressive_cpu} experts on CPU — slower generation")),
            note: format!("{} tokens", format_ctx(ctx)),
        });
    }

    scenarios
}

fn format_ctx(ctx: u64) -> String {
    if ctx >= 1_000_000 { return format!("{:.1}M", ctx as f64 / 1e6); }
    if ctx >= 1_000 { return format!("{}K", ctx / 1000); }
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
/// `available_vram_bytes`: GPU VRAM
/// `use_case`: affects the recommended-quant choice
pub fn quant_comparison_table(
    param_b: f64,
    arch: &ModelArch,
    available_vram_bytes: u64,
    use_case: UseCase,
    parallel_slots: u32,
) -> Vec<QuantOption> {
    // Quants we show in the advisor (sorted from highest to lowest quality)
    let show_quants = [
        "f16", "q8_0",
        "q6_k", "q5_k_m", "q5_k_s",
        "q4_k_m", "q4_k_s", "iq4_xs",
        "q4_0",
        "q3_k_m", "iq3_m", "iq3_xs",
        "q2_k", "iq2_xxs", "iq2_xs",
        "iq1_m",
    ];

    let mut options: Vec<QuantOption> = Vec::new();
    let mut best_quant: Option<String> = None;
    let mut best_score = 0u64;

    for &q_name in &show_quants {
        let qi = match find_quant(q_name) {
            Some(qi) => qi,
            None => continue,
        };

        // Skip large-MoE-only quants for dense or small models
        if qi.large_moe_only && param_b < 70.0 && !arch.is_moe() {
            continue;
        }

        let model_bytes = estimate_model_size_bytes(param_b, q_name);
        let model_gb = model_bytes as f64 / 1e9;
        let fits = model_bytes + gpu_overhead_bytes(512) < available_vram_bytes;

        let mut arch_for_q = arch.clone();
        // Update mmproj_bytes from arch (already set)

        let max_q8 = max_context(model_bytes, &arch_for_q, "q8_0", "q8_0",
            parallel_slots, 512, 0, available_vram_bytes, 1024, 0.05);
        let max_q4 = max_context(model_bytes, &arch_for_q, "q4_0", "q4_0",
            parallel_slots, 512, 0, available_vram_bytes, 1024, 0.05);

        let mut notes = Vec::new();
        if qi.is_imatrix { notes.push("Requires imatrix calibration for best quality".into()); }
        if qi.large_moe_only { notes.push("Designed for large MoE models; poor for dense".into()); }
        match qi.quality {
            QuantQuality::Reference => notes.push("Bit-accurate reference quality".into()),
            QuantQuality::Excellent => notes.push("Near-lossless; essentially equivalent to F16 for most tasks".into()),
            QuantQuality::VeryGood => {}
            QuantQuality::Good => {}
            QuantQuality::Acceptable => notes.push("Noticeable quality reduction on complex tasks".into()),
            QuantQuality::Fair => notes.push("Significant quality loss; only for maximum context or large MoE".into()),
            QuantQuality::Reduced | QuantQuality::VeryLow => {
                notes.push("Heavy quality reduction; avoid for production use on dense models".into());
            }
        }

        // Score for recommendation: balance of quality × context × fits
        let score = if fits {
            max_q8.min(128_000) * quality_weight(qi.quality)
        } else { 0 };
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
            quality: qi.quality,
            is_imatrix: qi.is_imatrix,
            large_moe_only: qi.large_moe_only,
            recommended: false, // filled in below
            quality_label: quality_label(qi.quality),
            notes,
        });
    }

    // Mark recommended
    if let Some(ref best) = best_quant {
        for opt in &mut options {
            if &opt.quant == best { opt.recommended = true; }
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
    }.into()
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
        (bpe * 64.0) as u64  // 64 = rough bytes/token for an "average" 7-30B model
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
        (VramRecommendation::Fit, "Configuration fits with headroom.".into())
    } else if total <= available_vram_bytes {
        (VramRecommendation::Tight, "Fits, but near the VRAM limit.".into())
    } else if total <= available_vram_bytes * 150 / 100 {
        (VramRecommendation::Risk, "Likely exceeds VRAM.".into())
    } else {
        (VramRecommendation::WontFit, "Significantly over VRAM budget.".into())
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Calibration: RTX 5090 32GB, Qwen3.6-27B Q4_K_M, 8 KV heads, 32 layers,
    // head_dim=128, mmproj=0.8GB, MTP depth 1, fit=1024 → ~212K context @ q8_0 KV
    fn qwen3_27b_arch() -> ModelArch {
        ModelArch {
            n_layers: 32,
            n_kv_heads: 8,
            head_dim: 128,
            mmproj_bytes: 800 * 1024 * 1024,
            mtp_depth: 1,
            expert_fraction: 0.65,
            ..Default::default()
        }
    }

    #[test]
    fn kv_calibration_qwen3_27b() {
        let arch = qwen3_27b_arch();
        let model_bytes = estimate_model_size_bytes(27.0, "q4_k_m");
        // Using RTX 5090 32GB
        let ctx = max_context(model_bytes, &arch, "q8_0", "q8_0", 1, 1024, 0,
            32 * 1024 * 1024 * 1024, 1024, 0.05);
        // Should be in the 180K–240K range
        assert!(ctx >= 180_000 && ctx <= 260_000,
            "Expected ~212K context, got {ctx}");
    }

    #[test]
    fn gemma3_local_attn_substantially_smaller_kv() {
        let dense = ModelArch { n_layers: 62, n_kv_heads: 16, head_dim: 256, ..Default::default() };
        let gemma = ModelArch::gemma3_heuristic(27.0);

        let ctx = 128_000u64;
        let kv_dense = kv_cache_bytes(&dense, ctx, 1, "f16", "f16");
        let kv_gemma = kv_cache_bytes(&gemma, ctx, 1, "f16", "f16");
        // Gemma alternating attention should use substantially less KV
        assert!(kv_gemma < kv_dense / 3,
            "Gemma KV ({kv_gemma}) should be < 1/3 of naive dense ({kv_dense})");
    }

    #[test]
    fn moe_weight_split_proportional() {
        let arch = ModelArch { n_experts: 8, expert_fraction: 0.65, ..Default::default() };
        let model = 46_000_000_000u64; // Mixtral-8x7B ~46GB
        let (vram0, ram0) = moe_weight_split(model, &arch, 0);
        assert_eq!(vram0, model);
        assert_eq!(ram0, 0);

        let (vram4, ram4) = moe_weight_split(model, &arch, 4); // half on CPU
        assert!(ram4 > 0 && vram4 < model);
        assert_eq!(vram4 + ram4, model);
        // ~32.5% of model should be on CPU (0.65 expert frac × 0.5 cpu ratio)
        let expected_ram = (model as f64 * 0.65 * 0.5) as u64;
        let delta = (ram4 as i64 - expected_ram as i64).unsigned_abs();
        assert!(delta < model / 100, "RAM bytes off by more than 1%");
    }

    #[test]
    fn quant_table_has_expected_entries() {
        assert!(find_quant("q4_k_m").is_some());
        assert!(find_quant("iq2_xxs").is_some());
        assert!(find_quant("f16").is_some());
        assert!(find_quant("nonexistent").is_none());
    }

    #[test]
    fn auto_size_returns_reasonable_context() {
        let arch = qwen3_27b_arch();
        let model_bytes = estimate_model_size_bytes(27.0, "q4_k_m");
        let result = auto_size(model_bytes, &arch, 32 * 1024 * 1024 * 1024,
            UseCase::General, 1, 1024);
        assert!(result.context_size >= 100_000,
            "Expected ≥ 100K context on 32GB for 27B Q4_K_M");
        assert_eq!(result.kv_quant_k, "q8_0");
        assert!(!result.scenarios.is_empty());
    }

    #[test]
    fn quant_comparison_table_marks_one_recommended() {
        let arch = ModelArch { n_layers: 32, n_kv_heads: 8, head_dim: 128, ..Default::default() };
        let opts = quant_comparison_table(27.0, &arch, 32 * 1024 * 1024 * 1024,
            UseCase::General, 1);
        let rec: Vec<_> = opts.iter().filter(|o| o.recommended).collect();
        assert_eq!(rec.len(), 1, "Expected exactly one recommended quant");
    }

    #[test]
    fn vram_assertions_work() {
        let est = estimate_vram(4u64 << 30, 4096, "q8_0", 512, 512, false, 0, None, 8u64 << 30);
        assert!(matches!(est.recommendation, VramRecommendation::Fit));

        let est2 = estimate_vram(40u64 << 30, 4096, "q8_0", 512, 512, false, 0, None, 16u64 << 30);
        assert!(matches!(est2.recommendation, VramRecommendation::WontFit));
    }
}
