# Phase 1 identity and Token Ingot closure receipt

## Objective

Turn the accepted Token Ingot direction into one deterministic production
master, a complete derivative matrix, and a machine-readable identity contract.

## Production identity

- Master: `assets/brand/token-ingot.svg`
- Variants: dark, light, current-color monochrome, one-color black, one-color
  white, and macOS template SVGs.
- Browser/PWA mark: `static/icon.svg` plus registered PNG/ICO derivatives.
- Human-readable contract: `docs/reference/identity-contract.md`.
- Machine-readable contract: `docs/reference/identity-contract.json`.
- Named CSS brand tokens are separate from semantic success/warning tokens.

## Derivative matrix

`export-matrix.json` is the reproducible SHA-256 inventory. It covers:

- Favicon ICO and SVG.
- Apple touch `180` PNG.
- Regular PWA `192` and `512` PNGs.
- Separately padded maskable PWA `192` and `512` PNGs.
- Tray/package sizes `16, 20, 22, 24, 32, 64, 128, 256, 512, 1024`.
- macOS `token-ingot.icns`, Windows `token-ingot.ico`, Linux hicolor PNGs.
- Transparent `1200x630` social artwork with no invented slogan.

All raster exports are generated from the SVG master by
`scripts/generate-brand-derivatives.mjs`. `small-size-contact-sheet.png` is the
human-readable legibility proof; `native-platform-contact-sheet.png` covers
dark/light tray lanes for macOS, Windows, and Linux. `raster-alpha-report.json`
records dimensions and transparent-pixel checks.

## Verification

| Check | Result |
|---|---|
| SVG XML/safety checks | PASS; see `svg-safety-report.json` |
| Export hashes and required sizes | PASS; see `export-matrix.json` |
| Small-size contact sheet | PASS; copper center remains legible at 16–512 px |
| Native platform contact sheet | PASS; dark/light macOS, Windows, and Linux lanes |
| Transparent-background proof | PASS; see `raster-alpha-report.json` |
| ICO/ICNS/package exports | PASS; Pillow ICO and `iconutil` ICNS exports validated |
| Binary static registration | PASS; `build.rs` uses `include_bytes!` and byte replies for binary assets |
| Manifest/favicon/apple-touch URLs | PASS |
| Binary route MIME/byte tests | PASS; `cargo test --test static_assets` (3 passed) |
| Release build | PASS; `cargo build --release` |
| Release-built welcome/auth UI | PASS; `welcome` capture refreshed |
| JavaScript validation/lint | PASS |
| Visual similarity review | PASS for local repository review; no legal clearance claim |

## Approval and caveat

Nick’s explicit Token Ingot selection and validation-sheet acceptance are
recorded in `production-master-approval.md`. The local similarity review found
no blocking conflict. External trademark/domain clearance remains a launch
owner responsibility before the public announcement.

## Gate

**PASS.** One approved deterministic master, complete derivatives, reproducible
export receipt, binary-safe static generator, visible UI integration, and
small-size proofs are present. Phase 2 may proceed.

## Rollback

Revert only Phase 1-owned brand, build-generator, manifest, and test files. Do
not delete the legacy root, model data, browser keys, or historical concept
files.
