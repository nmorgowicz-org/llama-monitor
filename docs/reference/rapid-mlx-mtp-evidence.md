# Rapid-MLX MTP Speculative Decoding — Evidence Record

| Field | Value |
|---|---|
| Status | Durable evidence record. Measurements only; no product decisions. |
| Runtime | rapid-mlx 0.11.1, pinned upstream commit [`206ed0e`](https://github.com/raullenchai/Rapid-MLX/tree/206ed0e03b7b6fc7b3b2e6f68a7b60467f6e5abe) |
| Stack | mlx 0.32.0, mlx_lm 0.31.3, Python 3.11 |
| Hardware | Apple Silicon M5 Max (single machine, single operator) |
| First captured | 2026-07-29 |
| Companions | Working state: `docs/plans/20260729-rapidmlx_speculative_decoding_handoff.md`. Product plan: Phase 6.5 of `docs/plans/20260718-final_rapidmlx_followups.md`. |

This document holds what was **measured** and what the **source says**. It deliberately
contains no roadmap, no UI design, and no enablement policy. Those live in Phase 6.5.

> **Reading the percentages.** Every throughput figure below is a **speedup ratio**
> (`new/old - 1`), not a fraction of time saved. "+59.4% generation" means 1.594× the
> tokens per second, which corresponds to 37% less generation time. Mixing the two
> conventions is the easiest way to misquote this document.

---

## 1. Root cause 1 — stale extractor produced a dead draft head

A pre-fix copy of upstream `scripts/extract_mtp_weights.py` selected norm tensors with
`endswith('norm.weight')`. That silently misses `pre_fc_norm_embedding.weight` and
`pre_fc_norm_hidden.weight`, because `"norm"` is mid-name in both. Those two never received
the `+1.0` RMSNorm convention shift (HF stores `x*(1+w)`; MLX `nn.RMSNorm` applies `x*w`),
leaving the MTP head's fc-input normalization inverted.

**Symptom profile — the reason this read as a capability limit rather than a bug:** ~0%
acceptance, no errors, no warnings, backbone entirely unaffected, tool calls still correct.

Upstream **has already fixed this**. Current `extract_mtp_weights.py:113-125` matches any
1-D norm weight and its comment names the symptom verbatim:

```python
# EVERY RMSNorm weight uses HF's ``(1 + w)`` convention; MLX's nn.RMSNorm
# applies ``x * w`` directly, so each norm weight needs +1.0. This MUST
# include the MTP-specific ``pre_fc_norm_embedding`` / ``pre_fc_norm_hidden``
# (the "norm" is mid-name, so an ``endswith('norm.weight')`` list silently
# missed them — leaving the MTP head's fc-input normalization inverted and
# producing ~0% draft acceptance). Match any 1-D norm weight instead.
for k in list(all_mtp_weights.keys()):
    if "norm" in k and k.endswith(".weight") and all_mtp_weights[k].ndim == 1:
        all_mtp_weights[k] = all_mtp_weights[k] + 1.0
```

**Contributing factor.** `qwen3_5_inject.py:725` calls
`mtp.load_weights(list(mtp_weights.items()), strict=False)`. A structurally valid but
semantically wrong head loads silently. There is no post-load sanity check anywhere on
the path.

**Cheapest detector:** `pre_fc_norm_embedding` mean should be ≈ **+0.56**, not ≈ **−0.44**.

---

## 2. Root cause 2 — sidecar inside the model directory double-shifts the trunk

Independent of root cause 1, and **not fixed upstream**.

`mlx_lm/utils.py:316` globs the trunk's weight shards:

```python
weight_files = glob.glob(str(model_path / "model*.safetensors"))
```

That pattern matches `model-mtp.safetensors`. `mlx_lm/models/qwen3_5.py:307-330` then
strips the `mtp.*` keys — so no stray keys survive — but their mere *presence* flips a
heuristic that shifts **every trunk norm**:

```python
has_mtp_weights = any("mtp." in k for k in weights)
should_shift_norm_weights = has_mtp_weights or has_unsanitized_conv1d
weights = {k: v for k, v in weights.items() if "mtp." not in k}
...
norm_keys = (".input_layernorm.weight", ".post_attention_layernorm.weight",
             "model.norm.weight", ".q_norm.weight", ".k_norm.weight")
for k, v in weights.items():
    if should_shift_norm_weights and any(k.endswith(sfx) for sfx in norm_keys):
        if v.ndim == 1:
            weights[k] = v + 1.0        # +1.0 on EVERY trunk norm
```

The heuristic is correct for a raw HF checkpoint, whose norms are stored `(1+w)` and do
need the shift. It is actively harmful on an **already-converted MLX** repo
(mlx-community, nightmedia, unsloth-MLX), whose norms are already in MLX convention: they
get shifted twice and the backbone degrades to gibberish.

**The trap ships in upstream's own tooling.** `extract_mtp_weights.py:188` writes
`output_file = mlx_dir / "model-mtp.safetensors"` — into the MLX model directory.
Following upstream's documented usage on a converted MLX checkpoint corrupts the trunk on
every subsequent load.

Same trunk shards, symlinked two ways, plain `mlx_lm.generate`, prompt
`"The capital of France is"`:

| Model dir contents | Output |
|---|---|
| trunk + `model-mtp.safetensors` | `'class.\n\n '` |
| trunk only | `'Paris.\n\n<think>\nThinking Process:\n\n1. **Analyze the input:**…'` |

`qwen3_next.py:455` performs the same strip. `qwen3_5_moe.py` does not.

---

## 3. Offline measurements — draft-vs-target agreement

**Method.** Draft-vs-target top-1 agreement at greedy: for each position, does the MTP
head's argmax match the backbone's own argmax two positions ahead. Computed in a single
model load, so it isolates head quality from the draft-k controller, batching, and server
plumbing. Probe: `mtp_probe2.py`, taking `<trunk_dir> <sidecar_path>`. Prompt: a 55-token
code-generation request, giving 54 comparable positions.

| Trunk | Sidecar | Agreement |
|---|---|---|
| `unsloth/Qwen3.6-27B-MLX-8bit` | `mlx-community/Qwen3.6-27B-MTP-4bit` (official) | **59.26%** (32/54) |
| `unsloth/Qwen3.6-27B-MLX-8bit` | nightmedia bf16-extracted, current upstream script | **61.11%** (33/54) |
| nightmedia mxfp8 27B, sidecar **outside** model dir | nightmedia bf16-extracted | **61.11%** (33/54) |
| nightmedia mxfp8 27B, sidecar **inside** model dir | either | 3.70% (2/54) |

**Convention counter-check.** Applying `+1.0` a second time to an already-correct head
*lowers* agreement — 61.11% → 42.59% (all norms) → 50.00% (`pre_fc_norm_*` only). The
current extractor output is therefore the correct convention, not accidentally right.

The 3.70% row was never a measurement of the MTP head. It is the *target* logits coming
from a double-shifted backbone (§2).

---

## 4. Served measurements

Fresh server per cell. Single trial (**n=1**) per cell — screening evidence, not
qualification. 512-token completions, `--max-num-seqs 1`, `--max-concurrent-requests 1`,
auto-K controller enabled, `num_speculative_tokens=4` requested.

### 4.1 Acceptance and controller behaviour

Raw counter deltas from the receipts in `tmp/`:

| Cell | attempts | accepts | ratio | rounds | parks | K histogram |
|---|---:|---:|---:|---:|---:|---|
| control: unsloth 8-bit + official 4-bit head | 255 | 248 | **0.9725** | 265 | 10 | K0=10, K1=255 |
| subject: nightmedia `qx64-hi` + extracted affine 8-bit head | 256 | 247 | **0.9648** | 266 | 10 | K0=10, K1=256 |
| subject: nightmedia MXFP8 trunk-only + same head | 258 | 245 | **0.9496** | 268 | 10 | K0=10, K1=258 |
| `qx64-hi`, prose workload | 292 | 211 | **0.7226** | 301 | 9 | K0=9, K1=292 |
| `qx64-hi`, 31,772-token code prompt | 257 | 245 | **0.9533** | 266 | 9 | K0=9, K1=257 |

**Acceptance is not inflated by a hidden denominator.** This was checked, because an
adaptive controller that parks on hard positions would bias conditional acceptance upward.
It does not here: parks are 3–4% of rounds, and the token accounting closes. Control:
265 rounds + 248 accepts = 513 against 514 measured completion tokens. So roughly **1.94
tokens per backbone forward** in the high-acceptance cells.

**Open observation — the controller looks nearly inert.** Park rate is flat at 3.0–3.8%
across the whole set and is *slightly higher* in the 96.5% cell than in the 72.3% cell.
An expected-value depth controller should park far more often on low-acceptance content.
On this workload set it does not discriminate. Unexplained; see Phase 6.5.

### 4.2 Throughput and latency

| Cell | accept | TG off | TG mtp | TG gain | TTFT off | TTFT mtp | End-to-end |
|---|---:|---:|---:|---:|---:|---:|---:|
| control, unsloth 8-bit | 97.3% | 14.47 | 17.63 | **+21.8%** | 10039 ms | 10217 ms | +15.7% |
| subject, `qx64-hi` | 96.5% | 21.87 | 32.82 | **+50.1%** | 10021 ms | 9891 ms | +31.2% |
| subject, MXFP8 | 95.0% | 17.04 | 27.17 | **+59.4%** | 10243 ms | 10254 ms | +38.4% |
| `qx64-hi`, prose | 72.3% | 23.68 | 30.57 | **+29.1%** | 3797 ms | 4098 ms | +22.0% |
| `qx64-hi`, 32k code | 95.3% | 16.47 | 21.58 | **+31.0%** | 52035 ms | 58124 ms | **+1.6%** |

End-to-end is `T_off / T_mtp - 1` where `T = TTFT + N / TG` at `N = 512`. All ten
throughput figures and all five end-to-end figures were recomputed from the raw receipt
fields and match.

**The 32k row is the product-relevant one.** Acceptance is excellent (95.3%) and
generation gains 31%, but TTFT regresses 11.7%, leaving **+1.6% end-to-end** for a
512-token completion. Acceptance alone does not predict user-visible benefit.

### 4.3 Unexplained anomaly — speedup inverts against acceptance

| Trunk | accept | TG gain | implied per-round cost |
|---|---:|---:|---:|
| unsloth 8-bit + 4-bit head (control) | 97.3% | +21.8% | **1.59×** |
| `qx64-hi` + 8-bit head | 96.5% | +50.1% | 1.29× |
| MXFP8 + 8-bit head | 95.0% | +59.4% | 1.20× |

All three produce ~1.94 tokens per round, so the differing gains are entirely per-round
overhead. The control has the **slowest** baseline and the **cheapest** sidecar (4-bit vs
8-bit) and should therefore gain the most. It gains the least, by a wide margin.

Candidate explanations, none tested: n=1 thermal or ordering effects; a genuinely more
expensive draft forward on the official head; an unrepresentative unsloth off-baseline.
**This is the single number most likely to invalidate a product claim** and it is
unresolved.

### 4.4 Receipt locations

| Path | Contents |
|---|---|
| `tmp/spec-decode-served-positive-control-v2/` | official-sidecar control |
| `tmp/spec-decode-served-off-v2/` | unsloth 8-bit off baseline |
| `tmp/spec-decode-qx64-subject-v2/`, `tmp/spec-decode-qx64-off-v2/` | `qx64-hi` pair |
| `tmp/spec-decode-mxfp8-subject-v2/`, `tmp/spec-decode-mxfp8-off-v2/` | MXFP8 pair |
| `tmp/spec-decode-qx64-prose-pair-v2/`, `tmp/spec-decode-qx64-code-32k-pair-v2/` | workload pairs |

**Void receipts.** `tmp/spec-decode-receipts/`, `tmp/spec-decode-warm-receipts/`, and
`tmp/spec-decode-control-receipts/` were all produced with a dead draft head (§1), and the
nightmedia ones additionally with a double-shifted trunk (§2). **They measure nothing about
MTP capability.** Retained only as a record of what the broken configuration looked like.

---

## 5. Draft depth is clamped to K=1 on Qwen3.5/3.6

`num_speculative_tokens` flows through the CLI and scheduler as `max_k`, but
`generator.py:~613` probes the real cache objects at loop start:

```python
_has_ssm_cache = any(hasattr(c, "rollback_state") for c in model_cache)
if not disable_auto_k:
    _max_k_hw = 1 if _has_ssm_cache else max(0, max_k)
```

Any model whose cache list contains an SSM slot is clamped to effective K=1. Observed
consequences:

- requested K=1, K=2, K=3, or K=4 with auto enabled all select only K∈{0,1};
- `disable_auto_k=true` bypasses the controller and fixes effective K=1 — it is **not**
  fixed K=4;
- `--force-spec-decode`, `--no-hybrid`, profile aliases, hidden legacy MTP flags,
  programmatic scheduler configuration, and direct generator `max_k` all reach the same
  clamp;
- there is no MTP K/SSM bypass environment variable.

**This is a correctness boundary, not conservative tuning.** The GatedDeltaNet patch stores
one `(conv_state, ssm_state)` snapshot. During K=2 verification of
`[target, draft1, draft2]`, that snapshot is after `[target, draft1]`. If `draft1` is
rejected, correct recovery needs the unavailable state after `[target]`. Rapid therefore
asserts when asked to roll SSM state back by more than one token. Removing the assert
restores the wrong recurrent state and silently violates lossless speculative decoding.
Safe hybrid K≥2 requires per-position snapshots or correct replay.

Sources:
[CLI mapping](https://github.com/raullenchai/Rapid-MLX/blob/206ed0e03b7b6fc7b3b2e6f68a7b60467f6e5abe/vllm_mlx/cli.py#L2012-L2038) ·
[scheduler mapping](https://github.com/raullenchai/Rapid-MLX/blob/206ed0e03b7b6fc7b3b2e6f68a7b60467f6e5abe/vllm_mlx/scheduler.py#L2626-L2637) ·
[clamp](https://github.com/raullenchai/Rapid-MLX/blob/206ed0e03b7b6fc7b3b2e6f68a7b60467f6e5abe/vllm_mlx/spec_decode/mtp/generator.py#L600-L641)

**Not yet observed directly.** The clamp emits
`[MTP-chain-of-K] … clamping max_k from 4 to 1` when it fires. That string appears nowhere
in the captured receipts, because server stderr was not retained. The K∈{0,1} histograms
are equally consistent with the controller simply preferring K=1 on EV grounds. The clamp
is proven from source, **inferred** for these specific runs.

### Unsloth's depth-2 guidance does not contradict this

[Qwen3.5](https://huggingface.co/unsloth/Qwen3.5-27B-GGUF/blob/3221f178a6b842d04f1fb42f1c413534adcc0a6a/README.md#L995-L999)
and [Qwen3.6](https://huggingface.co/unsloth/Qwen3.6-27B-GGUF/blob/82d411acf4a06cfb8d9b073a5211bf410bfc29bf/README.md#L169-L173)
recommend two speculative tokens for vLLM's `qwen3_next_mtp`; the
[Qwen3.6 MTP GGUF card](https://huggingface.co/unsloth/Qwen3.6-27B-MTP-GGUF/blob/5cb35eb3dcbf52dbce5f87dbc64df6aaffadcace/README.md#L48-L55)
recommends two for llama.cpp; the same cards recommend a different three-step/four-draft
SGLang setup. The number is a **backend-specific runtime width**, not the count of trained
MTP layers and not a portable model recommendation.

---

## 6. `aliases.json` is unreliable capability metadata

Two independent errors, both verified against source.

**6.1 The `supports_spec_decode` flag exists and does gate the MTP lane.** Every
`qwen3.6-27b-{4,6,8}bit` and `qwen3.6-27b-ud*` entry carries
`is_hybrid: false, is_hybrid_explicit: true, supports_spec_decode: false`.
`_mtp_path_label()` in `model_auto_config.py:2040+` reports `native` only when the family
ships a native MTP head **and** `supports_spec_decode=True`, else `disabled`. So the
mechanism claim holds.

**The inference does not.** "Upstream benchmarked this family and found MTP useless" is
unsupported by anything in the source and is directly contradicted by §3 and §4. Note that
`qwen3.6-35b-*` carries `is_hybrid: true` with the *same* `supports_spec_decode: false`,
consistent with the flag being applied family-wide from the hybrid lane rather than from a
27B-specific benchmark. **Treat the rationale as unknown.** Plausible readings, none
confirmed: a conservative default; blanket inheritance from the 35B hybrids; a decision
predating the §1 extractor fix.

**6.2 The `is_hybrid` value is factually wrong.** `unsloth/Qwen3.6-27B-MLX-8bit`
`config.json` `text_config.layer_types` has 64 entries: **48 `linear_attention`** and
16 `full_attention`, with `full_attention_interval: 4`. The 27B **is** a hybrid SSM model.
The alias says it is not.

This does not change §5 — the clamp keys off the runtime cache object, not the alias — so
both remain individually correct and never disagree at runtime. But it establishes the
general rule:

> **Never treat `aliases.json` as a source of architectural or capability truth.** It has
> now been shown wrong twice. Read `layer_types` from the model config instead.

---

## 7. Quantization and MoE support boundaries

### 7.1 Trunk and sidecar are independently packed

- The extractor reads only the destination config's top-level `bits` and `group_size` and
  calls affine `mx.quantize`. It does **not** reproduce `mode=mxfp*` or per-layer mixed
  overrides.
- Rapid infers the explicit sidecar's affine `(bits, group_size)` from its `fc.weight` /
  `fc.scales` (`qwen3_5_inject.py:520-557`), builds the MTP module at that packing, and
  validates all required sidecar shapes/dtypes before loading. `_detect_base_quantization`
  (the trunk reader) is used only when no explicit sidecar is passed.
- Supported explicit affine sidecar widths: **2/3/4/5/6/8 bits**, group sizes **32/64/128**.
- Therefore an MXFP4 target yields an affine 4-bit head by default, MXFP8 yields affine
  8-bit, and a `qx64-hi` target yields a uniform affine 6-bit head by default despite its
  per-layer 4/6-bit overrides. `--bits` / `--group-size` may deliberately choose another
  supported packing.
- An **MXFP-packed sidecar is not supported** by this injector; it expects full precision
  or uniformly affine `weight/scales/biases`. mxfp8 sidecars have no `biases`.
- `mlx_lm/utils.py:361` passes `mode=quantization.get("mode", "affine")`, so an mxfp8
  *trunk* is honoured correctly. **There is no "MTP requires an 8-bit trunk" rule** —
  §4 proves an affine 8-bit head on both a mixed 4/6-bit trunk and an MXFP8 trunk.

**Compatibility is structural, not provenance-safe.** The BF16 source should be the exact
unquantized parent/revision of the trunk. A same-shape head from a different fine-tune can
pass loading and still have poor acceptance, because it reuses the target trunk's
embeddings and LM head. The runtime checks structure, never training provenance.

### 7.2 MoE

Runtime support is explicit: both `qwen3_5` and `qwen3_5_moe` dispatch to the same
injector; the MTP head constructs `SparseMoeBlock` when `num_experts > 0`; the target
config must still declare `mtp_num_hidden_layers >= 1`.

Extractor support depends on source tensor layout:

- **Qwen3.6-35B-A3B is supported.** Its BF16 MTP tensors use fused `experts.gate_up_proj` /
  `experts.down_proj`; current upstream extraction converts these to canonical
  `switch_mlp.{gate,up,down}_proj`. Upstream commit `738a44e` reports an end-to-end
  Qwen3.6-35B-A3B result around 89% K=1 acceptance.
- **Official Qwen3.5-35B-A3B is not supported.** Its index uses 768 individually numbered
  expert tensors (`experts.<n>.{gate,up,down}_proj.weight`) and no fused `gate_up_proj`.
  The extractor does not stack these into `switch_mlp`; Rapid's required-key check will
  reject the resulting sidecar.

Capability detection must therefore distinguish family **and source layout**. There is no
single "MoE MTP supported" boolean.

---

## 8. Current extractor capabilities and gaps

Upstream `scripts/extract_mtp_weights.py` is useful but too narrow and unsafe to call
directly from a product workflow.

**Supported:** `--hf-model` repo ID with an indexed full-precision safetensors set; reads
`model.safetensors.index.json` and downloads every complete shard referenced by an `mtp.*`
key (shard-granular, may include substantial non-MTP tensors); applies the `+1.0` shift to
every 1-D `.weight`; converts Qwen3.6 fused MoE tensors to `switch_mlp`; affine-quantizes
every non-1-D MTP weight while leaving 1-D norms full precision; defaults 4-bit/group-64.

**Gaps and blockers:**

- no immutable `--revision`: a mutable repo default can silently change;
- no local authoritative-safetensors directory input;
- requires a sharded safetensors index — no unsharded safetensors, PyTorch, GGUF, or adapter input;
- no proof that the BF16 source is the parent of the target trunk;
- no source/config/tokenizer/embedding/LM-head identity check;
- no output provenance manifest or source/target hashes;
- reads only top-level target quantization; does not reproduce MXFP mode or per-layer overrides;
- official Qwen3.5 numbered-expert MoE layout is not stacked and must fail explicitly;
- shifts **any** 1-D `.weight` — safe for known Qwen norms, but an unexpected 1-D non-norm
  weight would be silently corrupted;
- **writes `model-mtp.safetensors` inside the target MLX directory** (§2);
- **may edit the target config in place** to add missing MTP depth.

The last two are unacceptable for llama-monitor. Never point the unwrapped script at a
user-owned trunk or an HF cache snapshot.

---

## 9. Artifact state

| Path | State |
|---|---|
| `/Users/nick/mlx-models/nightmedia-27b-mxfp8-mlx` | **Currently corrupted on load.** Contains `model-mtp.safetensors` and `model-mtp.safetensors.shifted.bak`. Both fail the norm preflight (`pre_fc_norm_embedding` mean ≈ −0.44), and either in-dir file triggers the §2 trunk glob defect. Not modified. |
| `/Users/nick/mlx-models/nightmedia-27b-mxfp8-mtp-fixed` | Symlinked trunk + correctly extracted head, but sidecar is **in-dir**, so it carries the same §2 defect. |
| `/Users/nick/mlx-models/control-unsloth8bit-official-mtp` | Prior investigation's control, built from a corrupted head. **Invalid — do not reuse.** |
| `scratchpad/nm-trunk-only` | Symlinked trunk, MTP files excluded. Generates correctly. Used as the MXFP8 subject. |
| `scratchpad/nm-mtp-fixed.safetensors` | Validated external sidecar. SHA-256 `5088ad4339b6d669b51ee66ab0e1ad214e5ca17d62bf643970ff09f09f25717a`. Served successfully. |
| `scratchpad/mtp_probe2.py` | The offline agreement probe. |
| `scratchpad/extract_mtp_weights.py` | Current upstream extractor, re-pulled 2026-07-29. |

**On `.shifted.bak`:** byte-level preflight shows both the live in-dir file *and* the
`.bak` carry the stale-extractor signature. Do not infer convention from the filename.
`nm-mtp-fixed.safetensors` is the only validated artifact (mean ≈ +0.56).

`mlx-community/Qwen3.6-27B-MTP-4bit` is a standalone **239 MB** sidecar
(`model_type: qwen3_5_mtp`), not a full model — the cheapest known-good positive control.

---

## 10. Install integrity and cleared suspects

- rapid-mlx 0.11.1: **267/267** `vllm_mlx` files hash-match `RECORD`. No local edits to
  `head.py`, `qwen3_5_inject.py`, or `generator.py`.
- Acceptance logic in `generator.py:730-879` was read and is **correct**. The greedy path
  compares `toks[:k_len] == drafts_i32`; `record_attempt()` is called `k_len` times per
  round. Not a suspect.
- Source of record: `~/.local/share/uv/tools/rapid-mlx/lib/python3.11/site-packages/vllm_mlx/`
  and `.../mlx_lm/`.

---

## 11. Known-unknowns

Carried forward to Phase 6.5. Nothing below has been measured.

1. Repeated-trial variance, thermal drift, and cell-ordering effects (everything above is n=1).
2. The §4.3 speedup inversion.
3. Why the depth controller barely parks at 72% acceptance (§4.1).
4. Direct observation of the K=1 clamp log (§5).
5. Tool-call warm/repeat/extension behaviour under MTP.
6. 65k and 131k prompt tiers; completion lengths other than 512.
7. Whether a same-shape wrong-parent sidecar is detectable before serving.
8. Greedy-lossless token parity between off and MTP runs.
