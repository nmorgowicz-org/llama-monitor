# Calibration Phase 3 — evaluator hardening

Date: 2026-08-14

Status: source-side evaluator gates complete; native Windows process-tree
qualification remains a Phase 10 return marker.

## Implemented contract

The `llama-bench` path uses a reusable bounded receipt primitive. It captures
stdout/stderr independently with a 2 MiB retention limit while draining excess
output, wall time, exit code, timeout, output-limit, OOM, and non-zero-exit
classifications. Unix launches use a dedicated process group and kill the group
on timeout; Windows job-tree qualification remains deferred to a Windows host.

Offline probes share an exclusive calibration lease. The evaluator uses three
measured repetitions, parses standard-deviation/repetition fields, rejects
malformed, non-finite, or negative metrics, checks independent wall-clock
plausibility, and captures optional cross-platform GPU VRAM/temperature
telemetry. Missing sensors remain explicitly degraded rather than treated as
zero.

Cancellation drops the exact child and timeout-kills its dedicated Unix
process group. Calibration journal transitions are written before each
planned/started/finished trial. Active local inference is rejected during
Calibration preflight, so no hidden server state is restored.

## Focused evidence

- Missing managed binary returns an actionable error.
- Fake OOM, non-zero, timeout, malformed JSON, non-finite, implausible, and
  oversized-output fixtures fail closed.
- Timeout kills the Unix descendant fixture.
- Lease contention is represented by `try_acquire_bench_lease`.
- Focused bench-runner tests pass; clippy and the Windows GNU check pass.

## Real managed help receipt

| Item | Value |
|---|---|
| Binary | `~/.config/llama-monitor/bin/llama-bench` |
| Binary SHA-256 | `641a7ccc957ddb0fdcb304a02ec1809aa18ac88d7b498a43c7dedd5ba75290c3` |
| `--help` exit | `0` |
| Help stdout SHA-256 | `3adc817fe09602bd6004d85cc94fee6eb5f8db251f2ac81d8a9c2af874e9ebc8` |
| Help stderr SHA-256 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` (empty) |

## Real managed-model trial

The local inventory is populated with both standalone-looking GGUFs and
companion/speculative artifacts. The smallest candidate,
`Qwen3.5-9B-The-Defiant-Fable-Uncnr-Heretic-NEO-MAX-MTP-Q4_K_M.gguf`, was a
valid managed-binary fixture. Earlier generic context failures came from other
candidates and did not establish that the inventory was unusable.

The exact managed binary completed this bounded macOS trial:

```text
llama-bench --offline --no-warmup -r 3 -p 512 -n 128 \
  -b 512 -ub 512 -m <config-root>/models/gguf/ \
  Qwen3.5-9B-The-Defiant-Fable-Uncnr-Heretic-NEO-MAX-MTP-Q4_K_M.gguf -o json
```

Receipt summary:

| Field | Value |
|---|---|
| Model type | `qwen35 9B Q4_K - Medium` |
| Model size | `6,969,004,032` bytes |
| Parameters | `9,197,093,888` |
| Build | `cb26014d9`, `10310` |
| Backend | Apple M5 Max, `MTL,BLAS` |
| GPU layers | `-1` (automatic full offload) |
| Prompt | `2273.411657 tok/s`, stddev `387.175163`, 3 samples |
| Generation | `70.428617 tok/s`, stddev `0.111508`, 3 samples |
| Result | JSON parsed successfully; no timeout/OOM/non-zero exit |

This closes the macOS real-model trial requirement. It is a bounded evaluator
receipt, not a claim that every GGUF in the inventory is compatible. Larger or
MTP/mmproj companion files still need model-specific selection and may fail
context creation.

## Explicit return marker

Native Windows process-tree/job cleanup and a Windows `.exe` receipt remain
Phase 10 Windows-host gates.
