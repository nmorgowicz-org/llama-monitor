# Native Windows Validation Receipt

- Date: 2026-08-12
- Branch: `feat/rapid-mlx-integration`
- Base commit under test: `f8dce76` (`fix(binary): stabilize native Windows validation`).
- Host: Windows 10 Enterprise, x86_64, build 26200
- Rust: stable `1.97.1`, native target `x86_64-pc-windows-msvc`
- Node: `v24.11.1`; npm `11.6.2`

## Passed

- Native `cargo build --release` with the full default tray/WebView features.
- Native `cargo clippy -- -D warnings`.
- `cargo fmt -- --check`.
- `cargo check --target x86_64-pc-windows-gnu`.
- `npm run validate-js`.
- `npm run lint` after `npm ci` installed the locked dependencies.
- `npm run validate-rebrand`.
- `npm run validate-release-contract`.
- Full native `cargo test` suite passed (5 ignored).
- Executable smoke test: `--version`, `--help`, and disposable headless startup were previously validated.

## Final status

The native Rust/JavaScript validation suite is green. Native Playwright UI
validation is still blocked by Portmaster: Chromium receives
`ERR_NETWORK_ACCESS_DENIED` when it navigates to the locally served app, even
though the release server starts successfully on loopback. The Windows user
must allow the Playwright Chromium executable (or the repository's local
loopback traffic) in Portmaster before rerunning the UI suite.

## Environment blockers

- The GNU cross-target check now passes with the current dependency graph.
- Release preflight could not start WSL2 because Virtual Machine Platform is not
  enabled. Re-run from Git Bash/WSL after enabling that Windows feature.

## Code changes made for Windows portability

- `ssh2` uses native WinCNG on Windows; non-Windows retains vendored OpenSSL.
- Unix-only model-adoption hardlink assertions are gated to Unix.
- Model-adoption symlink fixtures use the platform-specific standard-library APIs.
- Path validation and migration tests use platform-neutral path semantics.
- Rapid-MLX test servers bind retained loopback listeners to avoid Windows startup races.
