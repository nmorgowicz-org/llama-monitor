# Evidence Drift Report: 2026-07-18 Audit → 2026-07-19 Phase 0

## Rapid-MLX v0.10.12 Release Page

- URL: https://github.com/raullenchai/Rapid-MLX/releases/tag/v0.10.12
- Tag: v0.10.12
- Created: 2026-07-17T16:22:14Z
- Status: stable (not prerelease)
- Drift since audit (2026-07-18): NONE. Release still exists, unchanged.

## Audited Commit: 75b1fe3b3a8ab12967f64150524296f179dd9979

- URL: https://github.com/raullenchai/Rapid-MLX/commit/75b1fe3b3a8ab12967f64150524296f179dd9979
- Verified: EXISTS, intact
- Parent: d56ae629a9c4fd6d76ef713a7342bd1bf19a8dad (v0.10.12 release commit)
- Message: docs(reference): document 0.10.12 opt-in cache flags (#1129)
- Drift: NONE. Commit is immutable.

## Release Tag vs Audited Commit

- v0.10.12 (d56ae62) → audited (75b1fe3): 1 commit ahead (documentation-only)
- Audited commit is on main branch at time of audit.

## Current Main HEAD

- Current main HEAD: 73034c55118e4300cfd3e26c610a8d159467532e
- Audited commit: 75b1fe3b3a8ab12967f64150524296f179dd9979
- Drift: main has advanced significantly since audit (new commits after 75b1fe3).
- Implication: Phase 0 pins to audited 75b1fe3 as authoritative; current main is tracked separately.
- Latest tag remains v0.10.12. No newer tags published.

## llama.cpp b10068: 571d0d540df04f25298d0e159e520d9fc62ed121

- URL: https://github.com/ggml-org/llama.cpp/commit/571d0d540df04f25298d0e159e520d9fc62ed121
- Verified: EXISTS, intact
- Message: model: rotate injected K/V cache for DFlash (#25823)
- Drift: NONE. Commit is immutable.

## Installed Runtime vs Audit

- Audit target: v0.10.12
- Installed: v0.10.12 (updated from v0.10.10)
- Status: MATCH. Runtime now aligns with audited version.
- CLI evidence (serve --help) refreshed to v0.10.12. Only additions vs v0.10.10: --hybrid-cache-entries, --response-cache-entries (non-breaking).
