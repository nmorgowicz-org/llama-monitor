# Phase 6 model-library relocation receipt

Phase 6 is in progress. The explicit model-root keep/move planner is now
implemented; it is pure and deterministic, and no external model or HF cache is
moved implicitly.

## Verified

- `ModelRootChoice::KeepLegacy` records the selected external root and requires
  zero copy bytes.
- `MoveIntoFoundry` inventories GGUF/MLX/Transformers, Rapid-MLX runtimes,
  partial downloads, Hugging Face caches, and unknown entries without filename
  inference beyond explicit directory classes.
- Symlinked model entries and non-empty destinations fail before mutation.
- Plan IDs are deterministic over choice, roots, inventory, and retained roots.
- Move execution is copy-first, verifies every copied file, retains the source,
  writes a checkpoint journal and receipt atomically, resumes from completed
  entries, and is idempotent on replay.
- Model relocation remains separate from application-home Stage A; presets and
  sessions retain absolute paths until a relocation receipt authorizes rewrites.

## Remaining gates

- Add migration-center UI controls for the keep/move decision and native Windows
  model-root qualification.
- Expand large-file/cross-volume and incomplete-pair integration receipts on
  native filesystems.

## Verification

| Check | Result |
|---|---|
| `cargo test models::root_relocation` | passed |
| `cargo test` | 2,375 passed, 26 ignored (host-permission run) |
| `cargo clippy -- -D warnings` | passed in current closure run |
| Phase 0/2/3 validators | passed in current closure run |
