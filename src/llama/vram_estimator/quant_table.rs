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
