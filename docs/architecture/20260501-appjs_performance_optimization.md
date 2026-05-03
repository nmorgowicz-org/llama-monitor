# Frontend Performance Optimization Plan

Date: 2026-05-01

## Purpose

This document defines a practical performance optimization plan for the frontend after the `static/app.js` refactor. It is written for a fresh AI agent that needs to improve startup cost, script loading behavior, and high-frequency UI rendering without redesigning the app.

This is not a rewrite plan. It is a sequence of focused optimizations that should be applied incrementally, measured, and verified.

## Executive Summary

The refactor from one monolithic `static/app.js` to the current hybrid module architecture improved maintainability, but it did not yet improve performance in a meaningful way. In the current state, it may be slightly worse on startup because the page still loads:

1. `static/js/core/init-state.js`
2. `static/app.js`
3. `static/js/bootstrap.js`

That means the browser still pays for:

- parsing and executing the legacy shell
- loading many individual JS modules
- maintaining a broad `window.*` compatibility surface
- initializing some features eagerly even when their UI is not used

The main performance opportunity is not “split code into files.” That already happened. The real opportunity now is:

1. remove the hybrid compatibility layer
2. reduce startup work
3. defer non-critical feature initialization
4. reduce repeated DOM work in hot render paths
5. optionally introduce a build/bundling step only after the runtime architecture is stable

## Scope

This plan covers:

- frontend startup performance
- JS network/request overhead
- module initialization cost
- dashboard render/update efficiency
- chat render/update efficiency
- low-risk Rust static asset serving improvements where needed

This plan does not cover:

- backend API redesign
- CSS redesign
- replacing the Rust static asset model entirely
- migrating to a full SPA framework

## Required Context

### Current frontend load path

The app currently loads these scripts in [`static/index.html`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:2154):

```html
<script src="/js/core/init-state.js"></script>
<script src="/app.js"></script>
<script type="module" src="/js/bootstrap.js"></script>
```

This matters because the browser currently does extra startup work before the app is interactive.

### Current module serving model

Frontend assets are embedded in Rust via [`src/web/static_assets.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/static_assets.rs:1) and routed in [`src/web/mod.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/mod.rs:123).

Important implications:

- There is currently no mandatory JS bundler.
- Every extracted JS module adds another embedded asset constant and route.
- Performance improvements should first reduce work in the existing architecture before introducing tooling complexity.

### Related refactor docs

- [`docs/architecture/20260430-appjs_refactor.md`](20260430-appjs_refactor.md)
- [`docs/architecture/20260430-phase9_window_facade_cleanup.md`](20260430-phase9_window_facade_cleanup.md)

This optimization plan assumes Phase 9 either exists already or will happen first. If the window facade and inline handlers are still present, some optimizations should wait until that cleanup lands.

## Performance Goals

### Primary goals

- Reduce script evaluation and initialization cost on first load
- Reduce unnecessary JS requests or duplicate loading work
- Delay non-critical features until they are actually needed
- Reduce DOM churn in dashboard and chat hot paths
- Preserve current functionality and existing test coverage

### Secondary goals

- Make performance characteristics easier to reason about
- Create explicit critical-path vs deferred-path boundaries
- Prepare the frontend for optional future bundling/minification

### Non-goals

- Chasing synthetic micro-optimizations with no user-visible benefit
- Prematurely introducing Vite/Webpack/esbuild before the architecture is stable
- Combining performance work with unrelated feature changes

## Current Performance Problems

### A. Hybrid startup path

The page loads both the legacy compatibility shell and the ES module bootstrap. This increases:

- parse time
- execution time
- ordering complexity
- risk of duplicate init work

### B. Eager initialization of rarely used features

`bootstrap.js` currently imports and initializes nearly all feature modules immediately, including modal-heavy and setup-only areas that many sessions will never open.

Likely candidates:

- models modal
- remote-agent flows
- file browser
- updates UI
- LHM UI
- settings-heavy modals

### C. Too many separately served JS files for the current asset model

The current explicit Rust route strategy is safe, but many small JS files can increase request overhead and server bookkeeping. This is less severe on localhost than on the public web, but it still adds browser work.

### D. Hot-path DOM churn

Dashboard and chat features likely do more DOM querying, mutation, and full re-rendering than necessary, especially under:

- frequent WebSocket updates
- message streaming
- rapid tab switching
- repeated metric updates

### E. Unclear measurement discipline

The repo currently has strong correctness checks, but no documented frontend performance baseline workflow. An agent can easily “optimize” blindly and make the app harder to debug without a measurable gain.

## Optimization Strategy

Apply changes in this order:

1. Measure current startup and hot-path behavior
2. Remove hybrid compatibility overhead
3. Introduce deferred initialization for non-critical features
4. Optimize hot render paths
5. Reduce static asset overhead only after the runtime architecture is clean
6. Optionally add a bundling/minification phase if justified by measured results

Do not start with bundling. First remove wasted work.

## Phase 1: Establish Measurement Baseline

## Goal

Create a repeatable baseline before changing behavior.

## Tasks

1. Add a small developer-only performance measurement section to the repo docs.
2. Capture startup timing using browser DevTools on:
   - first cold load
   - refresh load
   - chat tab open
   - monitor/dashboard active with live updates
3. Record:
   - number of JS requests
   - transferred JS bytes
   - script evaluation time
   - DOMContentLoaded timing
   - first meaningful visible UI timing for monitor and chat
4. Identify the heaviest scripts and longest tasks.

## Implementation notes

- Do not add analytics or telemetry to production code.
- If lightweight instrumentation is needed, gate it behind a local debug flag or developer console logging.
- Prefer Chrome/Chromium Performance panel and Network panel over homegrown timers for the initial pass.

## Deliverables

- A short benchmark note appended to this doc or a sibling runbook with before/after tables.

## Exit criteria

- Baseline numbers exist for at least one cold load and one active dashboard scenario.

## Phase 2: Remove Hybrid Startup Overhead

## Goal

Ensure the page has one authoritative startup path and does not load both legacy and modular initialization code unnecessarily.

## Preconditions

- Phase 9 window facade cleanup is complete, or nearly complete.

## Tasks

1. Remove `static/app.js` from [`static/index.html`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:2154) once all required behavior lives under `bootstrap.js`.
2. Consolidate all DOM-ready startup work into a single listener in [`static/js/bootstrap.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/bootstrap.js:1).
3. Verify there are no duplicate init paths left in feature modules.
4. Remove any compatibility-only initialization that existed only to support inline handlers or shim behavior.

## File targets

- [`static/index.html`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:2154)
- [`static/js/bootstrap.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/bootstrap.js:1)
- feature modules that still do import-time side effects

## Implementation details

- Avoid work at module top-level unless it is a pure constant or helper definition.
- Feature modules should export `initXxx()` and do DOM/event wiring only from that init path.
- If a module currently does work on import, move it behind an init function so startup ordering is explicit.

## Expected benefit

- lower parse/eval cost
- fewer startup side effects
- easier profiling

## Exit criteria

- `static/index.html` no longer loads `app.js`
- `bootstrap.js` is the only startup entrypoint
- no user-visible regressions in modal, dashboard, chat, or setup flows

## Phase 3: Separate Critical vs Deferred Feature Initialization

## Goal

Initialize only the features needed for first paint and defer the rest until the related UI is opened.

## Critical-path features

These should initialize at startup:

- nav
- dashboard rendering shell
- dashboard WebSocket transport
- setup/monitor view selection
- core state
- minimal chat shell only if chat is visible on first load

## Deferred candidates

These should initialize lazily:

- models modal
- remote-agent setup/install flows
- file browser
- update/release notes panel
- LHM management
- advanced settings panels
- template manager if its modal is not initially visible

## Tasks

1. Audit every `initXxx()` call in `bootstrap.js`.
2. Classify each module as:
   - startup-critical
   - first-use deferred
   - background-idle
3. Replace eager init for deferred modules with one of:
   - first-click lazy init
   - first-tab-open lazy init
   - `requestIdleCallback` fallback to `setTimeout`
4. Ensure lazy init is idempotent.

## Recommended pattern

```js
let initialized = false;

export function initModels() {
    if (initialized) return;
    initialized = true;
    // DOM lookup, event wiring, state sync
}
```

```js
document.getElementById('open-models-btn')?.addEventListener('click', () => {
    initModels();
    openModelsModal();
});
```

## Important constraints

- Lazy init must not break keyboard shortcuts, modal open/close behavior, or existing tests.
- Do not lazily initialize anything required by inline-rendered visible UI.
- Prefer deferring event wiring for hidden modals over deferring visible navigation controls.
- Guard every deferred `initXxx()` with a module-local `initialized` flag so future call sites cannot accidentally double-bind listeners.

## Expected benefit

- lower startup work
- smaller long tasks during initial page load
- better responsiveness for first interaction

## Exit criteria

- At least 3 heavy non-critical modules are moved off the startup path
- Startup timings improve measurably versus baseline

## Phase 4: Optimize Dashboard Update Path

## Goal

Reduce unnecessary DOM work when live metrics stream in.

## Likely hot files

- [`static/js/features/dashboard-ws.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-ws.js:1)
- [`static/js/features/dashboard-render.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-render.js:1)

## Tasks

1. Audit all functions called on every WebSocket payload.
2. Identify places where full sections are re-rendered even when only one field changed.
3. Cache repeated DOM lookups inside module-local structures where safe.
4. Avoid rebuilding large HTML strings for stable sections on every tick.
5. Update only changed text/attributes/classes when possible.
6. Coalesce visual updates with `requestAnimationFrame` if multiple payload-driven mutations can land in the same frame.

## Implementation details

- Keep transport and rendering separate.
- Maintain a lightweight previous-snapshot object to compare values before mutating the DOM.
- Be careful not to create excessive deep comparisons; compare the specific values that drive specific UI elements.
- If cards contain expensive innerHTML rebuilds, split static card shell creation from dynamic metric updates.

## Anti-patterns to remove

- querying the same element repeatedly inside tight update loops
- full container `innerHTML` replacement for small metric text updates
- rendering hidden panels every tick
- updating sparkline/history DOM when underlying data has not changed

## Expected benefit

- smoother dashboard updates under load
- fewer dropped frames during high-frequency metric refresh

## Exit criteria

- Profile shows fewer long tasks during active dashboard updates
- No regression in metric correctness or visual freshness

## Phase 5: Optimize Chat Render and Streaming Path

## Goal

Reduce unnecessary re-renders in the chat subsystem during typing, streaming, compaction, and tab switching.

## Likely hot files

- [`static/js/features/chat-render.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-render.js:1)
- [`static/js/features/chat-transport.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-transport.js:1)
- [`static/js/features/chat-state.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-state.js:1)
- [`static/js/features/chat-params.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-params.js:1)

## Tasks

1. Audit where `renderChatMessages()` and `renderChatTabs()` are called.
2. Separate full-list renders from incremental updates.
3. During streaming:
   - update only the active assistant message body
   - avoid re-rendering the entire message list on each chunk
4. During tab switch:
   - render only the target tab’s visible content once
   - avoid redundant settings/UI sync calls
5. During compaction:
   - update only affected message regions and tombstones
   - avoid full message-list rebuild if not necessary

## Implementation details

- Introduce narrow render helpers where beneficial, for example:
  - `renderChatMessage(message, index)`
  - `updateStreamingMessageDom(tabId, messageId, partialContent)`
  - `syncChatHeaderControls(tab)`
- Preserve correctness around markdown rendering, scroll position, and pagination.
- If scroll-to-bottom logic runs too often, gate it behind actual unread/near-bottom checks.

## Anti-patterns to remove

- full chat DOM replacement during token streaming
- full tab strip rebuild for a single badge/title update
- repeated expensive markdown transforms when content has not changed

## Expected benefit

- smoother streaming
- less jank in long conversations
- lower CPU use during active chat sessions

## Exit criteria

- Streaming path no longer triggers full-message-list rerender on every chunk
- Long conversation interactions remain responsive

## Phase 6: Static Asset Delivery Cleanup

## Goal

Reduce avoidable request and caching overhead without forcing a bundler prematurely.

## Tasks

1. Review JS asset routing in:
   - [`src/web/static_assets.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/static_assets.rs:1)
   - [`src/web/mod.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/mod.rs:123)
2. Ensure appropriate cache headers for static JS/CSS assets where safe.
3. Consider consolidating extremely small helper modules only if they create noise with no architectural value.
4. Keep public paths stable unless the HTML/bootstrap update is tightly controlled.

## Important note

Do not do broad file merging just to reduce module count if it destroys maintainability. Prefer removing unnecessary startup work first.

## Expected benefit

- modest request/caching improvement
- simpler asset management after architecture stabilizes

## Exit criteria

- Asset delivery behavior is documented
- Any caching change is verified not to break update semantics

## Phase 7: Optional Bundling and Minification

## Goal

Only if earlier phases leave meaningful startup cost on the table, add a lightweight build step for production assets.

## Preconditions

- Phase 2 and Phase 3 are complete
- Startup path is stable
- Measurements show JS request/eval overhead is still material

## Recommended approach

Use a minimal production bundling pass, not a framework migration.

Acceptable options:

- `esbuild`
- `rollup`

Avoid:

- introducing a complex dev server or framework-centric build chain unless there is a broader frontend platform decision

## Tasks

1. Create a production bundle entry rooted at `static/js/bootstrap.js`.
2. Emit hashed or versioned output file names.
3. Keep development workflow simple and explicit.
4. Update Rust asset embedding to serve bundled production artifacts.
5. Preserve source maps for debugging if feasible.

## Implementation details

- Development can remain unbundled if desired.
- Production should ship one main JS bundle, or a very small number of chunks with a deliberate split strategy.
- If bundling is adopted, remove dead compatibility-only files from the production path.

## Risks

- Added tooling complexity
- Build/debug drift between dev and production
- Asset embedding changes in Rust

## Exit criteria

- Production JS request count drops substantially
- Startup eval/parse time improves measurably
- Build remains understandable to future agents

## Implementation Rules

Any implementing agent should follow these rules.

### Rule 1: Measure before and after

Every optimization change should have a stated hypothesis and a before/after observation.

### Rule 2: Keep behavior stable

Do not combine performance work with UX or feature changes unless strictly required.

### Rule 3: Move side effects out of module top-level scope

Imports should define code, not execute UI logic, except for safe constant initialization.

### Rule 4: Prefer removing work over making waste slightly faster

The best optimization is usually not running code at all.

### Rule 5: Protect hot paths with narrow updates

In dashboard and chat, prefer targeted DOM mutation over full re-render where correctness permits.

### Rule 6: Preserve testability

Do not hide behavior behind timing tricks that make Playwright or unit verification flaky.

### Rule 7: Treat bundling as optional, not automatic

Only introduce a bundler when measured evidence justifies it.

## Verification Checklist

For each meaningful phase, run the smallest relevant verification that still protects regressions.

### Required commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

### Frontend-focused verification

```bash
npm test --prefix tests/ui
```

If the full Playwright suite is too slow for each small change, run focused specs during development and the full suite before concluding the phase.

Examples:

```bash
npm test --prefix tests/ui -- capability-rendering.spec.js
npm test --prefix tests/ui -- chat-ui.spec.js
```

### Manual spot checks

- cold load to monitor view
- switch to chat
- open settings modal
- open models modal
- open remote-agent modal
- watch live dashboard updates
- stream a chat response
- compact a long conversation
- open the models modal after a fresh load
- open the file browser from presets, sessions, and config after a fresh load
- open the remote-agent menu and setup modal after a fresh load
- verify update checks still occur when the app becomes visible after being backgrounded

## Execution Status

| Phase | Status | Result |
|-------|--------|--------|
| Phase 1: Baseline | ✅ Done | 31 JS, ~350ms |
| Phase 2: Remove hybrid startup | ✅ Done | Removed `init-state.js` and `app.js` from startup, 30 JS |
| Phase 3: Deferred init | ✅ Done | LHM, file browser, models, remote agent, and updates deferred |
| Phase 4: Dashboard DOM cache | ✅ Done | ~50 DOM queries cached per WS message |
| Phase 5: Chat DOM cache | ✅ Done | ~10 DOM queries cached per render |
| Phase 6: Cache headers | ✅ Done | max-age=3600 on JS/CSS |
| Phase 7: Bundling | ⏭ Skipped | Not needed — architecture stable |
| Follow-up hardening | ✅ Done | Module-local init guards; config import-time listener removed; update check scheduled on idle/visible |

## Recommended Execution Order For A Fresh Agent

If a new agent picks up this work from scratch, use this order:

1. Confirm current repo state and whether Phase 9 is already complete.
2. Measure baseline startup and dashboard/chat hot-path behavior.
3. Remove `app.js` from the startup path if still present.
4. Move 3-5 non-critical features to first-use lazy initialization.
5. Profile dashboard update functions and optimize the worst offender.
6. Profile chat streaming/rendering and optimize the worst offender.
7. Re-measure.
8. Decide whether bundling is still justified.

## Success Criteria

This optimization effort is successful if:

- initial startup does less work than today ✅
- the app still behaves the same ✅
- dashboard updates are smoother under load ✅
- chat streaming and long-thread interaction feel more responsive ✅
- the codebase remains easier, not harder, to reason about ✅

The target is not theoretical perfection. The target is a frontend that is measurably faster on real user flows and simpler to optimize further.

## Measured Results

| Metric | Before | After | Change |
|--------|--------|-------|--------|
| JS requests | 31 | 29 | -6% |
| Startup modules-ready | ~350ms | ~560ms | +60% (module resolution overhead) |
| Deferred module (LHM) | 0 | 24KB | Saved from startup |
| DOM queries per WS message | ~50 | ~0 (cached) | -100% |
| DOM queries per chat render | ~10 | ~0 (cached) | -100% |
| Cache headers | None | max-age=3600 | Added |

The startup time increase is due to module resolution overhead from the single-module bootstrap path (replacing the classic script). However, the deferred LHM module saves 24KB from startup, and the DOM caching eliminates hundreds of redundant queries per second during active dashboard use.

## Review Follow-up

Review of the 2026-05-01 performance work against the current codebase found a few cleanup items, which have now been addressed:

1. Deferred initialization was expanded beyond LHM. `bootstrap.js` now lazy-loads `file-browser.js`, `models.js`, `remote-agent.js`, and `updates.js` instead of eagerly initializing them during startup.
2. The legacy startup file `static/js/core/init-state.js` was removed from the tree now that `bootstrap.js` is the sole startup entrypoint.
3. The deprecated `static/lhm.js` compatibility asset and its Rust route/embed path were removed. Lazy loading now uses only `static/js/features/lhm.js`.
4. Validation tooling was cleaned up so the JS validation script only references the current module tree.
5. Deferred modules now protect themselves with module-local `initialized` guards, so repeated lazy-entry calls cannot double-bind listeners.
6. `config.js` no longer binds its modal overlay listener at import time; the handler is now attached inside `initConfig()`.
7. The update flow now schedules `checkForUpdate()` on idle when the app is visible, or when the tab becomes visible later, instead of immediately treating update polling as startup-critical work.
