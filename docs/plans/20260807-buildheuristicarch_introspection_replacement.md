# `buildHeuristicArch` → introspection-first replacement

**State:** Not started. Deferred until after the current branch (`feat/rapid-mlx-integration`) merges, so it doesn't collide with in-flight Phase 10 spawn-wizard work.

## Why this exists

`buildHeuristicArch(name, paramB)` (`static/js/features/spawn-wizard.js:3130+`, ~150 lines) is a
filename-keyed lookup that supplies structural sizing details GGUF headers don't expose in a
standard field: hybrid-attention layer ratios (Qwen3-Coder-Next, Qwen3.6 DeltaNet), sliding-window
params (Gemma4), MoE expert counts/fractions, per-family head dims, etc. It's called from
`getEffectiveArch()` (`spawn-wizard.js:3066`) and from `spawn-wizard-hf-browse.js:263`, and it
matches families by substring on the model/repo name (`lower.includes('qwen3.6')`,
`lower.includes('gemma-4')`, etc.).

Per the standing "introspection-only, never filename checking" mandate (Phase 10e in
`docs/plans/20260718-final_rapidmlx_followups_execution.md`), this is in scope for replacement —
but it was explicitly *not* touched in the 10e pass because it isn't a simple "guess a property
from the filename" bug like the ones already fixed (paramB/MoE/MTP pre-fill, sampling-defaults
family detection). It's a deliberate fallback layer: `getEffectiveArch()` already overrides every
heuristic field with the real introspected value wherever GGUF/introspection supplies one
(`spawn-wizard.js:3095-3113`, "Override with introspection-only fields"). Ripping it out without a
replacement would silently break VRAM sizing for every hybrid/sliding-window family the moment a
GGUF header doesn't carry a field llama.cpp's own loader has to derive some other way.

## What real introspection can and can't give us

- `full_attention_interval`, `ssm_inner_size`, `ssm_state_size`, `ssm_conv_kernel` and friends
  already exist as real GGUF metadata keys (`src/llama/gguf_meta.rs:166+`,
  "Hybrid linear-attention (Qwen3-Next / DeltaNet: qwen35, qwen35moe, qwen3next)" section) and
  `n_attn_layers()` is already computed from them, not from a name. **Confirm first** whether every
  family currently hardcoded in `buildHeuristicArch` actually has these keys populated in its real
  GGUF files (some may — this needs verification against real files, not assumed from the source
  comments).
- Sliding-window fields (`local_kv_heads`, `key_length_swa`, `sliding_window`) are also already
  real GGUF keys per `gguf_meta.rs`'s `to_model_metadata()` mapping — same verification need.
- MoE expert count/used-count are already real fields (`expert_count`, `expert_used_count`) and
  already override the heuristic's guessed 64/8/128 tiers when present.
- What's *not* clearly available from GGUF alone: the qualitative "which named family is this"
  label itself (used elsewhere for UI display, not just sizing), and possibly some derived
  constants the heuristic hardcodes per family (e.g. Coder-Next's `36 * 32 * 128 * 128 * 2` linear
  state size) that may need deriving from real header fields instead of being a per-family magic
  number.

## Proposed approach (to validate at pickup, not a locked plan)

1. Audit `buildHeuristicArch`'s ~6 named families field-by-field against `GgufMetadata`'s real
   keys — for each field the heuristic sets, determine: (a) always available from GGUF today, (b)
   derivable from GGUF fields via a formula (not a name match), or (c) genuinely unavailable
   without a name-based family label.
2. For (a)/(b) fields: move the computation into `gguf_meta.rs`/`to_model_metadata()` (server
   side, same place `n_attn_layers()` already lives) so it's real introspection, not a JS-side
   filename guess. This also fixes remote/HF-streamed models for free once wired through the same
   `/api/model-defaults` HF-introspection path built in Phase 10e.
3. For (c) — if any field truly has no GGUF-derivable source — surface it as "unknown, pending
   model-specific qualification" in the UI rather than silently guessing from the name. Do not
   invent a heuristic replacement for the sake of having a number.
4. Once server-side introspection covers everything real, `buildHeuristicArch` becomes dead code
   in the same way `model_defaults.rs::get_model_defaults`/`get_model_presets` already are (see
   Phase 10e's audit) — delete it rather than leave it as unused legacy code.
5. Repeat the same audit for `spawn-wizard-mtp-draft.js`'s `detectMtpFromName` usage
   (`spawn-wizard-mtp-draft.js:221`) — flagged separately during the Phase 10e pass as "a different
   animal" (MTP draft-file discovery is a filesystem/companion-file search, not a property
   inference from the main model's own name) and needs its own scoping conversation before
   deciding whether it's in scope for the same treatment.

## Completion proof

- Every field `buildHeuristicArch` currently sets for a real, tested model file is confirmed to
  come from `GgufMetadata`/introspection, not a name match, for both local and HF-streamed models.
- Any field with no real GGUF source renders as an explicit "unknown/pending" state in the UI
  instead of a guessed value.
- `buildHeuristicArch` (and its two call sites) is deleted, not left in place unused.
- No VRAM-sizing regression for the families it used to cover (Coder-Next, Qwen3.6, Gemma4, and
  whatever else is in the current family list) — verify against real GGUF files for each on the
  M5 Max dev machine before calling this done.
