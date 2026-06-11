# Gemma4 MTP Draft Model Handling — Analysis & Plan

## Current State

### How MTP Works Today

**Qwen3.5/3.6 (Built-in MTP):**
- MTP heads grafted directly into the model file
- GGUF metadata has `general.architecture` + `mtp_depth` field
- No separate draft model needed — llama.cpp uses `--speculative draft-mtp,ngram-mod` with `--spec-draft-n-max 2`

**Gemma4 (External Draft MTP):**
- NO built-in MTP heads — requires a **separate draft model file**
- Unsloth provides MTP draft models in their `/MTP` subfolder:
  - `unsloth/gemma-4-31B-it-qat-GGUF/MTP/gemma-4-31B-it-Q8_0-MTP.gguf`
  - `unsloth/gemma-4-26B-A4B-it-qat-GGUF/MTP/gemma-4-26B-A4B-it-Q8_0-MTP.gguf`
  - `unsloth/gemma-4-31B-it-GGUF/MTP/gemma-4-31B-it-Q8_0-MTP.gguf` (non-QAT)
  - etc.
- llama.cpp uses `--model-draft <path>` + `--speculative draft-mtp,ngram-mod` with `--spec-draft-n-max 4`

### Current Detection

**Backend (`src/hf/mod.rs`):**
```rust
fn is_draft_assistant_hf(name_lower: &str, size_bytes: u64) -> bool {
    // Current patterns:
    let is_unambiguous = name_lower.contains("mtp-draft")
        || name_lower.contains("mtp_small")
        || name_lower.contains("mtp-heads")
        || name_lower.starts_with("mtp-");
    // Broad (requires size < 3GB):
    let is_broad = name_lower.contains("assistant")
        || name_lower.contains("draft-model");
}
```

**Frontend (`static/js/features/models.js`):**
```js
const isDraft = f.includes('mtp-draft') || f.includes('draft-model') ||
  f.includes('mtp_small') || f.includes('mtp-heads') || f.startsWith('mtp-');
```

**Problem:** The new Unsloth `-MTP.gguf` naming convention (e.g., `gemma-4-31B-it-Q8_0-MTP.gguf`) is NOT detected by either.

### Current VRAM Estimation

**Arch Heuristics (`vram_estimator/arch_heuristics.rs`):**
- Gemma4 heuristic sets `mtp_depth: 0` (no built-in MTP)
- MTP depth only set to 1 when filename contains "mtp" or "multi-token"
- This is correct for Gemma4 since MTP is external — the draft model's VRAM cost is its own file size, not the 1.5% internal multiplier

### Current Spawn Wizard

**Platform defaults (`spawn-wizard.js`):**
- Metal: MTP disabled by default ("reliable on Metal at moderate context" is outdated)
- CUDA: MTP enabled by default
- Default `--spec-draft-n-max`: 2 for built-in, 4 for external

---

## What Needs to Change

### 1. Draft Model Detection — Recognize `-MTP.gguf` Pattern

**Backend (`src/hf/mod.rs`):**
Add to `is_draft_assistant_hf`:
```rust
|| name_lower.ends_with("-mtp.gguf")
|| name_lower.ends_with("-mtp")
|| name_lower.contains("/mtp/")
```

**Frontend (`static/js/features/models.js`):**
Add to `_isDraftAssistantName`:
```js
n.endsWith('-MTP.gguf') || n.endsWith('-MTP') || n.includes('/MTP/')
```

**Frontend (`static/js/features/spawn-wizard.js`):**
Add to `detectMtpFromName`:
```js
lower.includes('-mtp')
```

### 2. Tier Compatibility — Match Draft to Main Model

**Problem:** An E2B draft model won't work with a 31B main model. Need to validate tier compatibility.

**New function in `src/hf/mod.rs`:**
```rust
/// Resolve Gemma4 tier from a name/repo (e2b, e4b, 12b, 26b-a4b, 31b)
fn resolve_gemma4_tier(name: &str) -> Option<&str> { ... }

/// Check if a draft model is compatible with the main model's tier
fn is_draft_compatible_with_tier(draft_name: &str, tier: &str) -> bool { ... }
```

### 3. Auto-Fetching MTP Draft Models

**When user loads a Gemma4 model in the spawn wizard:**

1. Scan local models dir for compatible MTP draft (tier match, size < 3GB, `-MTP` naming)
2. If found → pre-select it in the MTP assistant dropdown
3. If not found → show a "Download MTP Draft Model" button with:
   - Info about what MTP is and why it speeds up inference
   - Size estimate of the draft model
   - Link to the Unsloth repo

**New API endpoint:**
```
POST /api/spawn-wizard/mtp-draft-check
{
  "model_name": "gemma-4-31B-it-Q8_0.gguf",
  "quant_label": "Q8_0",
  "hf_repo": "unsloth/gemma-4-31B-it-GGUF"
}

Response:
{
  "ok": true,
  "draft_available": true,
  "draft_path": "/path/to/draft.gguf",
  "tier": "31b",
  "hf_download_url": null,           // null when draft available locally
  "hf_download_size_mb": 42,         // estimated size when download needed
  "recommended_spec_draft_n_max": 4  // 4 for Gemma4 external draft
}
```

**Unsloth repo resolution logic:**
- Detect Gemma4 model family from name
- Determine tier (31b, 26b-a4b, etc.)
- Detect QAT vs non-QAT
- Map to Unsloth repo: `unsloth/gemma-4-{tier}-it-{qat}-GGUF/MTP/{quant}-MTP.gguf`

### 4. QAT/Non-QAT Filename Collision

**Problem:** QAT and non-QAT Unsloth repos have identical filenames for MTP models:
- `unsloth/gemma-4-31B-it-qat-GGUF/MTP/gemma-4-31B-it-Q8_0-MTP.gguf`
- `unsloth/gemma-4-31B-it-GGUF/MTP/gemma-4-31B-it-Q8_0-MTP.gguf`

**Solution:** When downloading MTP draft models, append `-qat` to the filename if the source is a QAT repo:
- QAT: `gemma-4-31B-it-qat-Q8_0-MTP.gguf`
- Non-QAT: `gemma-4-31B-it-Q8_0-MTP.gguf`

### 5. Metal MTP Default — Enable by Default

Current code in `spawn-wizard.js`:
```js
// Line ~475:
if (platform === 'metal') {
  wizardState.hardware.mtpEnabled = false; // Too conservative!
}
```

Change to: `mtpEnabled = true` with a tooltip explaining MTP benefits and that users can disable if quality issues arise.

### 6. Spec Draft N Max Defaults

**Current behavior (correct):**
- 2 for built-in MTP (Qwen3.5/3.6)
- 4 for external draft (Gemma4)

**Issue:** The heuristic for Gemma4 needs to ensure `mtp_depth: 0` (no built-in) so the spawn wizard knows to use 4 as the default.

---

## Implementation Plan

### Phase 1: Detection Fixes
- [ ] Update `is_draft_assistant_hf()` to detect `-MTP.gguf` pattern
- [ ] Update `_isDraftAssistantName()` in JS to detect `-MTP.gguf` pattern
- [ ] Update `detectMtpFromName()` in JS to detect `-MTP` pattern

### Phase 2: Tier Compatibility
- [ ] Add `resolve_gemma4_tier()` function
- [ ] Add `is_draft_compatible_with_tier()` function
- [ ] Add `find_compatible_gemma4_mtp_draft()` function to scan local models dir

### Phase 3: Auto-Fetch
- [ ] Add `POST /api/spawn-wizard/mtp-draft-check` endpoint
- [ ] Add Unsloth repo resolution logic
- [ ] Add QAT filename renaming logic
- [ ] Add MTP draft info to spawn wizard response

### Phase 4: Frontend UI
- [ ] Add MTP download button/offer in spawn wizard
- [ ] Auto-select compatible local MTP draft
- [ ] Show MTP info tooltips
- [ ] Show draft model origin (local/HF) in model view

### Phase 5: Defaults
- [ ] Enable MTP by default on Metal
- [ ] Ensure `--spec-draft-n-max` defaults to 4 for Gemma4 external drafts

---

## Unsloth MTP Model Map

| Tier | QAT Repo | Non-QAT Repo | MTP Files |
|------|----------|-------------|-----------|
| E2B | `unsloth/gemma-4-e2b-it-qat-GGUF` | `unsloth/gemma-4-e2b-it-GGUF` | BF16-MTP, F16-MTP, Q4_0-MTP, Q8_0-MTP |
| E4B | `unsloth/gemma-4-e4b-it-qat-GGUF` | `unsloth/gemma-4-e4b-it-GGUF` | Same |
| 12B | `unsloth/gemma-4-12b-it-qat-GGUF` | `unsloth/gemma-4-12b-it-GGUF` | Same |
| 26B-A4B | `unsloth/gemma-4-26b-a4b-it-qat-GGUF` | `unsloth/gemma-4-26b-a4b-it-GGUF` | Same |
| 31B | `unsloth/gemma-4-31b-it-qat-GGUF` | `unsloth/gemma-4-31b-it-GGUF` | Same |
