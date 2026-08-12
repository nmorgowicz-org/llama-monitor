# Native Windows Validation Receipt

- Date: 2026-08-12
- Branch: `feat/rapid-mlx-integration`
- Base commit under test: `f918e69f41575d4935b51c08380063f2a1bee084`, plus the
  uncommitted Windows portability changes in this checkout.
- Host: Windows 10 Enterprise, x86_64, build 26200
- Rust: stable `1.97.1`, native target `x86_64-pc-windows-msvc`
- Node: `v24.11.1`; npm `11.6.2`

## Passed

- Native `cargo build --release` with the full default tray/WebView features.
- Native `cargo clippy -- -D warnings`.
- `cargo fmt -- --check`.
- `npm run validate-js`.
- `npm run lint` after `npm ci` installed the locked dependencies.
- `npm run validate-rebrand`.
- `npm run validate-release-contract`.
- Windows model-adoption tests: 10 passed.
- Full native suite after Portmaster directory trust: all tests passed, 5 ignored.
- Executable smoke test: `--version`, `--help`, disposable headless startup, token/database/key/certificate/log creation.

## Final status

The complete native Windows suite is green. Portmaster must trust the repository
directory so Cargo-generated loopback test binaries can communicate with their
local fixtures.

## Environment blockers

- GNU cross-target check could not build the vendored OpenSSL dependency because
  the Windows Perl installations lack the required OpenSSL Perl module set.
  Native MSVC build is now independent of that dependency through WinCNG.
- Release preflight could not start WSL2 because Virtual Machine Platform is not
  enabled. Re-run from Git Bash/WSL after enabling that Windows feature.

## Code changes made for Windows portability

- `ssh2` uses native WinCNG on Windows; non-Windows retains vendored OpenSSL.
- Unix-only model-adoption hardlink assertions are gated to Unix.
- Model-adoption symlink fixtures use the platform-specific standard-library APIs.
- Path validation and migration tests use platform-neutral path semantics.
- Rapid-MLX test servers bind retained loopback listeners to avoid Windows startup races.
