# Phase 11 — CI, release, packaging, and repository cutover

Status: **repository cutover and PR identity complete; release artifact and UI qualification gates remain open** (2026-08-12).

This receipt covers the source-controlled portion and authorized GitHub repository
cutover. It does not claim that a 2.0.0 release has been published.

## Latest CI qualification and local repair

PR #314 CI run `31603543480` passed all non-UI jobs, including Windows-target
clippy, Windows GNU release smoke, Linux/macOS release smoke, lint, and CodeQL.
The UI job completed with 268 passed, 5 skipped, and 2 failed tests:

- `core/app-shell.spec.js:176` — Gemma recommendation did not persist the
  installed template path in CI retries.
- `core/rapid-preset-visibility.spec.js:99` — Rapid-MLX control reachability
  timed out during repeated section navigation/details expansion.

Full Playwright artifacts are retained at
`/tmp/local-llm-foundry-ci-31603543480/playwright-report/`.

Local release-built diagnosis and repair are complete. The Gemma failure was a
layout race caused by the async VRAM strip appearing under the in-flight click;
the strip now keeps a stable footprint. The Rapid-MLX reachability timeout no
longer reproduces with the stable editor layout. Focused repetitions passed
Gemma 20/20 and Rapid reachability 10/10. The full local suite completed with
267 passed, 5 skipped, and 3 flaky-but-passed-on-retry, with no hard failures.
The remaining flakes are guided-generation startup/toast timing and do not
close remote CI or native Windows gates. A fresh GitHub run is required before
this receipt becomes release sign-off.

## Completed source gates

| Gate | Result |
|---|---|
| Release-please Cargo package identity | `local-llm-foundry` |
| Canonical release workflow assets | Four target builds under `local-llm-foundry-*` names |
| 2.0 compatibility bridge | Four matching `llama-monitor-*` aliases from the same builds |
| Windows archive contract | Canonical and legacy ZIPs require executable, `sensor_bridge.exe`, and `WebView2Loader.dll` |
| macOS archive contract | Canonical and legacy tarballs preserve matching payload names |
| Checksums | Fail-closed exact eight-asset manifest for 2.0.x; canonical-only policy encoded for 2.1.x |
| CI release path filters | Release workflows/configuration/build scripts trigger release-smoke coverage |
| CI identity | Canonical binary and cache keys; external `arc-llama-monitor*` runner labels retained |
| Source contract validator | `npm run validate-rebrand` passed |
| Release contract validator | `npm run validate-release-contract` passed, including frozen bridge fixture |
| Workflow syntax | CI and release YAML parsed successfully |
| Repository state | `git diff --check` passed |
| GitHub repository rename | `nmorgowicz-org/llama-monitor` → `nmorgowicz-org/local-llm-foundry` |
| Old web URL continuity | HTTP 301 redirects to the canonical repository |
| Git continuity | `git ls-remote` old and new URLs returned the same `main` HEAD |
| PR identity | #314 now has title `feat!: launch Local LLM Foundry 2.0 with backend-neutral Rapid-MLX` |
| PR body | Compact migration/architecture summary with 20-entry `BEGIN_COMMIT_OVERRIDE` block |
| Actions/API continuity | New repository API and workflow metadata reachable; runner inventory currently empty |

## Validation commands

```text
cargo fmt -- --check                 PASS
cargo clippy -- -D warnings          PASS
cargo test                            PASS — 1,238 passed, 13 ignored
npm run validate-js                   PASS
npm run lint                          PASS
npm run validate-rebrand              PASS
npm run validate-release-contract     PASS
cargo build --release                 PASS
git diff --check                      PASS
Focused Gemma Playwright repetitions  PASS — 20/20
Focused Rapid reachability repetitions PASS — 10/10
Full release-built Playwright suite  PASS — 267 passed, 5 skipped, 3 flaky retries
Preset-editor screenshot harness      PASS — fresh artifacts inspected
```

## Explicit return markers

These require the release owner and external GitHub state; they must remain open
until the final qualification phase:

1. Generate a release-please PR whose version is exactly `2.0.0` and whose PR
   body carries the breaking-change declaration.
2. Run the release workflow dry run or authorized prerelease and inspect all
   eight assets, archive layouts, checksums, and the frozen 1.x parser probe.
3. Restore/verify self-hosted runner registrations before release execution if
   the empty post-rename runner inventory is not expected.
4. Do not publish the public 2.0 announcement until Phase 12–14 native,
   updater, migration, and artifact probes pass.
