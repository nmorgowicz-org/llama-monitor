# Calibration Balanced planner receipt

Date: 2026-08-13

## Scope

This receipt covers the native deterministic Balanced candidate-planning slice
and its authenticated preflight exposure. It does not claim that the executor
launches Balanced jobs or that analysis, Pareto selection, noise reporting, or
pick verification are complete.

## Contract

- `src/calibration/candidates.rs` owns a typed llama.cpp-only factor catalog.
- Four core factors (context, batch, ubatch, and threads) map to L9 rows.
- A verified `-fa`/`--flash-attn` capability adds a fifth factor and selects L25.
- Every generated patch is applied to a cloned preset and validated through
  `request_from_preset`; Rapid-MLX and invalid context/batch relationships fail
  closed.
- Candidate IDs and row order are deterministic. The baseline remains the
  measured control, and duplicate mapped patches are not scheduled twice.
- Screen, final-array, verification, and total-trial ceilings are checked
  before a plan is returned. No larger design is selected implicitly.
- Preflight accepts an explicit workload and budget, reports deterministic
  Balanced rows, and rejects Thorough. Start remains fail-closed for Balanced
  until executor repetitions and pick verification are implemented.

## Focused validation

```text
cargo fmt                         PASS
cargo clippy -- -D warnings      PASS
cargo test calibration:: --lib    PASS — 20 tests
cargo check                       PASS
git diff --check                  PASS
```

The tests cover stable IDs/order, L9/L25 selection, budget fail-closed
behavior, `ubatch <= batch`, Rapid-MLX rejection, and capability-gated flash
attention. Executor/preflight wiring remains a subsequent slice.

## Gate result

**Pass — deterministic Balanced planner and preflight foundation.** The Phase
4 hard gate remains open until subprocess execution, measured analysis, and the
remaining receipts are implemented and independently validated. Balanced start
is intentionally rejected with an actionable gated message rather than
silently running an unverified single-trial approximation.
