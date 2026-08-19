# Calibration Phase 7 real-server receipt

Date: 2026-08-16

This is the macOS representative dense-Qwen receipt for the bounded Phase 7
server track. It is intentionally a qualification smoke test, not a general
speculative-decoding optimizer and not a claim about every workload.

## Fixture and launch policy

- Model: Qwen3.5 9B MTP GGUF from the local model inventory.
- Backend: managed llama.cpp, loopback-only calibration-owned server.
- Main KV: `q8_0/q8_0`; Flash Attention: `auto`.
- Context: `131072`; `--parallel 1`; reasoning budget: `8192`.
- Template: pinned Froggeric Qwen v22 fixture, revision recorded by the Phase 7
  plan.
- Speculative candidate: `draft-mtp,ngram-mod,ngram-map-k4v`, draft n-max `3`,
  draft p-min `0.20`, draft layers `all`, `--spec-default`, `-lv 4`.
- Control: matched no-spec server with the same load-time settings and
  `--spec-type none`.
- Requests: six bounded reasoning/design, multi-turn, tool-shaped, synthesis,
  and realistic 32K-cap requests; the control replayed the matched subset.

## Observed receipt

The candidate log showed active `draft-mtp` and `ngram-map-k4v` paths. Per
request draft acceptance observations were:

```text
0.40785  0.54619  0.44287  0.85897  0.38435  0.51537
```

Aggregate draft-MTP counters reached 6,090 generated drafts and 4,065 accepted
drafts. The n-gram path also emitted activity. The tool-shaped request produced
streamed `tool_calls`; the matched no-spec control produced no speculative
counters, as expected. Requests completed without a qualification stall.

These acceptance values are fixture-, template-, runtime-, and prompt-specific
observations. They are not persisted as a universal recommendation and do not
replace a user's own workload validation.

## Safety and fallback evidence

- MTP requires explicit introspection and managed-help capability evidence and
  remains `--parallel 1`.
- Unsupported or inert MTP/n-gram tracks are represented as `unsupported` and
  are omitted from recommendations; they do not block ordinary model launch.
- DFLASH remains fail-closed because this managed build has no verified active
  DFLASH capability signal. A requested flag alone is not evidence.
- Concurrency remains an explicit opt-in track and cannot replace the default
  single-user result.
- Rapid-MLX receives no llama.cpp settings or recommendations.

## Return markers

Native Windows managed-binary execution, process-tree cleanup, and native
capture are still required in the Windows handoff. Broader MoE optimization,
overnight search, and workload-general speculative optimization remain outside
the 2.0 bounded scope.
