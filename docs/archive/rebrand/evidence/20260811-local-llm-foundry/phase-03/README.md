# Phase 3 migration protocol receipt

Phase 3 is complete for the migration specification boundary. Mutation remains
owned by the later application-home migration core phase; this receipt proves
that planning, classification, schemas, and public error behavior are executable
before that mutation phase advances.

## Verified in this slice

- Versioned plan, journal, request, and receipt schemas exist.
- Root inventory refuses symlinks, special entries, unsafe relative paths, and
  non-empty destinations.
- Plan IDs are deterministic over source, destination, inventory, and retained
  resources.
- Execute re-plans before mutation and rejects changed source state as stale.
- A matching receipt makes re-execution idempotent and returns the prior receipt.
- SQLite migration uses `ChatStorage::backup()`; WAL/SHM files are not copied.
- Status and preview require `api-token`.
- Queue requires `db-admin-token`, exact confirmation, and a matching plan id.
- Queue writes are serialized and duplicate queue requests are rejected.
- Queue request bodies use `#[serde(default)]` for forward-compatible parsing.
- API reference documents status, preview, queue, restart, and retention rules.
- The fixture matrix covers fresh/empty, old-only, both/conflict, partial
  downloads, HF/model/runtime retention, critical TLS/cert/token/chat DB/WAL,
  symlink/special-entry refusal, custom roots, and cross-volume simulation.
- Public migration failures expose stable sanitized `error_code` values while
  retaining local filesystem diagnostics only in local logs/receipts.
- Journal and receipt schemas accept older payloads with missing optional fields.

## Verification

| Check | Result |
|---|---|
| `cargo test --test app_home_migration_fixtures` | 6 passed |
| `cargo test app_migration` | passed, including schema/error-code tests |
| `cargo test` | 2,350+ passed in final closure run |
| `node scripts/validate-phase3-migration.mjs --validate` | passed |
| `cargo fmt --check` | passed |
| `git diff --check` | passed |

## Deferred to mutation and native qualification phases

- Filesystem free-space checks, stale-lock recovery, interrupted-copy resume,
  process refusal, and native permission-denied behavior belong to Phase 4 and
  Phase 12 because they require mutation/runtime/platform harnesses.
- Malformed JSON and rate-limit behavior use shared API infrastructure and are
  rechecked in the Phase 12 end-to-end matrix.
