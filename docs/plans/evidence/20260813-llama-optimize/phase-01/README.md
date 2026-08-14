# Calibration Phase 1 evidence

Date: 2026-08-13

## Implemented

- Durable snapshots and append/fsync journals under the active
  `AppPaths.calibration_jobs_dir()` root.
- Protected job manifests and per-trial result journals for explicit restart
  recovery.
- Public receipt projection separate from the protected on-disk receipt.
- Authenticated list, poll, cancel, explicit resume, and terminal forget
  lifecycle operations.
- Filename-safe job identifiers; traversal, absolute-path, symlink, and
  non-directory job paths fail closed.
- Resume requires `RESUME_CALIBRATION`, refuses changed presets, abandons only
  unfinished suspected-crash trials, and reloads finished measurements without
  silently repeating them.
- Forget requires `db-admin-token` and `FORGET_CALIBRATION`; active jobs cannot
  be deleted.

## Focused validation

- Calibration tests: 28 passed.
- Auth routing tests: 39 passed; calibration route auth uses the same
  constant-time token helpers.
- API route smoke tests: 59 passed, including preflight/start/list/poll,
  receipt, apply, rollback, cancel, resume, and forget paths. Every route
  returned `401` without credentials rather than disappearing as `404`.
- Clippy and formatting pass.

## Remaining return markers

- Malformed JSON and destructive-field rejection matrix remains in the Phase 10
  API/security sweep.
- Native Windows restart/cancellation receipts remain a Phase 10 host gate.
