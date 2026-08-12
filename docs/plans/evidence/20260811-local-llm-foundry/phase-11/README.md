# Phase 11 — CI, release, packaging, and repository cutover

Status: **repository cutover and PR identity complete; release artifact gates remain open** (2026-08-12).

This receipt covers the source-controlled portion and authorized GitHub repository
cutover. It does not claim that a 2.0.0 release has been published.

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
