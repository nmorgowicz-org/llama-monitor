# Rapid-MLX Phase 8 — Remediation Plan (post-validation)

**Created:** 2026-07-18 · **Branch:** `feat/rapid-mlx-integration`

This doc turns the Phase 8 validation findings into precise, self-contained fix
instructions. It is written to be executed by **any** worker (a sub-agent or a
local model) **without re-deriving context** — every defect has an exact
`file:line`, the fix approach, the tests to add, and the gates to pass. Do the
items in the order listed; they share files, so **work one item at a time**.

Companion docs:
- Original plan / verified CLI reality: `docs/plans/20260718-rapid_mlx_phase8_parity_and_power_features.md`
- That doc's "Verified CLI Reality" (lines 27–109) is ground truth — do **not** re-derive it.

---

## Current State (baseline — do not re-investigate)

- The 6 Phase 8 items were implemented as 6 commits (one per item):
  `1f27646`=I1, `bf70afd`=I2, `4c2c3f6`=I3, `480ce80`=I4, `05e8a15`=I5, `29c2f49`=I6.
- **Item 5 is already fixed and committed** as `3b2b6e5` (`fixup! …bench integration`).
- HEAD = `3b2b6e5`; branch is **7 commits ahead** of `origin/feat/rapid-mlx-integration`; working tree is **clean**.
- Whole tree compiles; `cargo test` = green; `cargo clippy --all-targets -- -D warnings` = green **right now**. Keep it that way after every item.
- **Item 4 is SOLID** — only one tiny test gap remains (see Item 4 below).
- Remaining work: **Item 2** (substantial), **Items 1/3/6** (minor), **Item 4** (one test), plus a **Deferred Live Validation Pass**.

### Validation verdicts (evidence is in the per-item sections)
| Item | Verdict | What's left |
|---|---|---|
| I4 | ✅ SOLID | ✅ format-threading test added |
| I5 | ✅ FIXED (`3b2b6e5`) | live-pass contract test only |
| I1 | ✅ FIXED | finetune prefix, eligibility reasons, dead-code wrappers, real fixture all resolved |
| I3 | ✅ FIXED | tooltips corrected, invented constraint removed, defaults surfaced + tested |
| I6 | ✅ FIXED | `author` nullable fixed + tested; updater-dedup landed (live-pass Playwright parity confirmation still needed) |
| I2 | ✅ FIXED | live-pass Playwright check only (advisor producer, dead checks, parser bug, and tests all resolved) |

---

## Global Execution Rules (apply to every item)

1. **Hard gates after each item** (all must pass before the item is "done"):
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test` (full suite)
   - `git diff --check` (no whitespace errors)
2. **Getting real cargo output:** an `rtk` hook filters `cargo` in the shell. Prefix with `rtk proxy` for raw output (e.g. `rtk proxy cargo test doctor -- --nocapture 2>&1 | tail -80`). Exit codes may not surface through `rtk` — judge by text (`error[`, `test result: ok`). Inside a `zsh` script file, `command cargo …` also bypasses the hook and preserves exit codes.
3. **No `--json` exists** on any `rapid-mlx` subcommand — all parsers scrape human box-drawing/glyph text. Every scraper must be **version-guarded** (`rapid-mlx version`) and **degrade to "no result"/raw-text** on layout mismatch, never crash or block. Reuse `info_query::cached_version` / `info_query::MIN_TRUSTED_MINOR` (both now `pub(crate)`).
4. **Fixtures must be REAL captured CLI output, not hand-written strings that mirror the parser.** The recurring bug across items was fixtures without the real `│` box borders / multi-word headers. Capture actual output by running the CLI (read-only) and paste it verbatim. Follow `info_query.rs`'s `parses_real_rapid_mlx_info_output_contract` (`#[ignore]`d live-binary test) pattern for a live variant.
5. **Do NOT spawn servers or run Playwright inside per-item fixes.** All server-spawn / Playwright / real-Apple-Silicon gates are consolidated into the **Deferred Live Validation Pass** at the end.
6. **Commits:** after an item's gates pass, commit it as `fixup! <original item subject>` (so `git rebase -i --autosquash` folds it into the right commit). End the message with `Co-Authored-By: …`. Because items share files (`rapid_mlx_runtime.rs`, `spawn-wizard.js`, `index.html`, `info_query.rs`), **never run two fixers concurrently** — the shared working tree will corrupt.
7. **Read-only CLI is allowed** for verifying parsers: `rapid-mlx info <alias>`, `rapid-mlx models`, `rapid-mlx doctor`, `rapid-mlx serve --help`, `rapid-mlx bench --help`, `rapid-mlx version`. Sample aliases: `qwen3-0.6b-4bit`, `gemma3-1b-4bit`.

---

## Item 2 — Doctor / Troubleshooting (🔴 REWORK — do first)

Commit under review: `bf70afd`. The environment-doctor half works; the fixable-findings half and parity checks are missing.

**What already works (leave intact):** `DoctorFinding{finding_type,severity,message,section,fix:Option<FixAction>}` and the closed 3-variant `FixAction` enum at `src/state.rs:1627-1672`; doctor findings correctly always `fix: None`; apply-fix write-through at `src/web/api/sessions.rs:511-523`; the two panels are kept separate (`#server-error-details-body` / `#local-server-error-details-body`) — **do not merge them**; tool-parser flags live only as diagnostic fixes (absent from `escape_hatch.rs`).

### DEFECT 2A (critical) — preset-flag-advisor producer is entirely missing
`FixAction::…` is never constructed with `Some(...)` anywhere, so the "Apply fix" button (`static/js/features/dashboard-ws.js` ~line 1862, `doctor-fix-btn`) can never render.
- **Implement** a producer that diffs the active/selected model's `rapid-mlx info` profile (via `info_query::fetch_model_profile`, fields `tool_format` / `reasoning_parser`) against the active preset's launch args (`RapidMlxConfig` fields at `src/inference/rapid_mlx/mod.rs:56-64`, wired through `command.rs`/`launch.rs`).
- Emit `DoctorFinding`s with `fix: Some(FixAction::…)` (use exactly the enum's existing variants — read `state.rs:1627-1672` first) when `info` implies a tool-format / reasoning setting the preset doesn't set.
- Wire these findings into the same panel render path as doctor findings (`dashboard-ws.js` `loadDoctorFindings`, ~1810-1864). Reuse the apply-fix endpoint (`sessions.rs:511-523`) — do not reimplement it.
- Keep `FixAction` a **closed** enum (not an arbitrary flag patcher — that's Item 3's job).

### DEFECT 2B — two llama.cpp parity checks are dead code
`src/web/api/sessions.rs`: `check_gguf_path` (~:336) and `check_port_available` (~:386) are `#[allow(dead_code)]` and never called; only `check_llama_server_binary` (~:256, called ~:241 in `api_llama_cpp_diagnostics`) is wired. Wire both into `api_llama_cpp_diagnostics`, include their findings in the response, and remove the `#[allow(dead_code)]`.

### DEFECT 2C — doctor section-name parser truncates multi-word headers
`src/web/api/rapid_mlx_runtime.rs` ~line 1099 (`parse_doctor_output`, ~1084-1123) uses `.chars().take_while(|c| !c.is_whitespace())`, truncating `◆` headers to the first word. **Verified live:** "Required Packages"→"Required"; "Optional Packages" AND "Optional Tools" BOTH→"Optional" (collision); "HuggingFace Cache"→"HuggingFace"; "Shell Integration"→"Shell". Fix to capture the full header after the `◆` marker (trim, keep the whole name). Run `rapid-mlx doctor` and confirm no collisions.

### DEFECT 2D — version fetched but unused
The doctor endpoint (~990-1044) fetches `rapid-mlx version` (~1014-1021) but doesn't gate on it. Apply the `MIN_TRUSTED_MINOR` guard: below trusted minor → raw-output-only / no structured findings.

### DEFECT 2E — zero tests for the whole surface
`cargo test doctor` matches 0 tests. Add:
- `parse_doctor_output` contract test using **real** `rapid-mlx doctor` output; assert multi-word section names (no Optional collision), ✓/⚠/✗→severity, and the `Summary: N ok, M warnings, K issues` rollup. Optional `#[ignore]`d live variant.
- Flag-advisor unit test: profile with a `tool_format` + preset lacking the flag → finding with `fix: Some(...)`; matching preset → no fix finding; doctor findings still `fix: None`.
- `check_gguf_path` / `check_port_available` unit tests (nonexistent path → issue; bound port → not-free).

**Gates:** `rtk proxy cargo test doctor`, full `cargo test`, `cargo clippy --all-targets -- -D warnings`. Then `git commit -m "fixup! feat(sessions): doctor/troubleshooting integration for Rapid-MLX and llama.cpp"`.

**Resolution (2026-07-18):**
- **2A:** Added `flag_advisor_route` (`src/web/api/rapid_mlx_runtime.rs`, `GET /api/rapid-mlx/flag-advisor`), wired into `routes()`. It resolves the active session → preset → `RapidMlxConfig`, extracts an info-queryable model id via new `model_id_for_info()` (aliases / HF repos only; local-dir/GGUF sources degrade to no findings), calls `info_query::fetch_model_profile`, and diffs the profile against the preset's flags via the new pure function `build_flag_advisor_findings(profile, config)`. It emits `DoctorFinding{finding_type: Preset, fix: Some(FixAction::AddToolCallParser | EnableAutoToolChoice)}` when `tool_format` is set but the corresponding flag is off, and `fix: Some(FixAction::AddNoThinking)` when `reasoning_parser` is set, `enable_thinking == Some(false)`, and `no_thinking` isn't set. `FixAction` stays the closed 3-variant enum — no new variants added. `static/js/features/dashboard-ws.js`'s `loadDoctorFindings` now also fetches `/api/rapid-mlx/flag-advisor` and merges its findings into the existing render/apply-fix path (`doctor-fix-btn` → `POST /api/diagnostics/apply-fix`, unchanged, reused as-is). The two error panels (`#server-error-details-body` / `#local-server-error-details-body`) were left separate.
- **2B:** `check_gguf_path` and `check_port_available` (`src/web/api/sessions.rs`) lost their `#[allow(dead_code)]` and are now called from `api_llama_cpp_diagnostics`, which now also takes `state: AppState` (route registration updated) to read the active session's `LocalLaunchRequest::LlamaCpp` config (model path + port) and include their findings alongside `check_llama_server_binary`'s. Skipped (no findings) when there's no active llama.cpp session, matching prior comment intent.
- **2C:** `parse_doctor_output` (`src/web/api/rapid_mlx_runtime.rs`) no longer truncates `◆` section headers to the first word; it now captures the full trimmed header. Verified against live `rapid-mlx doctor` (rapid-mlx 0.10.12, Apple M5 Max): section names are `System`, `Python`, `Required Packages`, `Optional Packages`, `HuggingFace Cache`, `Network`, `Shell Integration`, `Optional Tools` — no collision between `Optional Packages` and `Optional Tools`.
- **2D:** `doctor_route` now gates structured findings on `info_query::cached_version`/`MIN_TRUSTED_MINOR`: below the trusted minor (or on a version-parse failure), `findings` is returned empty and only `raw_output` is populated; a `version_trusted` boolean was added to the response.
- **2E:** Added tests (all real-fixture-backed per the project rule): `parse_doctor_output_captures_full_multi_word_section_names` and `parse_doctor_output_maps_glyphs_to_severity_and_rollup_matches_summary_line` (`rapid_mlx_runtime.rs`), using a real captured `rapid-mlx doctor` transcript (`REAL_DOCTOR_OUTPUT`) — asserting no `Optional`/`Required`/`Shell` truncation-collision buckets, correct glyph→severity mapping, and rollup counts cross-checked against the fixture's own `Summary: 16 ok, 6 warnings, 0 issues` line; `flag_advisor_emits_fix_when_preset_missing_tool_call_flags`, `flag_advisor_is_silent_when_preset_already_matches_model_profile`, `flag_advisor_recommends_no_thinking_when_preset_wants_thinking_disabled` (`rapid_mlx_runtime.rs`) exercising `build_flag_advisor_findings` directly; `doctor_check_gguf_path_nonexistent_is_an_issue`, `doctor_check_gguf_path_empty_path_is_an_issue`, `doctor_check_gguf_path_existing_gguf_file_is_ok`, `doctor_check_gguf_path_wrong_extension_is_a_warning`, `doctor_check_port_available_reports_bound_port_as_in_use`, `doctor_check_port_available_reports_free_port_as_ok` (`sessions.rs`). `cargo test doctor` now matches 8 tests (was 0); full `cargo test` and `cargo clippy --all-targets -- -D warnings` are green.

Committed as `ba10140` (fixup! bf70afd).

---

## Item 1 — Live Info Resolution (🟡 MINOR)

Commit: `1f27646`. Plumbing, struct shape (no invented `pflash_tier`, single `is_finetune: bool`), version guard, and the profile endpoint (`rapid_mlx_runtime.rs:896-988`, correct 404/408/500) are all good.

### DEFECT 1A — finetune detection is dead code
`src/inference/rapid_mlx/info_query.rs:443-449`: the check `trimmed.starts_with("Model:")` runs on `line.trim()`, which leaves the box-drawing `│` prefix intact. Real output is `│ Model: mlx-community/Qwen3-0.6B-4bit  │`, so it never matches → `is_finetune` is **always false**. Fix: strip the leading `│` (and surrounding whitespace) before the `starts_with("Model:")` check (and strip the trailing `│`). The unit test `finetune_detection_marks_unknown_hf_repos` (~646-660) uses a border-less fixture that mirrors the bug — **replace its fixture with real `│`-bordered `rapid-mlx info <hf-repo>` output.**

### DEFECT 1B — eligibility `reasons` map drops real criterion lines
`info_query.rs:374-391`: the keyword filter (`"Declared" | "MoE" | "Precision" | "Drafter" | "Runtime" | "Supported"`) misses real labels — DFlash's `mlx-vlm 0.5.0+`; DDTree's `Spec tokens`, `Tree budget`, and `dtree-mlx runtime` (lowercase `runtime`). Broaden the matching (case-insensitive, and include the missing labels) so all per-criterion reasons `info` prints are captured. Verify against live `rapid-mlx info qwen3-0.6b-4bit` and `gemma3-1b-4bit`. (`supported: bool` is already correct.)

### DEFECT 1C — dead-code wrappers
`src/inference/rapid_mlx/model_resolver.rs:1274-1287`: `live_model_profile` / `live_model_list` are `#[allow(dead_code)]`; the endpoint calls `info_query::fetch_model_profile` directly, bypassing them. Either wire them in or delete them — don't leave dead `#[allow]`'d wrappers. (Preserve the launch-path stance at `model_resolver.rs:1256-1262`.)

**Gates + commit:** `fixup! feat(models): live alias/extras resolution via rapid-mlx info CLI`.

**Resolution (2026-07-18):**
- **1A:** `parse_model_profile` (`src/inference/rapid_mlx/info_query.rs`) now strips the leading/trailing `│` box border (and surrounding whitespace) via a new `unboxed` binding before checking `starts_with("Model:")`/`starts_with("Name:")`, so the `│ Model: mlx-community/Qwen3-0.6B-4bit                         │` line real `rapid-mlx info <hf-repo>` prints is now matched (verified live against `rapid-mlx info mlx-community/Qwen3-0.6B-4bit`). The unit test `finetune_detection_marks_unknown_hf_repos` was replaced to use a real `│`-bordered fixture (`REAL_INFO_HEADER`, captured from the live CLI) instead of the border-less fixture that mirrored the bug.
- **1B:** The eligibility-reasons capture (`info_query.rs`) no longer filters criterion lines by a fixed keyword allowlist (`"Declared" | "MoE" | "Precision" | "Drafter" | "Runtime" | "Supported"`); it now captures every key:value line printed inside a DFlash/DDTree eligibility box, verified live against `rapid-mlx info qwen3-0.6b-4bit` and `rapid-mlx info gemma3-1b-4bit` (both print `mlx-vlm 0.5.0+`, `Spec tokens`, `Tree budget`, and lowercase `dtree-mlx runtime` as criteria, none of which the old allowlist matched). While fixing this, a second, deeper bug was found and fixed: the per-section `Eligibility` accumulator was unconditionally reset (`eligibility = Some(Eligibility::default())`) whenever a *new* eligibility header (or an unrelated `##` header) was seen, silently discarding the *previous* section's already-accumulated `reasons` before they were ever written to `profile.dflash_eligibility`/`profile.ddtree_eligibility` — since real `rapid-mlx info` output always prints DFlash immediately followed by DDTree, this meant DFlash's reasons were **always** dropped in practice, not just for the missing keywords. Added `flush_eligibility_into(profile, section, elig)`, called at every section-transition point (DFlash header, DDTree header, generic `##` reset, and end-of-input) to merge the accumulator into the correct profile field before it's replaced/discarded. `supported: bool` is preserved unchanged (only overwritten when the accumulator captured a definite value). Added `eligibility_reasons_capture_all_real_criterion_lines_not_just_keyword_subset`, using a real captured two-box `rapid-mlx info qwen3-0.6b-4bit` eligibility transcript (`REAL_ELIGIBILITY_BLOCKS`) as the fixture, asserting both DFlash's and DDTree's previously-dropped criteria (including the cross-section-flush case) now land in `reasons`.
- **1C:** Deleted the dead `#[allow(dead_code)] live_model_profile` / `live_model_list` wrappers in `src/inference/rapid_mlx/model_resolver.rs` rather than wiring them in — the profile endpoint (`rapid_mlx_runtime.rs`) already calls `info_query::fetch_model_profile` directly and there was no other real caller to justify an indirection layer; `info_query::fetch_model_list` itself keeps its own pre-existing `#[allow(dead_code)]` (unrelated, unchanged) since nothing calls it yet. The launch-path free-form-alias stance/comment at `model_resolver.rs:1256-1262` (`alias_warnings`) was left byte-for-byte unchanged.
- **Tests:** `cargo test info_query` = 12 passed (was passing before too, plus the two new/updated tests), 1 ignored (live-binary contract test, also re-run manually against the installed `rapid-mlx` CLI and passing). Full `cargo test` = 579+581+... all green, 0 failed. `cargo clippy --all-targets -- -D warnings` = clean.
Committed as `30c380f` (fixup! 1f27646).

---

## Item 3 — Escape-Hatch Allowlist (🟡 MINOR — copy/UX only)

Commit: `4c2c3f6`. Security core is solid and fully tested (allowlist enforced at `launch.rs:145-153` and `434-441`, no free-text fallback, `--watchdog-ppid`/`--listen-fd` provably excluded with assertion tests). Fixes are all in `src/inference/rapid_mlx/escape_hatch.rs`.

### DEFECT 3A — inverted tooltip (verify against `rapid-mlx serve --help`)
`escape_hatch.rs:41-46` `pflash-sink-tokens` tooltip says "tokens at the **end** … (sink)". Real help: "**Leading** prompt tokens always kept by PFlash." Sink tokens are **leading**, not end — fix the wording.

### DEFECT 3B — inverted tooltip
`escape_hatch.rs:62-67` `pflash-query-window` says "Number of **leading** tokens used as the query key." Real help: "**Trailing** query window used to score middle blocks." Fix to trailing.

### DEFECT 3C — invented constraint
`escape_hatch.rs:55-60` `pflash-block-size` claims "Must be a power of two". Real help just says "Middle-token scoring block size (default: 128)". Remove the fabricated power-of-two claim.

### DEFECT 3D — defaults not surfaced
`EscapeFlagDescriptor` (`escape_hatch.rs:1-9`) has no `default` field, and the wizard renders empty number inputs with no placeholder (`static/js/features/spawn-wizard.js:2502-2519`). Add a `default` to the descriptor (verified values: `pflash-threshold` 32768, `pflash-keep-ratio` 0.20, plus the other pflash knobs from `serve --help`) and surface it as the input placeholder/help text. Confirm every default against real `rapid-mlx serve --help`.

**Gates + commit:** `fixup! feat(models): structured escape-hatch allowlist for Rapid-MLX serve flags`.

**Resolution (2026-07-18):**
- Verified every claim against a fresh, real `rapid-mlx serve --help` run on this machine (rapid-mlx CLI, Apple M5 Max) before touching any copy.
- **3A:** `escape_hatch.rs` `pflash-sink-tokens` tooltip changed from "Number of tokens at the **end** of a prompt to always treat as non-reusable (sink)" to "**Leading** prompt tokens always kept by PFlash." — matches real help text verbatim: `--pflash-sink-tokens PFLASH_SINK_TOKENS  Leading prompt tokens always kept by PFlash (default: 256).`
- **3B:** `pflash-query-window` tooltip changed from "Number of **leading** tokens used as the query key for cache lookups" to "**Trailing** query window used to score middle blocks." — matches real help: `--pflash-query-window PFLASH_QUERY_WINDOW  Trailing query window used to score middle blocks (default: 512).`
- **3C:** `pflash-block-size` tooltip's fabricated "Must be a power of two (e.g. 64, 128, 256)" claim removed; replaced with "Middle-token scoring block size, in tokens." — matches real help: `--pflash-block-size PFLASH_BLOCK_SIZE  Middle-token scoring block size (default: 128).` (no power-of-two constraint exists in the CLI).
- **3D:** Added `pub default: Option<&'static str>` to `EscapeFlagDescriptor` (`escape_hatch.rs`), populated for every numeric/enum pflash flag from the real `serve --help` defaults captured above: `pflash-threshold`→`"32768"`, `pflash-keep-ratio`→`"0.20"`, `pflash-min-keep-tokens`→`"2048"`, `pflash-sink-tokens`→`"256"`, `pflash-tail-tokens`→`"2048"`, `pflash-block-size`→`"128"`, `pflash-query-window`→`"512"`, `pflash-stride-blocks`→`"8"`, and `pflash` (enum, conditional default)→`"off (\"always\" for verified aliases)"`. Left `None` for the pure bool toggles (`pflash-include-tools`, `force-spec-decode`, `no-spec-decode`, `force-hybrid`, `no-hybrid`) — a checkbox has no meaningful "default value" placeholder. The struct already derives `serde::Serialize` and is served as-is by the existing `GET /api/rapid-mlx/escape-hatch-flags` route (`src/web/api/rapid_mlx_runtime.rs::escape_hatch_route`), so the new field threads to the frontend with no route change (`#[serde(skip_serializing_if = "Option::is_none")]` keeps bool-flag payloads unchanged). `static/js/features/spawn-wizard.js`'s `ensureEscapeHatchRendered` now reads `d.default`: it's appended to the tooltip text (`"<tooltip> (default: <default>)"`), rendered as a small `esc-hatch-default` label under the flag name, and — for numeric inputs — set as the `placeholder` (parsed via `parseFloat` so the enum's descriptive default string doesn't get misapplied to a number field, since enum flags render as a `<select>`, not a number input, so the placeholder path never touches it).
- Section gating (Rapid-MLX-only rendering) was not touched.
- **Tests:** added to `escape_hatch.rs`'s existing `#[cfg(test)] mod tests`: `numeric_and_enum_flags_have_a_surfaced_default` (every non-bool descriptor has `default.is_some()`), `pflash_threshold_and_keep_ratio_defaults_match_real_cli_help` (spot-checks 3 defaults against the literal `serve --help` values), `pflash_block_size_tooltip_has_no_fabricated_power_of_two_claim`, `pflash_sink_and_query_window_tooltips_match_real_cli_help_direction` (leading/trailing wording lock-in). No prior test asserted struct field count/shape, so nothing needed updating for the new field. `cargo test escape_hatch` = 11 passed, 0 failed.

Files changed (Job A / Item 3): `src/inference/rapid_mlx/escape_hatch.rs`, `static/js/features/spawn-wizard.js`.

<!-- coordinator: add fixup hash -->

---

## Item 6 — Compare-API Changelog (🟡 MINOR + 1 gate FAIL)

Commit: `29c2f49`. The Rust changelog feature is solid (fetch/cache reuse, 512KB + 250-commit bounds, rate-limit degradation, real-fixture test cross-checked live). Two fixes:

### DEFECT 6A — `author` is non-`Option` (deserialization fragility)
`src/inference/rapid_mlx/changelog.rs:47`: `CommitItem.author: GithubUser` (required). GitHub's compare API returns `"author": null` for commits whose email isn't linked to an account (bot/orphaned commits). One null author fails deserialization of the **entire** response, dropping all commits for that pair. Fix: make it `author: Option<GithubUser>` and fall back to the commit-author name (`commit.author.name`) when the top-level `author` is null. Add a fixture test with a `null` author entry.

### DEFECT 6B (gate FAIL) — updater-dedup refactor never landed
The plan scoped the updater-dedup refactor **into** Item 6 as a hard gate; `static/js/features/updater-shared.js` (64 lines, focus-trap helpers only) and `static/js/features/llama-updater.js` were left **byte-for-byte unchanged**. Land it: move the shared fetch-release / render-release-list logic into `updater-shared.js`; keep backend-specific bits (install/upgrade endpoints, confirm tokens) in `llama-updater.js` / `rapid-mlx-updater.js`. **Behavioral parity is itself a hard gate** — the refactor must not change either updater's existing install/upgrade/focus-trap semantics. This half needs Playwright verification in the Live Pass; do the code move + diff-review for parity here.

**Gates + commit:** `fixup! feat(rapid-mlx): GitHub compare-API changelog in updater surface`.

**Resolution (2026-07-18):**
- **6A:** `CommitItem.author` (`src/inference/rapid_mlx/changelog.rs`) changed from `GithubUser` to `Option<GithubUser>`. `summarize_commits` now reads `item.author.as_ref().map(|a| a.login.clone()).unwrap_or_default()` instead of `item.author.login` directly, so a top-level `"author": null` (GitHub's shape for commits whose email isn't linked to an account — bot/orphaned commits) no longer fails deserialization of the whole compare response; the existing login→`commit.author.name`→`"unknown"` fallback chain is unchanged, just now sourced through the `Option`. All pre-existing tests that construct `CommitItem` literals were updated to wrap `author` in `Some(...)`. Added `real_github_compare_api_fixture_with_null_author_still_deserializes`, a real-shaped two-commit fixture (one normal `"author": {"login": ...}"`, one with `"author": null` and `"committer": null`) asserting (a) the whole `CompareResponse` still deserializes, and (b) the null-author commit's displayed author falls back to `commit.author.name` (`"orphaned-bot"`). `cargo test changelog` = 15 passed, 0 failed.
- **6B:** Diffed `llama-updater.js` and `rapid-mlx-updater.js` before touching anything. Two pieces were byte-for-byte-equivalent-modulo-parameters and safe to hoist into `static/js/features/updater-shared.js`:
  - **`fetchReleaseList(url)`** — the fetch-headers-parse plumbing common to `llama-updater.js`'s `loadReleaseList()` (previously inlined `fetch('/api/llama-binary/releases', {headers})` + `if (!resp.ok) throw new Error(...)` + `resp.json()`) and `rapid-mlx-updater.js`'s `fetchReleases()` (previously inlined the same pattern but returning early on `!resp.ok` instead of throwing). The shared function always throws `Error('HTTP <status>')` on a non-OK response; each caller's own try/catch is unchanged, so the *policy* difference is preserved exactly — `loadReleaseList` still surfaces the error into the release-list DOM (`catch (err) { ... Failed to load releases: ${err.message} }`), `fetchReleases` still swallows it silently (`catch { /* silent */ }`) since a thrown error hits the same empty catch its old early-`return` used to reach. `llama-updater.js`'s post-fetch `data.error` check (which `rapid-mlx-updater.js` never had) was intentionally *not* folded into the shared helper — it stays as a caller-side `if (data.error) throw ...` in `loadReleaseList` only, so `rapid-mlx-updater.js`'s behavior (ignore `data.error`) is unchanged.
  - **`buildReleaseBadges({wrapperClass, badgeClass, isLatest, isCurrent})`** — the "latest"/"installed" badge-`<span>` construction inline in both files' `buildReleaseRow` functions was structurally identical (same wrapper `<span>`, same conditional child spans, same `textContent`, same append order), differing only in the CSS class strings (`llama-version-row-badges`/`llama-version-badge` vs `rapid-mlx-release-row-badges`/`rapid-mlx-badge`). Class names are now passed in by each caller; the produced DOM (order, textContent, class composition via `` `${badgeClass}--latest` ``) is unchanged from what each file built inline.
  - Both `llama-updater.js` and `rapid-mlx-updater.js` now `import { attachModalFocusTrap, detachModalFocusTrap, fetchReleaseList, buildReleaseBadges } from './updater-shared.js'`.
  - **Left in place, not hoisted** (with reason):
    - `buildReleaseRow` itself in both files — differs in row CSS class, `dataset` key (`tag` vs `version`), row click-selection behavior (llama re-derives the selected row from `dataset.tag` inside `showReleaseNotes`; rapid-mlx explicitly calls `selectReleaseRow(row)` before `showReleaseNotes`), age/meta text source (llama always shows `relativeAge`; rapid-mlx shows the release channel when non-stable, else `timeAgo`), and the install button's `dataset` fields — merging these would require behavior-changing normalization, which the task explicitly forbids.
    - `relativeAge` (llama-updater.js) vs `timeAgo` (rapid-mlx-updater.js) — superficially similar (both format an ISO date as a relative-age string) but not identical: `relativeAge` has minute/hour/day/month granularity, `timeAgo` only has day/week/month granularity and a `today` special-case. Forcing a shared implementation would change one or both updaters' displayed text, so both were left as-is.
    - Install/upgrade/repair/rollback endpoint calls, confirm-token flows (`getDbAdminToken`, `confirm: 'UPGRADE_RAPID_MLX_RUNTIME'` etc.), job polling (`pollJob`), and the changelog fetch/render (`fetchChangelog`, `renderChangelog`, `escapeHtml`) — these are Rapid-MLX-only with no llama.cpp equivalent (llama.cpp has no changelog/job-polling surface), so there is nothing to dedup.
    - `showReleaseNotes` in both files — different rendering pipelines (llama uses `marked` + PR-link rewriting + `DOMPurify`; rapid-mlx uses server-supplied `release_notes` HTML + `DOMPurify` + triggers the changelog section) with no shared subset beyond the `DOMPurify` call itself, which is too small a fragment to be worth extracting.
  - No JS unit tests exist in this repo; verified by `node --check` on all three files (syntax-valid ES modules) and a manual read-through diff of the before/after control flow for both `loadReleaseList`/`fetchReleases` and both `buildReleaseRow`s to confirm no DOM/behavior change. **Live Playwright parity confirmation (both updaters' install/upgrade/focus-trap/modal flows unchanged) is deferred to the Deferred Live Validation Pass**, per the task's hard-gate note.

Committed as `5112d53` (fixup! 29c2f49).

---

## Item 4 — HF/MLX Search Parity (✅ SOLID — one test only)

Commit: `480ce80`. Root-cause fix correct (`HfModelFormat` enum at `src/hf/mod.rs:671-689`, `filter` param at :752; route threads `format` at `src/web/api/hf.rs:138-145`; `presets.js` reuses `hfSearch()` with no duplicate impl; `filter=mlx` confirmed live). Only gap:

### DEFECT 4A — no test for format threading
The existing `route_hf_search` smoke test (`src/web/api/mod.rs:941`) exercises only the default `Gguf` path. Add a unit test asserting `format:"mlx"` in the request body threads through to an emitted `filter=mlx` (and `format:"gguf"`/absent → `filter=gguf`). Commit as its own `fixup! feat(models): HF/MLX model search parity with format filter`.

*(Optional, not required: the commit bundled two unrelated cleanup hunks into `spawn-wizard.js` — dead-brace removal + an `innerHTML`→`textContent` hardening. Harmless; leave unless doing a history cleanup.)*

**Resolution (2026-07-18):**
- The route-level `format` parsing (`src/web/api/hf.rs`'s `api_hf_search` closure) was inline (`body["format"].as_str().unwrap_or("gguf").to_lowercase()` + a `match`), so it wasn't independently unit-testable without a live HF call through the existing table-driven `route_hf_search` smoke test in `src/web/api/mod.rs` (which only asserts the route doesn't panic, not response content). Extracted the exact logic the route uses into a new pure function `pub(crate) fn parse_hf_format_param(raw: &str) -> crate::hf::HfModelFormat` (`src/web/api/hf.rs`) and had `api_hf_search` call it (`parse_hf_format_param(body["format"].as_str().unwrap_or("gguf"))`) — this covers the actual threading path (request body → `HfModelFormat` → `HfSearchParams.format`) with zero behavior change, no live call needed.
- Added `parse_hf_format_param_threads_mlx_and_gguf` (new `#[cfg(test)] mod tests` at the bottom of `src/web/api/hf.rs`) asserting `"mlx"`/`"MLX"` (case-insensitive) → `HfModelFormat::Mlx`, and `"gguf"`/`""`/`"bogus"` → `HfModelFormat::Gguf` (backward-compat fallback).
- Also added `hf_model_format_as_api_filter_threads_mlx_vs_gguf` to `src/hf/mod.rs`'s existing `#[cfg(test)] mod tests` (private `as_api_filter` is only reachable from within the `hf` module tree), asserting `HfModelFormat::Gguf.as_api_filter() == "gguf"`, `HfModelFormat::Mlx.as_api_filter() == "mlx"`, and `HfModelFormat::default().as_api_filter() == "gguf"` — covering the second half of the threading chain (`HfModelFormat` → the `filter=` query param actually sent to the HF search API at `src/hf/mod.rs:752`).
- Together the two tests cover the full `format:"mlx"`-in-body → `filter=mlx`-on-the-wire chain without a live HF fetch, per the task's "prefer asserting … the threading, not a live fetch" guidance.
- `cargo test hf` = all passing (`hf::tests::hf_model_format_as_api_filter_threads_mlx_vs_gguf` and `web::api::hf::tests::parse_hf_format_param_threads_mlx_and_gguf` both `ok`); full `cargo test` and `cargo clippy --all-targets -- -D warnings` remain green.

Files changed (Job B / Item 4): `src/web/api/hf.rs`, `src/hf/mod.rs`.

<!-- coordinator: add fixup hash -->

---

## Deferred Live Validation Pass (Apple Silicon M5 Max — run last, after all fixes)

These are the hard gates deliberately excluded from per-item fixes (they need a real server / browser). Run them once, together:

1. **Item 5 real bench:** spawn a small Rapid-MLX server, run a real benchmark through the dropdown, capture the real `rapid-mlx bench` text output, and (a) sanity-check the reported `t/s`/`ttft` against a manual `rapid-mlx bench <model> --base-url …` run, (b) tighten `benchmark.rs`'s bench-output fixture to the captured real text, (c) confirm no `llama-bench` labels leak for the Rapid-MLX path.
2. **Item 1 Playwright:** selecting a model in the wizard populates the extras surfaces (vision toggle, embedding-model field, spec-decode eligibility readout) matching a live `rapid-mlx info`. Real spawn smoke: launch a preset from a resolved profile; server starts.
3. **Item 2 Playwright:** diagnostics panel renders Rapid-MLX doctor findings + ≥1 llama.cpp parity finding; "Apply fix" appears on an advisor finding, persists to the preset, and is reflected on next launch. Panel degrades gracefully with `rapid-mlx` absent.
4. **Item 3 Playwright:** the Advanced section renders only for the Rapid-MLX backend (llama.cpp shows none); a spawn with one allowlisted flag applied works end to end.
5. **Item 4 Playwright:** preset editor can search+select a model (previously no search UI); MLX toggle visibly narrows results against a real HF response.
6. **Item 6 Playwright:** opening the Rapid-MLX updater modal and expanding "What's new" shows commit entries; the llama.cpp updater modal is behaviorally unchanged after the dedup refactor.

---

## Handoff Notes

- If a worker's window/quota expires mid-item, the **fixup commits** are the restore points — the next worker picks up from the last clean `cargo clippy`/`cargo test` green state (check `git status` + `git log --oneline`).
- Nothing here is on `main`; everything is `fixup!` commits on `feat/rapid-mlx-integration`, foldable with `git rebase -i --autosquash origin/feat/rapid-mlx-integration`.
- Do not weaken any existing hard gate to make an item pass (per the Builder/Verifier/Coordinator protocol in the parity plan doc).
