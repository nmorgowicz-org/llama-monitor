# Calibration Phase 2 — managed tool and argv receipt

Date: 2026-08-13  
Status: source-side gate passed; native Windows execution remains a Phase 10
return marker.

## Scope

Phase 2 now resolves only the exact siblings of the configured
`AppConfig.llama_server_path`:

- `llama-bench` / `llama-bench.exe` (required for measured calibration);
- `llama-fit-params` / `llama-fit-params.exe` (optional predictive helper).

Resolution rejects symlinks and non-regular files before executable identity
hashing. `--help` evidence records the executable identity, bounded output
hash, exact option tokens, exit code, and truncation state. The optional fit
helper degrades predictive pruning only; it never disables measured runs.

Typed benchmark argv is built from `LlamaCppCalibrationPatch` and validated
before execution. It enforces context/depth bounds, `ubatch <= batch`, valid
GPU-layer values, capability evidence for threads/flash-attention/MoE factors,
and preserves model paths with spaces as one argument. No candidate enters
`extra_args` or a shell command string.

## Evidence

| Gate | Receipt |
|---|---|
| Unix/Windows sibling naming | `src/inference/llama_cpp_tools.rs` tests |
| Exact help-token parsing | `flags_from_help` test |
| Path-space and typed argv golden contract | `src/calibration/argv.rs` test |
| Invalid batch and unsupported-factor fail-closed behavior | `src/calibration/argv.rs` test |
| Optional fit-helper degradation | `optional_fit_params` contract |
| Managed bench identity in preflight | `CalibrationPreflight.bench_sha256` |

Focused validation: `cargo test calibration::` — 30 passed. Formatting was
applied with `cargo fmt`. Full clippy, test, release, and Windows GNU checks
remain the phase boundary validation below.

## Native return marker

The source contract and Windows GNU compilation do not prove a native Windows
managed bundle can launch or expose the same flags. Capture real `.exe`
identity/help evidence and a tiny typed argv preflight on the Windows machine
under `phase-10/windows/` before final qualification.
