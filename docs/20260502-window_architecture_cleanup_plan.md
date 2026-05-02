# Frontend Window And Architecture Cleanup Plan

Date: 2026-05-02

## Purpose

This document is a supplemental follow-up to the `app.js` breakup and the 2026-05-01 performance optimization work.

The previous work improved maintainability and startup behavior, but it did not fully reach the desired end-state for the frontend architecture. The codebase still relies heavily on `window.*` bridges and centralized bootstrap wiring, and several feature slices still mix transport, state mutation, and rendering concerns.

This document defines what remains, why it matters, and how a future AI agent should execute the next cleanup safely.

## Relationship To Earlier Docs

This plan builds on:

- [`docs/20260430-appjs_refactor.md`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/20260430-appjs_refactor.md:1)
- [`docs/20260430-phase9_window_facade_cleanup.md`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/20260430-phase9_window_facade_cleanup.md:1)
- [`docs/20260501-appjs_performance_optimization.md`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/20260501-appjs_performance_optimization.md:1)

Those earlier phases got the frontend to a workable modular state. This document covers the next phase: turning the modular state into a cleaner architecture with explicit ownership and fewer global contracts.

## Executive Summary

The frontend no longer depends on one monolithic `static/app.js`, but it still behaves like a monolith in several important ways:

1. `window.*` is still used as a broad shared state and function bus.
2. `bootstrap.js` still acts as a central orchestrator for many feature relationships.
3. Some feature modules still combine transport logic, state mutation, and render-side behavior.
4. Several cross-feature contracts are implicit, stringly typed, and difficult to validate.

The result is that the code is better than before, but still not at the desired end-state.

The end-state should be:

1. feature modules import explicit dependencies instead of reaching through `window.*`
2. shared state lives in deliberate state modules, not on the browser global
3. rendering code owns rendering, transport code owns transport, and controllers coordinate between them
4. lazy loading and startup ordering are explicit and resilient
5. `window.*` remains only for true browser-global needs or intentionally temporary compatibility shims

## What Is Still Wrong

## A. `window.*` is still acting as an application bus

Examples in the current codebase:

- [`static/js/bootstrap.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/bootstrap.js:14) copies significant shared state onto `window`
- [`static/js/features/dashboard-ws.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-ws.js:1) reads and writes dashboard data via `window.prevValues`, `window.metricSeries`, and many render helpers on `window`
- [`static/js/features/chat-render.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-render.js:1) still calls many chat functions and state lookups through `window.*`
- [`static/js/features/remote-agent.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/remote-agent.js:1) exposes shared functionality on `window` for cross-module use

Problems caused by this:

- dependencies are hidden
- import graphs do not reflect real contracts
- refactors are riskier because many consumers are coupled by global names
- static checks are weaker
- lazy loading can fail in subtle ways when a global bridge is assumed to exist

## B. Some feature slices still mix too many concerns

The most obvious example is the dashboard slice:

- [`static/js/features/dashboard-ws.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-ws.js:1) handles WebSocket payload flow, shared state updates, dashboard status transitions, and many render-triggering calls
- [`static/js/features/dashboard-render.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-render.js:1) contains render helpers, but the contract between the transport side and render side is still mostly global

This makes the code harder to reason about because:

- state transitions are not clearly isolated
- render scheduling is not a first-class concern
- payload normalization is mixed into UI behavior

The chat slice has a similar, though less severe, issue:

- rendering, transport reactions, tab state, and user actions are split into multiple files
- but much of the coordination still happens via global functions and shared mutable browser-global state

## C. `bootstrap.js` still contains too much app-specific coordination

[`static/js/bootstrap.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/bootstrap.js:1) is currently acceptable, but it is trending toward becoming a second-generation monolith coordinator if left alone.

That file currently does several categories of work:

- shared state exposure
- feature startup sequencing
- lazy module coordination
- cross-feature bridges
- startup policy decisions

It should remain an entrypoint, but not become the permanent home for application wiring logic that really belongs to feature controllers or shared infrastructure modules.

## D. Validation still reflects the old architecture

The repo has useful checks, but frontend validation still mainly asks:

- does syntax parse
- does a referenced global appear somewhere

That is better than nothing, but it is not the same as explicit module contracts.

Long term, the better validation strategy is to reduce the need for special global checks at all.

## Cleanup Goals

The next architecture cleanup should aim to:

1. reduce `window.*` usage materially, not cosmetically
2. make cross-feature dependencies explicit through imports or narrow controller APIs
3. isolate state from rendering and transport in the most important slices
4. keep lazy loading intact and simpler to reason about
5. avoid a rewrite and preserve current behavior

## Non-Goals

This plan is not:

- a SPA framework migration
- a React/Vue/Svelte rewrite
- a wholesale bundler migration
- a CSS redesign
- a broad backend rewrite

The intent is to improve architecture within the current plain-JS embedded-asset model.

## Recommended Strategy

Do not attack the entire frontend in one pass.

Instead:

1. pick one feature slice
2. define explicit ownership for state, transport, and rendering in that slice
3. replace the relevant `window.*` bridges in that slice
4. verify behavior
5. repeat for the next slice

This should be done in multiple contained PRs unless there is a very strong reason to batch them.

## Suggested Execution Order

Recommended order:

1. Dashboard slice
2. Chat slice
3. Remote-agent slice
4. Bootstrap/shared-infra cleanup

Reasoning:

- dashboard has the clearest transport/render split opportunity
- chat is high value but more behaviorally dense
- remote-agent is somewhat self-contained but UI-heavy
- bootstrap cleanup is safer after the slices expose better contracts

## Phase 1: Inventory Global Contracts

## Goal

Create a current inventory of `window.*` state and function bridges, grouped by owner and consumer.

## Tasks

1. Enumerate every assignment to `window.*` in:
   - [`static/js/bootstrap.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/bootstrap.js:1)
   - [`static/js/features/**/*.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features)
   - [`static/js/compat/**/*.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/compat)
2. For each global, record:
   - who assigns it
   - who reads it
   - whether it is state, an action, a render helper, or a compatibility shim
3. Mark each item as:
   - keep temporarily
   - replace with import
   - replace with state module
   - replace with controller module
   - remove

## Deliverable

A small table added to this doc or a sibling working note.

## Exit Criteria

There is an explicit inventory of the remaining global contracts before code changes begin.

## Phase 2: Dashboard Slice Cleanup

## Goal

Turn the dashboard slice into a more explicit structure with:

- transport input
- state mutation
- render scheduling
- render helpers

## Current Target Files

- [`static/js/features/dashboard-ws.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-ws.js:1)
- [`static/js/features/dashboard-render.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-render.js:1)
- [`static/js/core/app-state.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/core/app-state.js:1)

## Recommended Structure

Create or evolve toward modules with roles similar to:

- `dashboard-state.js`
- `dashboard-transport.js`
- `dashboard-render.js`
- `dashboard-controller.js`

The exact filenames are less important than the ownership boundaries.

## Guidance

1. Move dashboard-specific mutable state access behind imported functions instead of direct `window.*` writes.
2. Normalize incoming WebSocket payloads before render decisions.
3. Have transport code publish updates into state, not directly micromanage the DOM.
4. Let a controller decide which render functions run and when.
5. Keep render helpers pure where possible: input data in, DOM updates out.

## Important Rule

Do not mix this with unrelated dashboard UI redesign.

## Exit Criteria

- `dashboard-ws.js` no longer depends on broad `window.*` render/state bridges
- dashboard render calls are driven through explicit imports or a controller API
- dashboard state ownership is clear

## Phase 3: Chat Slice Cleanup

## Goal

Reduce global chat coupling and clarify ownership between:

- chat state
- chat transport
- chat rendering
- tab/user actions

## Current Target Files

- [`static/js/features/chat-state.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-state.js:1)
- [`static/js/features/chat-transport.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-transport.js:1)
- [`static/js/features/chat-render.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-render.js:1)
- [`static/js/features/chat-params.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-templates.js:1)
- [`static/js/features/chat-templates.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-templates.js:1)

## Guidance

1. Define which module owns chat tab state mutation.
2. Replace `window.*` chat action calls with explicit imports where cycles do not prevent it.
3. If cycles do prevent it, introduce a narrow chat controller module rather than keeping many global bridges.
4. Keep render helpers focused on DOM responsibilities only.
5. Keep transport logic focused on request/stream handling and events.

## Important Rule

Do not try to solve every chat UX issue during this cleanup. Keep the work architectural.

## Exit Criteria

- chat modules communicate through explicit imports or a narrow controller
- chat rendering does not depend on a broad global action surface
- chat state ownership is clearer than it is today

## Phase 4: Remote-Agent Slice Cleanup

## Goal

Reduce cross-feature global coupling in the remote-agent flows.

## Current Target Files

- [`static/js/features/remote-agent.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/remote-agent.js:1)
- [`static/js/features/dashboard-ws.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-ws.js:1)
- [`static/js/features/settings.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/settings.js:1)

## Guidance

1. Identify which remote-agent APIs truly need to be visible outside the module.
2. Replace `window.setRemoteAgentStatus`-style bridges with imported APIs or a narrow shared module where practical.
3. Keep setup modal behavior, status display, and remote lifecycle actions clearly separated.
4. Avoid making remote-agent depend more heavily on bootstrap than it already does.

## Exit Criteria

- remote-agent functionality has a smaller public surface
- consumers use explicit imports or a narrow controller contract

## Phase 5: Bootstrap And Shared Infrastructure Cleanup

## Goal

Shrink `bootstrap.js` down to an entrypoint rather than a long-term coordination hub.

## Guidance

1. Move feature-specific lazy-loading policy into the owning slice where reasonable.
2. Reduce `bootstrap.js` state-copying to `window.*` as slice cleanups land.
3. Keep only:
   - top-level startup sequencing
   - essential compatibility shims
   - truly app-global browser setup
4. If a bridge remains, comment why it still exists and what would remove it.

## Exit Criteria

- `bootstrap.js` is smaller and more declarative
- feature coordination lives closer to the features it belongs to

## Implementation Rules For Future Agents

Any future AI agent implementing this plan should follow these rules.

### Rule 1: Clean one slice at a time

Do not opportunistically refactor dashboard, chat, and remote-agent all in one mixed patch unless the changes are extremely small and mechanical.

### Rule 2: Replace globals with explicit ownership, not indirection for its own sake

Do not create a useless wrapper module that simply re-exports ten globals. The point is to improve ownership and clarity.

### Rule 3: Avoid circular imports by design

If two modules need each other, stop and define a narrower shared module or controller instead of falling back to `window.*`.

### Rule 4: Preserve lazy-loading behavior

The cleanup must not accidentally move modal-heavy or rarely used features back onto the startup path.

### Rule 5: Keep behavior stable

No UI redesigns and no speculative UX changes while doing architecture cleanup.

### Rule 6: Document bridge removals

When removing a `window.*` bridge, note:

- what replaced it
- which consumers were updated
- whether any compatibility shims remain

### Rule 7: Prefer imported state helpers over raw imported mutable objects

When a slice becomes more structured, prefer helper functions such as:

- `getDashboardState()`
- `updateDashboardMetrics(payload)`
- `setActiveChatTab(id)`

over widespread direct writes from many files.

## Verification Checklist

For each slice cleanup:

1. run `cargo fmt -- --check`
2. run `cargo test`
3. run `./scripts/validate-js.sh`
4. run the smallest relevant frontend spec if available
5. manually exercise the touched slice

Manual checks by slice:

### Dashboard

- live dashboard updates still render
- attach/detach transitions still work
- context, throughput, generation, GPU, and system cards still update

### Chat

- tab switching
- rename tab
- stream a response
- edit/resend
- compaction UI

### Remote Agent

- open agent menu
- open setup modal
- status updates
- guided SSH flow
- start/stop/update/remove actions

## Recommended PR Strategy

Prefer several contained PRs over one giant architecture PR.

Suggested PR breakdown:

1. `refactor(ui): isolate dashboard state and render flow`
2. `refactor(ui): replace chat window bridges with module contracts`
3. `refactor(ui): narrow remote-agent public API`
4. `refactor(ui): reduce bootstrap global coordination`

If a single PR is used anyway, it should still be implemented and reviewed slice by slice.

## Success Criteria

This cleanup effort is successful if:

- the number of `window.*` bridges is materially smaller
- remaining globals are intentional and documented
- feature ownership is easier to explain in one pass
- lazy-loading remains intact
- future agents can make changes without discovering hidden global contracts late

The target is not “zero globals at any cost.” The target is a frontend whose contracts are explicit enough that future changes are predictable.
