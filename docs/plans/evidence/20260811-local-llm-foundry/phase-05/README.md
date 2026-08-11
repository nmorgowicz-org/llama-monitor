# Phase 5 path/config/resource default-root receipt

Phase 5 is in progress. Fresh installs now resolve the canonical
`local-llm-foundry` root, while populated legacy installs remain active until
explicit migration. The selected root is installed centrally for consumers that
run without an `AppConfig` parameter.

## Verified

- `AppPaths` selects canonical for fresh installs, canonical when populated,
  and legacy only when it is the populated compatibility root.
- Config, tokens, certificates, logs, tray, HF token/cache helpers, model cache,
  chat templates, Rapid-MLX stores, sidecars, scripts, and updater helpers use
  the selected-root authority.
- Windows early logging receives the selected `logs` directory.
- macOS/Linux canonical policy is frozen to `~/.config/local-llm-foundry`; the
  macOS Application Support path remains only a legacy certificate compatibility
  probe.
- Active helper scripts and current Windows/model-library docs use canonical
  defaults while preserving explicit paths and legacy-active behavior.
- Path-authority and Phase 0 literal validators pass.

## Remaining gate

- Audit and receipt-rewrite absolute paths persisted in user JSON/SQLite/model
  metadata. This is serialized with the model-library relocation work in Phase 6
  so no path is rewritten twice.

## Verification

| Check | Result |
|---|---|
| `node scripts/validate-phase2-path-authority.mjs --validate` | passed |
| `node scripts/validate-rebrand-phase0.mjs --validate` | passed |
| `cargo test` and `cargo clippy -- -D warnings` | passed in current closure run |
| `cargo check --target x86_64-pc-windows-gnu` | passed; compile-only from macOS |
| `npm run validate-js` / `npm run lint` | passed |

