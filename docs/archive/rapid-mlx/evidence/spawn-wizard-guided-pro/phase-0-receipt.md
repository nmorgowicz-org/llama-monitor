# Spawn Wizard Guided/Pro — Phase 0 evidence receipt

Captured 2026-08-08 from `d6189534099fb913a5c694f491c8fe52533de723` on branch
`feat/rapid-mlx-integration`; dirty list before the packet: none. Release binary identity:
`target/release/llama-monitor` (fresh `rtk cargo build --release`, 0 crates compiled;
`target/release/llama-monitor --version` => `llama-monitor 1.8.1`).

## Sources and checked contracts

- Controls: `static/js/features/spawn-wizard-groups.js::CONTROLS`; llama and Rapid grouping sources are
  `spawn-wizard-llama-ia.js::GROUPS` and `spawn-wizard-mlx-ia.js::GROUPS`.
- Payload authority: `spawn-wizard-spawn.js::buildSpawnPayload` and
  `spawn-wizard-rapid-mlx.js::buildRapidMlxConfig`; preset authority:
  `src/presets/mod.rs::ModelPreset`.
- The generated, checked-in 46-control contract is
  `tests/ui/core/fixtures/spawn-wizard-control-contract.json`. Its serializer/preset links are
  deliberately `human-audited`: the current code has no common semantic field catalogue, so Phase 0
  must not pretend that a DOM-id-to-Rust-field join is automatic.
- Route inventory: `route-inventory.json`; registered-capture inventory: `capture-inventory.json`.
- Fixture freeze: `fixture-freeze.json`. These are identifiers and required state only; no secret,
  real model path, or inferred runtime support is captured.

## Commands and G0 outcome

1. `rtk cargo build --release` — passed (0 crates compiled).
2. `rtk env SCREENSHOT_PORT=17800 node tests/ui/capture/index.mjs --scenario spawn-wizard` — exit 0,
   but produced only `model-profiles`, `model-source-cards`, `model-hf-base`, and
   `model-discover-trending`; it stopped before its remaining registered `captureShot` call sites
   without reporting a failure. Raw console record: `g0-spawn-wizard.log`. This is broken/skipped evidence, not a passing baseline.
3. `rtk env SCREENSHOT_PORT=17814 node tests/ui/capture/index.mjs --scenario spawn-wizard-engines` —
   exit 0 and freshly produced the 13 files recorded in `capture-inventory.json`; full raw
   stdout/stderr is `g0-spawn-wizard-engines.raw.log`. Its Rapid outputs were written below
   `wizard-llamacpp`, so they remain mis-grouped (diagnosis not resolved).
4. `rtk node scripts/validate-wizard-groups.mjs --write-contract` then
   `rtk node scripts/validate-wizard-groups.mjs` — passed: all 46 IDs exist exactly once and the
   checked-in contract matches.

## Contradictions frozen for remediation

- `spawn-wizard.js::_initViewMode` persists a selector but logs that Pro is not implemented.
- Existing `docs/screenshots/artifacts/wizard-llamacpp/spawn-wizard--llamacpp-local--pro-*.png`
  have no current capture producer.
- `spawn-wizard-engines` is registered as `wizard-llamacpp` although it emits Rapid-tagged images.
- The current `spawn-wizard` runner can complete successfully after silently omitting requested
  shots. Phase 1 must make this fail.
- `docs/reference/spawn-wizard.md` and the Phase 10 companion describe prior completion/tier claims;
  this receipt does not rewrite historical status. Current source remains the authority.

## Gate status

G0 is diagnostic-only. This packet is an evidence freeze awaiting verifier approval; visual
acceptance is explicitly deferred to Phase 1 after the harness becomes strict.
