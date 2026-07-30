# Cache benchmark results

## Scope and evidence boundary

These are on-device measurements on the Apple M5 Max / 64 GiB unified-memory
machine. They qualify cache policy for one interactive coding-agent client;
they do not establish multi-client server policy, universal model-family
behavior, or a disk-backed cache claim.

Rapid rows use source build `rapid-mlx 0.11.0+git.5fc6556c` and Qwen 3.6
35B-A3B 4-bit revision `6700c3e5bdeb050a379c8d2a4133f43f3647f20f`. Text
prefill is 512, one active sequence, PFlash off, and the workload is a frozen
workspace followed by `cold → repeat → follow-up → fork`. The fork represents
an agent taking a new branch from a long shared coding history.

## Rapid-MLX active-KV calibration coverage

The estimator uses the model's parsed MLX geometry, rather than a parameter-count
multiplier: Qwen 3.5/3.6 hybrid models grow only their full-attention KV layers
and keep DeltaNet state fixed; Gemma 4 separates sliding-window local layers
from global full-context layers. The source-build matrix below is the evidence
boundary for the current Rapid math. It is not a promise that an older tagged
Rapid release has the same active-KV behavior.

| MLX model / weight conversion | BF16 anchors | INT8 anchors | INT4 anchors | Receipt directory |
|---|---|---|---|---|
| Qwen3.5-9B | 32K, 65K, 131K | 32K, 65K, 131K, 160K, 200K | 32K, 65K, 131K | `qwen35-9b-source-5fc6556c-context-512/` |
| Qwen3.6-27B Polaris2 | 32K, 65K, 131K | 32K, 65K, 131K, 160K, 200K | 32K, 65K, 131K, 160K, 200K | `nightmedia-qwen36-27b-polaris-source-5fc6556c-context-512-hybrid/` |
| Qwen3.6-35B-A3B | source-build cache anchors at 32K, 65K, 131K, 160K, 200K | 131K, 160K, 200K | 131K, 160K, 200K | `unsloth-qwen36-35b-workspace-cache-*/` |
| Gemma 4 26B-A4B LM and VLM conversions | 32K, 65K, 131K | 32K, 65K, 131K, 160K, 200K | 32K, 65K, 131K, 160K, 200K | `ailexleon-gemma4-26b-*-source-5fc6556c-context-512/` |

All text-prefill anchors use the qualified 512-token prefill step. A context
target is a memory-model point, not proof of native context extension: launch
surfaces cap standard choices at model metadata's native context ceiling. A
higher manually entered value is marked advanced-only until RoPE/YaRN controls,
template headroom, and an extended-context benchmark are qualified.

## Rapid-MLX: cache turns 200K from minutes into seconds

| Context | Active KV | Retained cap | Cold TTFT | Fork TTFT | Saved tokens | Peak Metal |
|---:|---|---:|---:|---:|---:|---:|
| 131K | INT4 | 16 GiB | 117.88 s | 1.60 s | 134,205 | 29.43 GB |
| 160K | INT4 | 16 GiB | 216.78 s | 2.07 s | 161,434 | 31.03 GB |
| 200K | INT4 | 16 GiB | 317.85 s | 3.14 s | 202,997 | 33.47 GB |

The live-KV quantization result is source-build evidence only: repeat the
compact qualification set when the upstream fix appears in a tagged release.

## Rapid-MLX: retained-RAM capacity

| Context / KV | 8 GiB result | 16 GiB result | Policy conclusion |
|---|---|---|---|
| 160K INT8 | 1.89 s fork, 5 evictions | 1.95 s, 0 evictions | 16 GiB retains older branches only |
| 160K INT4 | 1.84 s, 0 evictions | 1.82 s, 0 evictions | 8 GiB is sufficient |
| 200K INT8 | 2.34 s, 6 evictions | 2.21 s, 0 evictions | 16 GiB retains older branches only |
| 200K INT4 | 2.29 s, 3 evictions | 2.17 s, 0 evictions | 8 GiB keeps newest fork fast |

**Recommendation:** 8 GiB is the qualified Rapid retained-cache baseline.
Offer 16 GiB as an explicit “retain more branches” option, not as a speed
preset. It did not materially improve the newest fork.

## Rapid-MLX: disk checkpoints are not a lower cache tier

At 200K INT4 / 8 GiB, `--kv-disk-checkpoint-interval 8192` performed four
successful writes totaling 16.78 GB, with zero loads and zero hook errors.
It added 56.5 s (24%) to cold TTFT (231.36 s → 287.85 s) and did not improve
the 2.29–2.88 s fork. The current source writes snapshots but does not
automatically reload an evicted RAM cache entry. Use interval `0` for normal
interactive workloads. Manual export/import remains an advanced restart
warm-start mechanism, not a disk-paging solution.

## llama.cpp: one live slot already provides fast same-session reuse

The Qwen 3.6 35B-A3B Q4_K_M GGUF used llama-server `10107 (c0bc8591e)` with
Q8_0 K/V, Flash Attention, 512 ubatch, one slot, and explicit 32 context
checkpoints spaced by 8192 tokens. The same live-server slot ran the same
four-request sequence with `-cram 0` and `-cram 8192`.

| Context | `-cram 0` fork | `-cram 8192` fork |
|---:|---:|---:|
| 131K | 0.548 s | 0.541 s |
| 160K | 0.673 s | 0.637 s |
| 200K | 0.784 s | 0.699 s |

Exact replay was 0.17–0.23 s in both conditions. This does **not** prove that
the host cache is useless: the live slot retained reusable context in both
conditions, so no branch was displaced. It does prove that `-cram 0` does not
disable normal one-slot same-session reuse. Keep Apple single-user Auto=`0`
until a separate multi-branch/slot-pressure test proves an extra host-cache
budget benefits the target workload.

Marker recall is identical between the two conditions in all twelve
context × phase pairs, so `-cram 0` costs no fidelity either — cold and repeat
score 5/5 at 131K, 160K, and 200K under both settings. Recall is the stronger
of the two signals here, because the sub-100 ms TTFT differences are near
measurement noise while a recall difference would not be.

**Reading recall in these receipts:** only the `cold` and `repeat` phases carry
scoreable recall. `followup` and `fork` rebuild the prompt without the CHECK_\*
constants and ask for a concise answer instead of `NAME=VALUE` lines, so recall
is not measurable there. Receipts captured before 2026-07-30 nonetheless record
`recall_rate: 0` for those phases, which is a scorer artifact and **not**
evidence that the cache corrupts long context; the two `5/5` outliers
(160K fork, 200K followup) are the model echoing markers from the prior
assistant turn. The harness now scores those phases `null`.

## Current product policy

1. Estimate mandatory memory as weights + active KV/context + runtime
   overhead + reserve.
2. Present Rapid retained cache as optional growth on top of that fit.
3. Recommend Rapid 8 GiB when it fits; expose 16 GiB as branch retention.
4. Do not recommend Rapid disk checkpoints or manual export/import for normal
   interactive agents.
5. Keep llama one-slot Apple Auto=`0`; explain that it disables optional host
   cache budget, not normal same-session reuse.

## Receipts

- `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-workspace-cache-capacity-160k-200k-v1/`
- `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-workspace-cache-disk-200k-int4-8g-v1/`
- `tests/fixtures/calibration/llama-cpp-receipts/qwen36-35b-heretic-cache-ram-0-8192-v1/`
