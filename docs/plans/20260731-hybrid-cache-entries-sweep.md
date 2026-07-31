# Sweeping `--hybrid-cache-entries` against Gemma

**Status:** planned, not run
**Owner:** local model (Builder→Verifier loop), escalate to Opus only on a contradictory result
**Depends on:** `6819265` (the control now exists in the UI, so a measured recommendation has
somewhere to land)

## Why this exists

`hybrid_cache_entries` has been passed to every cache-lane benchmark cell at a hardcoded `16`
since the lane was written (`scripts/rapid-mlx-benchmark-suite.mjs:194`). Nobody chose 16 on
evidence, and nothing in the suite varies it, so we have no idea whether it matters, in which
direction, or where it stops mattering. It is now a user-facing control in the preset editor
and spawn wizard, with an option list whose "16 — benchmark default" label is currently an
appeal to a number with no measurement behind it. This plan replaces that label's authority
with data, or removes the control if the answer is "makes no difference".

## What the flag actually does

It caps the number of distinct retained prefix *entries* — branches — the hybrid cache keeps
alive. It is not a size. `--cache-memory-mb` is the size, and the two ceilings are independent:
whichever binds first evicts. That interaction is the whole reason a sweep is worth running,
because the recommendation for a coding agent (many short-lived forks off one long shared
prefix) may be the opposite of the recommendation for one long linear conversation.

## The trap that makes a naive sweep worthless

**A sweep of 16 / 32 / 64 on the existing cache lane will return four identical rows.**

The cache lane's sequence is `['cold', 'repeat', 'followup', 'fork']`
(`rapid-mlx-benchmark-suite.mjs:522`). That produces on the order of four cache entries. An
entry cap of 16 is never the binding constraint at four branches, so raising it to 32 or 64
changes nothing and the run would "prove" the flag is inert when it was simply never engaged.

Two ways to make the axis discriminating, and the plan uses both:

1. **Sweep below the branch count.** Include `1` and `2`, which are guaranteed to force
   eviction on the existing four-phase sequence. This is the cheap half and it is a *positive
   control*: if `1` does not measurably hurt the `fork` phase, the flag is not doing what the
   help text says and the rest of the sweep is meaningless. Run this first and stop if it fails.
2. **Raise the branch count.** Add a `fork-wide` sequence that forks N times off one shared
   prefix, so that entry caps in the 8–64 range become reachable. This is the half that answers
   the question a coding-agent user actually has.

## Second constraint: the cache-enabled lane only

`--hybrid-cache-entries` is emitted inside `cache ? [...] : [...]` at
`rapid-mlx-benchmark-suite.mjs:738`. A cell built without `cache: true` never receives the flag
at all, so a sweep declared outside the cache lane produces rows that differ only in a label.
Every cell below passes `cache: true`.

## What we can actually observe

There is no cache-hit or eviction metric. `rapid_mlx_*` exposes dtype, turboquant mode and the
spec-decode counters, and nothing about the prefix cache. Reuse is inferred, not read:

- **Primary signal:** TTFT on the `repeat` and `fork` phases relative to `cold`. A branch that
  was evicted re-prefills, and at 32k+ that is a large, unambiguous TTFT jump — not a subtle one.
- **Secondary:** total wall time per phase, as a cross-check that the TTFT delta is real and not
  a scheduling artifact.
- **Guard:** the existing marker/answer scoring must stay flat. If entry pressure changes output
  quality rather than only latency, that is a correctness finding and outranks the whole sweep.
  Note that `marker_recall` is unscoreable on `followup`/`fork` — the markers are absent from
  those prompts — so a 0/5 there is an artifact, not evidence.

## Matrix

Model: a Gemma checkpoint, per the user's proposal. Gemma is the right subject here because
the sweep is about cache-entry bookkeeping rather than attention-architecture behaviour, and
Gemma is the least entangled with the hybrid-SSM caveats that complicate Qwen3.6-27B.

Fixed: `dtype: int8`, `prefillStepSize: 512`, `cache: true`, `cacheMemoryMb: 8192`,
`turboquant: none`, `diskCheckpointInterval: 0`, concurrency 1.

| Phase | Sequence | `hybridCacheEntries` | Contexts | Cells |
|---|---|---|---|---|
| A — positive control | `cold, repeat, followup, fork` | 1, 4, 16 | 32000 | 3 |
| B — branch-pressure | `fork-wide` (8 forks) | 1, 4, 16, 64 | 32000, 131072 | 8 |
| C — memory interaction | `fork-wide` (8 forks) | 4, 64 | 131072 | 4 at `cacheMemoryMb: 16384` |

Phase A gates B. Phase C only runs if B shows an effect, and exists to answer whether the entry
cap or the memory ceiling is the binding constraint — a `64`-entry win that evaporates at
8 GiB but survives at 16 GiB means the real recommendation is about memory, not entries.

## Implementation

`configuration()` already takes `hybridCacheEntries` and threads it into
`prefix_cache`/`hybrid_cache_entries`, so no plumbing change is needed — only a new suite:

```js
if (suite === 'cache-entries') { /* cells per the matrix above */ }
```

Declare it like `turboquant-scale`: **deliberately excluded from `all`**. This is a one-time
calibration pass, not part of the recurring per-model matrix, and folding it into `all` would
add a dozen long-context cells to every routine run.

The `fork-wide` sequence is the only genuinely new machinery. It needs to fork N times from one
shared prefix rather than once, and each fork must be distinguishable in the results so an
evicted branch is visible as a per-fork TTFT outlier rather than being averaged away.

## What the result changes

- **Effect found, monotonic:** set the option list's recommended value from the data and say so
  in the field hint, replacing "benchmark default".
- **Effect found, workload-dependent:** the hint becomes conditional (linear conversation vs
  branching agent), and this is a candidate for the Doctor teaching surface rather than a
  number in a dropdown.
- **No effect anywhere, including at `1`:** the flag does not do what its help text says on this
  runtime version. Record that, drop the control, and file it the same way the phantom flags
  were — a capability that exists in `--help` but not in behaviour is the same class of defect,
  just harder to detect.
- **Quality moves:** stop, escalate. That is not a tuning result.

## Open

- Which Gemma checkpoint, and whether the vision variant is in scope (it should not be —
  rapid-mlx vision is broken except possibly Gemma-4, and prefill batching interacts with the
  VLM path in ways that would confound this).
- Whether `fork-wide` belongs in the shared sequence vocabulary or stays local to this suite.
