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
| Isolated Playwright UI suite (initial) | 265 passed, 5 skipped, 1 flaky, 4 failed; all failures were diagnosed and repaired |
| Targeted repaired specs | Import-lab 4/4; model inventory + KV use-case 9/9; preset save 1/1; Rapid-MLX template 1/1; SPA deep-link 1/1 |
| Final isolated Playwright UI suite | 270 passed, 5 intentional skips, 0 failures, 0 flaky |

The inventory-cache/platform-state fix removes duplicate startup/library
requests and preserves provisional Rapid-MLX cards. Wizard and SPA tests now
wait on their actual initialization contracts and use deterministic fixtures.
The stale TLS placeholder expectation was corrected from
`llama-monitor.example.com` to `foundry.example.com`.

## Closure

Phase 8 is closed. The final release-built browser suite passed with 270 tests,
5 intentional skips, and no failures or flaky retries.
