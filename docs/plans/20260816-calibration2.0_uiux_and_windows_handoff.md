# Local LLM Foundry 2.0 Calibration UX and Windows validation handoff

**Status:** Native Windows Calibration qualification completed on 2026-08-17; broader migration/package and visual-parity receipts remain tracked in the Phase 12/13 ledgers.

**Audience:** A fresh Codex session with no conversational context.

**Repository:** nmorgowicz-org/local-llm-foundry
**Branch:** feat/rapid-mlx-integration
**Release:** Local LLM Foundry 2.0.0

**Source baseline (reviewed 2026-08-17):** `ffe25d8` (`feat(ui): complete Calibration 2.0 surface`).
This is a local baseline only; native Windows evidence must use the final pushed commit after Phase F.

**Authority documents**

- Rebrand: docs/plans/20260811-local_llm_foundry-rebrand.md
- Calibration architecture: docs/plans/20260813-llama_optimize.md
- Real-server contract: docs/plans/20260815-phase7-real_server_qualification.md
- Existing Windows setup: docs/plans/20260812-local_llm_foundry-windows-validation-handoff.md

This handoff adds the missing product-quality layer: Calibration must feel like a premium 2.0 feature, then the completed surface must be qualified on native Windows with evidence a context-free session can verify.

Current native result: the CUDA 13.3 Windows managed runtime completed Quick and Balanced calibration, apply/rollback, cancellation cleanup, and the real-server `latency_memory`, `mtp`, and `ngram` tracks using the Qwen3.5 GGUF and Froggeric v22.1 template. Evidence is recorded in `docs/plans/evidence/20260813-llama-optimize/phase-10/windows/README.md`.

## 1. Current truth and release boundary

The typed backend and basic UI already exist. Do not replace the calibration contracts or introduce a second settings object.

Implemented source-side capabilities include typed llama.cpp-only Quick/Balanced plans, managed sibling resolution, durable journals, cancellation and explicit resume, structured measurements, Pareto/noise/confidence analysis, baseline/help-default provenance, redacted receipts, derived-preset apply, post-apply validation/rollback, Spawn Wizard receipt reuse, and loopback server tracks. Rapid-MLX remains separate: no llama.cpp tuning patch may cross that boundary.

The current UI is functional but not 2.0-ready. It is a small modal with status text, simple candidate rows, baseline/default tables, qualification details, and apply/rollback buttons. It lacks a visual run timeline, live progress treatment, premium recommendation hierarchy, interactive comparisons, and clear drill-down of evidence. Do not close the Calibration UX gate until this document's UI and evidence gates pass.

The pushed CI run 31973233872 was green, including UI, Linux/macOS release smoke, Windows GNU smoke, Windows-target clippy, lint, and checks. Any UI change requires a new release-built local suite and fresh remote CI; that run is not evidence for new UI code. Older Windows receipts tied to commits such as `3069a1b` are historical platform evidence only and do not qualify the current Calibration UI.

## 2. Product objective

When a user chooses Calibrate preset, they must understand what is measured, how long or disruptive it may be, what is happening now, which result is recommended and why, what evidence is exact/compatible/related/noisy/stale/degraded, what changes if they create a calibrated preset, and how to inspect, reject, apply, or roll it back.

The experience must communicate measured evidence rather than imply a universal optimizer. It must be safe around an active model and make the derived-preset default unmistakable.

## 3. UX contract

### 3.1 Entry points

- Preset Editor remains the primary entry point: Calibrate this preset.
- Spawn Wizard may show exact or lower-confidence receipt reuse after local GGUF introspection, plus explicit optional post-download calibration.
- Rapid-MLX presets show an informational explanation and no llama.cpp action.
- Existing preset and active session remain unchanged until explicit apply.

### 3.2 Premium sheet/modal

Replace the plain 640px modal with a responsive sheet or large modal (desktop target approximately 960–1120px, narrow layout below 720px) that works at 1440x900, 1280x900, and 430x900.

Header and hero:

- Token Ingot identity consistent with the rest of the app.
- Model, preset, backend, and exact-fingerprint/evidence status pill.
- Headline such as “Find the best settings for this machine.”
- Chips for quantization, KV policy, context target, and runtime build.
- Visible “source preset remains unchanged” safety statement.
- Close action explains cancellation/resume behavior for a running job.

### 3.3 Preflight and progress

Preflight shows workload/objective, Quick vs Balanced, bounded trial count, duration range, active-server/disruption policy, selected tracks, and capability/degraded warnings before Start.

Running state shows:

- A real completed/planned progress bar.
- Phase: preflight, launching, warming, measuring, validating, analyzing, applying, or cleaning up.
- Current candidate and factor values.
- Completed/total, elapsed time, rolling throughput, and ETA range.
- Live memory, TTFT, prompt speed, decode speed, and correctness when exposed.
- Safe cancellation with cleanup language.
- Bounded expandable diagnostics/log content, never secrets or full prompts.

Never fabricate progress from time. An unavailable metric renders as unavailable/degraded, not zero.

### 3.4 Results and interaction

Recommendation hero:

- Recommended for this workload card with measured Fastest/Balanced/Max-context picks only when they exist.
- Prominent measured delta against the effective baseline.
- Confidence/noise and exact/compatible/related provenance badges.
- Primary Create derived preset action and secondary Review settings.
- Explicit source-preset preservation statement.

Metric cards:

- Decode/prompt tok/s, TTFT, and total latency.
- Memory/RSS/VRAM peak and context safety.
- Tool/schema correctness.
- Speculative activity/acceptance only when runtime emitted it.
- Visible unsupported/degraded state for unavailable tracks.

Comparison surface:

- Baseline versus recommendation side-by-side.
- Sortable Pareto table or compact chart.
- Batch/ubatch, threads, context, KV, flash mode, status, median, spread/MAD, confidence, and baseline delta columns.
- Filters by objective, track, status, noisy, and degraded.
- Expandable rows for repetitions, diagnostics, capability evidence, and exact runtime/model/workload identity.
- Both baseline.effective and baseline.llama_server_help_defaults, clearly labeled as different concepts.

Qualification surface:

- Separate sections/tabs for latency/memory, tools, speculation, and opt-in concurrency.
- Explicit no-spec baseline next to MTP/n-gram observations.
- DFLASH unsupported/inert warning, never silent success.
- Concurrency labeled opt-in and hardware-scoped.

Apply/rollback:

- Confirmation previews exact changed fields and derived-preset name.
- Source update is a separate, more explicit action.
- Post-apply validation has its own status/progress.
- Failed validation explains automatic rollback.
- Successful apply exposes rollback until dismissal.
- Optimistic conflicts explain that no user data was overwritten.

### 3.5 Accessibility and visual quality

Keyboard-complete focus order, visible focus, status/live announcements, text equivalents for charts/tables, reduced-motion behavior, dark/light theme overrides, narrow-layout support without horizontal scrolling, DOM-safe receipt/log rendering, and concise terminology are mandatory.

## 4. Implementation phases and gates

### Phase A — Freeze the UI data contract

Audit calibration snapshots/receipts and add only missing optional/defaulted display fields for progress, ETA, comparisons, and track summaries. Define stable UI states for current, stale/incompatible, noisy, regressed, resumable, missing-tool/degraded, unsupported, and failed. Add fixture receipts for every state; keep secrets, raw prompts, and unbounded logs out of public views.

Gate: Rust/API serialization tests pass, legacy receipts deserialize, Rapid-MLX cannot receive llama.cpp fields, and fixtures render every state without a live model.

### Phase B — Build the premium shell and run state

Refactor static/js/features/calibration.js into explicit preflight, running, results, apply, and rollback renderers. Add progress/phase/elapsed/ETA/live metrics/diagnostics and responsive styles in static/css/calibration.css. Preserve auth, polling backoff, cancellation, and DOM-safe rendering.

Gate: focused Playwright covers open, preflight, start, progress, cancellation, failure, and cleanup; release-built screenshots cover desktop/narrow, light theme, and reduced motion.

### Phase C — Build interactive results

Add recommendation hero, metric cards, baseline/winner comparison, Pareto table/chart, filters, expandable candidate details, provenance/confidence/noise badges, and track sections. Add text equivalents and exact identity details.

Gate: fixture-driven UI tests cover all finding states and track combinations; screenshot review accepts hierarchy/density; no unsafe dynamic HTML exists.

### Phase D — Make apply/rollback first-class

Add changed-settings preview, derived-preset naming, source-preservation copy, post-apply validation progress, automatic rollback explanation, conflict/stale recovery actions, and explicit source-update separation.

Gate: post_apply_fake_runtime_persists_passed_validation, post_apply_fake_runtime_rolls_back_failed_validation, and resume_fake_runtime_reuses_finished_results_once pass; focused Playwright covers confirm/cancel/success/rollback/conflict; no pre-confirmation mutation.

### Phase E — Screenshot and browser acceptance

Run cargo build --release first. Add manifest-backed capture scenarios for preflight, active progress, completed recommendation, baseline/winner, noisy/stale/degraded evidence, tool/MTP/concurrency sections, apply preview, validated apply, and rollback. Capture 1440x900, 1280x900, and 430x900, dark/light where applicable. Review actual release-built screenshots manually.

Gate: strict manifest/receipt checks, focused release-built Playwright, full isolated suite with at least 600 seconds, unused-screenshot check, no old brand, clipping, layout shift, or inaccessible control.

### Phase F — Source-side final gates and publication handoff

Run in order:

~~~

cargo clippy -- -D warnings
cargo test -- --test-threads=1
npm run validate-js
npm run lint
git diff --check
cargo build --release
cargo fmt --all -- --check
git status
~~~

Also run:

~~~
cargo test --test auth_routing -- --test-threads=1
cargo check --target x86_64-pc-windows-gnu
npm run validate-rebrand
npm run validate-release-contract
bash scripts/check-unused-screenshots.sh
~~~

The artifact/checksum validator's fixture self-test may pass before real 2.0 artifacts exist. Do not call artifact validation complete until a generated release directory is checked. scripts/release-preflight.sh may be blocked by missing cross/docker/osxcross tools; record that environment fact explicitly.

After Phase F, commit and push the complete UI/docs change. The Windows session must start from that pushed commit, not an uncommitted local tree.

## 5. Native Windows handoff

Read this document and the four authority documents in its header first. Do not infer native Windows behavior from macOS or GNU cross-compilation.

### 5.1 Starting checks

~~~
git status --short
git log -1 --oneline
Get-ComputerInfo | Select-Object WindowsProductName,WindowsVersion,OsBuildNumber
rustc -Vv
cargo -V
node --version
npm --version
npx playwright --version
~~~

Record stdout, stderr, exit codes, tool versions, and the tested commit in a repository-relative evidence file. Use disposable APPDATA and model/config roots for every migration or launch test; never target the real profile.

### 5.2 Native build/static gates

~~~
cargo clippy -- -D warnings
cargo test -- --test-threads=1
npm run validate-js
npm run lint
git diff --check
cargo build --release
cargo fmt --all -- --check
cargo test --test auth_routing -- --test-threads=1
npm run validate-rebrand
npm run validate-release-contract
~~~

Run the native executable as well; a target check is not native execution evidence.

### 5.3 Native Calibration checks

Use a disposable configured root and real managed Windows llama-server.exe, llama-bench.exe, and optional llama-fit-params.exe siblings. Verify:

- Sibling resolution follows configured root, never PATH or hard-coded paths.
- Help stdout/stderr, exit codes, SHA-256, capability signature, and optional tool degradation are recorded.
- A compatible small GGUF is in the disposable configured model root.
- Quick and, when practical, Balanced preflight counts are bounded.
- Polling, cancellation, process-tree/port cleanup, restart recovery, suspected-crash classification, and explicit resume work.
- Active-server stop/restore requires authorization and matching fingerprint.
- Apply previews changes, validates, rolls back on failure, and rejects optimistic conflicts without data loss.
- Receipts are redacted, bounded, loopback-only, and contain no secrets, private prompts, unsafe paths, or arbitrary extra_args.
- Filename heuristics never become model/capability evidence.
- Rapid-MLX never enters the llama.cpp catalog.

Store Windows Calibration evidence under the Calibration-owned Phase 10 directory:

~~~
docs/plans/evidence/20260813-llama-optimize/phase-10/windows/
~~~

Keep broader application-home, migration, runtime, packaging, and visual-parity receipts in their existing Phase 12/13 directories under `docs/plans/evidence/20260811-local-llm-foundry/`. Do not duplicate receipts across phase directories; link between ledgers when one receipt supports multiple gates.

Use separate files for machine manifest, managed-binary help, preflight, complete receipt, cancellation/cleanup, resume, apply/rollback, and degraded or failure states. Include hashes and repository-relative paths; never tokens or full user-home paths.

### 5.4 Native migration/runtime matrix

Run the existing Windows handoff matrix with the final commit: fresh canonical, legacy-only, identical dual roots, conflicting/partial/empty/permission-denied roots, interruption at each journal checkpoint, clear-auth-config, canonical and legacy aliases, logs/certs/keys/database/updater/binaries, remote-agent install/update/uninstall, scheduled-task cleanup, tray/WebView2 IPC, sensor bridge degradation, updater checksums/archive extraction/current-executable replacement, and ZIP/installer contents/icons/checksums/legacy bridge names.

Every row needs raw output, exit code, before/after tree manifest, relevant hashes, and a redacted pass/fail conclusion.

### 5.5 Native visual parity

Run release-built capture sequentially, never in parallel. Capture the Phase E Calibration scenarios at shared 1440x900 plus relevant 1280x900/narrow sizes: preflight, running progress/ETA/metrics, recommendation hero, baseline/winner and Pareto comparison, stale/noisy/degraded/unsupported evidence, apply preview, validated apply, rollback, and required migration/identity surfaces.

Rapid-MLX live runtime captures remain Apple-Silicon/macOS-only. Windows mocked Rapid UI scenarios are fixture evidence only, never native Rapid runtime proof.

Review typography, bundled fonts, wrapping, density, spacing, contrast, focus, chart/table readability, Token Ingot identity, and WebView2 rendering. Record accepted/rejected pairs in docs/plans/evidence/20260811-local-llm-foundry/phase-13/windows-macos-visual-parity-ledger.md.

### 5.6 Stop conditions

Stop and report on native start/sibling failure, child/port leak, lost or silently repeated trial, secret/path exposure, pre-confirmation mutation, rollback failure, mislabeled evidence, Rapid-MLX crossing, fabricated or inaccessible progress, migration data loss, updater/checksum/archive/alias divergence, WebView2/tray/sensor/agent failure, visual regression, or any CI failure. Do not waive a stop condition.

## 6. Required documentation and final handoff

Update this document's status and the Calibration/rebrand/Phase 7/Windows evidence ledgers before closing. Update current API/reference docs whenever the UI/API contract changes.

The final Windows summary must include the tested commit and branch, machine manifest, commands/exit codes, Calibration receipt IDs/hashes, migration/runtime/updater/package results, screenshot manifests and parity decisions, deferred MoE/Rapid-MLX/overnight work, and any blocker preventing Phase 12–14 closure.

### Fresh-session start prompt

~~~
You are continuing Local LLM Foundry 2.0 on Windows. Read
docs/plans/20260816-calibration2.0_uiux_and_windows_handoff.md first, then
the four authority documents in its header. Verify git status and the tested
commit. Do not infer native Windows behavior from macOS or GNU checks.
Complete the UI gates before executing sections 5.3–5.5. Use disposable roots,
native managed binaries, release-built screenshots, and repository-relative
evidence. Do not delete user data, expose secrets, or waive stop conditions.
Push only after all source-side UI gates pass; summarize receipts, hashes,
exit codes, visual parity decisions, and blockers before closing the phase.
~~~

## Current implementation status (2026-08-17)

Source-side Calibration UX is complete for the current llama.cpp contract. The release-built surface includes the hero identity/evidence treatment, Quick/Balanced bounded-run choice, live progress/ETA, sortable/filterable candidate comparison with expandable details, recommendation/provenance badges, latency/tool/speculation qualification cards, baseline/help-default provenance, safe derived-preset preview, and source-preserving apply/rollback messaging. Focused Calibration Playwright, JavaScript validation/lint, release build, full Rust tests, Clippy, and release-contract checks pass.

### Windows validation result

Native Windows validation completed on 2026-08-17 using a disposable profile, native x86_64 executable, managed llama.cpp `b10470`, CUDA 13.3, RTX 5090, the Qwen3.5 GGUF, and Froggeric v22.1. Quick and Balanced calibration passed; Balanced selected `balanced-l9-r04` (batch 1024, ubatch 512, context 57344). Apply/rollback, cancellation cleanup, and the final real-server qualification passed. Qualification tracks `latency_memory`, `mtp`, and `ngram` all completed without diagnostics. Evidence: `docs/plans/evidence/20260813-llama-optimize/phase-10/windows/README.md`.

The Windows launch path initially exposed an invalid upstream argument: the app-level `spec_draft_device=gpu` value became `--spec-draft-device gpu`, which llama.cpp rejects. The launcher now omits that legacy abstract value while preserving explicit upstream device names. The proven MTP configuration remains `--spec-draft-ngl all`, `n_max=3`, and `p_min=0.20`; no invented CUDA device flag is required. Qualification readiness is bounded at 180 seconds for cold model startup, and managed-server stderr is retained when readiness fails. A single bounded retry handles transient Windows loopback failure during the MTP probe.

### Visual and Mac handoff notes

Release-built Windows captures for preset editor, chat-template controls, lifecycle dialogs, evidence drawer, light/dark themes, narrow layout, and reduced motion were visually reviewed and look good. The chat-template screenshot uses a seeded macOS-style path only as fixture data; it is not a Windows runtime path and is not a portability issue. Rapid-MLX live runtime and Rapid-MLX-specific scenarios correctly remain Apple-Silicon/macOS-only; Windows skips are platform evidence, not failures. Calibration capture remains explicit opt-in so screenshots cannot launch a benchmark implicitly.

For Luna on macOS: do not change the cross-platform chat-template contract or add a CUDA-specific draft-device value. Keep the portable MTP settings and verify any Mac-side runtime-specific device handling independently against the managed llama.cpp help output. Broad MoE optimization and live Rapid-MLX calibration remain out of scope for this handoff.
