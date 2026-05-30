/// High-level VRAM/RAM usage estimator for llama-server configurations.
///
/// Inputs:
/// - model_size_bytes: size of the main GGUF model file.
/// - context_size: requested context length.
/// - kv_quant: KV cache quantization (e.g., "f16", "q8_0", "turbo3", etc.).
/// - batch_size / ubatch_size: batching parameters.
/// - speculative_decoding: whether speculative decoding is enabled.
/// - mmproj_size_bytes: size of multimodal projector, if any.
/// - n_cpu_moe: MoE offload parameter (if MoE model).
/// - available_vram_bytes: estimated available VRAM.
///
/// Outputs:
/// - Estimated VRAM and system RAM usage.
/// - A recommendation enum.

#[derive(Debug, Clone, serde::Serialize)]
pub struct VramEstimate {
    pub estimated_vram_bytes: u64,
    pub estimated_ram_bytes: u64,
    pub available_vram_bytes: u64,
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

/// Estimate VRAM usage for the given configuration.
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
    if model_size_bytes == 0 {
        return VramEstimate {
            estimated_vram_bytes: 0,
            estimated_ram_bytes: 0,
            available_vram_bytes,
            recommendation: VramRecommendation::Fit,
            note: "Model size is 0; cannot estimate.".into(),
        };
    }

    // Base: model weights.
    let mut vram = model_size_bytes;

    // KV cache estimate:
    // Heuristic: roughly (context_size * batch_size) * bytes_per_token.
    // bytes_per_token depends on KV quantization.
    let kv_bytes_per_tok = match kv_quant {
        "f16" => 64,
        "q8_0" => 32,
        "q4_0" | "q4_1" => 16,
        "turbo3" => 16,
        _ => 32,
    };
    let effective_batch = batch_size.max(ubatch_size).max(1);
    let kv_est = context_size
        .saturating_mul(effective_batch as u64)
        .saturating_mul(kv_bytes_per_tok);
    vram = vram.saturating_add(kv_est);

    // Speculative decoding adds overhead for draft model and extra KV.
    if speculative_decoding {
        vram = vram.saturating_add(model_size_bytes / 8);
    }

    // Multimodal projector.
    vram = vram.saturating_add(mmproj_size_bytes);

    // MoE: if n_cpu_moe is set, some experts are kept in RAM;
    // a rough heuristic: assume (experts_in_vram / total_experts) * model_size
    // is in VRAM, rest in RAM. For simplicity:
    // - If n_cpu_moe is Some(n): assume (total_experts - n) experts in VRAM.
    // - We don't know total experts; assume a base of 8 for typical MoE.
    let mut ram = 0u64;
    if let Some(cpu_moe) = n_cpu_moe {
        let total_experts = 8;
        let cpu_moe = cpu_moe.max(0) as u64;
        let vram_experts = total_experts - cpu_moe.min(total_experts);
        let vram_moe = model_size_bytes.saturating_mul(vram_experts) / total_experts.max(1);
        let ram_moe = model_size_bytes.saturating_sub(vram_moe);
        vram = vram.saturating_add(vram_moe);
        ram = ram.saturating_add(ram_moe);
    }

    // Ratio checks against available VRAM.
    let (recommendation, note) = if available_vram_bytes == 0 {
        (
            VramRecommendation::Risk,
            "No VRAM information; configuration may or may not fit.".into(),
        )
    } else if vram <= available_vram_bytes.saturating_mul(85) / 100 {
        (
            VramRecommendation::Fit,
            "Configuration fits within available VRAM with headroom.".into(),
        )
    } else if vram <= available_vram_bytes {
        (
            VramRecommendation::Tight,
            "Configuration fits, but VRAM usage is close to the limit. Consider reducing context size or using KV cache quantization.".into(),
        )
    } else if vram <= available_vram_bytes.saturating_mul(150) / 100 {
        (
            VramRecommendation::Risk,
            "Configuration likely exceeds VRAM; expect spillover to system RAM and slower performance. Reduce context size, use a smaller quantization, or lower batch size.".into(),
        )
    } else {
        (
            VramRecommendation::WontFit,
            "Configuration significantly exceeds VRAM; will be very slow or may fail. Reduce model size, context size, and/or use KV cache quantization.".into(),
        )
    };

    VramEstimate {
        estimated_vram_bytes: vram,
        estimated_ram_bytes: ram,
        available_vram_bytes,
        recommendation,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fit_small_model() {
        // 4 GB model, 8 GB VRAM, small context -> Fit
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
        matches!(est.recommendation, VramRecommendation::Fit);
    }

    #[test]
    fn test_tight_large_context() {
        // 12 GB model, 16 GB VRAM, large context -> Tight
        let est = estimate_vram(
            12u64 << 30,
            65536,
            "f16",
            1024,
            1024,
            false,
            0,
            None,
            16u64 << 30,
        );
        matches!(
            est.recommendation,
            VramRecommendation::Tight | VramRecommendation::Risk
        );
    }

    #[test]
    fn test_wontfit_huge_model() {
        // 40 GB model, 16 GB VRAM -> WontFit
        let est = estimate_vram(
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
        matches!(est.recommendation, VramRecommendation::WontFit);
    }

    #[test]
    fn test_moe_increases_vram_when_n_cpu_moe_low() {
        // n_cpu_moe=0 means more experts in VRAM.
        let est0 = estimate_vram(
            16u64 << 30,
            4096,
            "q8_0",
            512,
            512,
            false,
            0,
            Some(0),
            24u64 << 30,
        );
        let est4 = estimate_vram(
            16u64 << 30,
            4096,
            "q8_0",
            512,
            512,
            false,
            0,
            Some(4),
            24u64 << 30,
        );
        assert!(est0.estimated_vram_bytes >= est4.estimated_vram_bytes);
    }
}
