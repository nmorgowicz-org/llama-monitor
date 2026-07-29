# Rapid-MLX MTP — Working Handoff

**Purpose.** This is the context-relay doc for passing MTP speculative-decoding work
between frontier models across hourly quota boundaries. It is deliberately short enough to
read into a fresh context in full. It carries *state and open questions only*.

| | |
|---|---|
| Branch | `feat/rapid-mlx-integration` |
| Last updated | 2026-07-29 |
| Evidence record | `docs/reference/rapid-mlx-mtp-evidence.md` — every measurement, source citation, and artifact hash |
| Product plan | Phase 6.5 in `docs/plans/20260718-final_rapidmlx_followups.md` — roadmap, gates, integration contract |

> Do not add measurements, source quotes, or roadmap material to this file. They belong in
> the evidence record and Phase 6.5 respectively. Keep this under ~200 lines.

---

## 1. Where things stand

**MTP speculative decoding works on Qwen3.6-27B.** An earlier version of this document
concluded it was non-viable for the family. That was wrong, caused by two stacked tooling
bugs, and every measurement in it is void.

The two bugs, both *around* the model rather than in Rapid's decode path:

1. **A stale `extract_mtp_weights.py` produced a dead draft head.** It missed the `+1.0`
   RMSNorm shift on `pre_fc_norm_embedding` / `pre_fc_norm_hidden`. Symptom: ~0%
   acceptance, no errors, backbone fine, tool calls fine — which is exactly why it read as
   a capability limit. **Fixed upstream.**
2. **Writing the sidecar into the model directory silently corrupts the trunk.** `mlx_lm`
   globs `model*.safetensors`; the presence of `model-mtp.safetensors` flips a heuristic
   that shifts every trunk norm by `+1.0`, double-shifting an already-converted MLX repo
   into gibberish. Upstream's own extractor writes there by default. **Not fixed upstream.**

Served, with both fixed: **72–97% acceptance** and **+22% to +59% generation throughput**
across three trunk quantizations. The nightmedia MXFP8 model is **not** broken.

---

## 2. What is proven

- Both root causes, from source and from reproduction.
- Served end-to-end operation on three trunks (unsloth 8-bit, nightmedia `qx64-hi` mixed
  4/6-bit, nightmedia MXFP8) against an external sidecar.
- Sidecar quantization is **independent** of trunk quantization. There is no "MTP requires
  an 8-bit trunk" rule.
- The sidecar does **not** need to live in the trunk directory, and must not.
- Effective draft depth is **clamped to K=1** on Qwen3.5/3.6 by an SSM-cache rollback
  limitation. This is a correctness boundary, not tuning.
- `aliases.json` is unreliable capability metadata — wrong twice, including a checkable
  architectural fact (`is_hybrid: false` on a model that is 48/64 linear-attention layers).
- Benchmark harness is repaired and passes non-model validation.

## 3. What is not proven

Everything served is **n=1**. This is screening evidence, not qualification.

1. **The speedup inversion.** Highest acceptance produced the *smallest* gain: control
   (97.3%, +21.8%) vs MXFP8 (95.0%, +59.4%), at ~1.94 tokens/round in both. The control has
   the slowest baseline and cheapest sidecar and should have gained most. Unresolved, and
   the most likely thing to invalidate a product claim.
2. **The K=1 clamp was never directly observed.** Proven from source; inferred for these
   runs. Server stderr was not captured, so the `[MTP-chain-of-K] … clamping max_k` log
   line is absent from every receipt. The K∈{0,1} histograms are equally consistent with
   the controller simply preferring K=1.
3. **The depth controller looks nearly inert.** Park rate is flat at 3–4% whether
   acceptance is 72% or 97%, and is slightly *higher* at high acceptance. An EV controller
   should behave the opposite way.
4. Repeat-trial variance, thermal and ordering effects.
5. Tool-call warm/repeat/extension behaviour under MTP.
6. 65k/131k tiers; completion lengths other than 512.
7. Greedy-lossless token parity between off and MTP runs.
8. Whether a same-shape wrong-parent sidecar can be rejected before serving.

## 4. Load-bearing caveats

- **Acceptance does not predict benefit.** The 32k-prompt cell measured 95.3% acceptance
  and +31.0% generation, but TTFT regressed 11.7%, leaving **+1.6% end-to-end** on a
  512-token completion. Never ship an acceptance-only indicator.
- **All percentages in the evidence record are speedup ratios** (`new/old - 1`), not
  fractions of time saved. "+59.4%" is 1.594× tok/s, i.e. 37% less generation time.
- **The benchmark suite currently passes `--force-spec-decode` unconditionally** whenever a
  speculative config is present, because the tested aliases advertise
  `supports_spec_decode=false`. That is a recorded profile-eligibility override, not the
  product launch contract. Forced and naturally-eligible lanes must be split before any
  qualification claim.
- **`family="unknown"`** appears in the MXFP8 receipt's metric labels (symlinked trunk dir)
  where other cells say `family="qwen3.6"`. Anything aggregating on that label will
  silently drop the series.

## 5. Artifacts you must not trust

| Path | Why |
|---|---|
| `/Users/nick/mlx-models/nightmedia-27b-mxfp8-mlx` | Corrupted on load — in-dir `model-mtp.safetensors` triggers the trunk double-shift. Not modified; left as found. |
| `/Users/nick/mlx-models/nightmedia-27b-mxfp8-mtp-fixed` | Correct head, but still in-dir. Same defect. |
| `/Users/nick/mlx-models/control-unsloth8bit-official-mtp` | Built from a dead head. Invalid. |
| `tmp/spec-decode-{receipts,warm-receipts,control-receipts}/` | Produced with a dead head, some also with a corrupted trunk. Measure nothing. |

The **only** validated sidecar is `scratchpad/nm-mtp-fixed.safetensors`
(SHA-256 `5088ad43…25717a`). `mlx-community/Qwen3.6-27B-MTP-4bit` is the known-good
positive control (239 MB standalone sidecar).

**Norm sanity check, one line:** a valid head has `pre_fc_norm_embedding` mean ≈ **+0.56**.
≈ **−0.44** means the stale extractor.

---

## 6. Harness state

Committed on `feat/rapid-mlx-integration`:

| Commit | What |
|---|---|
| `c54615f` | Accept-ratio per-phase delta, control/subject roles, sidecar hashes/revisions, tensor-derived quantization, shifted-RMSNorm preflight, in-trunk-sidecar rejection, paired TG/TTFT/end-to-end/park reporting |
| `8217f55` | Full server stdout/stderr persisted per cell; `runtime.backend_log` with `clamp_verdict` / `effective_max_k` |
| `e74d841` | `--trials N` counterbalanced ABBA ordering, `--settle-seconds`, `trial_protocol` in the suite index |
| `f4c2b5f` | `--spec-decode-lane forced\|natural`, required and defaultless; lane verified against the backend log, mismatch fails the run |

`tmp/` holds the surviving `-v2` receipts and is untracked. The void receipt
directories are deleted.

⚠️ Two consequences for anyone re-running: spec-decode invocations now **require**
`--spec-decode-lane`, and the evidence record cites `tmp/…` paths that will not survive a
clean checkout.

---

## 7. Next action

Phase 6.5 sub-phase **6.5a**. Harness items 1–3 are done and unexercised — the tooling to
resolve §3.1 and §3.2 exists but has not been pointed at a model yet.

**The run is the next action.** Re-run the three-trunk matrix with `--trials 4`,
`--spec-decode-lane forced` (the aliases still declare `supports_spec_decode: false`, so
`natural` will not install MTP), `--speculative-tokens 2` so the clamp line can fire at
all, and a settle window. Then read `backend_log.clamp_verdict` — it should say
`clamp_observed`, which closes §3.2 — and check whether the inversion survives
counterbalancing, which closes or confirms §3.1.

Also run one `--spec-decode-lane natural` cell. It should show MTP never installing. That
is the evidence that natural eligibility is currently unavailable on this family, which is
itself a Phase 7 input.

Remaining 6.5a harness items before the gate: positive-control-fails-the-run enforcement,
65k/131k tiers, non-512 completions, tool-call warm/repeat/extension, greedy-lossless
parity, the `family="unknown"` label audit, and fact-derived capability.

Do **not** start artifact management, API wiring, or UI work until 6.5a's gate passes —
those are 6.5b and Phase 7, and they consume qualification output that does not exist yet.

---

## 8. Handoff protocol

When picking this up in a fresh context:

1. Read this file, then §11 of the evidence record (known-unknowns).
2. Read the full evidence record only if you need to *re-derive* a claim; it is long.
3. Read Phase 6.5 only when doing product work rather than measurement.
4. Update **this** file with state changes; update the evidence record with new
   measurements; update Phase 6.5 with scope decisions. Do not let them merge again — the
   previous single document reached 1113 lines and buried the 20% that was load-bearing.
