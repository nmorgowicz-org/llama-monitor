# Rapid-MLX hybrid-cache entry qualification record

**Status:** completed 2026-08-02

**Runtime:** Rapid-MLX 0.11.1

**Recommendation model:** `unsloth/Qwen3.6-35B-A3B-UD-MLX-4bit`

**Pinned revision:** `6700c3e5bdeb050a379c8d2a4133f43f3647f20f`

This document is the completed evidence record for the former
`--hybrid-cache-entries` implementation work order. It is not an active plan.
The maintained user-facing results and guidance are in:

- [`docs/reference/cache-benchmark-results.md`](../../reference/cache-benchmark-results.md)
- [`docs/reference/rapid-mlx-runtime.md`](../../reference/rapid-mlx-runtime.md)

## Decision

Use the following workload-based policy for new Rapid-MLX configurations:

| Entries | Qualified use | Evidence |
|---:|---|---|
| 4 | One continuously active conversation history | Kept one hot 32K branch boundary; insufficient after another root ran |
| 8 | Main agent plus one sequential child at backend parallelism 1 | Both roots resumed with cache hits and about 5,800 reused tokens |
| 16 | General agent workflows (recommended) | Retained three alternating roots without slowing the already-hot single-root path |
| 32 / 64 | Advanced retention headroom only | Not qualified as faster or as a default |

New Spawn Wizard and Preset Editor flows select 16. Existing presets that stored
Auto/null continue to omit the flag for backward compatibility. Rapid-MLX
0.11.1's own default is 0, which disables hybrid snapshot reuse.

This entry count is not a RAM allocation. It limits retained non-trimmable
snapshots inside the independent `--cache-memory-mb` ceiling. Raising the entry
count may let the runtime use more of that configured byte budget in practice,
but it does not raise the retained-cache byte ceiling.

## What was measured

The recommendation lane used thinking-on Qwen sampling defaults, INT8 KV,
`--prefill-step-size 512`, an 8 GiB retained-cache ceiling, and sequential
requests on a one-sequence backend. It measured direct Rapid-MLX prefix-cache
hit, miss, tokens-saved, and eviction counters; TTFT was interpreted only after
the metric gates passed.

Key observations:

- An exact Qwen repeat increments hit and tokens-saved counters, but the
  scheduler then performs a correctness fallback to full prefill because its
  non-trimmable recurrent state cannot trim one token. Exact-repeat counters are
  therefore mechanical evidence, not effective compute-saved evidence.
- The first Qwen sibling branch seeds a message-boundary snapshot. Later
  siblings can reuse it.
- With one hot 32K root, entries 4 and 16 both kept forks 2-8 near 1.2-1.3
  seconds TTFT. More entries did not make that hot path faster.
- In the main-plus-one-child floor, entries 4 lost both roots while entries 8
  retained both: approximately 3.69-3.87 seconds versus 0.65-0.70 seconds TTFT.
- With three alternating roots, entries 4 lost all three while entries 16
  retained all three: approximately 3.53-3.70 seconds versus 0.88-0.97 seconds
  TTFT.
- All recommendation-lane SAFE/UNSAFE branch verdicts passed.

## Valid receipts

Mechanism controls:

- `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-a3b-cache-entries-e2-repeat-watched-v2/`
- `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-a3b-cache-entries-e4-branch-watched-v2/`

Workload controls:

- `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-a3b-cache-entries-pressure-32k-e2/`
- `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-a3b-cache-entries-pressure-32k-e4/`
- `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-a3b-cache-entries-pressure-32k-e16/`
- `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-a3b-cache-entries-main-child-8k-e4/`
- `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-a3b-cache-entries-main-child-8k-e8/`
- `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-a3b-cache-entries-multiroot-8k-e4/`
- `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-a3b-cache-entries-multiroot-8k-e16/`

Earlier Gemma controls were useful for repairing the benchmark harness and
proving the snapshot lifecycle, but they do not determine the Qwen3.6 product
recommendation.

## Remaining research boundary

A separate concurrent-arrival stress lane may quantify queue wait and prompt
processing contention when an orchestrator accidentally launches two or three
subagents simultaneously. It must use group-level before/after metrics because
per-request metric deltas become ambiguous while requests overlap. This
follow-up is not required for the backend-parallelism-1 default above and does
not reopen this completed entry-count qualification.
