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

    // ── Sliding-window global head dimension (Gemma 4 style) ────────────────
    /// Head dimension for global (full-context) attention layers.
    /// When > 0, global layers use this dimension instead of `head_dim`.
    /// Gemma 4 uses 512 for global layers vs 256 for local sliding-window layers.
    pub global_head_dim: u32,

    // ── Hybrid linear attention (Qwen3-Coder-Next / DeltaNet style) ──────────
    /// Layers that use traditional softmax attention with a KV cache.
    /// 0 = all layers use KV cache (standard transformer).
    /// For hybrid models (e.g. DeltaNet + Attention), set this to the count
    /// of standard-attention-only layers; the rest use a fixed recurrent state.
    pub n_attn_layers: u32,
    /// Constant recurrent state size in bytes for non-KV linear attention layers.
    /// Independent of context length — does not grow with sequence length.
    /// 0 = not applicable.
    pub linear_attn_state_bytes: u64,

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

    // ── Hidden dimension (for CUDA compute buffer estimation) ────────────────
    /// Model embedding / hidden dimension (n_embd).
    /// Used to estimate CUDA compute buffer size: n_layers × n_embd × ubatch × 2 bytes × 4.
    /// 0 = unknown; CUDA overhead formula falls back to Metal-style flat estimate.
    /// Populated from GGUF `embedding_length` when introspecting a local file;
    /// heuristics set it for the most common families.
    pub n_embd: u32,
}

fn default_expert_fraction() -> f64 {
    0.65
}

impl ModelArch {
    /// Return a reasonable architecture heuristic from model name + parameter count.
    /// Used when the model hasn't been introspected yet (pre-download advisor).
    pub fn from_name_and_params(name: &str, param_b: f64) -> Self {
        let lower = name.to_ascii_lowercase();

        // ── EXAONE 4.5 family (dense, hybrid sliding-window + global attention) ──
        // 33B: 64 layers, 16 × (3 SWA + 1 global), 8 KV heads uniform,
        // head_dim 128, 4096-token sliding window, 1 MTP head.
        // Multimodal: 1.29B vision encoder (mmproj BF16 ≈ 2.58 GB).
        // Source: https://huggingface.co/LGAI-EXAONE/EXAONE-4.5-33B
        if lower.contains("exaone-4.5") || lower.contains("exaone4.5") {
            return Self::exaone45_heuristic(param_b);
        }

        // ── Special-case: Qwen3-Coder-Next (hybrid DeltaNet + MoE, 80B/3B) ──────
        // 48 layers (12 standard attn + 36 DeltaNet), 512 experts, native 262K.
        if lower.contains("coder-next") || lower.contains("qwen3-coder-next") {
            return Self::qwen3_coder_next_arch();
        }

        // ── Qwen3.6 family (hybrid DeltaNet, 1/4 standard attention layers) ─────
        // 27B (dense): 64 layers, 4 KV heads.
        // 35B-A3B (MoE): 40 layers, 2 KV heads, 256 experts, 9 active.
        //   "A3B" = 3B active PARAMETERS, not 3 experts.
        // davidau 40B expansion: 96 layers, 4 KV heads.
        // Also covers Qwopus3.6 and other Qwen3.6 derivatives.
        if lower.contains("qwen3.6")
            || lower.contains("qwen3-6")
            || lower.contains("qwopus3.6")
            || lower.contains("qwopus3-6")
            || lower.contains("qwopus36")
        {
            let is_35b_a3b = lower.contains("35b-a3b") || lower.contains("35b_a3b");
            let mut arch = if is_35b_a3b {
                Self::qwen36_35b_a3b_arch()
            } else {
                Self::qwen36_heuristic(param_b)
            };
            if lower.contains("mtp") || lower.contains("multi-token") {
                arch.mtp_depth = 1;
            }
            return arch;
        }

        // ── Qwen3.5 family (hybrid DeltaNet + MoE) ───────────────────────────────
        // Same 3:1 DeltaNet:Attention ratio as Qwen3.6.
        // 122B-A10B: 48 layers (12 attn + 36 DeltaNet), 256 experts, 9 active.
        // "A10B" = 10B active PARAMETERS, not 10 experts.
        if lower.contains("qwen3.5") || lower.contains("qwen3-5") {
            let mut arch = Self::qwen35_heuristic(param_b);
            if lower.contains("mtp") || lower.contains("multi-token") {
                arch.mtp_depth = 1;
            }
            return arch;
        }

        // ── Gemma 4 family (separate from Gemma 3) ───────────────────────────────
        // Different from Gemma 3: 1024-token sliding window (vs 512),
        // global layers use 512 head_dim + fewer KV heads (4/2 vs 16/8 local).
        // E2B/E4B: 35/42 dense layers, 512-token sliding window.
        // 12B unified: 48 dense layers, 1024-token sliding window.
        // 31B dense: 60 layers, 10 global, 50 local.
        // 26B-A4B MoE: 30 layers, 128 experts, 8 routed + 1 shared active.
        let is_gemma4 = lower.contains("gemma-4") || lower.contains("gemma4");
        if is_gemma4 {
            let mut arch = Self::gemma4_heuristic(&lower, param_b);
            if lower.contains("mtp") || lower.contains("multi-token") {
                arch.mtp_depth = 1;
            }
            return arch;
        }

        let is_gemma3 = lower.contains("gemma-3") || lower.contains("gemma3");

        // Detect MoE from "NB-AMB" / "NB_AMB" suffix (e.g. 26B-A4B, 122B-A10B)
        let moe_info = Self::parse_moe_suffix(name);

        // Detect MTP from filename keyword
        let mtp_depth = if lower.contains("mtp") || lower.contains("multi-token") {
            1u32
        } else {
            0
        };

        let mut arch = if is_gemma3 {
            Self::gemma3_heuristic(param_b)
        } else {
            Self::standard_heuristic(param_b)
        };

        arch.mtp_depth = mtp_depth;

        if let Some((total_b, active_b)) = moe_info {
            let sparsity = if total_b > 0.0 {
                active_b / total_b
            } else {
                0.0
            };
            arch.n_experts = if sparsity < 0.05 {
                512 // extremely sparse (Qwen3-Coder-Next style)
            } else if total_b > 100.0 {
                128
            } else if total_b > 50.0 {
                64
            } else if total_b > 20.0 {
                32
            } else {
                8 // Mixtral style
            };
            // n_experts_used = experts activated per token, not "active billions."
            // Derive from sparsity; exact values should come from introspection.
            arch.n_experts_used = if sparsity < 0.05 {
                11
            } else if sparsity <= 0.15 {
                9
            } else {
                8
            };
        }

        arch
    }

    /// Parse "NB-AMB" or "NB_AMB" MoE suffix, returning (total_params_b, active_params_b).
    ///
    /// Guardrails vs previous version:
    /// - total_b >= 7.0: avoids false positives on names like "llama-3-a4b".
    /// - active_b <= total_b: rejects obviously invalid suffixes.
    /// - Takes the last valid pattern (rightmost) to reduce confusion on odd names.
    fn parse_moe_suffix(name: &str) -> Option<(f64, f64)> {
        let src = name.to_ascii_lowercase();
        let bytes = src.as_bytes();
        let len = bytes.len();
        let mut best: Option<(f64, f64)> = None;

        let mut i = 0;
        while i < len {
            // Look for "-a" or "_a"
            if i + 2 < len && (bytes[i] == b'-' || bytes[i] == b'_') && bytes[i + 1] == b'a' {
                // Read digits after 'a' until 'b'
                let start = i + 2;
                let end = bytes[start..]
                    .iter()
                    .position(|&b| b == b'b')
                    .map(|p| start + p);

                if let Some(end_idx) = end {
                    let active_str = &src[start..end_idx];
                    if let Ok(active) = active_str.parse::<f64>() {
                        // Find last "<digits>b" before this marker
                        let before = &src[..i];
                        let total = before.rmatch_indices('b').find_map(|(bi, _)| {
                            let mut num_start = bi;
                            while num_start > 0
                                && before[..num_start]
                                    .chars()
                                    .next_back()
                                    .is_some_and(|c| c.is_ascii_digit() || c == '.')
                            {
                                num_start -= 1;
                            }
                            if num_start < bi {
                                before[num_start..bi].parse::<f64>().ok()
                            } else {
                                None
                            }
                        });

                        if let Some(total) = total {
                            // Enforce reasonableness:
                            // - total_b >= 7.0: avoids "llama-3-a4b" style false positives
                            // - active_b > 0 and <= total_b
                            if total >= 7.0 && active > 0.0 && active <= total {
                                best = Some((total, active));
                            }
                        }
                    }
                }
            }
            i += 1;
        }

        best
    }

    /// Standard full-attention architecture heuristic.
    /// Calibrated against confirmed model cards (meta-llama, unsloth, bartowski).
    pub(crate) fn standard_heuristic(param_b: f64) -> Self {
        // n_layers, n_kv_heads, head_dim
        let (n_layers, n_kv_heads, head_dim) = if param_b < 2.0 {
            (22u32, 4u32, 64u32)
        } else if param_b < 5.0 {
            (28, 4, 128) // Qwen2.5-3B, Phi-3-mini style
        } else if param_b < 10.0 {
            (32, 8, 128) // Llama-3.1-8B, Mistral-7B, Qwen2.5-7B
        } else if param_b < 25.0 {
            (40, 8, 128) // Llama-2-13B, Qwen2.5-14B, Mistral-22B range
        } else if param_b < 35.0 {
            // Qwen3-30B-A3B: 48 layers, 4 KV heads (GQA). Confirmed from HF.
            // Note: Qwen3-30B-A3B is MoE (128 experts, 8 active) — handled by MoE suffix parsing.
            (48, 4, 128)
        } else if param_b < 75.0 {
            // Qwen3-235B uses 94 layers with 4 KV heads; Llama-3.3-70B uses 80 layers, 8 KV.
            // Use Llama-70B as reference for the 70B range.
            // Confirmed from meta-llama/Llama-3.3-70B-Instruct and Qwen3 family docs.
            (80, 8, 128) // Llama-3.1/3.3-70B, Qwen2.5-72B
        } else {
            (94, 4, 128) // Qwen3-235B-A22B+; 4 KV heads confirmed from HF card
        };
        Self {
            n_layers,
            n_kv_heads,
            head_dim,
            expert_fraction: 0.65,
            ..Default::default()
        }
    }

    /// Gemma 3 alternating local/global attention heuristic.
    /// 1-in-6 layers use full global attention; rest use a 512-token sliding window.
    pub(crate) fn gemma3_heuristic(param_b: f64) -> Self {
        let (n_layers, global_kv_heads, head_dim) = if param_b < 5.0 {
            (34u32, 4u32, 256u32) // Gemma-3-4B
        } else if param_b < 14.0 {
            (52, 8, 256) // Gemma-3-12B
        } else {
            (62, 16, 256) // Gemma-3-27B
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
            ..Default::default()
        }
    }

    /// Gemma 4 alternating local/global attention heuristic.
    ///
    /// Key differences from Gemma 3:
    ///   - Sliding window: 1024 tokens (Gemma 3 used 512)
    ///   - Global layers: fewer KV heads but wider head_dim (512 vs 256)
    ///   - Different layer counts and attention patterns per size tier
    ///
    /// Source: https://huggingface.co/google/gemma-4-12B-it-qat-q4_0-unquantized
    fn gemma4_heuristic(name: &str, param_b: f64) -> Self {
        let named_e2b = name.contains("e2b");
        let named_e4b = name.contains("e4b");
        let named_12b = name.contains("12b");
        // Match "26B-A4B" / "26B_A4B" in name. A bare "a4b" tag alone is insufficient
        // (e.g. "31B-uncensored-a4b-test" is dense), but the param_b fallback
        // correctly classifies unnamed ~26B Gemma4 models as MoE.
        let named_26b_a4b = name.contains("26b-a4b") || name.contains("26b_a4b");
        let named_31b = name.contains("31b");
        let has_named_size = named_e2b || named_e4b || named_12b || named_26b_a4b || named_31b;

        let is_e2b = named_e2b || (!has_named_size && param_b < 6.0);
        let is_e4b = named_e4b || (!has_named_size && !is_e2b && param_b < 10.0);
        let is_12b = named_12b || (!has_named_size && !is_e2b && !is_e4b && param_b < 20.0);

        let (
            n_layers,
            global_layers,
            global_kv_heads,
            local_kv_heads,
            local_attn_window,
            n_experts,
            n_experts_used,
            n_embd,
        ) = if is_e2b {
            (35u32, 7u32, 1u32, 1u32, 512u32, 0u32, 0u32, 1152u32)
        } else if is_e4b {
            (42, 7, 2, 2, 512, 0, 0, 2048)
        } else if is_12b {
            // Gemma4-12B dense: 48 layers, n_embd = 3072
            (48, 8, 1, 8, 1024, 0, 0, 3072)
        } else if named_26b_a4b || (!has_named_size && param_b < 30.0) {
            // "A4B" is active parameter count. GGUF metadata confirms:
            // block_count=30, pattern 6×(5 local + 1 global), experts=128, used=8.
            // n_embd=2048 (Google mobile-efficient base; overridden by GGUF when present)
            (30, 5, 2, 8, 1024, 128, 8, 2048)
        } else {
            // Gemma4-31B dense: 60 layers, n_embd = 5120
            (60, 10, 4, 16, 1024, 0, 0, 5120)
        };

        Self {
            n_layers,
            n_kv_heads: global_kv_heads, // KV heads for global (full-context) layers
            head_dim: 256,               // head_dim for local sliding-window layers
            global_head_dim: 512,        // head_dim for global layers (Gemma4 uses wider)
            n_global_attn_layers: global_layers,
            local_attn_window,
            local_kv_heads,
            n_experts,
            n_experts_used,
            expert_fraction: 0.65,
            n_embd,
            ..Default::default()
        }
    }

    /// Qwen3.6 architecture heuristic (27B dense and davidau 40B expansion).
    ///
    /// 3:1 DeltaNet:Attention ratio → exactly 1/4 of layers are standard softmax attention.
    /// 27B: 64 total layers, 16 attn, 4 KV heads, head_dim 256.
    /// Davidau 40B expansion: 96 total layers, 24 attn, same head config.
    ///
    /// Note: 35B-A3B is handled separately by `qwen36_35b_a3b_arch` — it has a
    /// completely different layer count (40) and KV head count (2).
    fn qwen36_heuristic(param_b: f64) -> Self {
        let n_layers: u32 = if param_b > 35.0 { 96 } else { 64 };
        let n_attn_layers = n_layers / 4;
        let n_deltanet = n_layers - n_attn_layers;
        // DeltaNet state: 48 V-heads × 128² × 2 bytes per layer (confirmed for 27B)
        let linear_state = n_deltanet as u64 * 48 * 128 * 128 * 2;
        // n_embd = 5120 confirmed from GGUF embedding_length for both 27B and davidau 40B
        Self {
            n_layers,
            n_kv_heads: 4,
            head_dim: 256,
            n_attn_layers,
            linear_attn_state_bytes: linear_state,
            expert_fraction: 0.65,
            n_embd: 5120,
            ..Default::default()
        }
    }

    /// Qwen3.6-35B-A3B exact architecture (confirmed from HuggingFace model card).
    ///
    /// 40 total layers: 10 × (Gated Attention → MoE) + 30 × (Gated DeltaNet → MoE).
    /// Attention: 16 Q / 2 KV heads, head_dim 256.
    /// DeltaNet: 32 V-heads, 16 QK-heads, head_dim 128.
    /// MoE: 256 total experts, 8 routed + 1 shared = 9 active.
    /// "A3B" = 3 billion active parameters — NOT 3 active experts.
    fn qwen36_35b_a3b_arch() -> Self {
        let n_deltanet = 30u32;
        let linear_state = n_deltanet as u64 * 32 * 128 * 128 * 2; // 32 V-heads
        // n_embd = 4096 estimated (similar to Qwen3-30B-A3B base hidden dim)
        Self {
            n_layers: 40,
            n_kv_heads: 2,
            head_dim: 256,
            n_attn_layers: 10,
            linear_attn_state_bytes: linear_state,
            n_experts: 256,
            n_experts_used: 9,     // 8 routed + 1 shared
            expert_fraction: 0.85, // most params live in expert FFNs
            n_embd: 4096,
            ..Default::default()
        }
    }

    /// Qwen3.5 architecture heuristic (hybrid DeltaNet + MoE).
    ///
    /// Same 3:1 DeltaNet:Attention ratio as Qwen3.6.
    /// Confirmed: 122B-A10B has 48 layers (12 attn + 36 DeltaNet),
    ///   2 KV heads, head_dim 256, 256 total experts, 9 active (8+1).
    /// "A10B" = 10 billion active parameters — NOT 10 active experts.
    fn qwen35_heuristic(param_b: f64) -> Self {
        let n_layers: u32 = if param_b > 80.0 { 48 } else { 40 };
        let n_attn_layers = n_layers / 4;
        let n_deltanet = n_layers - n_attn_layers;
        // DeltaNet V-heads: 64 for 122B (confirmed), estimated 32 for smaller
        let deltanet_v_heads: u64 = if param_b > 80.0 { 64 } else { 32 };
        let linear_state = n_deltanet as u64 * deltanet_v_heads * 128 * 128 * 2;
        // n_embd: 7168 for 122B-A10B (Qwen3-235B class base dim), 4096 for smaller
        let n_embd: u32 = if param_b > 80.0 { 7168 } else { 4096 };
        Self {
            n_layers,
            n_kv_heads: 2,
            head_dim: 256,
            n_attn_layers,
            linear_attn_state_bytes: linear_state,
            n_experts: 256,
            n_experts_used: 9, // 8 routed + 1 shared
            expert_fraction: 0.85,
            n_embd,
            ..Default::default()
        }
    }

    /// EXAONE 4.5 family: dense, hybrid sliding-window + global attention.
    ///
    /// 33B confirmed specs from https://huggingface.co/LGAI-EXAONE/EXAONE-4.5-33B :
    /// 64 layers, pattern 16 × (3 SWA + 1 global) → 16 global + 48 local,
    /// 8 KV heads uniform across both layer types, head_dim 128, 4096-token SWA,
    /// 1 MTP head, multimodal (vision encoder 1.29B params = ~2.58 GB BF16 mmproj).
    fn exaone45_heuristic(param_b: f64) -> Self {
        // Only 33B released so far; table-driven for future variants.
        let (n_layers, n_global, n_kv) = if param_b < 15.0 {
            (32u32, 8u32, 8u32) // hypothetical smaller variant
        } else {
            (64, 16, 8) // 33B confirmed
        };
        // mmproj vision encoder: 1.29B params × 2 bytes (BF16) ≈ 2.58 GB
        let mmproj = if param_b > 20.0 { 2_580_000_000u64 } else { 0 };
        Self {
            n_layers,
            n_kv_heads: n_kv,
            head_dim: 128,
            n_global_attn_layers: n_global,
            local_attn_window: 4096,
            local_kv_heads: n_kv, // same KV head count for both layer types
            mtp_depth: 1,
            mmproj_bytes: mmproj,
            param_b,
            ..Default::default()
        }
    }

    /// Known-exact architecture for Qwen3-Coder-Next.
    ///
    /// Hybrid DeltaNet + MoE: 48 total layers, 12 standard softmax-attention
    /// layers and 36 DeltaNet (linear attention) layers. Only the 12 attention
    /// layers need a traditional KV cache; the DeltaNet layers use a fixed
    /// ~1.3 GB recurrent state regardless of context length.
    fn qwen3_coder_next_arch() -> Self {
        // Standard attention: 16 Q heads, 2 KV heads, head_dim 256
        // DeltaNet recurrent state: 36 layers × 32 V-heads × 128² × 2 bytes ≈ 1.2 GB
        let deltanet_state = 36u64 * 32 * 128 * 128 * 2;
        // n_embd = 7168 (235B-class model, same base hidden dim as Qwen3-235B-A22B)
        Self {
            n_layers: 48,
            n_kv_heads: 2,
            head_dim: 256,
            n_attn_layers: 12, // only these 12 layers use KV cache
            linear_attn_state_bytes: deltanet_state,
            n_experts: 512,
            n_experts_used: 11,    // 10 routed + 1 shared
            expert_fraction: 0.92, // nearly all params are in expert FFNs (80B/3B ratio)
            n_embd: 7168,
            ..Default::default()
        }
    }

    pub fn is_moe(&self) -> bool {
        self.n_experts > 1
    }

    pub fn has_local_attn(&self) -> bool {
        self.local_attn_window > 0 && self.n_global_attn_layers < self.n_layers
    }

    /// True if this is a hybrid linear-attention model (DeltaNet, SSM, etc.)
    /// where only n_attn_layers of n_layers use traditional KV cache.
    pub fn is_hybrid_attn(&self) -> bool {
        self.n_attn_layers > 0 && self.n_attn_layers < self.n_layers
    }
}
