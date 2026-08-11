# Phase 8 Frontend identity and migration UX

## Implemented

- Token Ingot favicon, touch/PWA icons, navigation mark, auth shell, compact
  popover, welcome shell, restart copy, and migration controls use the Local
  LLM Foundry identity.
- `static/js/core/identity.js` is the frontend identity registry. It exports
  canonical product/CLI/repository values, freezes all legacy browser-storage
  keys, and exposes the compatibility registry for diagnostics.
- Main and compact boot paths use the registry. Compact mode remains able to
  read the legacy preferences key and follows the same title/identity policy.
- App-home and model-root migration UX remains non-destructive until the user
  explicitly previews and authorizes the operation; the upgrade toast links
  directly to Settings.
- Auth compatibility retains the legacy cookie read path while writing the
  canonical session cookie.

## Validation receipt

| Check | Result |
|---|---|
| `npm run validate-js` | passed |
| `npm run lint` | passed |
| `cargo build --release` | passed; generated static routes/assets include identity module |
| `node tests/ui/capture/index.mjs --scenario welcome` | passed with elevated local process access; Token Ingot and migration toast visually verified |
| Isolated Playwright UI suite | 265 passed, 5 skipped, 1 flaky, 4 failed on first run |
| Targeted rerun of failed specs | 15 passed, 3 reproducible pre-existing state/request-count failures |

The three remaining UI failures are not identity failures: two assert a single
model inventory request where the current modal initialization makes two, and
one expects a KV use-case selection that is not applied before the assertion.
They must be resolved before the Phase 8 pass gate is closed. The stale TLS
placeholder expectation was corrected from `llama-monitor.example.com` to
`foundry.example.com`.

## Open gate

- [ ] Resolve and rerun the three reproducible UI state/request failures.
- [ ] Rerun the full isolated Playwright suite with zero failures before marking
  Phase 8 complete.
