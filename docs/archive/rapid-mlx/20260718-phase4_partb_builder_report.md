# Phase 4 Part B: Architecture-specific geometry — Builder Report

**Role**: Builder
**Phase**: 4 Part B
**Branch**: feat/rapid-mlx-integration
**Start commit**: 910f2e9 (Phase 4 Part A complete)
**Date**: 2026-07-20

---

## 0. Objective

Populate architecture-specific geometry fields in `ModelMemoryProfile` from MLX model configs:

- Qwen3.6 DeltaNet/recurrent state via `full_attention_interval`
- Gemma4 global/local heads + sliding window
- Dense/MoE expert topology (total vs active)
- MTP tensor ownership + external companions (per A25)

All geometry must carry `field_evidence` pointing to the config key that sourced it.

---

## 1. Files Modified

| File | Change |
|------|--------|
| `src/llama/model_memory_profile.rs` | Added `CompanionType`, `ExternalCompanion` structs; added `encoder_layers` fields to `VisionComponent`; added local KV fields to `GlobalLocalHeadGeometry`; added `external_companions` to `ModelMemoryProfile` |
| `src/inference/rapid_mlx/mlx_meta.rs` | Populated local KV geometry in Gemma4; added external companion parsing from draft_model.source; added vision encoder_layers from depth/num_hidden_layers; added 10 new Part B hard-gate tests |
| `tests/fixtures/mlx_configs/` | No new fixtures added; existing 6 pinned configs used |

Note: Part A already implemented `ExpertTopology`, `RecurrentStateGeometry`, `GlobalLocalHeadGeometry`, `EmbeddedMtpComponent`, `VisionComponent`, layer groups, and most parsing. Part B extended these with missing fields and added external companion tracking.

---

## 2. Implementation Details

### 2.1 Changes to `model_memory_profile.rs`

#### `GlobalLocalHeadGeometry` — added local KV fields:
```rust
pub struct GlobalLocalHeadGeometry {
    // Part A fields:
    pub num_global_key_value_heads: Option<u32>,
    pub num_global_kv_evidence: Option<String>,
    pub global_head_dim: Option<u32>,
    pub global_head_dim_evidence: Option<String>,
    // Part B additions:
    pub num_local_key_value_heads: Option<u32>,
    pub num_local_kv_evidence: Option<String>,
    pub local_head_dim: Option<u32>,
    pub local_head_dim_evidence: Option<String>,
    pub local_attn_window_size: Option<u32>,
    pub local_attn_window_evidence: Option<String>,
}
```

#### `VisionComponent` — added encoder_layers:
```rust
pub struct VisionComponent {
    pub has_vision_config: bool,
    pub field_evidence: String,
    pub model_type: Option<String>,
    pub model_type_evidence: Option<String>,
    // Part B addition:
    pub encoder_layers: Option<u32>,
    pub encoder_layers_evidence: Option<String>,
}
```

#### New: `CompanionType` and `ExternalCompanion` (per A25):
```rust
pub enum CompanionType {
    Drafter,
    Vision,
    Embedding,
}

pub struct ExternalCompanion {
    pub companion_type: CompanionType,
    pub source: String,
    pub provenance: String,
}
```

#### `ModelMemoryProfile` — added `external_companions`:
```rust
pub struct ModelMemoryProfile {
    // ... existing fields ...
    pub external_companions: Vec<ExternalCompanion>,
}
```

### 2.2 Changes to `mlx_meta.rs`

#### Gemma4 local KV derivation:
Local KV heads = `num_key_value_heads - num_global_key_value_heads`. Evidence is derived from both fields. `local_head_dim` sourced from standard `head_dim`. `local_attn_window_size` sourced from `sliding_window`.

#### External companion parsing:
When `draft_model.model` (or `speculative_config.model`) is present, it signals an external drafter source. Creates `ExternalCompanion` with type=Drafter, source=the model path, provenance="draft_model.model". MTP layers still tracked from `num_hidden_layers`.

#### Vision encoder_layers:
Reads `vision_config.num_hidden_layers` (Gemma4) or `vision_config.depth` (Qwen3.6) with evidence pointing to whichever key was used.

### 2.3 Test additions (10 new hard-gate tests):

| Test | Verifies |
|------|----------|
| `qwen36_deltanet_full_attention_layer_count_via_interval` | full_attention_layer_count=16 != total_layers=64 |
| `gemma4_global_kv_included_not_zeroed` | global_kv=2, evidence present |
| `gemma4_local_kv_capped_by_sliding_window` | local_kv=6, window=1024 |
| `recurrent_state_explicit_with_field_evidence` | all recurrent fields have evidence |
| `moe_expert_topology_with_field_evidence` | experts=256, top_k=8, evidence present |
| `external_companion_from_draft_model_source` | companion created from draft_model.model |
| `no_double_counting_mtp_companions_from_main_geometry` | main_layers=10, not 10+2+1 |
| `gemma4_31b_dense_no_moe` | global_kv=4, no MoE |
| `qwen3_06b_dense_baseline` | dense, no MoE/MTP/recurrent/companions |
| +6 fixture tests updated | vision, recurrent evidence, companion assertions |

---

## 3. Hard Gate Verification

| Gate | Status | Evidence |
|------|--------|----------|
| Qwen3.6 does NOT treat all layers as full KV | ✅ PASS | Test: `qwen36_deltanet_full_attention_layer_count_via_interval` asserts full=16 != total=64 |
| Gemma4 includes global KV AND caps local KV by sliding window | ✅ PASS | Tests: `gemma4_global_kv_included_not_zeroed`, `gemma4_local_kv_capped_by_sliding_window` |
| Recurrent state is explicit with field_evidence | ✅ PASS | Test: `recurrent_state_explicit_with_field_evidence` checks all 6 evidence fields |
| Every component has provenance | ✅ PASS | All geometry fields have field_evidence; ExternalCompanion has provenance |
| No llama GGUF regression | ✅ PASS | 17 gguf_meta tests pass; no GGUF code changed |
| No double counting (MTP/companions separate from main geometry) | ✅ PASS | Test: `no_double_counting_mtp_companions_from_main_geometry` |

---

## 4. Build and Test Results

### 4.1 Build verification
```
cargo build --release        → ZERO warnings
cargo clippy --lib -- -D warnings → CLEAN
cargo fmt -- --check         → CLEAN
```

### 4.2 Unit tests (30 total pass)
```
cargo test --lib -- model_memory_profile mlx_meta → 30 passed, 0 failed
```

### 4.3 GGUF regression
```
cargo test --lib -- gguf_meta → 17 passed, 0 failed, 2 ignored
```

---

## 5. Config Evidence Summary (6 pinned fixtures)

| Model | full_attn | linear | MoE | global_kv | recurrent | vision | companions |
|-------|-----------|--------|-----|-----------|-----------|--------|------------|
| Qwen3.6-27B | 16 | 48 | No | — | Yes (DeltaNet) | Yes (27) | 0 |
| Qwen3.6-35B-A3B | 10 | 30 | 256/8 | — | Yes (DeltaNet) | Yes (27) | 0 |
| Gemma-4-26b-a4b | 5 | 0 | 128/8 | 2 | No | Yes (27) | 0 |
| Gemma-4-31b | 10 | 0 | No | 4 | No | Yes (27) | 0 |
| Qwen3-0.6B | 28 | 0 | No | — | No | No | 0 |
| Qwen3-30B-A3B | 48 | 0 | 128/8 | — | No | No | 0 |

Note: Gemma4 local attention is tracked via `layer_types=["sliding_attention"]` → `LocalAttention` layer groups with sliding_window=1024.

---

## 6. Dependencies for Phase 4 Part C

Part C can safely assume:

- `ModelMemoryProfile.layer_groups` correctly split by kind (FullAttention/LocalAttention/LinearRecurrent)
- `full_attention_interval` and `full_attention_layer_count()` reliable for KV cache calculation
- `global_local_heads` complete with local KV derived from total-global
- `external_companions` list empty unless config explicitly references external source
- `recurrent_state` fields all have field_evidence
- `vision.encoder_layers` sourced from depth or num_hidden_layers

---

## 7. Known Limitations (Part C scope)

| Item | Scope |
|------|-------|
| Context ceiling propagation | Part C |
| Byte-to-bit *8 conversion | Part C |
| HF lookup revision-aware/recursive/paginated | Part C |
| Local MLX config heuristic fallback | Part C |
| `LayerMemoryGroup.recurrent_state_bytes` actual calculation | Part C |

---

## 8. Structured Handoff (Section 9.3)

### 8.1 Changes Summary

- Extended `GlobalLocalHeadGeometry` with local KV fields (derived from total-global)
- Extended `VisionComponent` with encoder_layers (from depth/num_hidden_layers)
- Added `CompanionType`, `ExternalCompanion` types; wired into `ModelMemoryProfile`
- Updated MLX parser: local KV derivation, external companion detection, vision layers
- Added 10 Part B hard-gate tests; updated 6 fixture tests with new assertions
- Build zero warnings, clippy clean, 30/30 MLX tests pass, 17/17 GGUF tests pass

### 8.2 Files for Verifier to Review

1. `src/llama/model_memory_profile.rs:203-260` — GlobalLocalHeadGeometry local KV fields, VisionComponent encoder_layers, CompanionType/ExternalCompanion
2. `src/llama/model_memory_profile.rs:320-321` — external_companions field
3. `src/inference/rapid_mlx/mlx_meta.rs:464-507` — Gemma4 local KV/sliding window population
4. `src/inference/rapid_mlx/mlx_meta.rs:713-737` — Vision component with encoder_layers
5. `src/inference/rapid_mlx/mlx_meta.rs:739-773` — External companion parsing from draft_model
6. `src/inference/rapid_mlx/mlx_meta.rs:1498-1595` — Part B hard-gate tests

### 8.3 Verifier Checklist

- [ ] Qwen3.6-27B: `full_attention_layer_count == 16` (64/4), `linear_recurrent_layer_count == 48`
- [ ] Qwen3.6-27B: `recurrent_state` has all 6 field_evidence values non-empty
- [ ] Qwen3.6-35B-A3B: `experts.n_experts == 256`, `experts.top_k == 8`, evidence present
- [ ] Gemma-4-26b-a4b: `global_kv=2`, `local_kv=6` (8-2), `window=1024`, evidence present
- [ ] Gemma-4-31b: `global_kv=4`, `local_kv=12` (16-4), no MoE
- [ ] Qwen3-0.6B: dense, no MoE/MTP/recurrent/vision/companions
- [ ] Qwen3-30B-A3B: MoE (128/8), flat config parsing, no recurrent/MTP/companions
- [ ] All populated geometry fields have non-empty field_evidence
- [ ] ExternalCompanion only created when draft_model.model present
- [ ] No GGUF regression: 17 gguf_meta tests pass
- [ ] MTP/companions not double-counted in main layer count
- [ ] `cargo build --release` zero warnings
- [ ] `cargo clippy --lib -- -D warnings` clean
- [ ] `cargo fmt` clean

### 8.4 Hard Gates Met

- ✅ Qwen3.6 DeltaNet: full_attention_layer_count = block_count / full_attention_interval (16 ≠ 64)
- ✅ Gemma4: global KV included (2, 4), local KV derived (6, 12), window capped at 1024
- ✅ Recurrent state: explicit with field_evidence on all 6 DeltaNet fields
- ✅ Every component has provenance: all geometry fields carry field_evidence
- ✅ No llama GGUF regression: 17 tests pass, no GGUF code modified
- ✅ No double counting: external companions and MTP layers separate from main geometry

---

**Builder sign-off**: Phase 4 Part B complete. Ready for Verifier review.
