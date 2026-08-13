# Native Windows qualification receipt — 2026-08-13

- Branch: `feat/rapid-mlx-integration`
- Commit: `f691a2b`
- Host: Windows 10 Enterprise, x86_64 (machine manifest in this directory)
- Binary: `target/release/local-llm-foundry.exe`
- All application-home cases used a unique disposable temporary root; no real
  user profile or model tree was touched.

## Passed

- Native release build after bundled Inter/Fira Code fonts and CSP changes.
- `cargo test --test static_assets`: 4 passed, including the WOFF2 route/MIME
  assertion.
- Fresh canonical startup: canonical root created, tokens/keys/database/certs
  created below it, legacy root not created.
- Legacy-only startup: legacy root remained selected, sentinel retained,
  canonical root was not created.
- Legacy-only migration status reported `legacy_active` with no writes.
- Both-roots migration status reported `conflict` with no writes.
- Both-roots normal startup exited nonzero and instructed the operator to use
  the explicit migration flow.
- Legacy-only migration preview produced a plan ID without creating the
  canonical root; explicitly confirmed execution copied the sentinel and
  preserved both source and destination roots.
- Release-built Windows settings capture passed the deterministic local-font
  assertion and produced the expected settings artifacts.
- Font-scale probe reported root/body sizes of `14.4px`, `16px`, and `19.2px`
  for `0.9`, `1.0`, and `1.2` respectively. Responsive clamp headings remain
  viewport-bounded as documented in the font plan.

## Still open

- Interrupted migration resume/rollback journal matrix.
- Tray/WebView2, sensor bridge, updater, remote-agent, and package/archive
  receipts on this commit.
- Fresh full CI run after all code and evidence changes.
