# Phase 4 application-home migration core receipt

Phase 4 is in progress. The restartable Stage A core is implemented and
validated; native permission/free-space/process qualification and receipt-scoped
cleanup remain explicit gates before the phase can close.

## Verified in this slice

- Root classification occurs before destination initialization.
- Live API/CLI queue requests write a durable marker; maintenance startup
  consumes it before normal resource initialization.
- Per-entry atomic staging, journaled checkpoints, resume, stale-plan rejection,
  and matching-receipt idempotency are implemented.
- Encryption key bytes are copied unchanged; SQLite uses `ChatStorage::backup()`
  and WAL/SHM sidecars are not independently copied.
- Models and recreatable runtime trees remain at the legacy root and are
  recorded as retained resources.
- Symlink, special-entry, unsafe-relative-path, collision, changed-source, and
  active-lock cases fail closed before unrelated writes.
- Authenticated status/preview/queue and receipt-scoped rollback
  preview/queue routes exist; rollback preserves the legacy source.
- Headless migration and rollback CLI equivalents exist.

## Remaining closure gates

- Add native permission-denied fixtures and verify free-space behavior on native
  filesystems (the cross-platform preflight is implemented).
- Add active legacy-process refusal and restart/kill injection at every journal
  checkpoint on native platforms.
- Add receipt-scoped cleanup endpoint/CLI with exact target validation.
- Execute Windows native migration/rollback/tray/task/package checks on the
  Windows development machine; macOS only supplies the target-aware compile
  check.

## Current verification

| Check | Result |
|---|---|
| `cargo test` | 2,361 passed, 26 ignored |
| `cargo test app_migration` | passed, including lock/rollback/schema/error tests |
| `cargo test --test app_home_migration_fixtures` | 6 passed |
| `cargo clippy -- -D warnings` | passed |
| `cargo build --release` | passed |
| `node scripts/validate-rebrand-phase0.mjs --validate` | passed |
| `node scripts/validate-phase2-path-authority.mjs --validate` | passed |
| `node scripts/validate-phase3-migration.mjs --validate` | passed |
| JS validation/lint and `git diff --check` | passed |
| `cargo check --target x86_64-pc-windows-gnu` | passed; compile-only from macOS |
