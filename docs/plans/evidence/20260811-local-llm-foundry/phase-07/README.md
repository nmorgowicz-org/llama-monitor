# Phase 7 — Package, crate, binary, CLI, and backend identity

Phase 7 establishes `local-llm-foundry` as the canonical Cargo package and
executable while retaining `llama-monitor` as a thin compatibility binary for
the 2.x line.

## Contract

- Cargo package: `local-llm-foundry`.
- Rust library crate: `llama_monitor` (stable internal/test import identity).
- Canonical binary: `local-llm-foundry`.
- Legacy binary: `llama-monitor`, invokes the same shared runner and emits one
  actionable deprecation warning.
- Clap help/version, argv parsing, exit codes, routes, and serialized/API
  identities remain shared.
- Backend technology names (`llama.cpp`, `llama-server`, and `src/llama/**`)
  remain unchanged.

## Implemented surface

- `Cargo.toml` uses `local-llm-foundry` as the package and canonical binary
  identity, with an explicit `llama_monitor` library crate and
  `llama-monitor` compatibility binary.
- Application startup lives in `src/runner.rs`; both thin binaries call the
  same `runner::run` implementation. The compatibility wrapper emits one
  actionable deprecation warning on stderr.
- Clap pins both `name` and `bin_name` to `local-llm-foundry`, so help output is
  canonical and byte-identical even when invoked through the legacy filename.
- `tests/binary_identity.rs` covers version/help parity, warning presence,
  invalid-argument exit-code parity, and canonical usage text.

## Validation receipt

| Check | Result |
|---|---|
| `cargo test --test binary_identity -- --nocapture` | 3 passed |
| `cargo clippy -- -D warnings` | passed |
| `cargo test` | 1,236 passed, 13 ignored |
| `npm run validate-js` | passed |
| `npm run lint` | passed |
| `git diff --check` | passed |
| `cargo build --release` | passed; both release binaries present |
| `cargo fmt -- --check` | passed |
| `cargo check --target x86_64-pc-windows-gnu` | passed |
| Release help comparison | canonical and legacy output identical |

## Explicit Windows return marker

- [ ] Re-run both binaries natively on Windows and verify `.exe` launch,
  console-subsystem behavior, `%APPDATA%` logging, scheduled-task aliases, and
  installed-binary discovery. Cross-target Cargo validation is performed here;
  native execution is reserved for the final Windows-machine pass.
