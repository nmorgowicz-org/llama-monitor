# Phase 2 path authority and migration protocol receipt

This receipt records the implementation boundary for the first migration-safe
runtime slice. It does not claim that package publishing, repository rename, or
external model relocation is complete.

## Implemented

- `AppPaths` is the single authority for canonical/legacy roots and resource
  directories used by configuration, certificates, tray logs, HF tokens, and
  the remote agent.
- Default root selection is read-only: a populated canonical root wins;
  otherwise the legacy root remains active until explicit migration.
- Encryption initialization now occurs before `AppConfig` protected-config and
  token-store initialization.
- Migration preview is authenticated with `api-token`.
- Migration queue is authenticated with `db-admin-token`, requires the exact
  confirmation phrase, validates the preview plan id, and writes a durable
  request marker for controlled restart.
- Startup consumes a queued request before resource initialization, re-plans,
  rejects stale plans, executes copy-first migration, and removes the queue
  marker only after a verified receipt is written.
- SQLite remains migrated through `ChatStorage::backup()`.
- Models and recreatable runtime trees remain at the legacy root and are listed
  in the receipt for explicit follow-up migration.
- Legacy and canonical remote-agent binary names are accepted during the 2.x
  bridge; canonical install paths and release assets are preferred.
- The form-auth cookie now uses the Foundry name while accepting the legacy
  cookie; logout expires both names.

## Closed Phase 2 gates

- The reviewed legacy-literal allowlist is generated from current source and
  fails closed on new path-authority literals; all current rows have an owning
  policy and the validator reports zero unowned rows.
- `AppPaths::set_active_root` installs the selected root after pure startup
  inspection, so helpers without an `AppConfig` parameter honor `--config-dir`
  and migration selection (certificates, HF token, model cache, chat templates,
  Rapid-MLX stores, and sidecars).
- Chat-template, model-cache, certificate, HF, runtime-sidecar, and tray/log
  consumers no longer derive a product root independently.
- Windows-only hard-link metadata and file-symlink operations have portable
  implementations; the Windows GNU target check is warning-free.
- Phase 3 migration-fixture completeness remains intentionally separate and is
  not claimed by this receipt.

## Verification

| Check | Result |
|---|---|
| `cargo check --lib` | passed |
| `cargo check --bin llama-monitor` | passed |
| `cargo test app_migration` | 12 passed |
| `cargo test` | 2,349 passed, 26 ignored |
| `cargo test template_overlay` | 14 passed |
| `cargo check --target x86_64-pc-windows-gnu` | passed, 0 errors / 0 warnings |
| `node scripts/validate-phase2-path-authority.mjs --validate` | passed; zero unowned rows |
| `npm run validate-js` | passed |
| `npm run lint` | passed |
| `cargo fmt --check` | passed after formatting |
| `git diff --check` | passed |

The Windows result is a cross-compilation type-check from macOS. It does not
execute a Windows binary or validate Windows installer/UI behavior; those remain
release-environment checks.

## Explicit non-claims

- No live user root was moved or deleted.
- External model roots, HF caches, certificates, and runtime downloads are
  not silently relocated.
- The package name, repository URL, signed installers, and 2.1 canonical-only
  artifact cutover remain later release-gated phases.
