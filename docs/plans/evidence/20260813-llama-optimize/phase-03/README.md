# Calibration Phase 3 — evaluator hardening (in progress)

Date: 2026-08-13  
Status: partial implementation; phase remains open.

The existing `llama-bench` sweep path now has a reusable bounded receipt
primitive (`run_bench_receipt`) and the legacy sweep helpers route through it.
The receipt captures stdout/stderr independently with a 2 MiB limit, wall
time, exit code, timeout, output-limit, OOM, and non-zero classifications. It
retains direct-child `kill_on_drop` cleanup on every platform.

Focused evidence:

- missing managed binary returns an actionable error;
- a fake benchmark emitting OOM is classified as `BenchFailureKind::Oom`;
- output truncation is represented in the receipt;
- focused bench-runner tests pass, clippy passes, and Windows GNU check passes.

Remaining Phase 3 gates are intentionally open: process-tree cleanup on
native Windows/Unix, exclusive calibration leases, warmup/repetition policy,
structured JSON validity/plausibility checks, telemetry snapshots, cancellation
receipts, real managed-bundle help/qualification, and server restoration
fingerprint checks.
