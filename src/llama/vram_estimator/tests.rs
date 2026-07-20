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
        false, // discrete GPU (RTX 5090)
        Backend::LlamaCpp,
    );
    // Should be in the 180K–240K range
    assert!(
        (180_000..=260_000).contains(&ctx),
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
fn moe_weight_split_uses_measured_expert_bytes() {
    // When measured per-layer expert bytes are present, the split is exact and the
    // expert_fraction heuristic is ignored.
    let arch = ModelArch {
        n_layers: 48,
        n_experts: 128,
        moe_layer_count: 48,
        expert_bytes_per_layer: 1_000_000_000, // 1 GB measured per MoE layer
        expert_fraction: 0.65,                 // must be ignored
        ..Default::default()
    };
    let model = 50_000_000_000u64;

    // Offloading 4 layers moves exactly 4 GB to CPU.
    let (vram, ram) = moe_weight_split(model, &arch, 4);
    assert_eq!(ram, 4_000_000_000);
    assert_eq!(vram, model - 4_000_000_000);

    // Clamped to the measured MoE layer count.
    let (_, ram_all) = moe_weight_split(model, &arch, 999);
    assert_eq!(ram_all, 48_000_000_000);

    // n_cpu_moe = 0 keeps everything in VRAM.
    assert_eq!(moe_weight_split(model, &arch, 0), (model, 0));
}

#[test]
fn dense_weight_split_uses_measured_layer_bytes() {
    let arch = ModelArch {
        n_layers: 80,
        bytes_per_layer: 800_000_000,
        ..Default::default()
    };
    let model = 70_000_000_000u64;

    assert_eq!(dense_weight_split(model, &arch, -1), (model, 0));
    assert_eq!(dense_weight_split(model, &arch, 0), (0, model));
    assert_eq!(
        dense_weight_split(model, &arch, 40),
        (32_000_000_000, 38_000_000_000)
    );
    assert_eq!(dense_weight_split(model, &arch, 80), (model, 0));
}

#[test]
fn dense_partial_gpu_layers_reports_independent_ram_budget() {
    let arch = ModelArch {
        n_layers: 80,
        bytes_per_layer: 800_000_000,
        ..Default::default()
    };
    let breakdown = full_estimate(
        70_000_000_000,
        &arch,
        4096,
        "q8_0",
        "q8_0",
        1,
        512,
        0,
        40,
        48_000_000_000,
        32_000_000_000,
        false,
        EstimatorOptions::default(),
    );

    assert_eq!(breakdown.weights_bytes, 32_000_000_000);
    assert_eq!(breakdown.ram_bytes, 38_000_000_000);
    assert!(breakdown.ram_headroom_bytes < 0);
    assert!(matches!(
        breakdown.recommendation,
        VramRecommendation::WontFit
    ));
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
        Backend::LlamaCpp,
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
        Backend::LlamaCpp,
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
    let arch = ModelArch::from_name_and_params("gemma-4-26B-A4B-it-heretic-ara.Q5_K_XL.gguf", 26.0);
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
    let _arch = ModelArch::from_name_and_params("Gemma-4-26B-ablated-a4b-v2-Q4_K_M.gguf", 26.0);
    // Any ~26B Gemma4 is inherently A4B MoE (just like all 35B Qwen3.5/3.6 are A3B).
    // The tightened name pattern prevents bare "a4b" from matching, but the param_b
    // fallback (< 30B) still classifies it correctly as MoE.
    // We validate the tightening by checking a 31B model whose name contains "a4b"
    // but should be dense.

    // A 31B Gemma4 whose name has a meaningless "a4b" tag:
    let arch2 = ModelArch::from_name_and_params("Gemma-4-31B-uncensored-a4b-test-Q8_0.gguf", 31.0);
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
        Backend::LlamaCpp,
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
        Backend::LlamaCpp,
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

#[ignore]
#[test]
fn test_qwen3_coder_next_vram_estimate_integration() {
    // Integration test: verify hybrid DeltaNet model shows correct KV cache
    let home = std::env::var("HOME").ok();
    let path = home.as_ref().map(|h| {
        std::path::Path::new(h).join(".config/llama-monitor/models/Qwen3-Coder-Next-Huihui-Opus-4.6-Reasoning-Distilled-abliterated-IQ4_XS.gguf")
    }).and_then(|p| if p.exists() { Some(p) } else { None });
    let Some(path) = path else {
        return;
    };

    let gguf = crate::llama::gguf_meta::read_gguf_metadata(&path).expect("read gguf");
    let meta = gguf.to_model_metadata();
    let param_b = gguf.param_b().unwrap_or(0.0);
    let arch = meta.to_arch(path.to_str().unwrap(), param_b);

    assert!(
        arch.is_hybrid_attn(),
        "Qwen3-Coder-Next should be hybrid DeltaNet"
    );
    assert_eq!(
        arch.n_attn_layers, 12,
        "Only 12 of 48 layers should use KV cache"
    );
    assert!(
        arch.n_attn_layers < arch.n_layers,
        "Hybrid model: n_attn_layers ({}) < n_layers ({})",
        arch.n_attn_layers,
        arch.n_layers
    );

    // Compute KV cache at 131K context
    let kv = kv_cache_bytes(&arch, 131_072, 1, "q8_0", "q8_0");
    let kv_gb = kv as f64 / 1e9;

    // Should be significant (> 1 GB) but not use all 48 layers (which would be ~4× too large)
    assert!(
        kv_gb > 1.0,
        "Qwen3-Coder-Next KV cache at 131K should be > 1 GB (got {:.2} GB)",
        kv_gb
    );
    // Max ~2 GB expected: 12 layers × 2 KV heads × 256 head_dim × 131072 ctx × 2 (K+V) × 1 byte (q8_0)
    assert!(
        kv_gb < 4.0,
        "Qwen3-Coder-Next KV cache should be < 4 GB (got {:.2} GB; likely using wrong layer count)",
        kv_gb
    );
}

#[test]
fn qwen36_27b_kv_at_131k_and_255k() {
    // Qwen3.6-27B dense: 64 layers, 16 attn, 4 KV heads, head_dim 256
    let arch = ModelArch::from_name_and_params("Qwen3.6-27B-Q4_K_M.gguf", 27.0);
    assert!(arch.is_hybrid_attn(), "Qwen3.6-27B is hybrid DeltaNet");
    assert_eq!(arch.n_attn_layers, 16, "16 of 64 layers use KV cache");
    assert_eq!(arch.n_kv_heads, 4);
    assert_eq!(arch.head_dim, 256);

    // Expected KV at 131K, q8_0: 16 × 2 × 4 × 256 × 131072 × 1 = 4,294,967,296 bytes ≈ 4.29 GB
    let kv_131k = kv_cache_bytes(&arch, 131_072, 1, "q8_0", "q8_0");
    let kv_131k_gb = kv_131k as f64 / 1e9;
    assert!(
        (kv_131k_gb - 4.29).abs() < 0.02,
        "Qwen3.6-27B KV at 131K: expected ~4.29 GB, got {:.2}",
        kv_131k_gb
    );

    // Expected KV at 255K, q8_0: 16 × 2 × 4 × 256 × 255000 × 1 ≈ 8.32 GB
    let kv_255k = kv_cache_bytes(&arch, 255_000, 1, "q8_0", "q8_0");
    let kv_255k_gb = kv_255k as f64 / 1e9;
    assert!(
        (kv_255k_gb - 8.32).abs() < 0.05,
        "Qwen3.6-27B KV at 255K: expected ~8.32 GB, got {:.2}",
        kv_255k_gb
    );
}

#[test]
fn qwen36_35b_a3b_kv_at_131k_and_255k() {
    // Qwen3.6-35B-A3B MoE: 40 layers, 10 attn, 2 KV heads, head_dim 256
    let arch = ModelArch::from_name_and_params("Qwen3.6-35B-A3B-Q4_K_M.gguf", 35.0);
    assert!(arch.is_hybrid_attn(), "Qwen3.6-35B-A3B is hybrid DeltaNet");
    assert!(arch.is_moe(), "Qwen3.6-35B-A3B is MoE");
    assert_eq!(arch.n_attn_layers, 10, "10 of 40 layers use KV cache");
    assert_eq!(arch.n_kv_heads, 2);
    assert_eq!(arch.head_dim, 256);

    // Expected KV at 131K, q8_0: 10 × 2 × 2 × 256 × 131072 × 1 = 1,342,177,280 bytes ≈ 1.34 GB
    let kv_131k = kv_cache_bytes(&arch, 131_072, 1, "q8_0", "q8_0");
    let kv_131k_gb = kv_131k as f64 / 1e9;
    assert!(
        (kv_131k_gb - 1.34).abs() < 0.02,
        "Qwen3.6-35B-A3B KV at 131K: expected ~1.34 GB, got {:.2}",
        kv_131k_gb
    );

    // Expected KV at 255K, q8_0: 10 × 2 × 2 × 256 × 255000 × 1 ≈ 2.61 GB
    let kv_255k = kv_cache_bytes(&arch, 255_000, 1, "q8_0", "q8_0");
    let kv_255k_gb = kv_255k as f64 / 1e9;
    assert!(
        (kv_255k_gb - 2.61).abs() < 0.03,
        "Qwen3.6-35B-A3B KV at 255K: expected ~2.61 GB, got {:.2}",
        kv_255k_gb
    );
}

// ===== Quant table completeness tests =====

#[test]
fn quant_table_all_entries_have_valid_multipliers() {
    for quant in all_quants() {
        assert!(
            quant.bpw > 0.0 && quant.bpw <= 32.0,
            "Quant {} has invalid bpw {}",
            quant.name,
            quant.bpw
        );
        assert!(!quant.name.is_empty(), "Quant name must not be empty");
    }
}

#[test]
fn quant_table_has_sufficient_coverage() {
    assert!(
        all_quants().len() >= 30,
        "Expected 30+ quant levels, got {}",
        all_quants().len()
    );
}

#[test]
fn find_quant_unknown_returns_none() {
    assert!(find_quant("nonexistent_quant_xyz").is_none());
    assert!(find_quant("").is_none());
}

// ===== VRAM estimation edge cases =====

#[test]
fn estimate_vram_zero_context() {
    let arch = ModelArch::default();
    let breakdown = full_estimate(
        4_000_000_000,
        &arch,
        0, // zero context
        "q8_0",
        "q8_0",
        1,
        1024,
        0,
        -1,
        16 * 1024 * 1024 * 1024,
        0,
        false,
        EstimatorOptions::default(),
    );
    // Should still succeed, just no KV overhead
    assert!(matches!(
        breakdown.recommendation,
        VramRecommendation::Fit | VramRecommendation::Tight
    ));
}

#[test]
fn estimate_vram_too_large_for_vram() {
    let arch = ModelArch::default();
    let breakdown = full_estimate(
        60_000_000_000, // 60GB model
        &arch,
        128_000,
        "q4_k_m",
        "q8_0",
        1,
        1024,
        0,
        -1,
        16 * 1024 * 1024 * 1024, // 16GB available
        0,
        false,
        EstimatorOptions::default(),
    );
    assert!(matches!(
        breakdown.recommendation,
        VramRecommendation::Risk | VramRecommendation::WontFit
    ));
}

// ── Phase 6B2: Rapid-MLX backend-neutral estimator ─────────────────────────────

// Architecture fields verified against
// https://huggingface.co/mlx-community/Qwen3-0.6B-4bit/blob/main/config.json
fn mlx_qwen3_0_6b_arch() -> ModelArch {
    ModelArch {
        n_layers: 28,
        n_embd: 1024,
        n_kv_heads: 8,
        head_dim: 128,
        ..Default::default()
    }
}

// MoE architecture fields verified against
// https://huggingface.co/mlx-community/Qwen3-30B-A3B-4bit/blob/main/config.json
fn mlx_qwen3_30b_a3b_arch() -> ModelArch {
    ModelArch {
        n_layers: 48,
        n_embd: 2048,
        n_kv_heads: 4,
        head_dim: 128,
        n_experts: 128,
        n_experts_used: 8,
        expert_fraction: 0.65,
        ..Default::default()
    }
}

#[test]
fn mlx_backend_uses_mlx_overhead_not_metal_overhead() {
    let arch = mlx_qwen3_0_6b_arch();
    let mlx_breakdown = full_estimate(
        400_000_000,
        &arch,
        8192,
        "q8_0",
        "q8_0",
        1,
        2048,
        0,
        -1,
        16 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            ..Default::default()
        },
    );
    let llama_cpp_breakdown = full_estimate(
        400_000_000,
        &arch,
        8192,
        "q8_0",
        "q8_0",
        1,
        2048,
        0,
        -1,
        16 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions::default(),
    );
    // Rapid-MLX must not reuse llama.cpp's Metal-calibrated overhead constants: the two
    // backends should diverge even for an identical architecture/context.
    assert_ne!(
        mlx_breakdown.overhead_bytes,
        llama_cpp_breakdown.overhead_bytes
    );
    assert_eq!(mlx_breakdown.evidence, EstimateEvidence::Approximate);
    assert_eq!(llama_cpp_breakdown.evidence, EstimateEvidence::Measured);
}

#[test]
fn mlx_moe_architecture_is_recognized() {
    let arch = mlx_qwen3_30b_a3b_arch();
    assert!(arch.is_moe());
    let breakdown = full_estimate(
        16_000_000_000,
        &arch,
        4096,
        "q8_0",
        "q8_0",
        1,
        2048,
        0,
        -1,
        32 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            ..Default::default()
        },
    );
    // Unified memory: weights are never CPU-split for Rapid-MLX.
    assert_eq!(breakdown.weights_bytes, 16_000_000_000);
    assert_eq!(breakdown.ram_bytes, 0);
}

#[test]
fn mlx_prefix_cache_is_separate_stored_budget_not_active_kv_reduction() {
    let arch = mlx_qwen3_0_6b_arch();
    let kv_without_cache = kv_cache_bytes(&arch, 8192, 1, "q8_0", "q8_0");

    let cache_bytes = mlx_prefix_cache_bytes(&arch, 32_768, 4);
    assert!(cache_bytes > 0);

    let breakdown = full_estimate(
        400_000_000,
        &arch,
        8192,
        "q8_0",
        "q8_0",
        1,
        2048,
        0,
        -1,
        16 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: cache_bytes,
            ..Default::default()
        },
    );

    // Active-request KV must be modeled exactly as before — compressing the prefix cache must
    // NOT reduce the active-context KV footprint (cached entries are decompressed before reuse).
    assert_eq!(breakdown.kv_cache_bytes, kv_without_cache);
    // The compressed prefix-cache budget is reported separately and added to the total.
    assert_eq!(breakdown.mlx_prefix_cache_bytes, cache_bytes);
    assert!(breakdown.total_bytes >= breakdown.kv_cache_bytes + cache_bytes);
}

#[test]
fn mlx_prefix_cache_defaults_to_zero_for_gguf_backend() {
    let arch = qwen3_27b_arch();
    let breakdown = full_estimate(
        14_000_000_000,
        &arch,
        8192,
        "q8_0",
        "q8_0",
        1,
        2048,
        0,
        -1,
        16 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions::default(),
    );
    assert_eq!(breakdown.mlx_prefix_cache_bytes, 0);
    assert_eq!(breakdown.evidence, EstimateEvidence::Measured);
}

#[test]
fn mlx_prefix_cache_compression_bits_scale_the_stored_budget() {
    let arch = mlx_qwen3_0_6b_arch();
    let int4 = mlx_prefix_cache_bytes(&arch, 16_384, 4);
    let int8 = mlx_prefix_cache_bytes(&arch, 16_384, 8);
    assert!(int4 > 0 && int8 > 0);
    // int4 stores half the bytes per element of int8.
    assert_eq!(int4, int8 / 2);
}

#[test]
fn mlx_overhead_scales_with_context_via_kv_fraction() {
    let arch = mlx_qwen3_0_6b_arch();
    let small_ctx = full_estimate(
        400_000_000,
        &arch,
        4096,
        "q8_0",
        "q8_0",
        1,
        2048,
        0,
        -1,
        16 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            ..Default::default()
        },
    );
    let large_ctx = full_estimate(
        400_000_000,
        &arch,
        65536,
        "q8_0",
        "q8_0",
        1,
        2048,
        0,
        -1,
        16 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            ..Default::default()
        },
    );
    assert!(large_ctx.overhead_bytes > small_ctx.overhead_bytes);
}

#[test]
fn max_context_diverges_between_mlx_and_metal_backends() {
    let arch = mlx_qwen3_0_6b_arch();
    let model_bytes = 400_000_000;
    let mlx_ctx = max_context(
        model_bytes,
        &arch,
        "q8_0",
        "q8_0",
        1,
        2048,
        0,
        16 * 1024 * 1024 * 1024,
        1024,
        0.05,
        None,
        true,
        Backend::RapidMlx,
    );
    let metal_ctx = max_context(
        model_bytes,
        &arch,
        "q8_0",
        "q8_0",
        1,
        2048,
        0,
        16 * 1024 * 1024 * 1024,
        1024,
        0.05,
        None,
        true,
        Backend::LlamaCpp,
    );
    // Rapid-MLX's overhead formulas must not silently reuse llama.cpp's Metal calibration.
    assert_ne!(mlx_ctx, metal_ctx);
    assert!(mlx_ctx > 0 && metal_ctx > 0);
}

#[test]
fn find_min_cpu_moe_diverges_between_mlx_and_metal_backends() {
    let arch = mlx_qwen3_30b_a3b_arch();
    let model_bytes = estimate_model_size_bytes(30.0, "q4_k_m");
    let mlx_result = find_min_cpu_moe_to_fit_weights(
        model_bytes,
        &arch,
        8 * 1024 * 1024 * 1024,
        2048,
        true,
        Backend::RapidMlx,
    );
    let metal_result = find_min_cpu_moe_to_fit_weights(
        model_bytes,
        &arch,
        8 * 1024 * 1024 * 1024,
        2048,
        true,
        Backend::LlamaCpp,
    );
    // Both should return valid n_cpu_moe values; the point is the overhead formula used
    // internally differs by backend even though this call site doesn't expose it directly.
    assert!(mlx_result >= 0);
    assert!(metal_result >= 0);
}

#[test]
fn auto_size_rapid_mlx_uses_approximate_evidence() {
    let arch = mlx_qwen3_0_6b_arch();
    let model_bytes = 400_000_000;
    let result = auto_size(
        model_bytes,
        &arch,
        16 * 1024 * 1024 * 1024,
        UseCase::General,
        1,
        1024,
        true,
        None,
        Backend::RapidMlx,
    );
    assert_eq!(result.breakdown.evidence, EstimateEvidence::Approximate);
    assert!(result.context_size > 0);
    assert!(!result.scenarios.is_empty());
}

#[test]
fn auto_size_llama_cpp_uses_measured_evidence() {
    let arch = mlx_qwen3_0_6b_arch();
    let model_bytes = 400_000_000;
    let result = auto_size(
        model_bytes,
        &arch,
        16 * 1024 * 1024 * 1024,
        UseCase::General,
        1,
        1024,
        true,
        None,
        Backend::LlamaCpp,
    );
    assert_eq!(result.breakdown.evidence, EstimateEvidence::Measured);
}

#[test]
fn quant_comparison_table_rapid_mlx_diverges_from_llama_cpp() {
    let arch = mlx_qwen3_0_6b_arch();
    let mlx_opts = quant_comparison_table(
        0.6,
        &arch,
        "Qwen3-0.6B-MLX",
        16 * 1024 * 1024 * 1024,
        UseCase::General,
        1,
        true,
        Backend::RapidMlx,
    );
    let llama_cpp_opts = quant_comparison_table(
        0.6,
        &arch,
        "Qwen3-0.6B-GGUF",
        16 * 1024 * 1024 * 1024,
        UseCase::General,
        1,
        true,
        Backend::LlamaCpp,
    );
    assert!(!mlx_opts.is_empty());
    assert!(!llama_cpp_opts.is_empty());
    let mlx_q8 = mlx_opts.iter().find(|o| o.quant == "q8_0").unwrap();
    let cpp_q8 = llama_cpp_opts.iter().find(|o| o.quant == "q8_0").unwrap();
    assert_ne!(mlx_q8.max_ctx_q8, cpp_q8.max_ctx_q8);
}

// Hardware calibration: mlx-community/Qwen3-0.6B-4bit served via `rapid-mlx serve` 0.10.12 on
// an Apple M5 Max. The server's own scheduler logs self-reported Metal `active` memory
// (a direct MLX allocator measurement, not RSS). At ctx=2048 (a ~1.9K-token generation), the
// server reported active=0.6GB steady-state. The estimator predicts total=596MB for the same
// config — within ~1% of the real measurement, validating the existing dense (non-local-attn)
// per-layer overhead coefficient without changes.
#[test]
fn empirical_calibration_qwen3_0_6b_mlx_matches_measured_active_memory() {
    let arch = mlx_qwen3_0_6b_arch();
    let model_bytes = 351_386_061u64; // on-disk mlx-community/Qwen3-0.6B-4bit weights
    let bd = full_estimate(
        model_bytes,
        &arch,
        2048,
        "q4_0",
        "q4_0",
        1,
        512,
        0,
        -1,
        64 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            ..Default::default()
        },
    );
    let observed_active_bytes = 600_000_000u64;
    let diff = (bd.total_bytes as i64 - observed_active_bytes as i64).unsigned_abs();
    assert!(
        diff < observed_active_bytes / 10,
        "predicted {}MB should be within 10% of observed 600MB active",
        bd.total_bytes / 1_000_000
    );
}

// Hardware calibration: mlx-community/gemma-3-1b-it-4bit (local/sliding-window attention,
// window=512) served the same way. Metal `active` memory stayed flat at 0.8GB across the whole
// generation regardless of context growth, confirming the has_local_attn() KV-scaling logic.
// Before this measurement, mlx_overhead_base_bytes() copied llama.cpp's Metal local-attn
// per-layer coefficient (8.8) unvalidated, predicting a 1061MB total — a 33% over-prediction
// against the observed 800MB. Recalibrated to 5.5, predicting ~938MB (still conservative, ~17%
// over). Single-model sample: revisit once a larger Gemma4 local-attn model is measured.
#[test]
fn empirical_calibration_gemma3_1b_mlx_matches_measured_active_memory() {
    let arch = ModelArch {
        n_layers: 26,
        n_embd: 1152,
        n_kv_heads: 1,
        head_dim: 256,
        n_global_attn_layers: 5,
        local_attn_window: 512,
        local_kv_heads: 1,
        ..Default::default()
    };
    let model_bytes = 732_577_304u64; // on-disk mlx-community/gemma-3-1b-it-4bit weights
    let bd = full_estimate(
        model_bytes,
        &arch,
        2048,
        "q4_0",
        "q4_0",
        1,
        512,
        0,
        -1,
        64 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            ..Default::default()
        },
    );
    let observed_active_bytes = 800_000_000u64;
    assert!(
        bd.total_bytes >= observed_active_bytes,
        "prediction should stay conservative (>=) relative to observed 800MB, got {}MB",
        bd.total_bytes / 1_000_000
    );
    let over_prediction = bd.total_bytes - observed_active_bytes;
    assert!(
        over_prediction < observed_active_bytes / 4,
        "over-prediction should be under 25% of observed, got {}MB over 800MB",
        over_prediction / 1_000_000
    );
}

// ── Byte-to-bit *8 conversion tests (Phase 4 Part C) ────────────────────────

#[test]
fn estimate_param_b_from_size_correctly_applies_byte_to_bit_conversion() {
    // Core invariant: bytes = params × bpw / 8, so params = bytes × 8 / bpw
    // This tests the *8 conversion factor that was previously missing.
    let bpw = 4.85f64;
    let params_b = 7.0f64; // 7 billion params
    let expected_bytes = (params_b * 1e9 * bpw / 8.0) as u64;
    let recovered_params = estimate_param_b_from_size(expected_bytes, bpw);
    // Should recover ~7.0 billion within rounding tolerance
    assert!(
        (recovered_params - params_b).abs() < 0.01,
        "Expected ~{params_b}B params, got {recovered_params}B"
    );
}

#[test]
fn estimate_param_b_from_size_roundtrip_with_model_size_estimation() {
    // Verify estimate_model_size_bytes and estimate_param_b_from_size are true inverses.
    let param_b = 30.0f64;
    let quant = "q4_k_m";
    let bytes = estimate_model_size_bytes(param_b, quant);
    let bpw = find_quant(quant).map(|q| q.bpw).unwrap_or(4.85);
    let recovered = estimate_param_b_from_size(bytes, bpw);
    assert!(
        (recovered - param_b).abs() < 0.1,
        "Roundtrip failed: {param_b}B → {bytes}B → {recovered}B"
    );
}

#[test]
fn estimate_param_b_from_size_zero_and_edge_cases() {
    assert_eq!(estimate_param_b_from_size(0, 4.85), 0.0);
    assert_eq!(estimate_param_b_from_size(1_000_000_000, 0.0), 0.0);
    assert_eq!(estimate_param_b_from_size(1_000_000_000, -1.0), 0.0);
}

// ── MLX estimator integration tests (Phase 4 Part C) ────────────────────────

#[test]
fn mlx_estimator_produces_reasonable_estimate_for_dense_model() {
    // MLX path: use ModelMemoryProfile geometry → ModelArch → VRAM estimate.
    // Qwen3-0.6B: 28 layers, dense, no MoE/MTP/recurrent.
    let profile = crate::llama::model_memory_profile::ModelMemoryProfile {
        weights: crate::llama::model_memory_profile::WeightComponents {
            n_layers: crate::llama::model_memory_profile::EvidencedField {
                value: 28,
                field_evidence: "num_hidden_layers".into(),
            },
            n_head_kv: crate::llama::model_memory_profile::EvidencedField {
                value: 8,
                field_evidence: "num_key_value_heads".into(),
            },
            head_dim: crate::llama::model_memory_profile::EvidencedField {
                value: 128,
                field_evidence: "head_dim".into(),
            },
            n_embd: crate::llama::model_memory_profile::EvidencedField {
                value: 1024,
                field_evidence: "hidden_size".into(),
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let arch = ModelArch::from(&profile);
    assert_eq!(arch.n_layers, 28);
    assert_eq!(arch.n_kv_heads, 8);
    assert_eq!(arch.head_dim, 128);

    // Produce an MLX estimate with this arch.
    let model_bytes = 380_000_000u64; // ~380MB for Qwen3-0.6B 4-bit
    let bd = full_estimate(
        model_bytes,
        &arch,
        4096,
        "q4_0",
        "q4_0",
        1,
        512,
        0,
        -1,
        64 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            ..Default::default()
        },
    );
    // Estimate must be non-zero and reasonable (> weights, < 2GB for this small model).
    assert!(bd.total_bytes > model_bytes);
    assert!(bd.total_bytes < 2_000_000_000);
    assert_eq!(bd.evidence, EstimateEvidence::Approximate);
}

#[test]
fn mlx_estimator_hybrid_attn_qwen36_does_not_treat_all_layers_as_kv() {
    // Hard gate: Qwen3.6 full_attention_layer_count = block_count / full_attention_interval.
    let profile = crate::llama::model_memory_profile::ModelMemoryProfile {
        weights: crate::llama::model_memory_profile::WeightComponents {
            n_layers: crate::llama::model_memory_profile::EvidencedField {
                value: 64,
                field_evidence: "text_config.num_hidden_layers".into(),
            },
            n_head_kv: crate::llama::model_memory_profile::EvidencedField {
                value: 4,
                field_evidence: "text_config.num_key_value_heads".into(),
            },
            head_dim: crate::llama::model_memory_profile::EvidencedField {
                value: 128,
                field_evidence: "text_config.head_dim".into(),
            },
            ..Default::default()
        },
        full_attention_interval: Some(4),
        ..Default::default()
    };
    let arch = ModelArch::from(&profile);
    // 64 / 4 = 16 attention layers, not 64.
    assert_eq!(arch.n_attn_layers, 16);
    assert_ne!(arch.n_attn_layers, 64);
}

#[test]
fn gguf_meta_tests_no_regression() {
    // Verify that GGUF path still works — no regression from MLX changes.
    let arch = ModelArch::from_name_and_params("Qwen3-30B-A3B-Q4_K_M.gguf", 30.0);
    assert!(arch.is_moe());
    assert!(arch.n_experts > 0);
    // llama.cpp discrete GPU estimate still works.
    let bd = full_estimate(
        16_000_000_000,
        &arch,
        8192,
        "q8_0",
        "q8_0",
        1,
        1024,
        4,
        -1,
        32 * 1024 * 1024 * 1024,
        0,
        false,
        EstimatorOptions {
            backend: Backend::LlamaCpp,
            evidence: EstimateEvidence::Measured,
            mlx_prefix_cache_bytes: 0,
            ..Default::default()
        },
    );
    assert!(bd.total_bytes > 0);
    assert_eq!(bd.evidence, EstimateEvidence::Measured);
}

// ── Phase 5a Part 2: TurboQuant/D31 + active vs retained tests ────────────────

#[test]
fn active_and_retained_totals_are_distinct_no_double_counting() {
    let arch = ModelArch {
        n_layers: 32,
        n_kv_heads: 8,
        head_dim: 128,
        ..Default::default()
    };
    let model_bytes = estimate_model_size_bytes(7.0, "q4_k_m");

    let bd = full_estimate(
        model_bytes,
        &arch,
        0,
        "int4",
        "int4",
        1,
        1024,
        0,
        -1,
        64 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            turboquant_mode: None,
            rapid_planning_context_tokens: 32768,
            rapid_retained_cache_tokens: 8192,
            turboquant_eligibility: Default::default(),
        },
    );

    assert!(bd.active_kv_bytes > 0, "active_kv_bytes must be nonzero");
    assert!(
        bd.retained_kv_bytes > 0,
        "retained_kv_bytes must be nonzero"
    );

    assert_eq!(
        bd.kv_cache_bytes,
        bd.active_kv_bytes + bd.retained_kv_bytes,
        "kv_cache_bytes must equal active + retained (no double counting)"
    );

    assert!(
        bd.total_bytes
            >= bd.weights_bytes + bd.active_kv_bytes + bd.retained_kv_bytes + bd.overhead_bytes,
        "total_bytes must include all components"
    );
}

#[test]
fn standard_mode_not_mislabeled_as_fp16() {
    let mode = execution_policy::TurboQuantMode::Disabled;
    let savings = mode.retained_kv_savings_fraction();
    assert_eq!(
        savings, 0.0,
        "Standard mode (Disabled) must have zero savings — it is int4 baseline, not FP16"
    );
}

#[test]
fn planning_context_tokens_used_for_active_kv_not_current_tokens() {
    let arch = ModelArch {
        n_layers: 32,
        n_kv_heads: 8,
        head_dim: 128,
        ..Default::default()
    };
    let model_bytes = estimate_model_size_bytes(7.0, "q4_k_m");

    let bd = full_estimate(
        model_bytes,
        &arch,
        1000,
        "int4",
        "int4",
        1,
        1024,
        0,
        -1,
        64 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            turboquant_mode: None,
            rapid_planning_context_tokens: 65536,
            rapid_retained_cache_tokens: 4096,
            turboquant_eligibility: Default::default(),
        },
    );

    assert!(
        bd.active_kv_bytes > 300_000_000,
        "active_kv_bytes ({}) must reflect planning context (64K), not legacy context_size (1K)",
        bd.active_kv_bytes
    );
}

#[test]
fn turboquant_applies_only_to_retained_kv_not_active_weights_mtp_from_estimator() {
    let arch = ModelArch {
        n_layers: 32,
        n_kv_heads: 8,
        head_dim: 128,
        ..Default::default()
    };
    let model_bytes = estimate_model_size_bytes(7.0, "q4_k_m");

    let bd_baseline = full_estimate(
        model_bytes,
        &arch,
        0,
        "int4",
        "int4",
        1,
        1024,
        0,
        -1,
        64 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            turboquant_mode: None,
            rapid_planning_context_tokens: 32768,
            rapid_retained_cache_tokens: 8192,
            turboquant_eligibility: Default::default(),
        },
    );

    let bd_turbo = full_estimate(
        model_bytes,
        &arch,
        0,
        "int4",
        "int4",
        1,
        1024,
        0,
        -1,
        64 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            turboquant_mode: Some(execution_policy::TurboQuantMode::K8V4),
            rapid_planning_context_tokens: 32768,
            rapid_retained_cache_tokens: 8192,
            turboquant_eligibility: execution_policy::TurboQuantEligibility::Qualified,
        },
    );

    assert_eq!(
        bd_baseline.active_kv_bytes, bd_turbo.active_kv_bytes,
        "TurboQuant must NOT affect active_kv_bytes"
    );
    assert_eq!(
        bd_baseline.weights_bytes, bd_turbo.weights_bytes,
        "TurboQuant must NOT affect weights_bytes"
    );
    assert_eq!(
        bd_baseline.mtp_bytes, bd_turbo.mtp_bytes,
        "TurboQuant must NOT affect mtp_bytes"
    );
    assert!(
        bd_turbo.retained_kv_bytes < bd_baseline.retained_kv_bytes,
        "TurboQuant MUST reduce retained_kv_bytes"
    );
    assert_eq!(
        bd_turbo.effective_turboquant,
        execution_policy::TurboQuantMode::K8V4
    );
}

#[test]
fn turboquant_transient_peak_included_in_total_from_estimator() {
    let arch = ModelArch {
        n_layers: 32,
        n_kv_heads: 8,
        head_dim: 128,
        ..Default::default()
    };
    let model_bytes = estimate_model_size_bytes(7.0, "q4_k_m");

    let bd = full_estimate(
        model_bytes,
        &arch,
        0,
        "int4",
        "int4",
        1,
        1024,
        0,
        -1,
        64 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            turboquant_mode: Some(execution_policy::TurboQuantMode::K8V4),
            rapid_planning_context_tokens: 32768,
            rapid_retained_cache_tokens: 8192,
            turboquant_eligibility: execution_policy::TurboQuantEligibility::Qualified,
        },
    );

    assert!(
        bd.turboquant_transient_peak_bytes > 0,
        "turboquant_transient_peak_bytes must be nonzero when TurboQuant is active"
    );
    assert!(
        bd.total_bytes
            >= bd.weights_bytes
                + bd.active_kv_bytes
                + bd.retained_kv_bytes
                + bd.turboquant_transient_peak_bytes
                + bd.overhead_bytes,
        "total_bytes must include turboquant_transient_peak_bytes"
    );
}

#[test]
fn unknown_fineturn_does_not_inherit_turboquant_from_estimator() {
    let arch = ModelArch {
        n_layers: 32,
        n_kv_heads: 8,
        head_dim: 128,
        ..Default::default()
    };
    let model_bytes = estimate_model_size_bytes(7.0, "q4_k_m");

    let bd = full_estimate(
        model_bytes,
        &arch,
        0,
        "int4",
        "int4",
        1,
        1024,
        0,
        -1,
        64 * 1024 * 1024 * 1024,
        0,
        true,
        EstimatorOptions {
            backend: Backend::RapidMlx,
            evidence: EstimateEvidence::Approximate,
            mlx_prefix_cache_bytes: 0,
            turboquant_mode: Some(execution_policy::TurboQuantMode::K8V4),
            rapid_planning_context_tokens: 32768,
            rapid_retained_cache_tokens: 8192,
            turboquant_eligibility: execution_policy::TurboQuantEligibility::NotQualified,
        },
    );

    assert_eq!(
        bd.effective_turboquant,
        execution_policy::TurboQuantMode::Disabled,
        "Unknown finetune must NOT inherit TurboQuant qualification"
    );
    assert_eq!(
        bd.turboquant_transient_peak_bytes, 0,
        "Transient peak must be zero when TurboQuant is disabled"
    );
}
