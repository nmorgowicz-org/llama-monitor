# Phase 0 baseline evidence and closure receipt

This directory is the source-backed baseline for the Local LLM Foundry 2.0
rebrand. It is append-only evidence; it does not authorize implementation
changes or cleanup of legacy data.

## Receipt metadata

- UTC capture: `2026-08-11T13:12:08Z` (initial checkout receipt)
- Checkout: `50c5fcc68f15c1bea940d73ecf05156459bddb76`
- Branch: `feat/rapid-mlx-integration`
- Initial worktree: only the pre-existing untracked `docs/brand/` and
  `docs/plans/20260811-local_llm_foundry-rebrand.md`; no implementation files
  were modified before the baseline was captured. Later implementation edits
  belong to their owning phases and are not attributed to Phase 0.

## Commands and results

| Command | Result |
|---|---|
| `git rev-parse HEAD` | `50c5fcc68f15c1bea940d73ecf05156459bddb76` |
| `date -u '+%Y-%m-%dT%H:%M:%SZ'` | `2026-08-11T13:12:08Z` |
| `rg -i -l 'llama-monitor|llama_monitor|llama monitor'` (archive excluded) | src 47 files, static 34, tests 78, docs 46, scripts 6, .github 7 |
| same scan, match count | src 246, static 101, tests 3874, docs 460, scripts 15, .github 61 |
| browser storage literal scan | 21 direct literal keys plus 10 named key constants; see `browser-storage.md` |
| source path consumer scan | `src/config.rs`, `src/main.rs`, `src/agent.rs`, `src/certs.rs`, `src/tray.rs`, model/library, HF, Rapid-MLX, and launch consumers; see `path-inventory.md` |

## Supplemental closure capture

- Generated UTC is recorded in the `timestamp_utc` fields of the JSON
  receipts and in `raw-inventory-commands.log`.
- The complete machine-generated receipts are `source-inventory.txt`,
  `path-consumers.tsv`, `identity-contract.json`, `resource-inventory.json`,
  `api-auth-matrix.tsv`, `browser-storage-inventory.tsv`,
  `release-surface-inventory.tsv`, `brand-asset-inventory.tsv`,
  `screenshot-scenario-manifest.json`, and `classification.tsv`.
- `raw-inventory-commands.log` preserves command stdout/stderr and exit codes;
  `produced-file-manifest.tsv` hashes every receipt (excluding itself).
- The fail-closed validator is
  `scripts/validate-rebrand-phase0.mjs`; it classified 5,892 current source
  matches and verified the produced-file hashes.
- Live roots were inspected with `lstat` metadata only: no file contents,
  database pages, token values, certificate material, or symlink targets were
  read. No symlink was followed. User-specific path components are replaced by
  deterministic redaction tokens while structural resource names remain
  visible.
- No checked-in Windows root fixture existed. `resource-inventory.json` records
  the required synthetic Windows cases (WAL, partial, reparse/symlink, and
  unknown entries) for the Phase 3 fixture owner rather than inventing live
  Windows data.

## Read set

The Phase 0 read set was: `AGENTS.md`; this plan; `docs/agents/security-details.md`,
`docs/agents/platform-details.md`, `docs/agents/playwright.md`; and current
references for model library, binary lifecycle, remote agent, Windows support,
CLI flags, API, UI design patterns, screenshot workflow, release workflows, and
`.github/release-please/**`. Historical plans and archive material are treated
as immutable evidence, not current product copy.

## Gate

**PASS for Phase 0 closure.** Every identity, path, API, browser-storage,
release, screenshot, and resource entry has an owning phase and policy. The
fail-closed checker rejects any new unclassified old-name match, the produced
file manifest matches disk, and the original baseline receipt proves that no
implementation file changed before Phase 0 capture. Phase 1 may proceed.

Open implementation risks remain intentionally fail-closed: Windows startup
ordering, direct path consumers, updater asset parsing, and binary static-byte
embedding must be fixed by their owning phases before release gates can pass.
