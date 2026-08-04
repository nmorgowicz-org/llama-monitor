# 2026-08-04 — Branch Audit, Capture Split, and Chat-Template UX Rework

Branch: feat/rapid-mlx-integration
Status: PLANNED

## Context

A Coordinator/local-model pass shipped Phase 9 (chat-template lifecycle) and
recorded it as "verified complete" in
`docs/plans/20260718-final_rapidmlx_followups_execution.md`. Those claims are
UNRELIABLE. An engineer reviewing actual screenshots on 2026-08-04 found real
defects in shipped UI:

1. `openChatTemplateLifecycleModal` in `static/js/features/presets.js` read
   `rel.version`, `latest.content_sha256`, `releasesResult.active_index` and
   `releasesResult.source` — none of which exist. `GET /api/chat-template/releases`
   in `src/web/api/spawn_wizard.rs` returns
   `{ok, releases:[{sha256, revision, source_url, fetch_url, installed_at, file}], active_sha256}`.
   HISTORY and DISCUSSIONS rendered permanently empty.
2. Rollback POST sent `{name, release: rel.version}`; backend expects `{name, sha256}`.
3. Jinja line-number gutter in the Create-fix editor collided with code text
   (missing min-width/padding on the `lineNumbers.style.cssText` div).

All three are FIXED and gate-clean (cargo build/clippy/test, npm run
validate-js/lint). Do not re-fix.

The lesson: a full Coordinator pass captured screenshots and did not look at
them. Every "verified" claim on this branch is suspect. This plan covers the
three workstreams that follow from that.

## Phase A — capture.mjs split (enables everything else)

`tests/ui/capture.mjs` is 6370 lines, 34 scenarios, ~186 capture call sites, and
interleaves three runtimes (llama.cpp remote-attach, llama.cpp local-spawn,
Rapid-MLX local-spawn) across two UI surfaces (Spawn Wizard, Preset Editor).
Reviewing a screenshot against its capturing code currently requires scrolling a
6000-line file. That is why the Phase 9 defects survived a full pass.

### Target layout

```
tests/ui/capture/
  index.mjs                  # runCli, SCENARIOS registry, arg parsing
  harness/
    server.mjs               # findAvailablePort, waitForHttp, spawnLlamaMonitor,
                             #   cleanupServer, seedConfig, cleanupTempHome
    fixtures.mjs             # seedRapidMlxCapturePreset, models-dir seeding,
                             #   spawn-wizard-engines nested-MLX fixture
    browser.mjs              # launchBrowser, gotoApp, loadAppDocument,
                             #   waitForMonitor, switchTab, viewport/theme toggling
    attach.mjs               # attachToServer, captureAuthShell, REMOTE_SERVER
    shot.mjs                 # captureShot, captureCloseUp,
                             #   captureElementScreenshot, captureSparklineClips,
                             #   cleanupScreenshotTabs, frames/gif helpers
    chat.mjs                 # clearExistingChats, createFreshChat,
                             #   activateScreenshotChat, sendChatPrompt,
                             #   waitForChat*, describeSuggestionsPanel,
                             #   attachSuggestionsResponseLogger, logChatState
  scenarios/
    core/                    # welcome, free-cache, chat, sidebar, guided-gen,
                             #   navbar, smoke, panels, dashboard
    models/                  # models, models-v2, model-discovery, filebrowser
    presets/                 # preset-editor, rapid-preset, evidence-drawer,
                             #   community-sources, discussions
    wizard-llamacpp/         # spawn-wizard, spawn-wizard-engines,
                             #   spawn-wizard-gif, spawn-wizard-hf-download
    wizard-rapidmlx/         # spawn-wizard-rapid-mlx-gif, rapid-mlx-runtime,
                             #   rapid-mlx-live, dashboard-rapid-mlx
    config/                  # settings, appearance-palette, tls
    features/                # tune-panel, benchmark-results, llama-updater,
                             #   chat-history-qa
    validation/              # sparkline, gifs
```

### Packet A1 — extract harness, zero behavior change
- Move the 30-odd helper functions (currently capture.mjs lines ~120–1010) into
  `harness/*.mjs` with named exports. `capture.mjs` becomes a thin re-export
  shim so nothing else breaks during the move.
- Acceptance: `node capture.mjs --list-scenarios` prints the same 34 names in
  the same order; `capture:sparkline` and `capture:gifs` produce
  byte-comparable-in-layout screenshots (spot-check 5).

### Packet A2 — per-scenario setup() hooks
- `runCli` currently branches on `scenarioName` for fixture seeding
  (`rapid-preset`/`discussions` -> `seedRapidMlxCapturePreset`;
  `spawn-wizard-engines` -> nested-MLX config fixture;
  `models`/`models-v2`/`panels` -> fake .gguf dir + `--models-dir`).
  Convert each scenario to `{ run, setup?, extraArgs? }`.
- Acceptance: no `scenarioName ===` string comparisons remain in the runner.

### Packet A3 — split scenario bodies into files
- One file per scenario (or per tight cluster), each exporting a default
  scenario object plus a `SHOTS` manifest: for every screenshot, its filename,
  a one-line INTENT string, and the selector/state it should show.
- Acceptance: no scenario file exceeds ~400 lines; every capture call has an
  INTENT string.

### Packet A4 — preserve npm scripts + add new ones
- `tests/ui/package.json` keeps `capture:list`, `capture:sparkline`,
  `capture:gifs` pointing at a preserved `capture.mjs` entry (either the shim or
  repointed to `capture/index.mjs`). Add `capture:group -- <group>` to run a
  whole directory, and `capture:manifest` to print filename+INTENT pairs for
  review.
- Acceptance: all three existing scripts work unchanged from a clean checkout.

### Packet A5 — delete the shim
- Only after A1–A4 land and one full audit pass (Phase C) has used the new
  layout. Update `tests/ui/README.md` and the "Register the scenario in
  SCENARIOS" instructions at capture.mjs:69.

## Phase B — chat-template lifecycle UI/UX rework

### B0 — Findings that change the scope

- **Rapid-MLX chat templates are already wired backend-side.**
  `src/inference/rapid_mlx/mod.rs` carries `chat_template_file` on both config
  and adapter (:275, :562, :694), and `build_launch_argv` (:987) builds a
  template overlay directory via `model_resolver::create_template_overlay` and
  launches from that. The A10/A27 "one selection layer, two appliers"
  architecture SHIPPED as an applier.
- **The selection layer never gained the Rapid-MLX half.**
  `static/js/features/spawn-wizard-rapid-mlx.js` contains zero chat-template
  references. `static/js/features/spawn-wizard-chat-template.js` contains zero
  runtime/engine references — it has no notion that two runtimes exist.
  So on the Rapid-MLX wizard path the user cannot pick a template at all, even
  though the launcher would honor one.
- **The docs are stale.** `docs/reference/spawn-wizard.md` "Runtime coverage —
  llama.cpp only today" (lines ~92–112) no longer matches the code.
- **The dual-surface duplication is real and unguarded.** Same doc, lines 70–91:
  two hand-written UIs, no automated parity check.

### B1 — Information architecture (the actual UX problem)

Today a user meets four controls with no explanation: Recommended, Manage /
Check-for-updates, History, Discussions, plus a Create-fix-from-discussion
editor. Nothing states what a chat template is, why they'd change it, or what
happens if they get it wrong. Proposed IA:

**Default (novice) view — one control plus one status line.**
- Status line, always visible: `Chat template: <name> — using the model's
  built-in template` OR `... — custom (Unsloth fix, installed 3 days ago)`.
- One primary button: **Use recommended template** (or **Revert to built-in**
  when a custom one is active). Everything else lives behind a single
  **Advanced** disclosure. Most users should never open it.
- Plain-language helper under the status line, one sentence:
  "Chat templates control how your messages are formatted before the model sees
  them. The built-in one usually works; a fixed template can repair broken
  tool-calling or thinking tags."

**Advanced disclosure (power user) — the Lifecycle modal, one entry point.**
Replace the button row with a single **Manage template…** that opens the
lifecycle modal. Inside, four labeled sections with explanatory subtitles:
- *Current* — name, sha256 (short), source URL, installed-at, active/inactive.
- *Updates* — "Check for a newer version of this template" + result.
- *History* — every installed release; each row shows short sha256, revision,
  source, installed-at, and a **Use this version** action (rollback). Uses
  `sha256` from the real response shape; this is where the bug was.
- *Community fixes* — was "Discussions". Subtitle: "Fixes other people have
  posted for this model's template on Hugging Face." Each row -> **Preview and
  install**, which opens the Create-fix editor pre-filled, runs the smoke test,
  and only activates on pass.

**Copy renames** (the cheapest, highest-leverage change):

| Today | Proposed | Why |
|---|---|---|
| Recommended | Use recommended template | Verb, states the effect |
| Manage / Check for updates | Manage template… | One door, not three |
| Discussions | Community fixes | "Discussions" is HF jargon |
| Create fix | Edit and install this fix | Says what the button does |
| History | Version history | Disambiguates from chat history |
| (rollback, unlabeled) | Use this version | Reversible-sounding |

Every destructive-feeling action needs an "you can undo this from Version
history" reassurance line.

### B2 — Shared component, one implementation

Extract a `static/js/features/chat-template-panel.js` that renders the status
line, the primary button, and the lifecycle modal from a single config object
`{ runtime: 'llama.cpp'|'rapid-mlx', surface: 'wizard'|'editor', getPath(),
setPath(), modelIdentity() }`. Both `spawn-wizard-chat-template.js` and the
`presets.js` modal field row become thin adapters. This retires the
"must land in both files" hazard documented in
`docs/reference/spawn-wizard.md:70`.

Acceptance per packet:
- **B2a** — panel module exists, llama.cpp wizard uses it, screenshots identical
  in intent to pre-change.
- **B2b** — Preset Editor uses it; the duplicated ~600 lines in `presets.js`
  (roughly lines 3170–3900) are deleted, not left dead.
- **B2c** — Rapid-MLX wizard renders the same panel; selecting a template sets
  `chat_template_file` on the Rapid-MLX preset and the overlay applier receives
  it. Verify end-to-end that `build_launch_argv` produces an overlay path.
- **B2d** — a JS unit or DOM test asserts the frontend reads exactly the field
  names `spawn_wizard.rs` emits (`sha256`, `revision`, `source_url`,
  `fetch_url`, `installed_at`, `file`, `active_sha256`). This is the regression
  guard for the bug that started this plan.
- **B2e** — `docs/reference/spawn-wizard.md` rewritten: delete "llama.cpp only
  today", document the overlay applier, replace the two-surfaces warning with
  the shared-component contract.

### B3 — Runtime-specific truth, surfaced

llama.cpp passes `--chat-template-file` directly; Rapid-MLX builds an overlay
model directory. Users should not have to know this, but the modal's *Current*
section should say, in one line, how the template reaches the runtime, because
when it fails the failure mode differs (missing flag vs. failed overlay, which
today only prints a `eprintln!` warning and silently falls back to the native
template — see `mod.rs:1002`). That silent fallback needs a surfaced warning in
the UI. Acceptance: overlay-creation failure produces a visible toast/status,
not just stderr.

## Phase C — full-branch defect sweep

Scope, not execution. The branch has 144 changed files vs `main` and 348
commits. Top-touched UI files: `static/js/features/presets.js` (58 commits),
`spawn-wizard.js` (50), `static/index.html` (50), `models.js` (36),
`static/css/spawn-wizard.css` (30), `modal-premium.css` (22),
`setup-view.js` (20), `hf-browse.js` (20), `vram-estimate.js` (14),
`spawn-wizard-chat-template.js` (10).

**Volume estimate:** 34 scenarios, ~186 capture call sites, and several
scenarios loop; realistic total is 250–400 screenshots. At the observed rate of
reviewing a screenshot against its capture-code intent, budget roughly 8–12
scenarios per session, i.e. 3–4 sessions for full coverage, assuming Phase A has
landed (without it, double that — the whole point of A is making C tractable).

**Method, per scenario:** run it, open every produced screenshot, and for each
compare three things: (1) filename, (2) the INTENT string / surrounding capture
code, (3) what is actually rendered. Log every mismatch — empty lists, placeholder
text, wrong tab, clipped modal, unreadable contrast, stale data — into a defect
register. Do not fix during the sweep; fix in a follow-up batch so the sweep
stays fast.

**Execution order — highest risk first:**

1. **Tier 1 — Phase 9 chat-template surfaces (local model's pass).**
   `preset-editor`, `discussions`, `rapid-preset`, `spawn-wizard` (chat-template
   step). Three confirmed defects already; assume more.
2. **Tier 2 — Rapid-MLX surfaces.** `rapid-mlx-runtime`, `rapid-mlx-live`,
   `dashboard-rapid-mlx`, `spawn-wizard-rapid-mlx-gif`. Newest code, thinnest
   review.
3. **Tier 3 — Spawn Wizard llama.cpp.** `spawn-wizard-engines`,
   `spawn-wizard-hf-download`, `spawn-wizard-gif`. 50 commits of churn.
4. **Tier 4 — Models/VRAM/HF.** `models`, `models-v2`, `model-discovery`,
   `evidence-drawer`, `community-sources`. `vram.rs`/`hf.rs` churn.
5. **Tier 5 — Chrome and theming.** `navbar`, `panels`, `appearance-palette`,
   `settings`, `tls`, `filebrowser`, `dashboard`.
6. **Tier 6 — Stable/older.** `welcome`, `chat`, `sidebar`, `guided-gen`,
   `free-cache`, `tune-panel`, `benchmark-results`, `llama-updater`,
   `chat-history-qa`, `sparkline`, `gifs`, `smoke`.

**Acceptance:** a defect register doc with one row per finding
(scenario, screenshot filename, expected, actual, severity), and a Tier 1–2
sign-off that every screenshot was individually viewed. "Ran the scenario" is
explicitly NOT sign-off — that was the failure mode being corrected.

## Phase D — noted follow-ups (record only, not scoped here)

- **D1 — Windows/5090 CPU pegging.** Loading a Qwen3.5-9B Q4_K_M GGUF via the
  llama.cpp path eventually pinned all CPUs to 100% during inference on a
  Windows/5090 box. Needs repro and triage. May already be resolved by the
  rearchitecture; unverified. Owner: whoever picks up llama.cpp supervisor work.
- **D2 — Windows rendering.** Pre-rearchitecture, the Spawn Wizard and Preset
  Editor rendered too small / hard to read on the same Windows box, and some
  dark-mode dropdowns were unreadable. Schedule a Windows-viewport screenshot
  pass once Phase A lands (the split makes a viewport-parameterized run cheap).
  May already be fixed; unverified. Sequence after Phase A, fold into Phase C
  Tier 5.

## Sequencing

A1–A4 -> B1/B2 (B can start on copy/IA in parallel with A) -> C Tier 1–2 ->
B2d regression guard -> C Tier 3–6 -> A5 -> D2 -> D1.

### Critical Files for Implementation
- /Users/nick/SCRIPTS/CLAUDE/llama-monitor/tests/ui/capture.mjs
- /Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/presets.js
- /Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/spawn-wizard-chat-template.js
- /Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/spawn-wizard-rapid-mlx.js
- /Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/inference/rapid_mlx/mod.rs
