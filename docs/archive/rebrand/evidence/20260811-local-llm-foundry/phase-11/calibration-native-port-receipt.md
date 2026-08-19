# Calibration native-port receipt

Date: 2026-08-13  
Commit: `fbb078f`  

## Ported boundaries

| Area | Foundry implementation | Upstream source credit |
|---|---|---|
| Bounded orthogonal arrays | `src/calibration/design.rs` generates L9/L25 using a prime-field construction with pairwise-balance tests | `bigattichouse/robust`, `optimize/taguchi/src/lib/arrays.c`, commit `a457b7f7f4a7a06b183fd55be4b8aced5d7f2541`, CC0-1.0 |
| Regular-file safety | `src/calibration/paths.rs` classifies metadata before canonicalization; `src/calibration/executor.rs` uses the helper for GGUF inputs | Upstream portability finding; no upstream source copied |

The design generator is deliberately bounded to the release-required L9/L25
shapes. It does not link `robust`, invoke its binaries, parse upstream human
output, or import `llama-optimize.py`.

## Validation

```text
cargo fmt                         PASS
cargo clippy -- -D warnings      PASS
cargo test calibration:: --lib   PASS — 17 tests
cargo test                       PASS
npm run validate-js               PASS
npm run lint                      PASS
git diff --check                  PASS
cargo build --release             PASS
```

The focused tests prove L9/L25 dimensions, level bounds, pairwise balance,
fail-closed column limits, regular-file acceptance, directory rejection,
missing-path classification, and Unix symlink rejection. Windows symlink
behavior remains a native Windows return marker.

## Gate result

**Pass for the native portability foundation.** Balanced executor integration,
noise/confidence/Pareto analysis, oracle fixtures, and native Windows receipts
remain open. The upstream notices and update procedure are in
`THIRD_PARTY_LICENSES.md`.
