# Windows Build Toolchain — Options for Swapping GNU → MSVC

**Date:** 2026-06-22
**Status:** Exploratory / not scheduled
**Related:** `20260621-windows_support_consolidation.md` (the Windows feature work this
supplements), `../reference/windows-support.md`

---

## 0. Why this doc exists

While validating the Windows tray-popover work on real hardware (2026-06-22) we re-confirmed
that the shipped Windows binary is built with the **GNU** toolchain
(`x86_64-pc-windows-gnu`) via `cross` against a remote Docker engine
(`scripts/build-single-target.sh`). Enabling the WebView2 tray popover (`wry`) on that target
makes the binary **dynamically** link `WebView2Loader.dll`, because the `webview2-com-sys`
crate only static-links the loader when `target_env = "msvc"`:

```rust
// webview2-com-sys 0.38.2, src/lib.rs
#[cfg_attr(target_env = "msvc",     link(name = "WebView2LoaderStatic", kind = "static"))]
#[cfg_attr(not(target_env = "msvc"), link(name = "WebView2Loader.dll"))]
```

So on GNU we must **ship `WebView2Loader.dll` next to the exe** (the bundle is now 3 files:
`llama-monitor.exe`, `sensor_bridge.exe`, `WebView2Loader.dll`). Switching to the **MSVC**
toolchain would static-link the loader and drop us back to 2 files — and unlocks other
flexibility (ARM64, native test execution, MSVC-only crates). This doc captures what that
switch would actually take, the trade-offs, and a recommendation.

**This is not a commitment to switch.** The current GNU + bundled-DLL path works and is the
lowest-risk way to ship the popover today.

---

## 1. Current state (baseline)

| Aspect | Today |
|---|---|
| Target | `x86_64-pc-windows-gnu` |
| Builder | `cross build` on a remote Docker engine (`CROSS_REMOTE=1`), runner `../llama-monitor-runner` |
| Linkage | `-C target-feature=+crt-static` (CRT statically linked; no mingw runtime DLLs needed) |
| Features | `--no-default-features --features native-tray,webview-popover` |
| WebView2 loader | **dynamic** → `WebView2Loader.dll` bundled in the zip |
| `ssh2` | `vendored-openssl` (Cargo.toml comment: "Non-issue — CI uses windows-gnu") |
| `rusqlite` | `bundled` (compiles SQLite C in-tree) |
| TLS | `rustls` + `ring` (no native OpenSSL link at runtime) |
| Tests on target | None — CI cross-compiles only; runtime verified manually on a Windows box |

Why GNU was chosen historically: it cross-compiles cleanly from Linux in one container
alongside the Linux/macOS targets (macOS via osxcross), and avoids the MSVC CRT + Windows SDK
licensing/provisioning problem. `ssh2`'s `vendored-openssl` builds painlessly with mingw.

---

## 2. What MSVC would buy us

1. **Static WebView2 loader → 2-file bundle.** No `WebView2Loader.dll`; `WebView2LoaderStatic.lib`
   is linked into the exe. Cleanest distribution.
2. **ARM64 Windows** (`aarch64-pc-windows-msvc`). There is no practical GNU ARM64 Windows story;
   MSVC is the only well-supported path. Relevant for Surface / Snapdragon X laptops.
3. **Ecosystem compatibility.** Some crates and future deps assume MSVC (e.g. certain
   `windows`-crate features, anything linking prebuilt MSVC `.lib`s). MSVC is the Rust
   *tier-1, default* Windows target; GNU is tier-1 but less exercised.
4. **Native test execution** (only if we move to a real Windows runner — see Option B). Today we
   cannot run `cargo test` for Windows in CI at all.
5. **Debugging:** real PDBs consumable by Windows debuggers / WinDbg / crash dumps.

---

## 3. The two ways to produce an MSVC build

### Option A — Cross-compile MSVC from the existing Linux runner via `cargo-xwin`

`cargo-xwin` wraps `xwin`, which downloads the **MSVC CRT + Windows SDK** headers/libs and uses
`clang-cl` + `lld-link` to target `*-pc-windows-msvc` from Linux. Stays on the current Linux
ARC runner.

**Work required:**
- Add `cargo-xwin` (and `clang`, `llvm`, `lld`) to `../llama-monitor-runner/Dockerfile`.
- Replace the Windows arm of `scripts/build-single-target.sh`:
  `cargo xwin build --release --target x86_64-pc-windows-msvc --features native-tray,webview-popover`
- First run downloads the SDK/CRT into an xwin cache; pre-warm it in the image so builds are
  offline-stable (mirrors the existing concern about NuGet restore for the sensor bridge).

**Risks / unknowns (must spike):**
- **C/C++ build deps under clang-cl:** `rusqlite` (bundled SQLite C) and any `cc`-driven build
  scripts must compile with `clang-cl`. Usually fine, but verify.
- **`ssh2` `vendored-openssl` on MSVC:** building OpenSSL for `*-windows-msvc` typically needs
  **perl** (and historically NASM). Either provision those, or switch `ssh2` away from
  `vendored-openssl` on Windows (it's a `windows` cfg dep anyway). This is the single biggest
  risk and the reason the Cargo.toml note exists.
- **Licensing:** `xwin` requires accepting the Microsoft SDK/CRT EULA (`--accept-license`).
  Acceptable for a personal project; note it in the Dockerfile.
- **`ring`/`aws-lc`:** ring builds fine for MSVC; confirm no NASM requirement for the chosen
  version.

**Pros:** keeps the single Linux runner; no new infra. **Cons:** the OpenSSL/perl + clang-cl
matrix is fiddly to get green the first time.

### Option B — Build on a real Windows runner (GitHub `windows-latest` or self-hosted)

Use a native MSVC toolchain on Windows.

**Work required:**
- Add a Windows runner (GitHub-hosted `windows-latest` for releases, or a self-hosted Windows
  ARC node to match the current self-hosted model).
- Windows arm of the build does `cargo build --release --target x86_64-pc-windows-msvc ...`.
- **Bonus:** build `sensor_bridge` natively (no cross-publish) **and** run `cargo test` for
  Windows for the first time.

**Pros:** maximum compatibility; native tests; simplest dependency story (MSVC + perl/NASM are
preinstalled on `windows-latest`). **Cons:** splits the build across OSes (Linux for
Linux/macOS, Windows for Windows); new runner to manage; self-hosted Windows ARC is non-trivial
to operate.

---

## 4. Cross-cutting impact (either option)

- **`Cargo.toml`:** the `ssh2 = { features = ["vendored-openssl"] }` line and its "non-issue on
  windows-gnu" comment must be revisited. Likely: gate `vendored-openssl` to non-MSVC, or drop
  it on Windows and rely on a system/provided OpenSSL, or confirm vendored builds under MSVC.
- **`webview2-com-sys`:** no code change — switching `target_env` to `msvc` flips it to the
  static loader automatically. Then **remove the `WebView2Loader.dll` bundling** step from
  `.github/workflows/release.yml` and update the bundle docs to "2 files."
- **`-C target-feature=+crt-static`:** on MSVC this links the static CRT (`/MT`) instead of the
  dynamic UCRT. Decide deliberately: `+crt-static` avoids a VCRuntime redistributable
  dependency on the target (good for zero-touch), at the cost of a larger exe. Keep it for
  parity with today's "no runtime deps" posture.
- **CI lint:** `.github/workflows/ci.yml` currently runs
  `cargo clippy --target x86_64-pc-windows-gnu --no-default-features --features native-tray,webview-popover`.
  Switch the target to `-msvc` (and `rustup target add` / xwin accordingly) so lint matches the
  shipped binary.
- **`docs/reference/windows-support.md`** and the sensor-bridge doc: update toolchain, bundle
  contents, and the WebView2 loader story.

---

## 5. Recommendation

- **Short term: stay on GNU + bundle `WebView2Loader.dll` (3 files).** It ships the WebView2
  popover today with zero toolchain risk and keeps the single Linux runner.
- **If/when we want the 2-file bundle or ARM64 Windows:** spike **Option A (`cargo-xwin`)** on a
  branch. The make-or-break is getting `ssh2`/OpenSSL + `rusqlite` to link under `clang-cl`; time-box
  it. If that proves painful, fall back to **Option B (Windows runner)**, which also unlocks
  native Windows `cargo test` — arguably worth it on its own given we cannot test Windows in CI
  today.
- **Do not** attempt to force the MSVC static `.lib` into a GNU binary — it is COFF/MSVC-ABI and
  will not link against mingw.

## 6. Definition of done for a future swap

- [ ] Windows release builds `*-pc-windows-msvc` green in CI (and the SDK/CRT cache is
      pre-warmed / reproducible).
- [ ] `ssh2`, `rusqlite`, `ring`/rustls all link under MSVC; app runs on a clean Windows box.
- [ ] WebView2 popover works with the **static** loader; `WebView2Loader.dll` removed from the
      bundle (2 files) and from docs.
- [ ] `+crt-static` decision recorded; no VCRuntime redistributable required on target.
- [ ] (Option B only) `cargo test` runs on Windows in CI.
- [ ] CI Windows clippy retargeted to `-msvc`.
- [ ] `aarch64-pc-windows-msvc` added if ARM64 is in scope.
