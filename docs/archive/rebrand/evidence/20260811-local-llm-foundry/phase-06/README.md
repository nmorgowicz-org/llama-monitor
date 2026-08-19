# Phase 6 model-library relocation receipt

Phase 6 is complete for the source-level macOS/Linux and cross-target contract.
The explicit model-root keep/move planner is integrated with the authenticated
model API and Migration settings center. Native Windows execution remains an
explicit Phase 12 return marker.

## Verified

- `ModelRootChoice::KeepLegacy` records the selected external root and requires
  zero copy bytes.
- `MoveIntoFoundry` inventories GGUF/MLX/Transformers, Rapid-MLX runtimes,
  partial downloads, Hugging Face caches, and unknown entries without filename
  inference beyond explicit directory classes.
- Symlinked model entries and non-empty destinations fail before mutation.
- Plan IDs are deterministic over choice, roots, inventory, content hashes,
  persistence rewrites, and retained roots.
- Move execution is copy-first, SHA-256 verifies every copied file, retains the
  source, writes a checkpoint journal and receipt atomically, resumes from
  completed entries, and is idempotent on replay.
- `KEEP_LEGACY_MODEL_ROOT` writes a receipt-backed root-selection marker without
  copying or deleting model resources.
- Authenticated `/api/models/root-relocation/status` and `/preview` plus
  db-admin-gated `/execute` expose the exact plan and stale-plan protections.
- The Migration settings center exposes Keep Legacy and Copy into Foundry choices;
  both retain the source until a later explicit cleanup action.
- Persisted absolute model paths are rewritten once only when the move receipt
  authorizes the relocation.
- Execution revalidates previews with the same persistence-file inputs used to
  create them, so authorized path-rewrite plans do not produce a false stale
  preview rejection.

## Explicit return marker

- [ ] Run native Windows model-root qualification on the user’s Windows machine:
  `%APPDATA%` source/destination, reparse-point refusal, ACL-preserving copy,
  cross-volume resume, incomplete-pair inventory, sidecar/cache receipts, and
  restart activation of the persisted selection marker.
- [x] Cross-volume behavior is represented by copy-first execution and tested on
  the host filesystem; no same-volume rename optimization is required.
- [x] Incomplete downloads, sidecars, caches, unknown entries, and persisted
  absolute paths are included in the preview and receipt inventory.

## Verification

| Check | Result |
|---|---|
| `cargo test models::root_relocation` | passed |
| `cargo test` | 1,236 passed, 13 ignored (current host run) |
| `cargo clippy -- -D warnings` | passed in current closure run |
| Phase 0/2/3 validators | passed in current closure run |
| `cargo test root_relocation` | passed after API/UI integration |
| `npm run validate-js` | passed with model-root migration module |
