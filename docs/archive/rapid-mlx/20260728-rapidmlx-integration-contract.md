# Rapid-MLX Integration Contract

**Status:** Implemented. This document records the product/architecture decisions that make
Rapid-MLX a first-class MLX loader across backend and frontend surfaces, and the specific
defects that were found and fixed while getting there. It is a decision record, not an
execution plan — for field-by-field implementation detail, see the reference docs listed in
[§9](#9-where-to-look-for-implementation-detail).

**Superseded plan:** `docs/archive/rapid-mlx/20260727-phase6_rapidmlx_integration_completion_handoff.md`
(prior version of this document) drove the work described here. It is kept for history; this
document is the current-state replacement.

## 1. Scope

Rapid-MLX is a first-class MLX loader alongside llama.cpp/GGUF. The Spawn Wizard, Preset
Editor, welcome-screen preset cards, launch backend, and canonical VRAM estimator all describe
and launch the same effective configuration for a Rapid-MLX preset. Schema and UI are kept
broad enough that another MLX loader (e.g. MTPLX, deferred) can be added later without a
rewrite.

Explicitly out of scope for this integration (see [§8](#8-non-goals-and-deferred-work)):
MTPLX, MTP/speculative-decoding qualification, RoPE/YaRN context extension, GGUF→MLX lossy
conversion, modern-Qwen vision parity.

## 2. Evidence hierarchy

When resolving Rapid-MLX behavior, trust in this order:

1. Current local source and tests.
2. Current source-build Rapid `serve --help`, exact binary identity, and captured receipts.
3. Local MLX `config.json`, safetensors index, tokenizer metadata, generation metadata.
4. Bounded Hugging Face metadata fetches for the exact repo/revision.
5. `rapid-mlx info <model>` profile output, guarded by exact executable/version compatibility.
6. Model-family/name heuristics only as a degraded fallback.

The benchmark corpus underpinning the KV/cache contracts below qualifies
`rapid-mlx 0.11.0+git.5fc6556c`. An older tagged release is not assumed to share the same
live-KV behavior — requalify the compact dtype matrix when the upstream KV fix ships in a
tagged build.

## 3. Reasoning and KV dtype contract

- `--kv-cache-dtype` accepts `bf16`, `int8`, `int4`.
- `--reasoning` is a bare boolean switch (no value). It pins the **effective** active KV dtype
  to `int8` regardless of the requested dtype, because sub-4-bit KV drops ~20pt on AIME-class
  math for Qwen3 thinking variants. `--reasoning off` does not exist as a flag; "off" is
  represented by omitting `--reasoning` and instead emitting `--no-thinking`.
- Reasoning defaults ON (not "auto") — reasoning is the expected normal use case for the models
  this integration targets. `RapidMlxExecutionPolicy::new_with_eligibility` resolves:

  ```text
  reasoning_mode = true  -> effective INT8, regardless of requested dtype
  reasoning_mode = false -> effective dtype = requested BF16/INT8/INT4
  dtype omitted           -> effective INT4
  ```

- Requested and effective dtype are both surfaced through the API and UI. `/api/vram-estimate`
  returns both `execution_policy.kv_cache_dtype` (requested) and `effective_kv_dtype`. The
  Wizard/Editor/cards show "INT4 → INT8 (reasoning profile)" whenever reasoning overrides the
  requested dtype.
- Three concepts are kept distinct and never conflated: `enable_thinking` (request default),
  `reasoning_effort` (request default), and `--reasoning` (server launch profile that pins KV).

## 4. Active-KV and retained-cache accounting contract

### 4.1 Active KV

The estimator does not use parameter count as a KV multiplier. It reads model geometry
(`ModelArch`) and, where a receipt-calibrated slope exists (`rapid_slopes.rs`), uses that slope;
otherwise it falls back to a formula-based geometry calculation:

```text
KV bytes = effective KV layers × KV heads × head dimension × context tokens × slots
           × (K bytes/element + V bytes/element)
```

- Qwen 3.5/3.6 hybrid DeltaNet: `effective KV layers = total layers / full_attention_interval`;
  recurrent/linear layers contribute fixed state separately, not linearly with context.
- Gemma 4 local/global attention: global layers use full context × global KV geometry, local
  layers use `min(context, sliding window)` × local KV geometry, with global/local head
  dimensions modeled separately.
- Receipt-calibrated slopes exist for four architecture tracks (Hybrid DeltaNet, Sliding
  Window, MLA, Standard fallback), detected purely from `ModelArch` fields (layer counts,
  KV head counts, sliding-window presence) — never from model name matching, so finetunes and
  distillations are covered without needing a name allowlist.

### 4.2 Retained prefix cache

`--cache-memory-mb` is a capacity **ceiling** for the retained in-memory prefix cache. It is not
an additional allocation layered on top of a second token-derived retained-cache reservation
representing the same state. The contract:

```text
mandatory memory =
    weights + active KV (selected context, effective dtype) + fixed recurrent state
  + mmproj/MTP where applicable + runtime/working overhead

optional retained-cache reservation =
    prefix_cache_enabled ? configured --cache-memory-mb ceiling : 0

total admission estimate = mandatory memory + optional retained-cache reservation + system reserve
```

When an explicit cap (`mlx_prefix_cache_bytes > 0`) is set, token-derived retained KV is **not**
also added — it is zeroed. Token-derived retained size remains available as informational
capacity math (estimated bytes per retained prefix, prefixes/branches fitting in the cap,
cap utilization at 131K/160K/200K), never summed with the cap.

Benchmark-calibrated retained-cache policy (Qwen 3.6 35B-A3B measured lanes):

| Context / active KV | 8 GiB | 16 GiB | Product conclusion |
|---|---|---|---|
| 160K INT8 | newest fork fast; evictions occur | no evictions | 16 GiB retains older branches |
| 160K INT4 | no material deficit | no material speed gain | 8 GiB sufficient |
| 200K INT8 | newest fork fast; more evictions | no evictions | 16 GiB retains older branches |
| 200K INT4 | newest fork fast despite evictions | no material speed gain | 8 GiB baseline |

8 GiB is the qualified default; 16 GiB is offered for retaining older branches, never labeled a
performance mode. Disk checkpoints stay off by default (`disk_checkpoint_interval = 0`) — a
measured 200K INT4 run wrote 16.78 GB, loaded nothing, and increased cold TTFT by 56.5s without
improving the fork; they are snapshot/manual warm-start machinery, not a cache tier.

## 5. Command construction contract

| Concern | Contract |
|---|---|
| Hybrid mode | Typed `RapidMlxHybridMode {Auto, Force, Disable}` enum (not an escape-hatch pair). `Auto` emits neither flag; `Force` emits `--force-hybrid`; `Disable` emits `--no-hybrid`. Mutually exclusive by type. `Auto` auto-detects to `Force` when local/HF `config.json` (including nested `text_config`) shows `full_attention_interval > 1`. |
| Tool-call / reasoning parsers | `None` = Auto (omit flag). Every explicit value is capability-gated against the live `serve --help` snapshot before being emitted as `--tool-call-parser` / `--reasoning-parser`. |
| Sampling defaults | All 8 fields (`--default-temperature`, `--default-top-p`, `--default-top-k`, `--default-min-p`, `--default-repetition-penalty`, `--default-presence-penalty`, `--default-frequency-penalty`) plus `--max-tokens` are wired from preset fields, each capability-gated. `max_tokens` maps to `--max-tokens`, **not** the nonexistent `--default-max-tokens`. Unsupported-but-configured fields surface a flag-advisor warning rather than being silently dropped. |
| Text prefill | `prefill_step_size` defaults to 512, always emitted as `--prefill-step-size`, validated 1–2048, and kept distinct from `prefill_batch_size` (a different flag). Vision may raise this to 1024/1536/2048 but never above 2048. |
| PFlash | Hardcoded to `"off"` by default (`--pflash off`) — confirmed in benchmarking to cause recall/context-compression regressions on coding/architecture tasks where full-context retention matters. Auto/Always remain selectable but are labeled as changing effective prompt retention and breaking benchmark comparability. |
| Cache defaults | Managed presets always pass an explicit `--cache-memory-mb` (8192/16384) or `--disable-prefix-cache` — never rely on Rapid's own ~20%-of-RAM auto-detection by omission. |
| Disk checkpoints | `disk_checkpoint_interval` stays `0`; no UI path recommends or enables it. |

## 6. Estimator, Wizard, Editor, and card parity

All four surfaces (Spawn Wizard, Preset Editor, welcome-screen cards, `/api/vram-estimate`)
build requests through the same canonical builder (`vram-estimate.js` /
`rapidEstimatePolicyFromConfig`) so a saved config yields identical totals everywhere. Explicit
user overrides survive async profile/metadata enrichment (fields are only auto-filled when
still nullish). The `/api/rapid-mlx/models/{id}/unified-profile` endpoint merges three
evidence sources — MLX config geometry, `rapid-mlx info` behavioral flags, and explicit
model_type mappings — with documented priority rules (config wins for hybrid_mode; `rapid-mlx
info` first for parser fields) and surfaces warnings when sources disagree. See
`docs/reference/spawn-wizard.md` and `docs/reference/vram-estimator.md` for field-level detail.

## 7. Context ceilings

Standard context choices: 32768, 65536, 131072, 163840, 200000, 262144. The canonical API
returns `native_context_limit` and `context_extension_required`; values above the model's
metadata ceiling are marked advanced/unqualified rather than emitting fabricated
`--rope-scaling yarn` flags. RoPE/YaRN-based extension is deferred (see
[§8](#8-non-goals-and-deferred-work)).

## 8. Non-goals and deferred work

Do not block future work on these; they are intentionally out of this integration's scope:

- MTPLX (another MLX loader) — schema/UI are kept broad enough to add it later.
- MTP/speculative-decoding qualification (backend/API exist; no Wizard UI yet).
- RoPE/YaRN context-extension controls and math.
- GGUF-to-MLX lossy conversion.
- Modern-Qwen vision parity for Rapid-MLX — the vision runtime is not currently qualified and
  must stay explicit in UI/docs until proven otherwise.
- Automatic disk-backed paging of evicted Rapid prefix entries.
- TurboQuant — UI wired, but launch keeps it disabled pending per-model qualification.

## 9. Where to look for implementation detail

- `docs/reference/rapid-mlx-runtime.md` — install/upgrade/repair/rollback, capability evidence, cache policy.
- `docs/reference/vram-estimator.md` — full estimator field reference, ModelArch, architecture heuristics.
- `docs/reference/spawn-wizard.md` — wizard steps, engine selection, Rapid-MLX UX.
- `docs/reference/cache-benchmark-results.md` — retained-cache benchmark receipts.
- `docs/reference/model-runtime-benchmarking.md` — cross-backend benchmark methodology.
- `docs/reference/rapid-mlx-telemetry.md` — dashboard cards, runtime metrics.
- `docs/reference/hf-model-library.md`, `docs/reference/community-source-catalog.md`,
  `docs/reference/memory-management.md` — Phase 7/8 surfaces built on top of this integration.

Key source files: `src/inference/rapid_mlx/{mod,command,compatibility,capabilities,info_query,mlx_meta,escape_hatch}.rs`,
`src/web/api/{rapid_mlx_runtime,vram}.rs`, `src/llama/vram_estimator/{execution_policy,estimate,rapid_slopes}.rs`,
`src/llama/model_memory_profile.rs`, `static/js/features/{spawn-wizard,presets,setup-view,vram-estimate}.js`.
