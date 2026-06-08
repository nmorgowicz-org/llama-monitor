//! Architecture-aware VRAM estimator for llama-server configurations.
//!
//! Handles:
//! - Standard full-attention (Llama, Mistral, Qwen, …)
//! - Sliding-window / alternating-attention (Gemma 3/4)
//! - MoE expert offloading (Mixtral, Qwen-MoE, DeepSeek, …)
//! - Multi-Token Prediction heads (DeepSeek-R1 style)
//! - Vision projector (mmproj) VRAM
//! - Pre-download quant comparison table
//! - Auto-size recommendation for a given use case

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
    Reference,  // F16 / F32 — bit-exact
    Excellent,  // Q8_0
    VeryGood,   // Q6_K, Q5_K_M/S, IQ4_XS
    Good,       // Q4_K_M/S, IQ4_NL
    Acceptable, // Q4_0, Q3_K_L/M, IQ3_M/S
    Fair,       // Q3_K_S, IQ3_XS/XXS (MoE-class)
    Reduced,    // Q2_K, IQ2_*
    VeryLow,    // IQ1_*
}

/// All supported quantization levels, from best to most compressed.
#[allow(dead_code)]
pub fn all_quants() -> &'static [QuantInfo] {
    QUANT_TABLE
}

/// Look up a quant by name (case-insensitive).
pub fn find_quant(name: &str) -> Option<&'static QuantInfo> {
    let lower = name.to_ascii_lowercase();
    QUANT_TABLE.iter().find(|q| q.name == lower.as_str())
}

static QUANT_TABLE: &[QuantInfo] = &[
    // Reference
    QuantInfo {
        name: "f32",
        label: "F32",
        bpw: 32.0,
        kv_bpe: 4.0,
        quality: QuantQuality::Reference,
        is_imatrix: false,
        large_moe_only: false,
    },
    QuantInfo {
        name: "f16",
        label: "F16",
        bpw: 16.0,
        kv_bpe: 2.0,
        quality: QuantQuality::Reference,
        is_imatrix: false,
        large_moe_only: false,
    },
    QuantInfo {
        name: "bf16",
        label: "BF16",
        bpw: 16.0,
        kv_bpe: 2.0,
        quality: QuantQuality::Reference,
        is_imatrix: false,
        large_moe_only: false,
    },
    // Lossless / near-lossless
    QuantInfo {
        name: "q8_0",
        label: "Q8_0",
        bpw: 8.5,
        kv_bpe: 1.0,
        quality: QuantQuality::Excellent,
        is_imatrix: false,
        large_moe_only: false,
    },
    // High quality
    QuantInfo {
        name: "q6_k",
        label: "Q6_K",
        bpw: 6.5625,
        kv_bpe: 0.75,
        quality: QuantQuality::VeryGood,
        is_imatrix: false,
        large_moe_only: false,
    },
    QuantInfo {
        name: "q5_k_m",
        label: "Q5_K_M",
        bpw: 5.69,
        kv_bpe: 0.625,
        quality: QuantQuality::VeryGood,
        is_imatrix: false,
        large_moe_only: false,
    },
    QuantInfo {
        name: "q5_k_s",
        label: "Q5_K_S",
        bpw: 5.52,
        kv_bpe: 0.625,
        quality: QuantQuality::VeryGood,
        is_imatrix: false,
        large_moe_only: false,
    },
    QuantInfo {
        name: "q5_0",
        label: "Q5_0",
        bpw: 5.5,
        kv_bpe: 0.625,
        quality: QuantQuality::VeryGood,
        is_imatrix: false,
        large_moe_only: false,
    },
    // Good quality
    QuantInfo {
        name: "q4_k_m",
        label: "Q4_K_M",
        bpw: 4.85,
        kv_bpe: 0.5,
        quality: QuantQuality::Good,
        is_imatrix: false,
        large_moe_only: false,
    },
    QuantInfo {
        name: "q4_k_s",
        label: "Q4_K_S",
        bpw: 4.58,
        kv_bpe: 0.5,
        quality: QuantQuality::Good,
        is_imatrix: false,
        large_moe_only: false,
    },
    QuantInfo {
        name: "q4_0",
        label: "Q4_0",
        bpw: 4.55,
        kv_bpe: 0.5,
        quality: QuantQuality::Acceptable,
        is_imatrix: false,
        large_moe_only: false,
    },
    QuantInfo {
        name: "q4_1",
        label: "Q4_1",
        bpw: 4.7,
        kv_bpe: 0.5,
        quality: QuantQuality::Acceptable,
        is_imatrix: false,
        large_moe_only: false,
    },
    // imatrix high
    QuantInfo {
        name: "iq4_xs",
        label: "IQ4_XS",
        bpw: 4.25,
        kv_bpe: 0.5,
        quality: QuantQuality::VeryGood,
        is_imatrix: true,
        large_moe_only: false,
    },
    QuantInfo {
        name: "iq4_nl",
        label: "IQ4_NL",
        bpw: 4.5,
        kv_bpe: 0.5,
        quality: QuantQuality::Good,
        is_imatrix: true,
        large_moe_only: false,
    },
    // 3-bit range
    QuantInfo {
        name: "q3_k_m",
        label: "Q3_K_M",
        bpw: 3.875,
        kv_bpe: 0.375,
        quality: QuantQuality::Acceptable,
        is_imatrix: false,
        large_moe_only: false,
    },
    QuantInfo {
        name: "q3_k_s",
        label: "Q3_K_S",
        bpw: 3.4375,
        kv_bpe: 0.375,
        quality: QuantQuality::Fair,
        is_imatrix: false,
        large_moe_only: false,
    },
    QuantInfo {
        name: "q3_k_l",
        label: "Q3_K_L",
        bpw: 4.0,
        kv_bpe: 0.375,
        quality: QuantQuality::Acceptable,
        is_imatrix: false,
        large_moe_only: false,
    },
    // imatrix 3-bit
    QuantInfo {
        name: "iq3_m",
        label: "IQ3_M",
        bpw: 3.6875,
        kv_bpe: 0.375,
        quality: QuantQuality::Acceptable,
        is_imatrix: true,
        large_moe_only: false,
    },
    QuantInfo {
        name: "iq3_s",
        label: "IQ3_S",
        bpw: 3.5,
        kv_bpe: 0.375,
        quality: QuantQuality::Fair,
        is_imatrix: true,
        large_moe_only: false,
    },
    QuantInfo {
        name: "iq3_xs",
        label: "IQ3_XS",
        bpw: 3.3125,
        kv_bpe: 0.375,
        quality: QuantQuality::Fair,
        is_imatrix: true,
        large_moe_only: false,
    },
    QuantInfo {
        name: "iq3_xxs",
        label: "IQ3_XXS",
        bpw: 3.0625,
        kv_bpe: 0.375,
        quality: QuantQuality::Fair,
        is_imatrix: true,
        large_moe_only: false,
    },
    // 2-bit range — meaningful mainly for very large MoE with heavy CPU offload
    QuantInfo {
        name: "q2_k",
        label: "Q2_K",
        bpw: 2.625,
        kv_bpe: 0.25,
        quality: QuantQuality::Reduced,
        is_imatrix: false,
        large_moe_only: false,
    },
    QuantInfo {
        name: "iq2_m",
        label: "IQ2_M",
        bpw: 2.6875,
        kv_bpe: 0.25,
        quality: QuantQuality::Reduced,
        is_imatrix: true,
        large_moe_only: true,
    },
    QuantInfo {
        name: "iq2_s",
        label: "IQ2_S",
        bpw: 2.5,
        kv_bpe: 0.25,
        quality: QuantQuality::Reduced,
        is_imatrix: true,
        large_moe_only: true,
    },
    QuantInfo {
        name: "iq2_xs",
        label: "IQ2_XS",
        bpw: 2.3125,
        kv_bpe: 0.25,
        quality: QuantQuality::Reduced,
        is_imatrix: true,
        large_moe_only: true,
    },
    QuantInfo {
        name: "iq2_xxs",
        label: "IQ2_XXS",
        bpw: 2.0625,
        kv_bpe: 0.25,
        quality: QuantQuality::Reduced,
        is_imatrix: true,
        large_moe_only: true,
    },
    // 1-bit — experimental
    QuantInfo {
        name: "iq1_m",
        label: "IQ1_M",
        bpw: 1.75,
        kv_bpe: 0.125,
        quality: QuantQuality::VeryLow,
        is_imatrix: true,
        large_moe_only: true,
    },
    QuantInfo {
        name: "iq1_s",
        label: "IQ1_S",
        bpw: 1.5625,
        kv_bpe: 0.125,
        quality: QuantQuality::VeryLow,
        is_imatrix: true,
        large_moe_only: true,
    },
    // Unsloth Ternary Quant (TQ) — unique to Unsloth's UD pipeline
    QuantInfo {
        name: "tq1_0",
        label: "TQ1_0",
        bpw: 1.69,
        kv_bpe: 0.125,
        quality: QuantQuality::VeryLow,
        is_imatrix: true,
        large_moe_only: true,
    },
    QuantInfo {
        name: "tq2_0",
        label: "TQ2_0",
        bpw: 2.0,
        kv_bpe: 0.25,
        quality: QuantQuality::Reduced,
        is_imatrix: true,
        large_moe_only: true,
    },
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
    fn standard_heuristic(param_b: f64) -> Self {
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
    fn gemma3_heuristic(param_b: f64) -> Self {
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
        // Only match explicit "26B-A4B" — a bare "a4b" can appear in unrelated
        // fine-tune tags (e.g. "ablated-a4b-v2"). The param_b fallback at line 654
        // covers unnamed ~26B Gemma4 models.
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
        ) = if is_e2b {
            (35u32, 7u32, 1u32, 1u32, 512u32, 0u32, 0u32)
        } else if is_e4b {
            (42, 7, 2, 2, 512, 0, 0)
        } else if is_12b {
            (48, 8, 1, 8, 1024, 0, 0)
        } else if named_26b_a4b || (!has_named_size && param_b < 30.0) {
            // "A4B" is active parameter count. GGUF metadata confirms:
            // block_count=30, pattern 6×(5 local + 1 global), experts=128, used=8.
            (30, 5, 2, 8, 1024, 128, 8)
        } else {
            (60, 10, 4, 16, 1024, 0, 0)
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
        Self {
            n_layers,
            n_kv_heads: 4,
            head_dim: 256,
            n_attn_layers,
            linear_attn_state_bytes: linear_state,
            expert_fraction: 0.65,
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
        Self {
            n_layers: 40,
            n_kv_heads: 2,
            head_dim: 256,
            n_attn_layers: 10,
            linear_attn_state_bytes: linear_state,
            n_experts: 256,
            n_experts_used: 9,     // 8 routed + 1 shared
            expert_fraction: 0.85, // most params live in expert FFNs
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
        Self {
            n_layers,
            n_kv_heads: 2,
            head_dim: 256,
            n_attn_layers,
            linear_attn_state_bytes: linear_state,
            n_experts: 256,
            n_experts_used: 9, // 8 routed + 1 shared
            expert_fraction: 0.85,
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
        Self {
            n_layers: 48,
            n_kv_heads: 2,
            head_dim: 256,
            n_attn_layers: 12, // only these 12 layers use KV cache
            linear_attn_state_bytes: deltanet_state,
            n_experts: 512,
            n_experts_used: 11,    // 10 routed + 1 shared
            expert_fraction: 0.92, // nearly all params are in expert FFNs (80B/3B ratio)
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

/// Fixed GPU context + compute-buffer overhead (CUDA/Metal/ROCm).
/// Scales slightly with ubatch_size.
pub fn gpu_overhead_bytes(ubatch_size: u32) -> u64 {
    // 300 MB base (CUDA context, KV allocator metadata, etc.)
    // + approx 0.15 MB per ubatch unit above 512
    let base = 300 * 1024 * 1024;
    let ubatch_extra = ((ubatch_size.saturating_sub(512)) as u64) * 150 * 1024;
    base + ubatch_extra
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
    let overhead = gpu_overhead_bytes(ubatch_size);
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
) -> u64 {
    if available_vram_bytes == 0 {
        return 0;
    }
    let (weight_vram, _) = moe_weight_split(model_size_bytes, arch, n_cpu_moe);
    let mmproj = arch.mmproj_bytes;
    let mtp = mtp_overhead_bytes(model_size_bytes, arch.mtp_depth);
    let linear_state = arch.linear_attn_state_bytes; // constant; doesn't scale with context
    let overhead = gpu_overhead_bytes(ubatch_size);
    let fixed = weight_vram + mmproj + mtp + linear_state + overhead;

    let usable = (available_vram_bytes as f64 * (1.0 - headroom_fraction)) as u64;
    if fixed >= usable {
        return 0;
    }
    let kv_budget = usable - fixed;

    // Binary search for context such that kv_cache_bytes(ctx) ≤ kv_budget
    // For non-sliding-window models we can solve directly; for Gemma we binary-search.
    let mut ctx = if arch.has_local_attn() {
        binary_search_context(arch, ctk, ctv, parallel_slots, kv_budget)
    } else {
        direct_max_context(arch, ctk, ctv, parallel_slots, kv_budget)
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
    // If even the minimum context doesn't fit, report zero rather than returning 512 and OOMing.
    if kv_cache_bytes(arch, lo, parallel_slots, ctk, ctv) > kv_budget {
        return 0;
    }
    let mut hi = 2_097_152u64; // 2M upper bound
    while lo + 1 < hi {
        let mid = lo + (hi - lo) / 2;
        let cost = kv_cache_bytes(arch, mid, parallel_slots, ctk, ctv);
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
        let ctx = max_context(
            model_bytes,
            &arch,
            "q8_0",
            "q8_0",
            1,
            1024,
            0,
            32 * 1024 * 1024 * 1024,
            1024,
            0.05,
            None,
        );
        // Should be in the 180K–240K range
        assert!(
            ctx >= 180_000 && ctx <= 260_000,
            "Expected ~212K context, got {ctx}"
        );
    }

    #[test]
    fn gemma3_local_attn_substantially_smaller_kv() {
        let dense = ModelArch {
            n_layers: 62,
            n_kv_heads: 16,
            head_dim: 256,
            ..Default::default()
        };
        let gemma = ModelArch::gemma3_heuristic(27.0);

        let ctx = 128_000u64;
        let kv_dense = kv_cache_bytes(&dense, ctx, 1, "f16", "f16");
        let kv_gemma = kv_cache_bytes(&gemma, ctx, 1, "f16", "f16");
        // Gemma alternating attention should use substantially less KV
        assert!(
            kv_gemma < kv_dense / 3,
            "Gemma KV ({kv_gemma}) should be < 1/3 of naive dense ({kv_dense})"
        );
    }

    #[test]
    fn moe_weight_split_proportional() {
        let arch = ModelArch {
            n_layers: 8, // Mixtral-8x7B has 32 layers; use 8 here so n_cpu_moe=4 = half
            n_experts: 8,
            expert_fraction: 0.65,
            ..Default::default()
        };
        let model = 46_000_000_000u64; // Mixtral-8x7B ~46GB
        let (vram0, ram0) = moe_weight_split(model, &arch, 0);
        assert_eq!(vram0, model);
        assert_eq!(ram0, 0);

        let (vram4, ram4) = moe_weight_split(model, &arch, 4); // 4 of 8 layers = half on CPU
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
        let result = auto_size(
            model_bytes,
            &arch,
            32 * 1024 * 1024 * 1024,
            UseCase::General,
            1,
            1024,
            false, // not unified memory in this test
            None,  // no training context cap in test
        );
        assert!(
            result.context_size >= 100_000,
            "Expected ≥ 100K context on 32GB for 27B Q4_K_M"
        );
        assert_eq!(result.kv_quant_k, "q8_0");
        assert!(!result.scenarios.is_empty());
    }

    #[test]
    fn quant_comparison_table_marks_one_recommended() {
        let arch = ModelArch {
            n_layers: 32,
            n_kv_heads: 8,
            head_dim: 128,
            ..Default::default()
        };
        let opts = quant_comparison_table(
            27.0,
            &arch,
            "Qwen3.6-27B-Q4_K_M.gguf",
            32 * 1024 * 1024 * 1024,
            UseCase::General,
            1,
            false,
        );
        let rec: Vec<_> = opts.iter().filter(|o| o.recommended).collect();
        assert_eq!(rec.len(), 1, "Expected exactly one recommended quant");
    }

    // ── Ground-truth architecture lookup tests ────────────────────────────────
    // Validated against actual HuggingFace model cards (unsloth/, meta-llama/, etc.)

    #[test]
    fn qwen3_30b_a3b_is_standard_moe_not_deltanet() {
        // Qwen3-30B-A3B: standard transformer + MoE. NOT hybrid DeltaNet.
        // Source: unsloth/Qwen3-30B-A3B-GGUF model card.
        // 48 layers, 32 Q / 4 KV heads, 128 experts total, 8 active.
        let arch = ModelArch::from_name_and_params("Qwen3-30B-A3B-Instruct-GGUF", 30.0);
        // Should be MoE
        assert!(arch.is_moe(), "Qwen3-30B-A3B must be flagged MoE");
        // Should NOT be hybrid (no n_attn_layers < n_layers)
        assert!(
            !arch.is_hybrid_attn(),
            "Qwen3-30B-A3B is standard transformer, not hybrid DeltaNet"
        );
        assert_eq!(
            arch.linear_attn_state_bytes, 0,
            "No DeltaNet state for Qwen3-30B-A3B"
        );
    }

    #[test]
    fn qwen3_235b_a22b_is_standard_moe() {
        // Qwen3-235B-A22B: standard transformer + MoE.
        // Source: unsloth/Qwen3-235B-A22B-GGUF model card.
        // 94 layers, 64 Q / 4 KV heads, 128 experts total, 8 active.
        let arch = ModelArch::from_name_and_params("Qwen3-235B-A22B-GGUF", 235.0);
        assert!(arch.is_moe(), "Qwen3-235B-A22B must be MoE");
        assert!(
            !arch.is_hybrid_attn(),
            "Qwen3-235B-A22B is standard transformer"
        );
    }

    #[test]
    fn qwen36_27b_is_hybrid_deltanet() {
        // Qwen3.6-27B: hybrid DeltaNet + dense FFN.
        // Source: davidau 40B model card citing base arch.
        // 64 total layers, 16 standard attention (1/4), 48 DeltaNet (3/4).
        // 4 KV heads, head_dim 256.
        let arch = ModelArch::from_name_and_params("Qwen3.6-27B-Instruct-GGUF", 27.0);
        assert!(arch.is_hybrid_attn(), "Qwen3.6-27B must be hybrid DeltaNet");
        assert_eq!(
            arch.n_attn_layers, 16,
            "Qwen3.6-27B has 16 standard attn layers"
        );
        assert_eq!(arch.n_layers, 64, "Qwen3.6-27B has 64 total layers");
        assert_eq!(arch.n_kv_heads, 4, "Qwen3.6-27B has 4 KV heads");
        assert_eq!(arch.head_dim, 256, "Qwen3.6-27B head_dim is 256");
        assert!(!arch.is_moe(), "Base Qwen3.6-27B is dense");
    }

    #[test]
    fn qwen36_27b_kv_cache_uses_only_attn_layers() {
        // The critical correctness check: KV cache is for 16 layers, NOT 64.
        let arch = ModelArch::from_name_and_params("Qwen3.6-27B-Instruct-GGUF", 27.0);
        let kv_128k = kv_cache_bytes(&arch, 128_000, 1, "f16", "f16");
        // Expected: 16 attn layers × 2 × 4 KV heads × 256 head_dim × 2 bytes × 128K tokens
        // = 16 × 2 × 4 × 256 × 2 × 128,000 = 3,355,443,200 bytes ≈ 3.1 GB
        let expected = 16u64 * 2 * 4 * 256 * 2 * 128_000;
        assert_eq!(
            kv_128k, expected,
            "Qwen3.6-27B KV at 128K should use only 16 attn layers, not 64"
        );
        // Naive (wrong) calculation would give 4× more: 64 layers × same = 12.6 GB
        let naive_wrong = 64u64 * 2 * 4 * 256 * 2 * 128_000;
        assert!(
            kv_128k < naive_wrong / 3,
            "Correct KV ({kv_128k}) should be < 1/3 of naive calculation ({naive_wrong})"
        );
    }

    #[test]
    fn davidau_40b_expansion_gets_96_layers() {
        // DavidAU's 40B expansion of Qwen3.6-27B: 96 layers (64 × 1.5).
        // Source: DavidAU model card.
        let arch = ModelArch::from_name_and_params(
            "Qwen3.6-40B-Claude-4.6-Opus-Deckard-Heretic-Uncensored-Thinking-NEO-CODE-Di-IMatrix-MAX",
            40.0,
        );
        assert!(
            arch.is_hybrid_attn(),
            "40B expansion should be hybrid DeltaNet"
        );
        assert_eq!(arch.n_layers, 96, "40B expansion has 96 layers");
        assert_eq!(
            arch.n_attn_layers, 24,
            "40B expansion has 24 standard attn layers"
        );
        assert_eq!(arch.n_kv_heads, 4, "Same KV head config as base");
    }

    #[test]
    fn qwen3_coder_next_has_512_experts_and_12_attn_layers() {
        // Qwen3-Coder-Next: 80B/3B, 48 layers (12 attn + 36 DeltaNet), 512 experts.
        // Source: unsloth/Qwen3-Coder-Next-GGUF model card.
        let arch = ModelArch::from_name_and_params("Qwen3-Coder-Next-GGUF", 80.0);
        assert!(arch.is_hybrid_attn(), "Coder-Next must be hybrid DeltaNet");
        assert_eq!(arch.n_layers, 48);
        assert_eq!(arch.n_attn_layers, 12);
        assert_eq!(arch.n_experts, 512);
        assert_eq!(arch.n_experts_used, 11);
    }

    #[test]
    fn qwen3_moe_not_confused_with_qwen36() {
        // "Qwen3" without ".6" should NOT get the DeltaNet treatment.
        // Qwen3-30B-A3B is standard transformer + MoE.
        let arch30 = ModelArch::from_name_and_params("bartowski/Qwen3-30B-A3B-GGUF", 30.0);
        assert!(
            !arch30.is_hybrid_attn(),
            "Standard Qwen3 MoE is not hybrid DeltaNet"
        );

        // Qwen3.6 SHOULD get it.
        let arch27 = ModelArch::from_name_and_params("unsloth/Qwen3.6-27B-Instruct-GGUF", 27.0);
        assert!(arch27.is_hybrid_attn(), "Qwen3.6 is hybrid DeltaNet");
    }

    #[test]
    fn llama_70b_is_standard_transformer() {
        // Llama-3.3-70B: standard transformer, 80 layers, 8 KV heads, 128 head_dim.
        // Source: meta-llama/Llama-3.3-70B-Instruct model card.
        let arch = ModelArch::from_name_and_params("Llama-3.3-70B-Instruct-GGUF", 70.0);
        assert!(!arch.is_hybrid_attn(), "Llama-70B is standard transformer");
        assert!(!arch.is_moe(), "Llama-70B is dense");
        assert_eq!(arch.linear_attn_state_bytes, 0, "No DeltaNet state");
        // Standard heuristic for 70B: 80 layers, 8 KV heads, 128 head_dim
        assert_eq!(arch.n_layers, 80);
        assert_eq!(arch.n_kv_heads, 8);
        assert_eq!(arch.head_dim, 128);
    }

    #[test]
    fn vram_assertions_work() {
        let est = estimate_vram(
            4u64 << 30,
            4096,
            "q8_0",
            512,
            512,
            false,
            0,
            None,
            8u64 << 30,
        );
        assert!(matches!(est.recommendation, VramRecommendation::Fit));

        let est2 = estimate_vram(
            40u64 << 30,
            4096,
            "q8_0",
            512,
            512,
            false,
            0,
            None,
            16u64 << 30,
        );
        assert!(matches!(est2.recommendation, VramRecommendation::WontFit));
    }

    // ── Specific model filename parsing tests ─────────────────────────────────

    #[test]
    fn gemma4_31b_dense_gets_alternating_attention() {
        // Source: https://kaitchup.substack.com/p/gemma-4-31b-and-26b-a4b-architecture
        // 60 layers (10 global + 50 local), 1024-token sliding window.
        // Global layers: 4 KV heads, 512 head_dim. Local layers: 16 KV heads, 256 head_dim.
        let arch = ModelArch::from_name_and_params(
            "Gemma-4-Gembrain-31B-it-uncensored-heretic.i1-Q4_K_S.gguf",
            31.0,
        );
        assert!(arch.has_local_attn(), "Gemma-4 should use local attention");
        assert_eq!(arch.head_dim, 256, "Gemma4 local layers use 256 head_dim");
        assert_eq!(
            arch.global_head_dim, 512,
            "Gemma4 global layers use 512 head_dim"
        );
        assert_eq!(
            arch.local_attn_window, 1024,
            "Gemma4 uses 1024-token sliding window"
        );
        assert_eq!(arch.n_layers, 60, "Gemma4-31B has 60 layers");
        assert_eq!(
            arch.n_global_attn_layers, 10,
            "Gemma4-31B has 10 global attention layers"
        );
        assert_eq!(
            arch.n_kv_heads, 4,
            "Gemma4-31B global layers have 4 KV heads"
        );
        assert_eq!(
            arch.local_kv_heads, 16,
            "Gemma4-31B local layers have 16 KV heads"
        );
        assert_eq!(arch.mtp_depth, 0, "31B dense Gemma has no MTP in name");
        assert!(!arch.is_moe(), "31B dense Gemma is not MoE");
    }

    #[test]
    fn gemma4_26b_a4b_gets_moe_and_alternating_attention() {
        // Source: same reference. 30 layers, 128 total experts, 8 active.
        // "A4B" = 4B active PARAMETERS — not 4 active experts.
        let arch =
            ModelArch::from_name_and_params("gemma-4-26B-A4B-it-heretic-ara.Q5_K_XL.gguf", 26.0);
        assert!(
            arch.has_local_attn(),
            "Gemma-4 MoE should use local attention"
        );
        assert!(arch.is_moe(), "26B-A4B should be MoE");
        assert_eq!(arch.n_experts, 128, "Gemma4-26B-A4B has 128 total experts");
        assert_eq!(
            arch.n_experts_used, 8,
            "Gemma4-26B-A4B has 8 active experts"
        );
        assert_eq!(
            arch.local_attn_window, 1024,
            "Gemma4 uses 1024-token sliding window"
        );
        assert_eq!(arch.n_layers, 30, "Gemma4-26B-A4B has 30 layers");
        assert_eq!(
            arch.n_global_attn_layers, 5,
            "Gemma4-26B-A4B has 5 global layers"
        );
        assert_eq!(
            arch.n_kv_heads, 2,
            "Gemma4-26B-A4B global layers have 2 KV heads"
        );
        assert_eq!(
            arch.local_kv_heads, 8,
            "Gemma4-26B-A4B local layers have 8 KV heads"
        );
    }

    #[test]
    fn gemma4_26b_a4b_kv_256k_q8_approx() {
        // Gemma4-26B-A4B (confirmed from config):
        //  - 30 layers: 5 global (full ctx) + 25 local sliding-window
        //  - global: 2 KV heads, head_dim=512
        //  - local: 8 KV heads, head_dim=256, window=1024
        //  - Local layers only keep up to 1024 tokens in KV cache.
        // At 256k context, q8_0 KV, 1 slot:
        //   Global: 5 × 2 × 512 × 262144 × 2 × 1 = 2,684,354,560
        //   Local:  25 × 8 × 256 × 1024  × 2 × 1 =   104,857,600
        //   Total ≈ 2,789,212,160 ≈ 2.60 GiB
        let arch = ModelArch::from_name_and_params("gemma-4-26B-A4B-it-qat-UD-Q4_K_XL.gguf", 26.0);
        let kv = kv_cache_bytes(&arch, 262_144, 1, "q8_0", "q8_0");
        let gi = kv as f64 / (1_073_741_824.0);

        assert!(
            (2.5..=2.8).contains(&gi),
            "KV for gemma-4-26B-A4B@256k q8 should be ~2.6 GiB, got {:.2} GiB ({})",
            gi,
            kv
        );
    }

    #[test]
    fn gemma4_a4b_tightened_ignores_random_a4b_tag() {
        // The "a4b" pattern in gemma4_heuristic is now tightened:
        // it must include "26b-a4b" / "26b_a4b", not a bare "a4b".
        // A fine-tune named "Gemma-4-26B-ablated-a4b-v2" should NOT be
        // forced into the 26B-A4B MoE profile.
        let arch = ModelArch::from_name_and_params("Gemma-4-26B-ablated-a4b-v2-Q4_K_M.gguf", 26.0);
        // Without explicit "26b-a4b", it falls back to param_b logic
        // (param_b < 30 and no named size → treated as 26B MoE fallback),
        // but the bare "a4b" is not enough on its own.
        // We can validate that the pattern is tightened by checking
        // a model whose name is clearly not the 26B-A4B but contains "a4b".

        // A 31B Gemma4 whose name has a meaningless "a4b" tag:
        let arch2 =
            ModelArch::from_name_and_params("Gemma-4-31B-uncensored-a4b-test-Q8_0.gguf", 31.0);
        // This should get 31B dense arch (60 layers), not 26B-A4B MoE (30 layers)
        assert_eq!(
            arch2.n_layers, 60,
            "31B dense should not be confused with 26B-A4B MoE"
        );
        assert!(
            !arch2.is_moe(),
            "31B should be dense, not MoE, despite 'a4b' tag"
        );
    }

    #[test]
    fn moe_suffix_ignores_small_model_false_positives() {
        // "llama-3-a4b" → total 3 < 7 → should NOT match as MoE.
        let arch = ModelArch::from_name_and_params("meta-llama/llama-3-a4b-test-GGUF", 8.0);
        assert!(
            !arch.is_moe(),
            "llama-3-a4b should not be parsed as MoE (total_b < 7)"
        );
    }

    #[test]
    fn gemma4_12b_is_dense_unified_architecture() {
        let arch = ModelArch::from_name_and_params("gemma-4-12B-it-qat-Q4_0.gguf", 11.95);
        assert!(!arch.is_moe(), "Gemma4-12B is dense, not the 26B MoE");
        assert_eq!(arch.n_layers, 48);
        assert_eq!(arch.n_global_attn_layers, 8);
        assert_eq!(arch.n_kv_heads, 1);
        assert_eq!(arch.local_kv_heads, 8);
        assert_eq!(arch.head_dim, 256);
        assert_eq!(arch.global_head_dim, 512);
        assert_eq!(arch.local_attn_window, 1024);
    }

    #[test]
    fn gemma4_qat_q4_0_is_recommended_as_near_reference_quality() {
        let arch = ModelArch::from_name_and_params("gemma-4-12B-it-qat-Q4_0.gguf", 11.95);
        let opts = quant_comparison_table(
            11.95,
            &arch,
            "gemma-4-12B-it-qat-Q4_0.gguf",
            16 * 1024 * 1024 * 1024,
            UseCase::General,
            1,
            false,
        );
        let q4 = opts.iter().find(|o| o.quant == "q4_0").unwrap();
        assert_eq!(q4.quality, QuantQuality::Excellent);
        assert!(q4.recommended);
        assert!(q4.notes.iter().any(|n| n.contains("QAT target")));
    }

    #[test]
    fn gemma4_31b_kv_cache_uses_global_head_dim() {
        // Gemma4-31B (confirmed from config and architecture analysis):
        //  - 60 layers: 10 global (full ctx) + 50 local sliding-window
        //  - global: 4 KV heads, head_dim=512
        //  - local: 16 KV heads, head_dim=256, window=1024
        //  - Local layers only keep up to 1024 tokens in KV cache.
        // At 128K context, f16 KV, 1 slot:
        //   Global: 10 × 4 × 512 × 128_000 × 2 (K+V) × 2 bytes
        //   Local:  50 × 16 × 256 × 1_024  × 2 × 2 bytes (limited by window)
        let arch = ModelArch::from_name_and_params("Gemma-4-31B-it-GGUF", 31.0);
        let kv = kv_cache_bytes(&arch, 128_000, 1, "f16", "f16");
        let global = 10u64 * 4 * 512 * 128_000 * 2 * 2;
        let local = 50u64 * 16 * 256 * 1_024 * 2 * 2;
        assert_eq!(
            kv,
            global + local,
            "Gemma4-31B KV must use global_head_dim=512 and sliding-window for local layers"
        );
    }

    #[test]
    fn qwen3_27b_mtp_gets_mtp_depth() {
        let arch = ModelArch::from_name_and_params(
            "Qwen3.6-27B-uncensored-heretic-v2-Native-MTP-Preserved-Q4_K_S.gguf",
            27.0,
        );
        assert_eq!(arch.mtp_depth, 1, "MTP in filename should set mtp_depth=1");
        assert!(!arch.is_moe(), "27B dense should not be MoE");
    }

    #[test]
    fn qwen3_35b_a3b_gets_moe() {
        // Source: Qwen/Qwen3.6-35B-A3B HF model card.
        // 40 layers (10 attn + 30 DeltaNet), 2 KV heads, 256 experts, 9 active.
        // "A3B" = 3B active PARAMETERS — not 3 active experts.
        let arch =
            ModelArch::from_name_and_params("Qwen3.6-35B-A3B-uncensored-heretic-Q4_K_M.gguf", 35.0);
        assert!(arch.is_moe(), "35B-A3B should be MoE");
        assert!(arch.is_hybrid_attn(), "35B-A3B is hybrid DeltaNet");
        assert_eq!(arch.n_layers, 40, "35B-A3B has 40 total layers");
        assert_eq!(
            arch.n_attn_layers, 10,
            "35B-A3B has 10 standard attention layers"
        );
        assert_eq!(arch.n_kv_heads, 2, "35B-A3B has 2 KV heads");
        assert_eq!(arch.n_experts, 256, "35B-A3B has 256 total experts");
        assert_eq!(
            arch.n_experts_used, 9,
            "35B-A3B has 9 active experts (8 routed + 1 shared)"
        );
    }

    #[test]
    fn qwen35_122b_a10b_is_hybrid_deltanet() {
        // Source: unsloth/Qwen3.5-122B-A10B-MTP-GGUF model card.
        // 48 layers (12 attn + 36 DeltaNet), 2 KV heads, 256 experts, 9 active.
        // "A10B" = 10B active PARAMETERS — not 10 active experts.
        let arch = ModelArch::from_name_and_params("Qwen3.5-122B-A10B-MTP-GGUF", 122.0);
        assert!(
            arch.is_hybrid_attn(),
            "Qwen3.5-122B must be hybrid DeltaNet"
        );
        assert!(arch.is_moe(), "122B-A10B must be MoE");
        assert_eq!(arch.n_layers, 48, "122B has 48 total layers");
        assert_eq!(
            arch.n_attn_layers, 12,
            "122B has 12 standard attention layers"
        );
        assert_eq!(arch.n_kv_heads, 2, "122B has 2 KV heads");
        assert_eq!(arch.n_experts, 256, "122B has 256 total experts");
        assert_eq!(
            arch.n_experts_used, 9,
            "122B has 9 active experts (8 routed + 1 shared)"
        );
        assert_eq!(arch.mtp_depth, 1, "MTP in name sets mtp_depth=1");
    }

    #[test]
    fn qwopus_122b_a10b_large_moe_iq3s() {
        let arch = ModelArch::from_name_and_params(
            "Qwopus3.5-122B-A10B-Kimi-K2.6-distill-abliterated.i1-IQ3_S.gguf",
            122.0,
        );
        assert!(arch.is_moe(), "122B-A10B should be MoE");
        // Should have many experts (128 for 122B+ MoE)
        assert!(
            arch.n_experts >= 64,
            "Large MoE should have ≥64 experts in heuristic"
        );

        // On 32GB VRAM, IQ3_S at 122B needs heavy CPU offload
        let model_bytes = estimate_model_size_bytes(122.0, "iq3_s");
        let vram_32gb = 32u64 * 1024 * 1024 * 1024;

        // Verify that with n_cpu_moe auto-sizing, it fits on 32GB
        let result = auto_size(
            model_bytes,
            &arch,
            vram_32gb,
            UseCase::General,
            1,
            1024,
            false,
            None,
        );
        // Should recommend substantial CPU offload
        assert!(
            result.n_cpu_moe.unwrap_or(0) > 0,
            "Large 122B MoE should need CPU offload on 32GB"
        );
    }

    #[test]
    fn gemma_alternating_attention_kv_much_less_than_dense() {
        // Verify the Gemma alternating attention design is more memory-efficient than a
        // naive dense transformer with many KV heads and full context.
        // With our corrected formula (full KV allocation for local layers too),
        // KV is larger than with window-only optimization but still significantly
        // better than a dense baseline with more heads and layers.
        let arch_gemma = ModelArch::gemma3_heuristic(27.0);
        let arch_dense = ModelArch {
            n_layers: 62,
            n_kv_heads: 16,
            head_dim: 256,
            ..Default::default()
        };
        let ctx = 128_000u64;
        let kv_gemma = kv_cache_bytes(&arch_gemma, ctx, 1, "f16", "f16");
        let kv_dense = kv_cache_bytes(&arch_dense, ctx, 1, "f16", "f16");

        // The dense baseline must be meaningfully larger.
        // We rely on that relative gap instead of a hard absolute cap that was
        // tied to the earlier sliding-window-only approximation.
        assert!(
            kv_dense > kv_gemma * 2,
            "Dense naive calculation should be > 2× Gemma's alternating attention KV"
        );
    }

    #[test]
    fn exaone45_33b_has_correct_arch() {
        // Source: https://huggingface.co/LGAI-EXAONE/EXAONE-4.5-33B
        // 64 layers, 16 × (3 SWA + 1 global), 8 KV heads uniform,
        // head_dim 128, 4096-token sliding window, 1 MTP head.
        let arch = ModelArch::from_name_and_params("EXAONE-4.5-33B-Q4_K_M.gguf", 33.0);
        assert!(!arch.is_moe(), "EXAONE 4.5-33B is dense");
        assert!(!arch.is_hybrid_attn(), "EXAONE 4.5 is SWA not DeltaNet");
        assert!(
            arch.has_local_attn(),
            "EXAONE 4.5 has sliding-window attention"
        );
        assert_eq!(arch.n_layers, 64);
        assert_eq!(arch.n_kv_heads, 8);
        assert_eq!(arch.head_dim, 128);
        assert_eq!(arch.n_global_attn_layers, 16, "16 full-context layers");
        assert_eq!(arch.local_attn_window, 4096, "4096-token sliding window");
        assert_eq!(arch.local_kv_heads, 8, "same KV heads for local layers");
        assert_eq!(arch.mtp_depth, 1, "1 MTP head");
        assert!(
            arch.mmproj_bytes > 2_000_000_000,
            "vision encoder mmproj ≈ 2.58 GB"
        );
    }
}
