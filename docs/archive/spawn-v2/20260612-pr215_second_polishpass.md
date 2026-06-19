# PR #215 — Second Polish Pass (UX, Clarity, Onboarding)

Purpose:
- Make the app immediately understandable to a non-expert user (first-time local-LLM user) while still being fully functional for power users.
- Remove/soften internal jargon in all primary user flows.
- Standardize terminology.
- Improve tooltips, guidance, and visual hierarchy.
- This document is the canonical spec for a fresh AI agent to implement all changes.

General rules:
- Preserve all existing behavior, functionality, and advanced options.
- Only adjust:
  - Labels
  - Descriptions
  - Tooltips
  - Short hints
- Never rename critical internal IDs, classes, or data attributes unless absolutely required; prefer cosmetic changes in text nodes and attributes (aria-label, title) and small HTML edits.
- If an area is already good, leave it alone.

Terminology to enforce (globally):
- Use:
  - "server" for the running llama.cpp process.
  - "model" when referring to the AI (GGUF).
  - "preset" as "saved configuration."
- Avoid:
  - "spawn" in user-facing labels (internally fine).
  - "endpoint" in primary UI (ok only where strictly API-relevant).
  - Raw CLI/implementation talk at top level; push to "advanced" zones.

High-level change areas:
1. Landing page
2. Setup view / Preset grid
3. Presets panel / No-presets message
4. Spawn Wizard entry
5. Wizard steps and labels
6. Wizard hardware / advanced controls
7. Wizard "Preset Parameters" step
8. Wizard final "Spawn" step
9. Dashboard / Metrics labels and hints

Below: each task is given:
- ID
- Area
- Current text
- Location(s)
- Required change
- Notes for implementation

========================================
1. LANDING PAGE
========================================

T-001: Landing tagline too technical
- Area: Landing page brand
- Current:
  - static/index.html:220
    - "Local inference, fully observed"
- Issue:
  - Too abstract and developer-centric for beginners.
- Change:
  - Replace with:
    - "Run and monitor local AI models on your Mac."
- Notes:
  - Keep "Llama Monitor" as brand name; this line is just the subtitle.

========================================
2. SETUP VIEW / PRESET GRID AREA
========================================

T-002: "Connect to Endpoint" heading
- Area: Landing / connection
- Current:
  - static/index.html:238
    - "Connect to Endpoint"
- Issue:
  - "Endpoint" is too technical as primary label.
- Change:
  - Replace with:
    - "Connect to a running model"
- Notes:
  - This is the card/section where users paste a URL like localhost:8001.

T-003: "Recent" section label (recent endpoints)
- Area: Landing / recent list
- Current:
  - static/index.html:243
    - <span class="setup-section-title">Recent</span>
- Issue:
  - Vague; doesn't say "recent what."
- Change:
  - Replace with:
    - "Recent servers"
- Notes:
  - These are remembered server/session connections.

T-004: "New Configuration" card label
- Area: Setup view / primary CTA
- Current:
  - static/index.html:277
    - "New Configuration"
- Issue:
  - "Configuration" is too abstract as a first-time CTA.
- Change:
  - Replace with:
    - "New model"
- Notes:
  - This card opens the wizard to set up a new model config.

T-005: VRAM legend accessibility
- Area: Setup view / VRAM legend
- Current:
  - static/index.html:307–313
    - "Fits in VRAM"
    - "Tight fit"
    - "May exceed VRAM"
- Issue:
  - "VRAM" is GPU jargon on Mac; better phrase for general users.
- Change:
  - Replace with:
    - "Fits in memory"
    - "Tight fit"
    - "May exceed memory"
- Notes:
  - Keep colors and structure.

T-006: First-time onboarding hint
- Area: Setup view JS
- Current:
  - static/js/features/setup-view.js:307
    - hint.textContent = 'First time? Use the wizard to pick a model and configure your server.';
- Issue:
  - Too terse; uses "wizard", "server" without framing.
- Change:
  - New text:
    - 'New here? Open the setup wizard to pick a model and start your first local AI in a few steps.'
- Notes:
  - This hint should be clear, friendly, and action-oriented.

T-007: Preset card chips: "ctx" abbreviation
- Area: Setup view / preset metadata
- Current:
  - static/js/features/setup-view.js:361
    - Template like: `${ctxK}k ctx`
- Issue:
  - "ctx" is abbreviation; not obvious for beginners.
- Change:
  - Replace with:
    - `${ctxK}k context`
- Notes:
  - Keep same visual style (chip), just expand wording.

T-008: Preset card chips: KV cache display
- Area: Setup view / preset metadata
- Current:
  - static/js/features/setup-view.js:362
    - KV chip: `${ctkDisplay}/${ctvDisplay}` (or similar)
- Issue:
  - No explanation for what "KV" is.
- Change:
  - Prefix with a clearer label and tooltip:
    - Chip label: `KV: ${ctkDisplay}/${ctvDisplay}`
    - Tooltip: "KV cache precision (how accurately the model stores past tokens). q8_0 is recommended for most users."
- Notes:
  - Ensure tooltip is added via title or data-tooltip on the chip element.

T-009: Preset card chips: tooltip text
- Area: Setup view / preset metadata
- Current:
  - static/js/features/setup-view.js:429
    - 'Click to quickly edit context or KV cache settings'
- Issue:
  - Too technical; unclear value.
- Change:
  - New text:
    - "Click to adjust context length and memory settings."
- Notes:
  - Keep click behavior.

T-010: Preset card start button clarity
- Area: Setup view / preset actions
- Current:
  - static/js/features/setup-view.js:405
    - '▶ Start' / 'Set up model →'
- Issue:
  - "Start" is ok but vague: start what?
- Change:
  - Replace:
    - '▶ Start server' for presets that can start
    - 'Set up model →' is fine; keep as-is.

========================================
3. PRESETS PANEL / NO-PRESETS MESSAGE
========================================

T-011: "No presets saved yet" message
- Area: Presets panel
- Current:
  - static/js/features/presets.js:535
    - 'No presets saved yet. Use the Spawn Wizard to create one.'
- Issue:
  - Uses "Spawn Wizard" term.
- Change:
  - New text:
    - "No presets saved yet. Use the setup wizard to create one."
- Notes:
  - Keep link/button behavior that opens the wizard.

T-012: Preset meta chips in presets panel ("k ctx")
- Area: Presets panel
- Current:
  - static/js/features/presets.js:561
    - `${Math.round(preset.context_size / 1024)}k ctx`
- Issue:
  - Same "ctx" abbreviation as T-007.
- Change:
  - New text:
    - `${Math.round(preset.context_size / 1024)}k context`

T-013: Preset meta chips in presets panel (KV)
- Area: Presets panel
- Current:
  - static/js/features/presets.js:563–564
    - `KV: ${ctk}/${ctv}`
- Issue:
  - Jargon without explanation.
- Change:
  - Add tooltip to the KV chip:
    - Tooltip: "KV cache precision for past tokens. q8_0 is recommended for most users."
- Notes:
  - Keep label "KV: ..." but make it approachable via tooltip.

========================================
4. SPAWn WIZARD ENTRY
========================================

T-014: "Spawn Server" button in setup view
- Area: Wizard entry
- Current:
  - static/index.html:390
    - "Spawn Server"
- Issue:
  - "Spawn" is internal jargon.
- Change:
  - New label:
    - "Open setup wizard"
- Notes:
  - Internally, it still spawns; we just rename the button text.

========================================
5. WIZARD STEPS AND LABELS
========================================

T-015: Wizard header title
- Area: Wizard header
- Current:
  - static/index.html:2886
    - "Spawn Llama-Server"
- Issue:
  - "Spawn" and "Llama-Server" are internal.
- Change:
  - New title:
    - "Start a local model server"
- Notes:
  - This is what the user sees at top of wizard modal.

T-016: Wizard step labels
- Area: Wizard header / step indicator
- Current:
  - static/js/features/spawn-wizard.js:163
    - const STEP_LABELS = ['Profile', 'Model', 'Hardware', 'Parameters', 'Summary', 'Spawn'];
- Issue:
  - Too abstract for non-experts.
- Change:
  - New labels:
    - Step 1: "How you’ll use it"
    - Step 2: "Choose model"
    - Step 3: "Hardware & memory"
    - Step 4: "Settings"
    - Step 5: "Review settings"
    - Step 6: "Start server"
- Notes:
  - Keep internal step indices and logic unchanged.
  - Update any hard-coded step tooltip text in static/index.html:2890–2900 to match new labels:
    - Step 1 tooltip: "Step 1: How you’ll use it"
    - Step 2 tooltip: "Step 2: Choose model"
    - Step 3 tooltip: "Step 3: Hardware & memory"
    - Step 4 tooltip: "Step 4: Settings"
    - Step 5 tooltip: "Step 5: Review settings"
    - Step 6 tooltip: "Step 6: Start server"

========================================
6. WIZARD HARDWARE / ADVANCED CONTROLS
========================================

Goal:
- Keep all knobs, but clarify language, add plain-language tooltips, and visually separate advanced stuff.

T-017: GPU layers tooltip
- Area: Wizard Hardware
- Current:
  - static/index.html:3289–3294
    - Label: "GPU layers"
- Issue:
  - Non-experts don’t know what “GPU layers” means.
- Change:
  - Keep label: "GPU layers"
  - Update/add tooltip (title or info icon) to:
    - "Which parts of the model run on the GPU. ‘Auto (recommended)’ chooses the best fit for your system."
- Notes:
  - Options: "Auto (recommended)", "All layers on GPU", "Manual" are fine.

T-018: Context size hint
- Area: Wizard Hardware
- Current:
  - static/index.html:3302–3306
    - Label: "Context size"
    - Hint: "Primary tradeoff: more context uses more KV memory."
- Issue:
  - Too technical; missing simple explanation.
- Change:
  - New hint:
    - "How much text history the model can consider at once. Larger = slower and more memory."
- Notes:
  - Keep the quick-picks (Chat/RP/Agent).

T-019: KV cache quant labels
- Area: Wizard Hardware
- Current:
  - static/index.html:3321–3329
    - "K cache quant"
    - "V cache quant"
- Issue:
  - Too opaque.
- Change:
  - New labels:
    - "K cache quant (KV precision)"
    - "V cache quant (KV precision)"
  - Add tooltip (shared or per-field):
    - "Controls how accurately the model stores past tokens. q8_0 is recommended for most users."
- Notes:
  - "Unified" toggle can stay as-is.

T-020: MTP / speculative decoding explanation
- Area: Wizard Hardware / MTP
- Current:
  - static/index.html:3344–3395
    - "Multi-Token Prediction (MTP)"
    - "Speculative decoding: the model drafts multiple tokens per step and verifies them in one pass..."
- Issue:
  - Accurate but dense.
- Change:
  - Add a simpler top-level line before technical explanation:
    - "Try to speed up replies by guessing several words at once. Usually safe, but you can test with and without."
  - Keep the existing technical explanation as a smaller, secondary note.

T-021: Speculative decoding modes overwhelming
- Area: MTP / speculative decoding
- Current:
  - static/index.html:3506–3566
    - Many modes: ngram-mod, ngram-simple, ngram-map-k, ngram-map-k4v, draft-mtp
- Issue:
  - Intimidating for all but advanced users.
- Change:
  - Keep all options.
  - Wrap them in a collapsible "Advanced modes" section or visually group:
    - Default visible options:
      - "None (disabled)"
      - "Recommended" (if applicable)
    - Others under "Advanced modes" or similar.
  - Add hint:
    - "For most users, the default is enough. These modes are for advanced tuning."
- Notes:
  - Implement via simple details/summary or class-based collapse, consistent with existing UI patterns.

T-022: Batch size / Ubatch labels
- Area: Advanced options
- Current:
  - static/index.html:3408–3419
    - "Batch size (-b)"
    - "Ubatch size (-ub)"
- Issue:
  - Exposes raw flags directly.
- Change:
  - New labels:
    - "Prompt batch size" with small note:
      - "Advanced — affects how fast prompts are processed. Internal flag: -b"
    - "Uniform batch size" with note:
      - "Advanced — for experienced users only. Internal flag: -ub"
- Notes:
  - Keep existing defaults and descriptions; just clarify language.

T-023: "Extra args" label
- Area: Wizard step 4: Parameters
- Current:
  - static/index.html:3901
    - "Extra args" with title referencing "Additional llama-server CLI flags..."
- Issue:
  - Very implementation-focused.
- Change:
  - New label:
    - "Extra command-line arguments"
  - New tooltip:
    - "If you know llama.cpp, you can add custom flags here. Example: --log-verbosity 2 --no-perf"
- Notes:
  - Keep existing example; just soften wording.

========================================
7. WIZARD "PRESET PARAMETERS" STEP
========================================

T-024: "Preset Parameters" section title
- Area: Wizard step 5
- Current:
  - static/index.html:3925
    - "Preset Parameters"
- Issue:
  - Too internal; unclear purpose.
- Change:
  - New title:
    - "Preset settings"

T-025: "Preset Parameters" description
- Area: Wizard step 5
- Current:
  - static/index.html:3926–3928
    - "Every flag that will be saved with this preset. Save it for quick reuse, then click Next to launch."
- Issue:
  - "Every flag" is CLI-speak.
- Change:
  - New description:
    - "These are the settings that will be saved with this profile. Save as a preset so you can reuse it later, then click Next to start the server."

T-026: Preset save button label
- Area: Wizard step 5
- Current:
  - static/index.html:3933–3934
    - "Save as Preset"
- Issue:
  - Slightly awkward.
- Change:
  - New label:
    - "Save preset"

========================================
8. WIZARD FINAL "SPAWN" STEP
========================================

T-027: Final step guardrail text
- Area: Wizard step 6
- Current:
  - static/js/features/spawn-wizard.js:1252
    - info('Spawn starts the server with the configuration shown above.');
- Issue:
  - "Spawn" is jargon.
- Change:
  - New text:
    - "This will start the server with the configuration shown above."

T-028: Final "Spawn Server" button
- Area: Wizard step 6
- Current:
  - static/index.html:3960
    - "Spawn Server"
- Issue:
  - Jargon at the final action.
- Change:
  - New label:
    - "Start server"

========================================
9. DASHBOARD / METRICS LABELS AND HINTS
========================================

Goal:
- Keep layout intact; add concise tooltips and hints so users understand if metrics are "good."

T-029: Inference Metrics section
- Area: Dashboard
- Current:
  - static/index.html:495–497
    - "Inference Metrics"
- Issue:
  - "Inference" is jargon.
- Change:
  - New title:
    - "Performance & metrics"
- Notes:
  - Keep any data bindings; change only visible text.

T-030: THROUGHPUT widget label
- Area: Dashboard
- Current:
  - static/index.html:510
    - "THROUGHPUT"
- Issue:
  - OK for experts, unclear for many.
- Change:
  - New label:
    - "Speed (throughput)"
- Notes:
  - Keep internal IDs; change label text only.

T-031: GENERATION widget label
- Area: Dashboard
- Current:
  - static/index.html:557
    - "GENERATION"
- Issue:
  - Vague.
- Change:
  - New label:
    - "Generation details"

T-032: CONTEXT WINDOW widget label
- Area: Dashboard
- Current:
  - static/index.html:591
    - "CONTEXT WINDOW"
- Issue:
  - Common LLM term but not universally known.
- Change:
  - New label:
    - "Context & memory"
- Notes:
  - Optionally add tooltip: "How much context is in use vs available."

T-033: SLOT ACTIVITY widget label
- Area: Dashboard
- Current:
  - static/index.html:638
    - "SLOT ACTIVITY"
- Issue:
  - Internal llama.cpp concept.
- Change:
  - New label:
    - "Active sessions"
- Notes:
  - Add tooltip: "Slots are parallel conversation sessions the server is handling."

T-034: REQUEST ACTIVITY widget label
- Area: Dashboard
- Current:
  - static/index.html:660
    - "REQUEST ACTIVITY"
- Issue:
  - OK, but can be more specific.
- Change:
  - New label:
    - "Requests"

T-035: MODEL & DECODING widget label
- Area: Dashboard
- Current:
  - static/index.html:677
    - "MODEL & DECODING"
- Issue:
  - "Decoding" is jargon.
- Change:
  - New label:
    - "Model info"

T-036: Speculative decoding chip text
- Area: Model info
- Current:
  - static/index.html:681
    - "Speculative decoding unavailable"
- Issue:
  - Too technical as a default chip.
- Change:
  - New text:
    - "Speculative decoding: not in use"
- Notes:
  - When enabled, say "Speculative decoding: active" with a small tooltip describing what it does.

T-037: GPU widget labels
- Area: Dashboard
- Current:
  - static/index.html:692
    - "GPU"
- Issue:
  - Acceptable, but add short hint for beginners.
- Change:
  - Keep label "GPU"
  - Add tooltip or small helper text:
    - "Graphics/memory usage for model acceleration."

T-038: Add short "is this good?" hint in dashboard
- Area: Dashboard
- Current:
  - No centralized "this is healthy" hint.
- Issue:
  - Users see numbers but lack context.
- Change:
  - Add a small, non-intrusive hint near the metrics area (e.g., under "Performance & metrics"):
    - "Typical targets on M5 Max: 10–15+ tokens/sec, low memory spikes, no red warnings."
- Notes:
  - Implement as a muted helper line or tooltip; do not dominate the view.

========================================
10. GLOBAL TERMINOLOGY PASS
========================================

T-039: Global "spawn" to "start" replacements
- Scan:
  - static/index.html
  - static/js/features/spawn-wizard.js (only user-facing strings, not internal logic)
  - static/js/features/setup-view.js
  - static/js/features/presets.js
- Rule:
  - In buttons, headings, step labels, short descriptions:
    - "Spawn Server" → "Start server"
    - "Spawn Wizard" → "Setup wizard"
    - "Spawn" (as a step or action label) → "Start server"
  - Internally (function names, internal logs, comments) can remain as-is.

T-040: Global "endpoint" to "server" in primary UI
- Rule:
  - In user-visible text (headings, placeholders, hints):
    - "endpoint" → "server" or "connection"
  - Leave "endpoint" where it is strictly describing an HTTP endpoint for integrations/API.

T-041: Global "ctx" to "context"
- Rule:
  - Anywhere chips or hints say "ctx", change to "context".
  - Already covered by T-007 and T-012; ensure no others remain.

========================================
IMPLEMENTATION ORDER (for the agent)
========================================

Recommended order (to avoid confusion):

1) Global terminology:
   - T-039, T-040, T-041
   - Do find-and-replace carefully with context-aware checks.

2) Landing page + setup view:
   - T-001 to T-010

3) Presets panel:
   - T-011 to T-013

4) Wizard entry + header:
   - T-014 to T-016

5) Wizard hardware + advanced:
   - T-017 to T-023

6) Wizard preset + spawn:
   - T-024 to T-028

7) Dashboard + metrics:
   - T-029 to T-038

8) Final pass:
   - Confirm all "spawn", "endpoint", "ctx" replacements are consistent
   - Ensure all tooltips are added where specified
   - Ensure no accidental breakage of JS bindings

========================================
10.0 Agent workflow and approach (non-sleep tasks)
========================================

Purpose:
- Provide a safe, incremental workflow for an AI agent (with no prior
  context) to implement all non-sleep polish tasks (T-001..T-041,
  D-001..D-029) without breaking functionality or over-engineering.

General principles:
- One change at a time.
- Only modify text labels, descriptions, tooltips, and small UI hints.
- Do not:
  - Change internal APIs
  - Change data structures
  - Change complex layout logic
  - Invent new features or refactor unrelated code
- Preserve:
  - All existing IDs, classes, data-test attributes (unless a task
    explicitly says to change them).
  - All existing JS bindings and CSS hooks.
- After each logical phase:
  - Run:
    - cargo fmt
    - cargo check (or cargo build if required)
    - Existing tests
  - If anything breaks, fix it before continuing.

Suggested implementation order (phases):

Phase A: Orientation

- Read:
  - This document (full).
  - static/index.html
  - static/js/features/spawn-wizard.js
  - static/js/features/setup-view.js
  - static/js/features/presets.js
  - Any dashboard-ws / telemetry UI modules you find under static/js/
- Build a quick mental map of:
  - Where each changed label appears.
  - Where each tooltip is wired.
  - Where IDs and classes are referenced between HTML and JS.

Phase B: Global terminology sweep

Goal: Make high-impact, low-risk text changes across the app.

Tasks:
- T-039: Global "spawn" → "start"
- T-040: Global "endpoint" → "server" (in primary UI)
- T-041: Global "ctx" → "context"

Rules:
- Only affect user-facing text (HTML text nodes, button labels,
  tooltips, JS strings shown in UI).
- Keep internal identifiers:
  - Class names, IDs, function names (e.g., spawnLlamaMonitor)
  - File names, scenario keys, internal comments are okay to leave.
- Use:
  - Localized, context-aware edits.
  - Double-check that the change didn’t alter JS logic or data binding.

Verification:
- Confirm:
  - No "Spawn Server" buttons visible; show "Start server".
  - No "Spawn Wizard" in primary UI; show "Setup wizard".
  - No confusing "ctx" abbreviation in chips.

Phase C: Landing page and setup view

Tasks:
- T-001 to T-010

Focus:
- Rewrite landing/subtitle text.
- Improve "Connect to Endpoint" → "Connect to a running model".
- Improve preset card tooltips and first-time hint.
- Make the page clear for a non-expert user.

Rules:
- Keep structure; change text.
- Ensure tooltips are concise and beginner-friendly.

Verification:
- Open the app; confirm:
  - Immediate clarity of purpose.
  - No jargon in primary buttons and labels.

Phase D: Presets panel

Tasks:
- T-011 to T-013

Rules:
- Ensure "No presets saved yet" message uses “Setup wizard”.
- Expand “ctx” → “context”; add KV tooltip.

Phase E: Spawn wizard entry and step labels

Tasks:
- T-014 to T-016

Focus:
- Rename wizard entry button and header.
- Replace abstract step labels with clear, user-facing wording.

Rules:
- Only change labels and step titles.
- Keep internal IDs for wizard steps and buttons.

Phase F: Wizard hardware and advanced controls

Tasks:
- T-017 to T-023

Focus:
- Clarify tooltips for GPU layers, context size, KV cache,
  speculative/MTP, batch size, extra args.
- Wrap advanced modes in “Show advanced”-style sections
  without removing any options.

Rules:
- Do not change ranges, defaults, or underlying behavior.
- Use collapsible groups where appropriate, following existing
  patterns in the UI.

Phase G: Wizard preset and final steps

Tasks:
- T-024 to T-028

Focus:
- Make “Preset Parameters” → “Preset settings”
- Make final step language clear: “Start server” not “Spawn”.

Phase H: Dashboard and metrics

Tasks:
- T-029 to T-038

Focus:
- Rename metrics sections:
  - “Inference Metrics” → “Performance & metrics”
  - “THROUGHPUT” → “Speed (throughput)”
  - “SLOT ACTIVITY” → “Active sessions”
  - etc.
- Add concise tooltips and a small “is this good?” hint.

Rules:
- Do not alter how metrics are computed or rendered.
- Only update labels, tooltips, and short helper text.

Phase I: Documentation updates

Tasks:
- D-001 to D-029

Focus:
- Align all docs, screenshots, and reference material with the new
  terminology and UX.
- Use D-029 (Consistency check) as a final sweep.

Rules:
- Update text to match UI precisely.
- Mark all screenshots that need recapture.
- Update capture.mjs as specified in D-008..D-013.

Overall quality bar:

- Code:
  - Clean, formatted (cargo fmt), compiles, tests pass.
- UI:
  - No confusing jargon in primary user flows.
- Docs:
  - Consistent with UI.
  - No stale references to old labels in public docs.

========================================
11. DOCUMENTATION IMPACT & STRATEGY
========================================

This section specifies all documentation, screenshot, and reference changes
required to align with the polish. It is written so a fresh agent can apply
changes mechanically.

General rules:
- All changes are terminology, clarity, and emphasis—no new architecture.
- Match the UI exactly after polish:
  - "Spawn Server" → "Start server"
  - "Spawn Wizard" → "Setup wizard"
  - "Connect to Endpoint" → "Connect to a running model"
  - "Recent endpoints" → "Recent servers"
  - "New Configuration" → "New model"
  - "ctx" → "context"
  - "Inference Metrics" → "Performance & metrics"
  - "THROUGHPUT" → "Speed (throughput)"
  - "SLOT ACTIVITY" → "Active sessions"
  - "Compact Mode" → "Focus mode"
- When updating docs:
  - Preserve technical accuracy.
  - Prefer beginner-friendly wording in headings and bullet text.
  - Keep internal field names, APIs, and stable IDs unchanged.

========================================
11.1 README.md changes
========================================

File: README.md

Tasks:

D-001: Global terminology replacements
- In all user-facing prose:
  - "Spawn Server" → "Start server"
  - "Spawn Wizard" → "Setup wizard"
  - "spawn" (verb describing starting llama-server) → "start"
  - "endpoint" (as user-facing) → "server" or "running model"
  - "ctx" → "context"
  - "Inference Metrics" → "Performance & metrics"
  - "THROUGHPUT" → "Speed (throughput)"
  - "SLOT ACTIVITY" → "Active sessions"
  - "Compact Mode" → "Focus mode"
- Keep internal code terms (e.g., env vars, API fields) intact.

D-002: Hero positioning
- Update any tagline/subtitle to align with new tone:
  - Emphasize:
    - "Run and monitor local AI models on your Mac."
    - "Hardware-aware Setup wizard."
    - "VRAM estimator that keeps you safe from OOM."
- Ensure first mention of key concepts:
  - "Setup wizard"
  - "VRAM estimator"
  - "Preset library"
  - "Performance & metrics"
  - "Active sessions"
  - "Focus mode"

D-003: Feature bullets
- Where the README lists features:
  - Describe:
    - "Performance & metrics panel: real-time visibility into speed,
      active sessions, and system load."
    - "Active sessions view: see parallel conversations at a glance."
    - "Focus mode: compact layout for uninterrupted chat and monitoring."
    - "Preset library: curated, model-specific configurations
      (Qwen, Gemma, gpt-oss, etc.)."
- Replace any bullets referencing:
  - "Spawn Wizard" → "Setup wizard"
  - "Spawn Server" → "Start server"

D-004: README screenshots
- All screenshots embedded in README must be updated after UI polish.
- Do NOT leave screenshots showing old labels:
  - "Spawn Server", "Inference Metrics", "SLOT ACTIVITY", "Compact Mode".
- In the README markdown, add TODO comments at each screenshot location:
  - Example:
    - "<!-- TODO: Recapture after polish; show 'Start server',
       'Recent servers', 'Performance & metrics' -->"

========================================
11.2 Screenshots: recapture, promote, retire
========================================

Path: docs/screenshots/

D-005: Identify impacted screenshots
- Any screenshot showing:
  - "Spawn Server"
  - "Spawn Wizard"
  - "Connect to Endpoint"
  - "Recent endpoints"
  - "New Configuration"
  - "Inference Metrics"
  - "THROUGHPUT"
  - "SLOT ACTIVITY"
  - "Compact Mode"
- Must be recaptured after polish.

D-006: Hero screenshots to capture
After polish, capture and promote into docs/screenshots and README:

- Home dashboard:
  - Shows:
    - "Start server" button
    - "Connect to a running model" card
    - "Recent servers" list
    - Preset cards with new "context" and GPU layers labels
- Setup wizard:
  - Step 1: "How you’ll use it" (profile)
  - Step 3: "Hardware & memory" showing VRAM estimator bar
    and "Fits in memory" wording
  - Step 6: "Start server" button
- Performance & metrics:
  - Top nav with:
    - "Performance & metrics"
    - "Speed (throughput)"
    - "Active sessions"
    - "Requests"
- Focus mode:
  - Show compact layout clearly; include label if present.
- Chat:
  - Show "New conversation" (if implemented) or main chat view.

D-007: Keep vs retire
- Recapture:
  - All wizard step screenshots
  - All dashboard/metrics screenshots used in public docs
- Keep internally (don’t use in public README or marketing):
  - Current screenshots as “before polish” reference (optional in repo).
- Do not use:
  - Old screenshots with outdated labels anywhere user-facing.

========================================
11.3 Screenshot capture script (capture.mjs)
========================================

File: tests/ui/capture.mjs

High-level:
- Single Puppeteer harness; runs named scenarios sequentially.
- Scenario registry at ~line 3374: const SCENARIOS = { ... }
- Outputs:
  - Primary: docs/screenshots/artifacts/
  - Temp frames: tests/ui/capture-frames/
- Many scenarios depend on UI structure; all should remain safe if
  we preserve IDs and data attributes (as the polish plan requires).

Concrete changes:

D-008: Welcome scenario (spawn wizard entry)
- Area: scenarioWelcome
- Comment:
  - Change:
    - `// Shot 2: click "New Server Wizard" and capture step 0...`
    - to:
    - `// Shot 2: click the Setup wizard button and capture step 0...`
- Fallback selector:
  - Current:
    - `:has-text("New Server Wizard")`
  - Must update to:
    - `:has-text("Setup wizard")`
    (or exact final button label chosen in T-014/T-039).
- Rationale:
  - This is the only place in the script that directly asserts this
    text; if we don’t update it, the fallback selector will fail after
    the polish.

D-009: Endpoint → server wording in logs
- Area: attachToServer helper comments
- Change:
  - `[CAPTURE] Endpoint URL not confirmed in display (non-fatal)`
  - to:
  - `[CAPTURE] Server URL not confirmed in display (non-fatal)`
- Rationale:
  - Cosmetic alignment with "server" terminology.

D-010: Dashboard scenario naming
- Area: scenarioDashboard, scenarioGifs
- Optional but recommended:
  - Rename:
    - `dashboard-inference-section.png`
      → `dashboard-performance-section.png`
    - `inference-metrics.gif`
      → `performance-metrics.gif`
- Rationale:
  - "Inference Metrics" is changing to "Performance & metrics."

D-011: Spawn wizard scenario names
- Area: scenario keys 'spawn-wizard', 'spawn-wizard-gif',
  'spawn-wizard-hf-download', and their outputs
- Decision:
  - Do NOT change scenario keys or function names (internal CLI usage).
- Do:
  - Update header comments describing these scenarios:
    - "Spawn Wizard" → "Setup wizard"
    - E.g.:
      - "Scenario: Spawn Wizard flow" → "Scenario: Setup wizard flow"
- Rationale:
  - Keep external behavior and filenames stable; comments reflect new UX.

D-012: Selectors stability
- The script heavily relies on:
  - button[data-tab="${name}"]
  - #view-setup, #view-monitor
  - #setup-endpoint-url
  - #setup-attach-btn
  - #spawn-wizard-overlay.open
  - #chat-input, #chat-telemetry-btn, #chat-focus-mode-btn
  - #gpu-section, #system-section
  - #suggestions-*, .suggestion-item
- Requirement:
  - Polish must preserve these IDs and data attributes.
  - If any are renamed, capture.mjs must be updated in lockstep.

D-013: Run guidance
- Add a short comment at the top of capture.mjs:
  - "Run this after the UX polish pass is merged to produce final
     docs screenshots using the updated UI labels."

========================================
11.4 Reference docs: per-file changes
========================================

These are in docs/reference/*.md. For each:

D-014: api.md
- Update user-facing terminology:
  - "Spawn Server" → "Start server"
  - "Spawn Wizard" → "Setup wizard"
  - "Connect to Endpoint" → "Connect to a running model"
  - "Session" (as UI concept) → "model profile" where appropriate
- Keep internal API field names unchanged.

D-015: capabilities.md
- If it documents status labels shown in UI:
  - "Inference only" → "Basic"
  - "Limited" → "Partial"
  - "OK" → "Full"
- Ensure prose matches the new labels and rationale.

D-016: chat.md
- Align with "Conversations" and "New conversation":
  - "New tab" → "New conversation"
  - Clarify: “Tabs represent active conversations.”
- "Compact Mode" → "Focus mode"

D-017: cli-flags.md
- Adjust:
  - "Local llama.cpp spawn" → "Local llama.cpp launch"
  - "Spawn Wizard" → "Setup wizard"
- Keep all flags exactly as-is.

D-018: dashboard.md
- Update:
  - "Inference Metrics" → "Performance & metrics"
  - "THROUGHPUT" → "Speed (throughput)"
  - "SLOT ACTIVITY" → "Active sessions"
  - "Request Activity" → "Connection details"
- "Session" (UI sense) → "Model" or "Model profile"
- Ensure capability popover labels align with D-015.

D-019: inference-tuning.md
- Update:
  - "Spawn Wizard" → "Setup wizard"
- Add short note:
  - "When configuring models, use the Setup wizard or Preset Editor,
     which incorporate these tuning recommendations."
- Ensure "context" instead of "ctx" in all prose.

D-020: realtime-communication.md
- Update:
  - "Inference Metrics" → "Performance & metrics"
  - "SLOT ACTIVITY" → "Active sessions"
- Where WebSocket fields map to visible UI labels, align descriptions.

D-021: remote-agent.md
- "Session" (UI concept) → "Model profile/model"
- "Inference only" → "Basic" in capability-related prose

D-022: security.md
- "Session" (UI sense) → "Model profile"
- "System Tray" → "Menu bar"

D-023: spawn-wizard.md → setup-wizard.md
- Rename file:
  - spawn-wizard.md → setup-wizard.md
- Inside:
  - "Spawn Wizard" → "Setup wizard"
  - "Spawn Server" → "Start server"
  - "Connect to Endpoint" → "Connect to a running model"
  - "Session" (UI concept) → "Model profile"
- Align step descriptions with new labels:
  - "How you’ll use it"
  - "Choose model"
  - "Hardware & memory"
  - "Settings"
  - "Review settings"
  - "Start server"
- Elevate:
  - Hardware-aware configuration
  - VRAM-safe recommendations
  - Preset-integrated guidance

D-024: ui-design-patterns.md
- Update:
  - "Compact Mode" → "Focus mode"
  - "Spawn Server" → "Start server"
  - "Spawn Wizard" → "Setup wizard"
  - "Connect to Endpoint" → "Connect to a running model"
  - "New Configuration" → "New model"
- Ensure references to:
  - "New conversation"
  - "Performance & metrics"
  - "Active sessions"

D-025: vram-estimator.md
- Ensure:
  - It is explicitly referenced in README as a key feature.
  - Intro paragraph explains in plain language:
    - "Estimates whether a model and context length will fit in your
       system’s memory before you start."
- Use "memory" language more than "VRAM" in headings; keep VRAM as
  technical detail.

D-026: windows-sensor-bridge-implementation.md
- No user-facing terminology changes required.

========================================
11.5 New / updated content recommendations
========================================

D-027: README “Getting started” paragraph
Add to README.md a concise getting-started section:
- Explain:
  - On first run: click "Start server" → "Setup wizard"
  - Wizard detects hardware, suggests settings.
  - Choose model → review → start → open a conversation.
- Use plain language aligned with polish.

D-028: Create quick-start guide
- Create: docs/reference/quick-start.md
- Outline:
  - First run flow (3–5 minutes):
    - Open app → Start server → Setup wizard
    - Choose model (local or via HuggingFace)
    - Review recommended settings (VRAM estimator + presets)
    - Launch and open a new conversation
  - Use new terminology only.

D-029: Consistency check
- After all updates:
  - Scan:
    - README.md
    - dashboard.md
    - setup-wizard.md
    - ui-design-patterns.md
    - capabilities.md
    - chat.md
  - Confirm:
    - No "Spawn Wizard" / "Spawn Server" in public prose
    - No "Connect to Endpoint"
    - No "ctx" in prose
    - "Inference Metrics" → "Performance & metrics"
    - "SLOT ACTIVITY" → "Active sessions"
    - "Compact Mode" → "Focus mode"
  - Fix any stragglers.

========================================
12. SLEEP HANDLING & LOW-POWER MODE
========================================

IMPORTANT:
- Canonical design: docs/plans/20260612-sleep_handling.md
- Do NOT delete that doc yet. Treat it as authoritative for sleep handling.
- This section is an implementation-ready spec distilled from that design.
  If there is ambiguity, prefer the reasoning in 20260612-sleep_handling.md.

Purpose:
- Integrate a sleep / low-power model so that:
  - llama-server always stays fully running.
  - llama-monitor backend and UI reduce or stop non-essential work
    (telemetry, GPU reads, WebSocket broadcasts, animations, etc.)
    when the user is not actively looking.
- Provide simple, premium manual controls and safe auto-sleep behavior.
- Ensure a coherent "sleep → wake → resume" experience when the user
  closes and reopens their browser.

Implementation constraints:
- Never kill, restart, or detach llama-server.
- Never shut down the backend HTTP listener or the tokio runtime.
- Keep:
  - Basic health awareness
  - Crash/OOM detection
  - Chat DB operations
  - Session autosave
  - External client access

========================================
12.0 Agent workflow and approach
========================================

This is guidance for an AI agent implementing this sleep-handling work.
Follow this workflow to avoid scattered changes and hard-to-track issues.

1) Orientation (before writing code)

- Read:
  - docs/plans/20260612-sleep_handling.md (full)
  - This section (12.*) of 20260612-pr215_second_polishpass.md
- In the codebase, locate and scan:
  - AppState definition (src/main.rs)
  - GPU poller loop (src/main.rs)
  - System metrics poller loop (src/main.rs)
  - Llama metrics poller (src/llama/poller.rs)
  - Remote agent poller (src/agent.rs)
  - WebSocket handler and broadcast logic (src/web/ws.rs)
  - Existing client-visibility handling in backend and frontend
  - Existing “Battery Saver” / power-saver logic in frontend JS
  - Session management (active_session_id, restore logic)
- Build a short mental map:
  - Where intervals are configured
  - Where visibility is tracked
  - Where restore/attach logic currently lives

2) Phase 1: Backend core (T-042..T-051)

Goal: wire sleep_mode into all hot loops and expose safe control endpoints.

Order:
- T-042: Add sleep_mode channel to AppState.
- T-043: Define SleepModeConfig and sleep_mode_config in AppState.
- T-044: Integrate loading from ui-settings.json with safe defaults.
- T-045: GPU poller sleep guard (simple, self-contained).
- T-046: System metrics poller sleep guard.
- T-047: Llama metrics poller sleep guard, coordinated via llama_poll_notify.
- T-048: Remote agent poller sleep guard.
- T-051: Implement wake-on-activity hooks in WS and relevant routes.
- T-050: Add /api/sleep-mode GET/POST endpoints.

Rules:
- Make each guard local and readable; do not introduce global macros.
- Always preserve the current behavior when sleep_mode is false.
- Do not break existing tests; run cargo test and UI tests after Phase 1.

Verification:
- Confirm:
  - App compiles.
  - Existing behavior unchanged when sleep_mode = false.
  - GET /api/sleep-mode returns expected JSON.
  - POST /api/sleep-mode/toggle and /set correctly toggle mode.

3) Phase 2: Visibility + WebSocket (T-049, T-053..T-056)

Goal: integrate client visibility modes and broadcast throttling.

Order:
- T-053: Extend client-visibility frontend logic (active/idle/sleep).
- T-054: Backend: track per-connection mode, adjust intervals.
- T-049: Broadcast loop respects sleep_mode and client modes.
- T-055: Frontend responds to sleep_mode (freeze telemetry, animations).
- T-056: Ensure active chat generation forces active mode.

Rules:
- Client-visibility is backward-compatible: ignore mode field if missing.
- Never drop the WebSocket connection; only slow/limit payloads.

Verification:
- Use devtools/network to confirm:
  - Slower broadcasts when all clients in sleep mode.
  - No crash on reload; no stuck telemetry.
  - Sleep mode visible in logs/debug output.

4) Phase 3: UI controls (T-057, T-058)

Goal: give users clear manual control.

Order:
- T-057: Extend nav-cockpit pill (Active / Low Power / Auto).
- T-058: Add “Sleep Mode” controls to Settings → Performance.

Rules:
- Must be obvious but not intimidating.
- Avoid exposing internal intervals in primary UI.

Verification:
- Manually cycle modes; confirm:
  - GET /api/sleep-mode reflects state.
  - UI labels, colors, tooltips are clear.
  - Settings persist across reload.

5) Phase 4: Reopen-aware restore (T-060..T-062)

Goal: ensure user can “sleep, close tab, reopen” and get back in context.

Order:
- T-060: Implement /api/sessions/restore-hint.
- T-061: Frontend: use restore-hint on load to resume or suggest.
- T-062: Ensure behavior is consistent with auth on/off.

Rules:
- No breaking changes to existing session/attach behavior.
- Prefer incremental, safe fallback: if unsure, do nothing special.

Verification:
- Test flows:
  - Normal reopen while server running
  - Reopen after sleep mode
  - With auth enabled and disabled

6) Phase 5: Auto-sleep + polish (T-052, T-059, tuning)

Goal: implement automatic sleep and optional tray toggle.

Order:
- T-052: Auto-sleep background task.
- T-059 (optional): Tray popover sleep toggle.
- Tune intervals and behaviors based on practical testing.

Rules:
- Be conservative. Default intervals should protect:
  - Crash detection
  - External clients
  - Basic health awareness

Verification:
- Simulate idle:
  - Leave tab hidden, no activity.
  - Confirm sleep_mode becomes true after configured interval.
- Return to tab:
  - Confirm auto-wake, telemetry resumes.

========================================
12.1 Backend: sleep_mode flag and config
========================================

T-042: Add sleep_mode channel to AppState
- In AppState:
  - pub sleep_mode: Arc<tokio::sync::watch::Sender<bool>>
  - true: sleep active; false: normal
- Initialize:
  - sleep_mode = false by default.

T-043: Add sleep_mode_config
- In AppState:
  - pub sleep_mode_config: Arc<Mutex<SleepModeConfig>>
- Define:
  struct SleepModeConfig {
      pub auto_sleep_when_all_hidden: bool,
      pub auto_sleep_idle_secs: Option<u64>,
      pub sleep_gpu_interval_secs: u64,
      pub sleep_sys_interval_secs: u64,
      pub sleep_llama_interval_secs: u64,
      pub sleep_ws_interval_ms: u64,
  }
- Recommended defaults:
  - auto_sleep_when_all_hidden: true
  - auto_sleep_idle_secs: Some(600)
  - sleep_gpu_interval_secs: 15
  - sleep_sys_interval_secs: 15
  - sleep_llama_interval_secs: 15
  - sleep_ws_interval_ms: 10_000

T-044: Load from config (ui-settings.json)
- On startup:
  - Read ui-settings.json
  - If a sleep_mode section exists, merge it into SleepModeConfig.
  - If missing, use defaults.

T-045: GPU poller sleep guard
- In GPU poller loop (src/main.rs):
  - At loop top:
    - let asleep = state.sleep_mode.borrow_and_update();
    - if asleep:
        - thread::sleep(Duration::from_secs(cfg.sleep_gpu_interval_secs));
        - continue;
  - Otherwise proceed as normal.

T-046: System metrics poller sleep guard
- Similar in system metrics loop:
  - If asleep:
    - thread::sleep(Duration::from_secs(cfg.sleep_sys_interval_secs));
    - continue;
  - Else proceed as normal.

T-047: Llama metrics poller sleep guard
- In llama poller loop (src/llama/poller.rs):
  - If asleep:
    - Use longer interval or await llama_poll_notify.
  - If awake:
    - Use normal interval.
- On sleep_mode change:
  - Call llama_poll_notify.notify_waiters()
    so poller re-evaluates.

T-048: Remote agent poller sleep guard
- Same pattern:
  - If asleep: throttle/pause via longer interval or await notify.
  - If awake: normal.

T-049: WebSocket broadcast behavior
- In ws.rs broadcast loop:
  - Before sleep(interval_ms):
    - let asleep = state.sleep_mode.borrow_and_update();
    - If asleep:
      - interval_ms = max(interval_ms, cfg.sleep_ws_interval_ms)
  - When asleep:
    - Optionally send a minimal heartbeat + session status + critical flags
      instead of full telemetry.

T-050: Backend endpoints for manual control
- Add:
  - GET /api/sleep-mode
    - Returns:
      - { "enabled": bool, "config_summary": { ... } }
  - POST /api/sleep-mode/toggle
    - Toggles sleep_mode on/off.
    - Requires api-token.
  - POST /api/sleep-mode/set
    - Body: { "enabled": true | false }
    - Requires api-token.
- Ensure:
  - Rate-limit-friendly.
  - No auth bypass.

T-051: Wake-on-activity
- Implement wake rules:
  - sleep_mode = false when:
    - New WebSocket connection from browser.
    - Any API call implying “user wants full telemetry” (e.g., /api/gpu, /api/metrics).
    - Chat send via llama-monitor.
    - Frontend sends explicit “wake” message.
- In WebSocket handler:
  - On open: if connections > 0, disable or re-evaluate sleep.
  - On close: if no connections, maybe re-enable based on config.

T-052: Auto-sleep background task
- Add a background task:
  - Interval: 30–60s.
  - Tracks:
    - Number of WebSocket connections.
    - Last UI-relevant activity time.
  - If:
    - sleep_mode is false, AND
    - no WebSocket connections (or all report hidden), AND
    - last_activity_elapsed > auto_sleep_idle_secs
  - Then:
    - sleep_mode = true.
- This ensures:
  - Closing tab and walking away → eventually goes to sleep.

========================================
12.2 Frontend: visibility, sleep, and behavior
========================================

T-053: Extend client-visibility message
- Currently:
  - { type: 'client-visibility', visible: bool }
- Extend to:
  - {
      type: 'client-visibility',
      visible: bool,
      mode: 'active' | 'idle' | 'sleep'
    }
- In dashboard-ws.js (or power-saver module):
  - Track:
    - isTabVisible (visibilitychange)
    - lastInteractionTime (mouse/keyboard/touch)
  - Decide mode:
    - active: visible + recent activity
    - idle: visible + no activity for X seconds
    - sleep: hidden or explicit sleep mode
  - Send client-visibility with mode.
  - Apply body classes:
    - .power-saver when sleep
    - .power-idle when idle

T-054: Backend uses client modes
- In ws.rs:
  - Maintain per-connection mode.
  - For broadcast intervals:
    - If any client active → normal interval.
    - If any client idle → intermediate interval.
    - If all sleep or no connections → sleep interval.

T-055: Respect sleep_mode in UI
- Backend broadcasts sleep_mode in WS payload.
- Frontend behavior:
  - If sleep_mode is true:
    - Freeze telemetry:
      - Do not update GPU/telemetry panels.
      - Do not render sparkline points.
      - Reduce or pause animations.
  - If sleep_mode is false:
    - Restore normal updates.

T-056: Chat streaming prevents sleep
- If a generation is in progress
  (e.g., generation_tokens_per_sec > 0):
  - Frontend treats as “active”:
    - mode = 'active'
  - Backend:
    - Do not auto-sleep during active streaming.

========================================
12.3 UI/UX: manual controls and settings
========================================

T-057: Nav cockpit pill for sleep mode
- Extend/enhance the existing nav-cockpit state pill:
  - Support modes:
    - "Active"
    - "Low Power"
    - "Auto"
- Behavior:
  - Click cycles:
    - Active → Low Power → Auto → Active
- Tooltips:
  - Active: "Full telemetry and UI updates."
  - Low Power: "Telemetry minimized; llama-server stays running."
  - Auto: "Automatically enables Low Power when hidden or idle."
- Styling:
  - Active: neutral/green accent.
  - Low Power: amber/yellow accent.
  - Auto: blue accent.
- Accessibility:
  - aria-label: "Connection mode: Low Power. Telemetry minimized."
- Wire to:
  - GET /api/sleep-mode
  - POST /api/sleep-mode/set

T-058: Settings → Performance: sleep controls
- In Settings → Performance (where Dashboard Refresh Rate lives), add:
  - "Sleep Mode" section:
    - Toggle: "Enable Auto Sleep"
    - Option: "Auto-sleep when tab is hidden" (default on)
    - Option: "Auto-sleep after no activity for:"
      - [3 / 5 / 10 / 30 minutes]
  - Short explanation:
    - "Sleep Mode minimizes telemetry and UI activity while keeping
       your llama-server running."
- Persist to ui-settings.json via existing settings endpoints.

T-059: Optional: tray popover toggle
- For desktop workflows, add one row to tray popover (compact.html):
  - "Low Power Mode: [On/Off]"
- Calls POST /api/sleep-mode/toggle.
- Optional for later polish.

========================================
12.4 Browser close / reopen: restore-hint
========================================

T-060: Add /api/sessions/restore-hint
- Response:
  {
    "server_running": bool,
    "has_active_session": bool,
    "active_session_id": string | null,
    "active_session_status": string | null,
    "has_chat_tabs": bool,
    "suggested_action": "resume_active" | "suggest_recent_attach" | "none"
  }
- Logic:
  - resume_active:
    - server_running + active_session + status == "Running"
  - suggest_recent_attach:
    - server_running but no active session
  - none:
    - No obvious restore path.

T-061: Frontend reopen-aware behavior
- On page load (after auth):
  - Call /api/sessions/restore-hint
  - If "resume_active":
    - switchView('monitor')
    - Restore chat tabs and previous position if possible.
  - If "suggest_recent_attach":
    - Show setup view with a banner:
      - "We detected a running server from your recent session. Resume?"
      - On click: attach and restore chats.
  - If "none":
    - Default setup view behavior.
- This ensures:
  - Returning user sees:
    - Their running server
    - Their chats
    - A clear path back into context.

T-062: Auth compatibility
- With auth enabled:
  - Prompt first, then run restore-hint flow.
- With auth disabled:
  - Run restore-hint flow directly.
- Sleep mode never changes auth behavior.

========================================
12.5 Implementation phases (summary)
========================================

Implement in this exact order; each phase must compile, pass tests, and
preserve existing behavior before moving on.

P-1: Backend sleep_mode core
- T-042, T-043, T-044, T-045, T-046, T-047, T-048, T-050, T-051

P-2: WebSocket + visibility integration
- T-049, T-053, T-054, T-055, T-056

P-3: UI controls
- T-057, T-058

P-4: Reopen-aware restore
- T-060, T-061, T-062

P-5: Auto-sleep + polish
- T-052, T-059 and tuning

========================================
END OF SLEEP HANDLING SECTION
========================================

End of spec.
