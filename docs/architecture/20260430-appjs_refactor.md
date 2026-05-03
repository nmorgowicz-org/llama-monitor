# `static/app.js` Refactor Plan

Date: 2026-04-30

## Purpose

This document defines a safe, phased plan to break [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js) apart without changing user-visible behavior. It is written so an AI agent can pick up the work with no previous context and execute the refactor incrementally.

The end goal is to replace the current monolithic browser script with a structured frontend architecture that is easier to debug, safer to modify, and less likely to regress unrelated features.

This plan does not implement the refactor. It specifies the architecture, sequencing, safety rules, and verification steps required to complete it.

## Executive Summary

`static/app.js` is now `9059` lines and contains roughly `317` top-level functions. It is not only large; it currently acts as:

- The entire frontend entrypoint
- A global state container
- A DOM utility library
- A transport layer for `fetch` and WebSocket activity
- A renderer for dashboard, chat, sessions, presets, remote-agent UI, setup view, LHM UI, and update UI
- The bootstrapping system for all startup listeners and keyboard shortcuts

The file can be split safely, but it should not be done as a single-step rewrite.

The safest approach is:

1. Keep behavior identical during the refactor.
2. Introduce a thin compatibility layer for global handlers.
3. Extract shared utilities and state first.
4. Extract features one at a time.
5. Keep one bootstrap entrypoint at all times.
6. Validate each phase with existing UI flows before proceeding.

## Required Context

### Current Static Asset Model

The backend embeds frontend assets directly in Rust:

- [`src/web/static_assets.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/static_assets.rs:1)
- [`src/web/mod.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/mod.rs:123)

Important implications:

- The app does not currently use a frontend build step.
- `index.html`, CSS files, `app.js`, `lhm.js`, `sw.js`, and `compact.html` are compiled into the Rust binary via `include_str!`.
- Splitting `app.js` requires adding more static asset constants and routes, or otherwise changing the static-asset serving strategy.
- A refactor that assumes a bundler already exists will be wrong.

### Current Frontend Entry

[`static/index.html`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:2157) currently loads one browser script:

```html
<script src="/app.js?v=20240428"></script>
```

Important implications:

- The browser currently receives a single classic script, not an ES-module graph.
- A module-based split is possible, but the migration must account for global handlers and Rust asset serving.

### Inline Handler Constraint

[`static/index.html`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:1) uses extensive inline event handlers such as:

- `onclick="doAttach()"`
- `onclick="openPresetModal('new')"`
- `onclick="toggleVizSwitcher('gpu')"`
- `onclick="sendChat()"`

There are about `157` inline `onclick` attributes in the file.

Important implications:

- Many functions must remain globally reachable during the transition.
- A direct move to private module-scoped functions will break the page.
- The first compatibility target is not "no globals"; it is "globals only through an explicit facade."

### Existing Dirty Worktree

At the time this plan was written, the repository had local modifications in:

- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js)
- [`tests/ui/chat-ui.spec.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/tests/ui/chat-ui.spec.js)

Any refactor agent must re-check the worktree before editing and must not overwrite unrelated user changes.

## What `app.js` Currently Contains

The file already has identifiable feature groupings. That is the main reason this refactor is feasible.

### 1. Shared UI Utilities and Metric Helpers

Early sections define helpers such as:

- `switchTab`
- `toggleSidebarCollapse`
- `animateNumber`
- `formatMetricNumber`
- `formatMetricAge`
- `escapeHtml`
- sparkline rendering
- request-activity rendering
- decoding-config rendering

Representative location:

- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:1)

### 2. Shared Global State

The file uses both `window.*` and file-scope mutable state:

- `window.prevValues`
- `window.metricSeries`
- `window.requestActivity`
- `window.recentTasks`
- `window.liveOutputTracker`
- `presets`
- `sessions`
- `activeSessionId`
- `activeSessionPort`
- `remoteAgentInProgress`
- `lastServerState`
- `lastLlamaMetrics`
- `lastSystemMetrics`
- `lastGpuMetrics`

Representative location:

- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:521)

### 3. Settings, Config, Models, and Remote-Agent Management

This area includes:

- settings collection and persistence
- config modal
- GPU environment loading
- remote-agent setup modal
- guided SSH flow
- remote-agent install/start/stop/update/remove
- remote-agent menu
- user preferences
- models modal

Representative ranges:

- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:594)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:1035)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:2550)

### 4. File Browser, Toasts, Presets, Sessions, Attach/Start/Stop Flows

This area includes:

- file browser modal
- toast helpers
- preset modal and CRUD
- config extraction
- server start/attach/detach/stop
- session list and session modal

Representative ranges:

- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:3351)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:3702)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:4349)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:4582)

### 5. WebSocket and Dashboard Rendering

This is the most sensitive section because it updates large parts of the UI from a single payload:

- WebSocket creation
- session-state refresh polling
- GPU and system visualization history
- GPU card rendering
- system card rendering
- inference card rendering
- badges, logs, endpoint status, agent status, view switching

Representative ranges:

- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:4876)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:5332)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:5568)

### 6. Chat System

This is effectively a full sub-application embedded in the same file:

- tab state
- chat persistence
- compaction
- markdown rendering
- chat send/streaming
- message rendering
- template manager
- explicit-mode policy
- model params
- style panel
- keyboard shortcuts

Representative ranges:

- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:6135)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:6177)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:7131)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:7477)

### 7. LHM, Setup View, and App Update UI

Later sections include:

- Windows LHM notification/install flow
- keyboard shortcut modal
- setup/monitor view transitions
- quick stats / setup screen
- chat style / chat font preferences
- app version and self-update flow

Representative ranges:

- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:7997)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:8549)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:8837)

## Problems To Solve

### A. Global Handler Coupling

Inline handlers require global function names. This prevents safe encapsulation unless an explicit compatibility layer is introduced.

### B. Global Mutable State

The current file does not have a single source of truth. State is scattered across:

- `window.*`
- top-level `let`s
- DOM state
- `localStorage`
- backend settings
- WebSocket payload snapshots

### C. Scattered Bootstrap

The file currently installs many startup listeners in multiple places. There are about `15` `DOMContentLoaded` registrations plus additional global listeners.

This makes startup order fragile and hard to reason about.

### D. Feature Interleaving

The same section often mixes:

- API calls
- state mutation
- DOM lookup
- DOM updates
- timers
- keyboard handlers
- modal open/close behavior

### E. Duplicate Helpers

`escapeHtml` currently appears three times:

- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:86)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:1930)
- [`static/app.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/app.js:2534)

This suggests real drift risk during future edits.

### F. Asset Routing Scalability

The current Rust static-asset strategy is acceptable for a few files, but it becomes increasingly tedious as JS is split into many modules. The refactor must either:

- continue explicit per-file embedding and routing, or
- improve the static asset strategy as part of the refactor

The recommended initial plan keeps the current strategy and only expands it. Avoid changing both frontend architecture and asset-serving architecture at the same time unless necessary.

## Refactor Goals

### Primary Goals

- Split `static/app.js` into coherent, smaller files
- Preserve all existing functionality
- Preserve all routes and APIs
- Preserve current visual behavior
- Preserve current data persistence behavior
- Preserve current keyboard shortcuts and startup flows
- Reduce hidden coupling by centralizing state and bootstrapping

### Secondary Goals

- Make future debugging easier
- Make feature ownership clearer
- Make it possible to test modules individually later
- Make follow-on refactors possible, including reducing inline handlers

### Non-Goals

- Rewriting the UI
- Changing backend APIs
- Removing existing features
- Introducing a mandatory frontend build system
- Redesigning CSS
- Replacing all inline event handlers in the same phase as the file split

## Recommended Target Architecture

## Decision

Use native browser ES modules plus a temporary `window` compatibility facade.

This is the best long-term target because:

- No bundler is required
- Modern code boundaries become available immediately
- Modules can be introduced incrementally
- Inline handlers can keep working through explicit `window` exports

Do not start with a pure-global "many classic scripts" design unless the module route proves unworkable. That approach reduces file size but preserves too much accidental coupling.

## Proposed Directory Layout

Create a new `static/js/` tree and shrink `static/app.js` to either:

- a small compatibility loader, or
- remove it entirely after `index.html` switches to `type="module"`

Recommended layout:

```text
static/
  js/
    bootstrap.js
    compat/
      globals.js
    core/
      api.js
      app-state.js
      dom.js
      events.js
      format.js
      storage.js
      timers.js
      constants.js
    features/
      nav.js
      settings.js
      config-modal.js
      gpu-env.js
      remote-agent.js
      remote-agent-setup.js
      file-browser.js
      toast.js
      presets.js
      sessions.js
      attach-detach.js
      dashboard-ws.js
      dashboard-inference.js
      dashboard-hardware.js
      dashboard-badges.js
      chat-state.js
      chat-render.js
      chat-transport.js
      chat-templates.js
      chat-params.js
      lhm.js
      setup-view.js
      updates.js
      shortcuts.js
```

This exact naming can be adjusted, but the separation of concerns should remain.

## Architectural Rules For The Refactor

Any implementation agent should follow these rules.

### Rule 1: Keep One Bootstrap Entry

There must be exactly one top-level browser entrypoint responsible for startup ordering:

- register startup listeners
- initialize state
- wire modules
- expose compatibility globals

Recommended file:

- `static/js/bootstrap.js`

### Rule 2: No Feature Module Reads Random Globals

Feature modules should not reach into arbitrary top-level variables. Shared data should come from a central app-state module.

Allowed:

- `import { appState } from './core/app-state.js'`

Disallowed:

- ad hoc access to top-level mutable variables spread across files

### Rule 3: Keep DOM Selectors Local To Features

DOM references for chat should live in chat modules.
DOM references for presets should live in preset modules.
DOM references for remote-agent setup should live in remote-agent modules.

Shared DOM helpers may exist, but not giant global selector registries for the entire app.

### Rule 4: Transport And Rendering Must Be Separated

At minimum:

- API request construction belongs in API/transport modules
- state mutation belongs in state modules
- DOM updates belong in renderer/feature modules

This is especially important for:

- WebSocket handling
- chat streaming
- remote-agent actions

### Rule 5: Inline Handler Compatibility Must Be Explicit

During migration, functions used by inline handlers must be attached to `window` through one dedicated compatibility file.

Recommended pattern:

- modules export real functions
- `compat/globals.js` imports those functions and assigns only the required public surface to `window`

This prevents accidental global leakage.

### Rule 6: One Copy Of Each Shared Helper

There must be one authoritative implementation for helpers such as:

- `escapeHtml`
- metric formatting
- version comparison
- localStorage key definitions
- toast creation

### Rule 7: Preserve Behavioral Ordering

Some flows depend on current ordering:

- presets load before setup preset sync
- settings apply before some UI state renders
- chat tabs initialize before some chat controls update
- WebSocket updates drive multiple UI regions

When moving code, preserve ordering unless intentionally improved and verified.

## Target State Breakdown

### Core Layer

#### `core/app-state.js`

Purpose:

- own the mutable shared state for the app

Suggested contents:

- app-level UI state
- dashboard state snapshots
- session state
- chat global state
- remote-agent transient state
- visualization preferences
- timers or timer IDs only if truly shared

Suggested top-level shape:

```js
export const appState = {
  ui: {},
  dashboard: {},
  sessions: {},
  presets: {},
  remoteAgent: {},
  chat: {},
  updates: {},
  lhm: {},
};
```

Do not preserve every current global exactly as-is if it can be grouped safely.

#### `core/api.js`

Purpose:

- wrap `fetch`
- standardize JSON request/response behavior
- centralize content-type defaults
- centralize basic error shaping

Do not change endpoint names.

#### `core/format.js`

Purpose:

- own `escapeHtml`
- own number/date/version formatting helpers

#### `core/storage.js`

Purpose:

- centralize `localStorage` keys and reads/writes

This should cover keys such as:

- `sidebarCollapsed`
- `llama-monitor-last-endpoint`
- `llama-monitor-last-session`
- `llama-monitor-chat-style`
- `llama-monitor-enter-to-send`
- `llama-monitor-chat-font`
- `llama-monitor-preferences`
- `update-dismissed`
- template storage keys
- viz preference storage keys

#### `core/events.js`

Purpose:

- centralize app startup and shared event registration
- avoid scattered `DOMContentLoaded` listeners

### Feature Layers

#### Dashboard

Split into:

- WebSocket transport and payload dispatch
- inference-card rendering
- hardware-card rendering
- badges/logs/status strip rendering

Suggested modules:

- `features/dashboard-ws.js`
- `features/dashboard-inference.js`
- `features/dashboard-hardware.js`
- `features/dashboard-badges.js`

#### Remote Agent

Current remote-agent logic is large enough to justify separate modules for:

- config-panel remote-agent controls
- guided SSH setup
- remote-agent setup modal
- shared remote-agent helpers

Suggested modules:

- `features/remote-agent.js`
- `features/remote-agent-setup.js`

#### Chat

The chat feature should not remain a monolith inside the new architecture.

Suggested split:

- `features/chat-state.js`
- `features/chat-render.js`
- `features/chat-transport.js`
- `features/chat-templates.js`
- `features/chat-params.js`

#### Presets / Sessions / File Browser

These can be separated cleanly because they are already mostly domain-specific:

- `features/presets.js`
- `features/sessions.js`
- `features/file-browser.js`

#### Setup View / Updates / LHM

These are small enough to isolate and remove from the critical dashboard/chat path:

- `features/setup-view.js`
- `features/updates.js`
- `features/lhm.js`
- `features/shortcuts.js`

## Phased Implementation Plan

This refactor should be done in phases. Do not combine phases unless the prior phase is already validated.

## Phase 0: Baseline, Inventory, and Safety Rails

### Goal

Create the baseline needed to refactor safely without changing behavior.

### Tasks

1. Re-check worktree status and note pre-existing modifications.
2. Create a temporary inventory of:
   - all global functions referenced from `index.html`
   - all `DOMContentLoaded` listeners
   - all `keydown`, `resize`, `beforeunload`, `click`, `setInterval`, and long-running timer registrations
   - all `window.*` state fields
3. Create a mapping from feature area to DOM regions:
   - server/dashboard
   - chat
   - logs
   - settings
   - remote-agent
   - sessions
   - presets
   - setup view
   - release notes
4. Record every `fetch('/api/...')` endpoint used by the frontend.
5. Capture a manual regression checklist before code moves begin.

### Output

- a verified function/handler inventory
- a verified endpoint inventory
- a manual QA checklist

### Exit Criteria

- No code behavior changes yet
- The agent knows exactly which globals must be preserved in the first migration steps

## Phase 1: Introduce Module Bootstrapping Without Feature Changes

### Goal

Switch from one huge classic script to a module bootstrap while preserving current function names and behavior. Begin scaffolding `core/app-state.js` alongside bootstrap so Phase 3+ modules have a home to write to immediately.

### Tasks

1. Add `static/js/bootstrap.js`.
2. Add `static/js/compat/globals.js`.
3. Add `static/js/core/format.js`.
4. Add `static/js/core/app-state.js` — scaffold the shape (see Phase 2), migrate the most obvious shared state (dashboard metric snapshots, session state, chat global state). This avoids refactoring state ownership twice.
5. Move only the safest shared helpers first:
   - `escapeHtml`
   - formatting helpers
   - version compare helper
   - minor stateless DOM helpers if truly generic
6. Keep feature logic where it is for now if necessary, but import shared helpers from modules.
7. Export current inline-handler functions through `compat/globals.js` to `window`.
8. Update [`static/index.html`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:2157) to load the new module entrypoint.
9. Update Rust static asset embedding and routes for new JS assets.

### Notes

- This phase is about proving the new loading model, not reducing most of the complexity yet.
- Do not simultaneously rewrite dashboard, chat, and remote-agent logic.
- The compatibility layer should be explicit and small enough to audit.
- Starting `app-state.js` here means Phase 2 becomes a refinement pass (moving remaining state, cleaning up accessors) rather than a big-bang migration.

### Exit Criteria

- The app still loads successfully
- All inline handlers still function
- Shared helper duplication begins to disappear
- There is one clear startup path
- `app-state.js` exists with the core shape and the most obvious shared state migrated

## Phase 2: Centralize Shared State (Refinement Pass)

### Goal

Finish moving remaining ad hoc globals into `core/app-state.js` (scaffolded in Phase 1) and clean up accessors. This is a refinement pass, not a big-bang migration.

### Tasks

1. Move any shared mutable state not yet migrated in Phase 1 into structured sub-objects.
2. Replace direct `window.*` mutations where possible.
3. Keep compatibility accessors only where needed by legacy code during transition.
4. Define clear ownership for:
   - dashboard metric histories
   - session state
   - remote-agent transient state
   - chat tab global state
   - update/LHM/setup view state

### Important Caution

Do not force all state into one flat object. Group by feature domain.

### Exit Criteria

- New modules import shared state from one place
- No functionality regresses
- `window` is no longer the primary state store

## Phase 3: Extract Dashboard Transport and Rendering

### Goal

Isolate the real-time dashboard logic because it is one of the most coupled and highest-risk areas.

### Suggested Split

- `dashboard-ws.js`
  - WebSocket creation
  - onmessage dispatch
  - reconnect/close/error behavior if introduced later
- `dashboard-inference.js`
  - throughput, generation, context, slot activity, decoding config
- `dashboard-hardware.js`
  - GPU/system card rendering
  - history buffers
  - viz preference application
- `dashboard-badges.js`
  - logs
  - header badges
  - endpoint strip state
  - attach/detach visual state

### Tasks

1. Move rendering helpers used only by the dashboard into dashboard modules.
2. Move history ring buffers and metric-series handling into dashboard-owned state.
3. Refactor `ws.onmessage` into:
   - payload normalization
   - state update
   - render dispatch
4. Preserve current payload field handling exactly.
5. Preserve empty-state behavior for:
   - remote endpoint
   - sensor unavailable
   - backend unavailable

### Risks

- The WebSocket handler currently drives multiple unrelated UI areas.
- The order of state updates and renders matters.
- Some visual fallbacks are inferred, not explicit.

### Verification Strategy

Before extraction, capture the exact sequence of DOM mutations per WebSocket message by logging:

- which elements are read from (`textContent`, `innerHTML`, `classList`)
- which elements are written to
- the order of reads vs writes

Use a lightweight `MutationObserver` or console-logged wrapper around the current `ws.onmessage` handler. After extraction, replay the same messages and diff the mutation logs to confirm identical behavior.

### Exit Criteria

- Dashboard works identically after extraction
- WebSocket logic is no longer mixed with chat, presets, sessions, and modal code
- DOM mutation logs match before/after extraction

## Phase 4: Extract Presets, Sessions, Attach/Detach, and File Browser

### Goal

Remove the operational server/preset/session flows from the remaining monolith.

### Suggested Split

- `presets.js`
- `sessions.js`
- `attach-detach.js`
- `file-browser.js`
- `toast.js`

### Tasks

1. Move preset CRUD and modal handling into `presets.js`.
2. Move session list/modal/activate/create/delete into `sessions.js`.
3. Move attach/start/stop/detach flows into `attach-detach.js`.
4. Move file-browser modal logic into `file-browser.js`.
5. Move toast helpers into a dedicated toast module and update all callers.

### Important Caution

Keep these interactions intact:

- loading presets updates setup preset selector
- switching sessions reloads presets
- attach/detach affects view transitions and badge state
- settings persistence still tracks endpoint changes

### Exit Criteria

- Preset/session flows no longer depend on one giant file context
- Toast and file-browser helpers are shared modules

## Phase 5: Extract Remote-Agent Feature Properly

### Goal

Separate remote-agent logic into maintainable modules without changing the SSH/agent management behavior.

### Why This Gets Its Own Phase

Remote-agent code currently mixes:

- settings state
- SSH data assembly
- host-key trust flow
- detection/install/start/update/stop/remove flows
- modal state
- config-panel state
- endpoint-derived defaults
- toast/status/progress/timeline UI

It is one of the most stateful parts of the file.

### Suggested Split

- `remote-agent.js`
  - config-panel controls
  - generic remote-agent actions
  - status/progress/timeline rendering
- `remote-agent-setup.js`
  - setup modal
  - guided SSH host-key flow
  - "finish setup" behavior

### Tasks

1. Centralize SSH payload creation.
2. Centralize remote-agent status rendering.
3. Deduplicate repeated release-check, detect, and action patterns.
4. Preserve opt-in semantics:
   - typing a target must not auto-contact the machine
   - SSH actions remain explicit
5. Preserve firewall-blocked handling and setup-button behavior.

### Exit Criteria

- Remote-agent logic is isolated
- Inline handlers still work through the facade
- No SSH behavior changes unintentionally

## Phase 6a: Extract Chat State and Transport

### Goal

Move chat persistence, tab state, and streaming transport out of the monolith. This is the foundation for Phase 6b.

### Suggested Split

- `chat-state.js`
  - tab collection
  - active tab
  - busy flags
  - persistence scheduling
- `chat-transport.js`
  - `/api/chat`
  - streaming decode
  - abort controller
  - summarization requests

### Tasks

1. Move chat persistence into `chat-state.js`.
2. Keep save/load wire format backward compatible with the backend.
3. Move streaming transport (fetch, decode, abort) into `chat-transport.js`.
4. Preserve current compaction behavior and tombstone structure.
5. Ensure render code in the legacy shell can still read from `chat-state.js` and call `chat-transport.js`.

### Important Caution

Chat touches:

- localStorage
- backend persistence
- streaming transport
- markdown rendering
- syntax highlighting
- explicit-mode policy
- UI badges
- setup of multiple panels

Do not split it in a way that causes render/transport state desynchronization.

### Exit Criteria

- Chat state and transport are isolated modules
- Legacy render code still works against the new modules
- Streaming behavior remains stable
- Compaction and persistence remain intact

## Phase 6b: Extract Chat Rendering, Templates, and Params

Status Note: 2026-05-02

The remaining 6b follow-ups were closed on `feature/window-architecture-context-metrics`:

1. chat import is wired back into the chat header UI
2. explicit-policy reset/clear now rely on native settings-modal input bubbling instead of importing settings dirty-state logic into the chat slice

### Goal

Move chat rendering, template manager, and model params panel out of the monolith.

### Suggested Split

- `chat-render.js`
  - tabs
  - messages
  - compaction markers
  - unread indicators
  - markdown rendering hooks
- `chat-templates.js`
  - system prompt templates
  - explicit-mode policy
- `chat-params.js`
  - model parameter panel
  - system prompt panel
  - style/font/enter-to-send controls

### Tasks

1. Move rendering functions into `chat-render.js`.
2. Move template manager into `chat-templates.js`.
3. Move model params panel into `chat-params.js`.
4. Preserve message actions:
   - copy
   - regenerate
   - export/import
5. Preserve keyboard shortcuts and tab switching behavior.

### Exit Criteria

- Chat feature is no longer interleaved with dashboard or settings logic
- All chat rendering is in dedicated modules
- Streaming behavior remains stable
- Compaction and persistence remain intact

## Phase 7: Extract LHM, Setup View, Updates, and Global Shortcuts

Status Note: 2026-05-02

Phase 7 is now complete on `feature/window-architecture-context-metrics`.

The remaining setup-view and shortcuts global handshakes were removed:

1. `sessions.js` now imports `showConnectingState()` directly from `setup-view.js`
2. `setup-view.js` no longer republishes its helpers onto `window`
3. `shortcuts.js` no longer republishes shortcut modal helpers onto `window`

### Goal

Move the remaining peripheral systems out of the final legacy shell.

### Suggested Split

- `lhm.js`
- `setup-view.js`
- `updates.js`
- `shortcuts.js`
- `nav.js`

### Tasks

1. Move LHM notification/install/start/uninstall flow into `lhm.js`.
2. Move setup/monitor view transitions and quick-stats UI into `setup-view.js`.
3. Move app version, release notes, and self-update flow into `updates.js`.
4. Consolidate global keyboard listeners into `shortcuts.js`.

### Exit Criteria

- No significant business logic remains in the legacy shell
- Bootstrap only composes features

## Phase 8: Remove Legacy Shell and Minimize Global Facade

Status Note: 2026-05-02

Phase 8 is now complete on `feature/window-architecture-context-metrics`.

What closed it:

1. remaining feature-to-feature `window.*` bridges were replaced with direct imports
2. deferred file-browser access now goes through a shared lazy launcher module instead of bootstrap globals
3. settings/config/presets/sessions/attach-detach no longer coordinate through `window.*`
4. the remaining facade is reduced to the tiny compatibility file for formatting helpers plus normal browser globals

### Goal

Finish the migration by shrinking the compatibility surface.

### Tasks

1. Replace remaining large legacy sections with imports.
2. Reduce `window` exports to only functions still needed by inline handlers.
3. Optionally begin converting inline handlers in HTML to `addEventListener` wiring.
4. Once inline handlers are removed, remove the `window` facade entirely.

### Exit Criteria

- `static/app.js` is either removed or reduced to a trivial compatibility shim
- Bootstrapping is module-first
- Global namespace usage is minimal or eliminated

## Implementation Order Inside Each Phase

For every extraction phase, follow this micro-sequence:

1. Move stateless helpers first.
2. Move feature-local constants and storage keys.
3. Move feature state.
4. Move pure rendering functions.
5. Move transport/API calls.
6. Reconnect event handlers and globals.
7. Run verification before continuing.

This order reduces breakage because rendering and formatting are easier to validate than transport and event flow.

## Backend Changes Required By The Refactor

Any implementation agent must account for the Rust asset layer.

### Minimum Required Changes

1. Add new embedded JS assets to [`src/web/static_assets.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/static_assets.rs:1).
2. Add matching routes in [`src/web/mod.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/mod.rs:123).
3. Update [`static/index.html`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:2157) to load the new bootstrap.

### Recommended Strategy

For Phase 1, keep explicit Rust routes for the small number of new JS files (bootstrap, globals, format, app-state). This is verbose but safe and limits the surface area.

**After Phase 1 is validated**, write a small helper in `static_assets.rs` that auto-discovers files in `static/js/` using a macro or build script, rather than hand-adding embed + route per file. This prevents "asset routing drift" (new JS modules 404ing because a route was forgotten) as we add modules in later phases.

Do not combine the frontend refactor with a deeper static-asset re-architecture at the same time as a feature extraction phase.

## Smoke Test Recommendation

After each phase, run a lightweight Playwright smoke test to catch obvious breakage before manual QA. The test should hit the major flows in ~30 seconds:

1. **App boot** — navigate to `/`, verify no console errors
2. **Attach/detach** — attach to endpoint, verify dashboard renders, detach
3. **Chat send** — send a message, verify response appears
4. **Open settings** — click settings, verify modal opens
5. **Open presets** — click presets, verify modal opens

This is not a full test suite — just a "did we break the basics" gate. Existing Playwright tests (chat-ui.spec.js) already cover deeper flows.

## Validation and Regression Matrix

Every phase must be validated manually. Existing tests are not enough to guarantee UI parity for this refactor.

### Core App Load

- App loads without console-breaking errors
- Sidebar collapse still persists
- Setup view still appears on first load
- Fonts/CDN markdown/highlight integrations still work

### Dashboard / Monitor

- Attach to endpoint works
- Detach works
- Local start/stop works
- WebSocket live updates still refresh:
  - throughput
  - generation
  - context
  - slot activity
  - request activity
  - GPU metrics
  - system metrics
  - logs
  - badges
- Historic badge behavior remains correct
- Empty-state messages remain correct

### Presets

- Load presets on startup
- Create preset
- Edit preset
- Copy preset
- Delete preset
- Reset presets
- Preset selection persists

### Sessions

- Open session modal
- Create attach session
- Create spawn session
- Switch session
- Delete session
- Quick connect / quick start actions

### Settings / Config

- Settings modal opens/closes
- dirty indicator works
- save settings still persists
- config modal still saves server path and GPU env

### Remote Agent

- open remote-agent panel
- guided SSH flow still renders host-key scan/trust states
- detect works
- install works
- start works
- stop works
- restart works
- update works
- remove works
- firewall-blocked state still surfaces correctly

### Chat

- tabs load
- add/close/switch tabs
- rename tab
- send message
- streaming assistant response
- stop generation
- copy message
- regenerate
- export/import chat
- manual compact
- auto compact settings
- template manager
- explicit mode toggle
- model params panel
- enter-to-send toggle
- unread badge and scroll-to-bottom control

### LHM / Windows-Specific

- notification opens
- disable/enable still works
- install/start/uninstall still work
- UAC warning flow remains functional

### Updates

- app version renders
- update pill appears only when appropriate
- release notes panel opens/closes
- dismiss works
- self-update flow still starts and reconnect polling still works

### Keyboard Shortcuts

- settings modal save shortcut
- keyboard shortcuts modal shortcut
- chat tab shortcuts
- Escape closes the correct modal only

## Recommended Manual Checkpoints By Phase

### After Phase 1

- app boot
- attach/detach
- chat send
- open settings

### After Phase 3

- monitor dashboard
- logs
- GPU/system cards
- WebSocket-driven badge changes

### After Phase 4

- presets
- sessions
- file browser
- start/stop/attach flows

### After Phase 5

- remote-agent happy path
- remote-agent error path
- firewall-blocked path

### After Phase 6a

- chat send/stream
- tab persistence
- compaction

### After Phase 6b

- template manager
- model params panel
- chat rendering fidelity
- chat import wiring works
- explicit-policy/settings dirty-state coupling is removed

### After Phase 7

- setup view
- LHM flow
- update flow
- all keyboard shortcuts
- setup-view and shortcuts no longer require their own `window.*` facade

## Specific Risks and How To Avoid Them

### Risk 1: Breaking Inline Handlers

Symptoms:

- buttons stop doing anything
- console errors like `functionName is not defined`

Mitigation:

- maintain an explicit `window` facade until HTML handlers are removed
- do not assume module exports are globally visible

### Risk 2: Breaking Startup Order

Symptoms:

- empty dropdowns
- missing event listeners
- chat controls present but non-functional

Mitigation:

- move to one bootstrap in a controlled phase
- initialize presets, settings, setup view, and chat in deliberate order

### Risk 3: WebSocket Update Regression

Symptoms:

- metrics freeze
- logs stop updating
- attach/detach visuals become inconsistent

Mitigation:

- isolate the WebSocket handler carefully
- keep payload-to-render sequencing identical during first extraction

### Risk 4: State Drift During Split

Symptoms:

- one module mutates state that another module no longer reads
- stale UI or ghost state

Mitigation:

- centralize shared state before aggressive feature extraction

### Risk 5: Duplicate Persistence Logic

Symptoms:

- settings save in one place but not another
- chat tabs persist stale or malformed payloads

Mitigation:

- centralize API/storage helpers
- preserve backend wire format exactly

### Risk 6: Asset Routing Drift

Symptoms:

- new JS modules 404
- app loads partially

Mitigation:

- every new frontend file must have a matching Rust embedded asset route

## Known Oddities To Preserve Or Clean Up Carefully

These are not necessarily bugs to fix during the split unless explicitly targeted. They are points where refactoring can accidentally change behavior.

- Duplicate `escapeHtml` implementations currently exist.
- `appState.wsData` is used by remote-agent UI and hardware/system rendering.
- `window.lhmResolve` is used as a temporary bridge for the LHM overlay flow.
- `lastGpuData` is used for hardware viz rerender after visualization-style changes.
- Settings-saving side effects occur from multiple interactions, including endpoint changes and preset changes.
- Some UI state is persisted in `localStorage`, some in backend settings, and some only in memory.

Treat these as compatibility constraints first, cleanup targets second.

## Concrete To-Do List For The Implementing Agent

This is the recommended execution checklist.

### Preparation

- Re-run `git status --short`
- Re-check `static/index.html`, `static/app.js`, `src/web/static_assets.rs`, `src/web/mod.rs`
- Generate a current list of inline handler names from `index.html`
- Generate a current list of `DOMContentLoaded` listeners and top-level globals from `app.js`

### Phase 1

- Add `static/js/bootstrap.js`
- Add `static/js/compat/globals.js`
- Add `static/js/core/format.js`
- Add `static/js/core/app-state.js` — scaffold shape, migrate obvious shared state
- Move shared formatting helpers to `format.js`
- Update Rust static routes for new JS files
- Update `index.html` script loading
- Verify app boots

### Phase 2 (Refinement)

- Move remaining shared state not yet migrated in Phase 1
- Clean up accessors
- Verify attach/detach, settings, chat still work

### Phase 3

- Capture DOM mutation logs per WebSocket message before extraction
- Extract dashboard helpers and WebSocket logic
- Verify monitor tab thoroughly
- Confirm DOM mutation logs match before/after

### Phase 4

- Extract toast, file browser, presets, sessions, attach-detach
- Verify operational flows thoroughly

### Phase 5

- Extract remote-agent setup and control flows
- Verify happy path and failure paths

### Phase 6a

- Extract chat state and transport
- Verify streaming, persistence, compaction

### Phase 6b

- Extract chat rendering, templates, params
- Verify templates, params, rendering fidelity
- Verify chat import/export both remain supported
- Verify explicit-policy editing no longer imports settings dirty-state logic

### Phase 7

- Extract setup view, updates, LHM, shortcuts
- Verify all peripheral flows
- Verify setup-view and shortcuts no longer rely on cross-module `window.*` bridges

### Phase 8

- Reduce legacy shell
- Minimize `window` facade
- Optionally begin removing inline handlers
- Verify no app-owned cross-module `window.*` bridges remain outside the compatibility shim

## Acceptance Criteria For The Full Refactor

The refactor is complete when all of the following are true:

- `static/app.js` is no longer a monolithic implementation file
- frontend code is split into coherent modules by feature and core concern
- one bootstrap entrypoint initializes the app
- inline handler compatibility is explicit and minimal
- shared mutable state is centralized
- duplicate helpers are removed
- Rust serves all new JS assets correctly
- all major UI flows still work
- no known user-visible behavior regressions remain

## Final Recommendation

Proceed with the refactor.

Do it in phases.

Do not treat this like the CSS split. CSS was mostly declarative and low-coupling. This JavaScript refactor crosses state, transport, rendering, persistence, keyboard shortcuts, and backend static-asset plumbing. The correct success criterion is behavioral stability, not simply "many smaller files."

If time pressure forces prioritization, the best extraction order is:

1. bootstrap + compatibility globals
2. shared state + shared helpers
3. dashboard/WebSocket
4. presets/sessions/attach-detach/file browser
5. remote agent
6. chat
7. setup/LHM/updates/shortcuts

That order keeps the highest-risk shared runtime pieces under control while still moving toward the long-term goal.
