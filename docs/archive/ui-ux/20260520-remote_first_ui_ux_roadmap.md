# Remote-First UI/UX Roadmap

**Date:** 2026-05-20
**Status:** Proposed
**Priority:** High

## Goal

Improve the existing remote-first llama-monitor experience without expanding the local "Spawn Local Server" path. The work in this document is meant to make remote attach, remote telemetry, chat workflow, and session navigation feel like one coherent workspace instead of several loosely connected surfaces.

This plan is written so an agent can execute it without additional product context.

## Non-Goals

- Do **not** redesign or expand the local `Spawn Local Server` flow beyond keeping it working.
- Do **not** change authentication architecture, TLS/mTLS, or agent protocol design unless required for small UX-facing additions already described here.
- Do **not** replace existing chat/guided-generation features. The goal is to make them easier to understand, resume, and trust.

## Product Context

The app already has strong remote capabilities:

- Attach to a remote llama.cpp endpoint.
- Read inference metrics immediately after attach.
- Upgrade to full host telemetry via the remote agent.
- Manage multiple chat tabs, full-text message search, archived/hidden conversations, guided generation, compaction, and personas.

The main UX gap is not missing core capability. It is that several flows are fragmented:

- Remote attach starts from a simple URL field even though session history already exists.
- Telemetry depth is documented, but not clearly explained in the primary UI.
- Settings and runtime configuration boundaries are not always obvious from the current controls.
- Chat organization and search are split across several small surfaces instead of one fast control plane.
- Important workflow state still lives only in browser-local storage or is not persisted at all.

## Current Implementation Anchors

Primary files likely involved in this plan:

- [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html)
- [static/js/features/setup-view.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/setup-view.js)
- [static/js/features/sessions.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/sessions.js)
- [static/js/features/attach-detach.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/attach-detach.js)
- [static/js/features/remote-agent.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/remote-agent.js)
- [static/js/features/dashboard-ws.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-ws.js)
- [static/js/features/dashboard-render.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-render.js)
- [static/js/features/settings.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/settings.js)
- [static/js/features/chat-sessions-sidebar.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-sessions-sidebar.js)
- [static/js/features/chat-search.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-search.js)
- [static/js/features/chat-transport.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-transport.js)
- [static/js/features/chat-notes.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-notes.js)
- [static/js/features/chat-suggestions.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-suggestions.js)
- [static/js/features/chat-quick-guide.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-quick-guide.js)
- [static/js/features/chat-state.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-state.js)
- [static/js/core/app-state.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/core/app-state.js)
- [src/web/api.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/api.rs)
- [src/chat_storage.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/chat_storage.rs)

Relevant docs that must stay aligned as work ships:

- [docs/reference/dashboard.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/dashboard.md)
- [docs/reference/remote-agent.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/remote-agent.md)
- [docs/reference/chat.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/chat.md)
- [docs/reference/api.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/api.md)

## Workstreams

## Workstream 1: Remote Session Home / Resume Workspace

### Problem

The setup screen is still optimized for typing a fresh endpoint URL, even though the app already stores sessions and can identify prior attach targets. Remote-first users are likely reconnecting to a small set of hosts, so the setup screen should behave like a workspace resume surface.

### Scope

Transform the remote setup path into a recent-endpoints dashboard:

- Show recent attach sessions prominently on the setup screen.
- Include one-click reconnect.
- Show last active time.
- Show last known session status where available.
- Show last known remote-agent / telemetry state if it can be derived from saved session or last-known UI state.
- Keep the existing manual endpoint entry, but demote it below the recent list.

### Frontend changes

- Extend the setup view in [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html) with a recent remote sessions section inside or adjacent to the existing attach card.
- Reuse session data already loaded by [static/js/features/sessions.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/sessions.js).
- Add a dedicated setup renderer in [static/js/features/setup-view.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/setup-view.js) instead of keeping setup state limited to:
  - `llama-monitor-last-session`
  - `llama-monitor-last-endpoint`
- Add connect actions that call existing attach logic through [static/js/features/attach-detach.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/attach-detach.js).

### Backend changes

- Prefer reusing `/api/sessions`.
- If `/api/sessions` does not expose enough attach metadata for good setup cards, extend it with:
  - last attach endpoint
  - last active timestamp
  - last known status
  - optional last known agent/telemetry summary

### UX rules

- The first primary CTA on setup should be `Reconnect` or `Resume` for the most recent remote session when one exists.
- Manual endpoint entry remains available and visible.
- The spawn card remains present but unchanged in purpose.

### Acceptance criteria

- A user with prior attach sessions can reconnect from setup without retyping a URL.
- Recent remote sessions are more visually prominent than the raw URL field.
- The setup screen still works correctly for first-run users with no saved sessions.
- No regressions to the local spawn card.

---

## Workstream 2: Telemetry Grade and Remote Health Center

### Problem

The app already distinguishes among `Full telemetry`, `Inference only`, `Limited`, firewall-blocked agent, update available, and degraded compatibility states. That model is clear in docs but underexposed in the primary UI. Users should not need to infer why GPU/system cards are empty.

### Scope

Make telemetry depth a first-class UI state across:

- header endpoint strip
- header agent badge
- Server tab cards
- empty states and warning copy

### Required state model

Create a single derived remote health / telemetry state instead of scattering display decisions across multiple files.

Minimum states:

- `local_full`
- `remote_inference_only`
- `remote_agent_connecting`
- `remote_agent_connected`
- `remote_agent_degraded`
- `remote_agent_firewall_blocked`
- `remote_agent_update_available`
- `remote_partial_sensors`
- `remote_error`

### Frontend changes

- Add a shared state derivation helper, likely in [static/js/features/dashboard-ws.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-ws.js) or a small new helper module.
- Feed that state into:
  - [static/js/features/remote-agent.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/remote-agent.js)
  - [static/js/features/dashboard-render.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-render.js)
  - setup and attach UI where helpful
- Upgrade the header Agent badge from a passive status lamp into a small remote health center:
  - state label
  - effective telemetry grade
  - direct next action
- Add explicit card-level copy for limited states:
  - `Inference only: attach succeeded, but host telemetry requires the remote agent`
  - `Agent running but HTTP unreachable`
  - `Connected with partial sensor coverage`
  - `Connected in degraded compatibility mode`

### Copy rules

- Do not use Windows-specific guidance unless the remote target is known to be Windows.
- Sensor-specific warnings must reflect the likely source:
  - missing agent
  - blocked HTTP
  - unavailable CPU temp
  - missing GPU tooling
  - protocol mismatch / degraded mode

### Acceptance criteria

- A remote attach user can tell within a few seconds which telemetry tier they currently have.
- Every limited or degraded state has a next action visible in the main UI.
- Generic “unavailable” card copy is replaced with actionable, state-specific copy where possible.
- The remote-agent tooltip/modal stays consistent with the header badge and Server tab.

---

## Workstream 3: Settings vs Configuration Trust Cleanup

### Problem

The current UI and docs already try to separate user preferences from runtime configuration, but the Settings experience still risks feeling broader than what [static/js/features/settings.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/settings.js) actually persists and restores. Any control that appears live but is not actually owned by `collectSettings()` / `applySettings()` undermines trust.

### Scope

Audit every Settings control and classify it as one of:

- persisted app setting
- browser-local preference
- runtime configuration handoff
- deprecated / misleading control to remove

### Required output

Produce a control inventory table during implementation. For each visible control in Settings:

- control ID
- displayed section
- current persistence owner
- actual save path
- actual restore path
- final disposition

### Frontend changes

- Update Settings UI copy and section labels so ownership is explicit:
  - runtime config controls should open the Configuration modal instead of pretending to live in Settings
  - browser-local preferences should be labeled as browser-local if kept client-side
- Wire missing controls if they are intended to be real settings.
- Remove or demote controls that should not be there.

### Backend changes

- Only add backend fields if a control is intended to be a shared app setting.
- Do not bloat `/api/settings` with purely browser-local presentation state unless that state is intentionally being promoted to shared preference storage as part of Workstream 5.

### Acceptance criteria

- Every visible control in Settings has a clear persistence/ownership model.
- No control appears editable while silently doing nothing.
- Settings, Configuration modal, and docs all describe the same ownership boundaries.

---

## Workstream 4: Unified Workspace Search / Command Palette

### Problem

Conversation title filtering, full-text message search, archived/hidden views, and per-row quick actions are split across different surfaces. Power users need one fast control surface.

### Scope

Add a workspace command palette / omnibox that unifies:

- conversation title search
- full-text message search
- archived conversations
- hidden conversations
- direct actions:
  - switch conversation
  - open matching message
  - pin/unpin
  - archive/unarchive
  - hide/unhide
  - duplicate
  - rename
  - delete

### Interaction model

- Keyboard shortcut: `Ctrl+K` / `Cmd+K`
- Keep existing `Ctrl+Shift+F` full-text search working initially
- The command palette may internally reuse the existing message search engine and sidebar grouping logic
- Start with one global palette; do not create separate palettes per page

### Frontend changes

- Add modal/palette markup to [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html).
- Reuse logic from:
  - [static/js/features/chat-search.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-search.js)
  - [static/js/features/chat-sessions-sidebar.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-sessions-sidebar.js)
- Prefer a new dedicated module, for example `static/js/features/workspace-command-palette.js`, instead of overloading the existing sidebar search module.

### Data requirements

- Search result rows should identify:
  - conversation title
  - visibility bucket
  - match type (`conversation`, `message`, `action`)
  - message snippet if applicable

### Acceptance criteria

- A user can switch to a conversation or jump to a message match from one keyboard-first surface.
- Archived and hidden chats are discoverable without opening secondary sections first.
- Existing sidebar filter/search features still work if a user prefers them.

---

## Workstream 5: Workflow State Persistence and Cross-Device Preferences

### Problem

Important workflow state is scattered:

- some settings are app-backed
- some preferences are browser-local
- some drafts are not persisted at all

That weakens the “workspace” model, especially for remote users working across sessions or browsers.

### Scope

Handle two related categories:

1. Persist the main composer draft per tab.
2. Promote selected browser-local workflow preferences to shared storage when they should follow the user across browsers/devices.

### Part A: Per-tab main composer draft persistence

Persist draft text for the primary chat composer, per tab.

Requirements:

- restore draft after reload
- restore draft after attach/detach flow
- restore draft after transient disconnect
- keep drafts tab-specific
- clear draft on successful send only after the message is accepted for transmission

Likely files:

- [static/js/features/chat-transport.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-transport.js)
- [static/js/features/chat-state.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-state.js)
- [src/chat_storage.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/chat_storage.rs) if drafts are promoted into persisted tab state
- [src/web/api.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/api.rs) if API support is required

Preferred implementation:

- Persist drafts in the same durable tab/session model used for chat tabs rather than only `localStorage`.
- If a smaller first step is needed, `localStorage` is acceptable only if the code clearly marks it as an interim implementation and the API/schema work is deferred deliberately.

### Part B: Promote selected browser-local preferences

Audit current local-only preferences and decide which should become shared user/workspace settings.

Initial candidates:

- chat style
- font scale
- enter-to-send behavior
- date format
- notes sidebar open/collapsed state
- custom suggestion categories where they affect real workflow

Do **not** automatically promote every visual preference. Use this rule:

- If the preference meaningfully affects workflow continuity, shared storage is preferred.
- If it is purely cosmetic and highly device-specific, browser-local may remain acceptable.

### Acceptance criteria

- Unsent chat drafts survive page reload and reconnect flows.
- At least the highest-value workflow preferences are intentionally classified as shared or local.
- The final ownership model is documented in code comments and docs.

---

## Workstream 6: Next Reply Plan Summary

### Problem

The app already has powerful steering mechanisms, but they are distributed across several surfaces. Users can easily lose track of what will influence the next assistant response.

### Scope

Add a compact “next reply plan” summary near the composer that shows the active steering inputs for the next response.

Minimum contents:

- active persona
- explicit mode state
- context notes presence
- quick guide state
- active suggestion/draft override state
- armed story beat / surprise state
- compaction / rolling memory presence when relevant

### UX constraints

- Keep the summary compact by default.
- Use chips, short lines, or a small disclosure panel.
- It should answer one question: “What is currently steering the next reply?”
- It must not duplicate the full prompt debug inspector.

### Frontend changes

- Add summary container markup near the chat composer in [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html).
- Build the summary from existing tab state used by:
  - [static/js/features/chat-notes.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-notes.js)
  - [static/js/features/chat-suggestions.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-suggestions.js)
  - [static/js/features/chat-quick-guide.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-quick-guide.js)
  - [static/js/features/chat-templates.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-templates.js)
  - [static/js/features/chat-transport.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-transport.js)

### Acceptance criteria

- A user can identify the active steering stack without opening multiple side panels.
- The summary updates immediately when steering state changes.
- The summary does not become visually noisy in compact layouts.

---

## Workstream 7: Sidebar Metadata Accuracy and Density

### Problem

The conversation sidebar is already strong, but it appears to underuse backend metadata for inactive/lazy-loaded tabs. That makes large chat collections less informative than they should be.

### Scope

Make the sidebar more accurate and scalable by relying on server metadata instead of only loaded message arrays.

### Required improvements

- Use backend `message_count` or equivalent for inactive tabs.
- Continue showing persona, explicit badge, and context pressure when known.
- Avoid zero/blank counts caused by lazy-loaded message arrays.
- Ensure archived and hidden sections remain accurate even before a tab has been opened in the current browser session.

### Backend changes

- Confirm the chat tab/list API returns all metadata needed by the sidebar.
- Extend it if necessary with:
  - message count
  - last updated timestamp
  - visibility
  - active persona label
  - explicit level
  - last known context percentage if available

### Frontend changes

- Update [static/js/features/chat-sessions-sidebar.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-sessions-sidebar.js) to prefer tab metadata fields over `tab.messages`.
- Ensure search/palette surfaces also use the same reliable metadata.

### Acceptance criteria

- The sidebar shows stable, believable counts and metadata even for chats not opened in the current session.
- Metadata consistency improves rather than regresses when history is paged/lazy-loaded.

---

## Suggested Execution Order

Implement in this order:

1. Workstream 2: Telemetry Grade and Remote Health Center
2. Workstream 1: Remote Session Home / Resume Workspace
3. Workstream 3: Settings vs Configuration Trust Cleanup
4. Workstream 7: Sidebar Metadata Accuracy and Density
5. Workstream 5: Workflow State Persistence and Cross-Device Preferences
6. Workstream 6: Next Reply Plan Summary
7. Workstream 4: Unified Workspace Search / Command Palette

Reasoning:

- Workstream 2 clarifies the core remote product model first.
- Workstream 1 makes setup/resume align with that clarified model.
- Workstream 3 removes trust gaps before adding more settings-related persistence.
- Workstream 7 strengthens the chat/session data model before building the command palette on top of it.
- Workstream 5 improves workflow continuity.
- Workstream 6 surfaces active steering state once persistence is more reliable.
- Workstream 4 is valuable but easiest to do cleanly after metadata and persistence work are more stable.

## Validation Checklist

Run at minimum after each substantial workstream:

```bash
git diff --check
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
cargo build --release
npm run validate-js
npm run lint
```

If any change touches chat/navigation/settings UI in a meaningful way, also run Playwright UI tests:

```bash
cd tests/ui
npm test
```

If visuals change in setup, chat sidebar, telemetry surfaces, settings, or remote-agent flows, update screenshots via the harness documented in `AGENTS.md` and `tests/ui/README.md`.

## Required Docs Updates When Shipping

Update these docs in the same PR(s) as the implementation:

- [docs/reference/dashboard.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/dashboard.md)
  - telemetry grades
  - header agent badge behavior
  - remote limited/degraded states
- [docs/reference/remote-agent.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/remote-agent.md)
  - updated main setup flow
  - richer badge/health center behavior
  - troubleshooting copy
- [docs/reference/chat.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/chat.md)
  - composer draft persistence
  - next reply plan summary
  - command palette if shipped
  - sidebar metadata behavior
- [docs/reference/api.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/api.md)
  - any new or changed fields for session/tab metadata and persisted UI settings

## Delivery Guidance For Agents

If implementing this as multiple PRs, keep each PR centered on one workstream or one tightly related pair:

- PR 1: telemetry grade + health center
- PR 2: remote setup/resume workspace
- PR 3: settings/config cleanup
- PR 4: sidebar metadata + persistence foundation
- PR 5: composer draft persistence + next reply plan
- PR 6: command palette

Preferred PR title patterns:

- `feat(ui): make remote telemetry grade explicit`
- `feat(sessions): add remote workspace resume flow`
- `fix(settings): align settings ownership with persisted state`
- `feat(chat): persist per-tab drafts and surface next reply plan`
- `feat(chat): add workspace command palette`

If a single PR ships more than one meaningful user-facing item, add a `BEGIN_COMMIT_OVERRIDE` / `END_COMMIT_OVERRIDE` block to the PR body per `AGENTS.md`.
