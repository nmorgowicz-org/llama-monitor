# Calibration Phases 5–8 source-side evidence

Date: 2026-08-16

This receipt closes the remaining macOS/source-side evidence reconciliation
before the native Windows qualification pass. It records test evidence and
the intentionally bounded product scope; it does not infer native Windows
runtime behavior from cross-compilation.

## Phase 5 apply/rollback/resume evidence

The focused Rust tests passed outside the sandbox so the fake runtime could
exercise its loopback lifecycle:

| Test | Result |
|---|---|
| `post_apply_fake_runtime_persists_passed_validation` | passed (2 matching tests) |
| `post_apply_fake_runtime_rolls_back_failed_validation` | passed (same focused run) |
| `resume_fake_runtime_reuses_finished_results_once` | passed |
| `server_qualification` focused suite | 6 passed |
| Full Rust suite (`cargo test -- --test-threads=1`) | 1,316 passed, 14 ignored |

The first two tests prove a successful post-apply validation is persisted and
that a failed validation removes the derived preset/restores the source. The
resume test proves finished trial results are reused exactly once while
unfinished work remains resumable. The server-qualification suite proves
health/readiness failure, bounded response handling, tool correctness,
unsupported tracks, cancellation, and cleanup behavior in the deterministic
fake runtime.

## Phase 8 finding policy

The minimum 2.0 finding states are represented by existing typed evidence and
are documented here rather than by a broad Doctor redesign:

| User-facing state | Source-backed evidence | Product behavior |
|---|---|---|
| Current | exact `POST /api/calibrations/match` fingerprint | May be shown as measured evidence and reused. |
| Stale / incompatible | compatible or related match warnings, or no match | Never presented as exact; user must confirm or calibrate. |
| Noisy | `CalibrationAnalysis.noise_warning` and spread/confidence fields | Show warning; do not silently promote the noisy row. |
| Regressed | post-apply validation failure and rollback record | Restore the prior preset immediately; never keep the failed candidate. |
| Resumable | suspected-crash journal state and explicit resume confirmation | Finished rows are reused; suspected rows are not retried silently. |
| Missing tool | managed sibling capability preflight error/degraded evidence | Disable only the optional predictive/track capability; measured calibration remains available. |

This is the 2.0-safe boundary: findings remain attached to the calibration
receipt and existing UI review surfaces, while a canonical aggregate Doctor
endpoint and broad legacy tuning consolidation remain post-2.0 work. No
generic Tune card may convert a non-exact or degraded finding into an
actionable llama.cpp patch.

## Validation commands

The following source-side gates passed on this checkout:

```text
cargo clippy -- -D warnings
cargo test -- --test-threads=1
cargo test --test auth_routing -- --test-threads=1
cargo test post_apply_fake_runtime -- --test-threads=1
cargo test resume_fake_runtime_reuses_finished_results_once -- --test-threads=1
cargo test server_qualification -- --test-threads=1
npm run validate-js
npm run lint
npm run validate-rebrand
npm run validate-release-contract
git diff --check
cargo build --release
cargo fmt --all -- --check
cargo check --target x86_64-pc-windows-gnu
```

All commands were run with repository-relative paths. Loopback-dependent Rust
tests were run outside the sandbox; the sandbox-only run is not treated as
evidence.

## Explicit return markers

- Native Windows `.exe` process-tree, application-home, updater, package,
  tray/WebView2, sensor-bridge, and screenshot receipts belong to the Windows
  handoff and remain open.
- A full aggregate Doctor endpoint and broad MoE/Rapid-MLX calibration are not
  2.0 release blockers.
