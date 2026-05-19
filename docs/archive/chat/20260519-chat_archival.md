# Chat Archive / Hidden Conversations Implementation Plan

**Date:** 2026-05-19
**Status:** Planned
**Priority:** High

## Goal

Add first-class, durable conversation visibility controls for chat tabs so users can:

- hide a sensitive chat immediately
- archive chats they do not want in the main working set
- restore archived or hidden chats later
- trust that hidden chats are not still exposed through obvious UI surfaces like the sidebar, collapsed labels, or full-text search

This feature must feel like a premium 2026 product surface, not a bolted-on admin toggle. The UX must be obvious to non-technical users and fast enough for real privacy-sensitive use.

## Summary

Implement a single backend visibility model for chat tabs:

- `active`
- `archived`
- `hidden`

Do **not** treat this as an extension of the current delete/trash flow. Archive/hidden are durable, non-destructive states stored in SQLite and surfaced through the API. Delete remains a separate destructive action.

The product should ship with:

1. A simple, visible archive flow for ordinary organization
2. A fast hide flow for sensitive chats
3. A dedicated restore surface for archived and hidden chats
4. Strict filtering so hidden chats do not leak through normal navigation/search

## Why This Is Needed

Current behavior is insufficient:

- `DELETE /api/chat/tabs/{id}` hard-deletes the tab and cascades its messages from SQLite
- frontend restore is only in-memory undo via `chat.tabTrash`
- hidden/archive state does not exist in the schema or API
- sensitive chat titles can still appear in the collapsed sidebar label
- cross-chat full-text search currently searches all tabs

Relevant current files:

- [src/chat_storage.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/chat_storage.rs)
- [src/web/api.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/api.rs)
- [static/js/features/chat-state.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-state.js)
- [static/js/features/chat-sessions-sidebar.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-sessions-sidebar.js)
- [static/js/features/chat-search.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-search.js)
- [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html)

## Product Principles

1. Privacy must be real, not cosmetic.
   - If a chat is hidden, it must not remain trivially visible in normal sidebar/search flows.

2. “Hide” and “Delete” must never be confused.
   - Hidden/archived means safely retained.
   - Delete means destructive removal.

3. The fast path matters.
   - A user under social pressure should be able to hide the active conversation in one obvious action.

4. The recovery path must be understandable.
   - Users must know where archived and hidden chats go, and how to restore them.

5. Use one shared model, not separate ad hoc systems.
   - Archive and hidden are two user-facing behaviors built on one durable visibility field.

6. Security posture must not regress.
   - New visibility behavior must preserve the current `api-token` route protection model.
   - Hidden content must not leak through new UI, search, or API paths.

## Definitions

### Active

Normal conversation. Visible in:

- main conversation sidebar
- collapsed sidebar label if selected
- normal title filter
- normal message search
- top chat/tab UI if still rendered

### Archived

Conversation intentionally removed from the main working set, but not privacy-sensitive by default.

Visible only in:

- dedicated `Archived` management section or surface
- archived-aware search if explicitly requested

Not visible in:

- main active conversation list
- normal title filter
- normal message search
- collapsed label unless explicitly restored/opened

### Hidden

Conversation intentionally concealed for privacy. It is still durable and restorable, but should require a more deliberate reveal path than archive.

Visible only in:

- dedicated `Hidden Chats` surface
- only after deliberate reveal/unlock action in the UI

Not visible in:

- main active conversation list
- collapsed label
- normal title filter
- normal message search
- standard archive list unless the design explicitly merges management surfaces

### Delete

Destructive removal from the database. Existing delete/trash remains distinct from archive/hidden.

## Non-Goals

- encryption-at-rest for individual chats
- password-protected vault semantics
- OS-level secure screen mode
- multi-user permissions
- stealth mode that rewrites browser history, screenshots, or external logs

Those can be explored later, but are not required for this feature.

## User Stories

### Primary

- As a user, I can hide the active chat immediately when someone walks into the room.
- As a user, I can archive chats I want out of the sidebar but still keep.
- As a user, I can find and restore archived chats later.
- As a user, I can restore hidden chats later from a deliberate privacy-aware surface.
- As a user, I can trust that hidden chats are not still discoverable through obvious search or collapsed UI.

### Secondary

- As a user, I can understand the difference between `Archive`, `Hide`, and `Delete` without reading docs.
- As a user, I can see clear counts for archived and hidden chats.
- As a user, I can undo accidental archive/hide actions.

## Recommended Product Behavior

### Core Model

Implement a single `visibility` field with values:

- `active`
- `archived`
- `hidden`

Optional supporting fields:

- `visibility_changed_at`
- `archived_at`
- `hidden_at`

Minimum viable implementation only requires `visibility` plus existing `updated_at`. However, a timestamp for visibility changes is useful for sorting and future analytics.

### Fast Hide

The active conversation should expose a prominent one-click `Hide Now` action.

Recommended placement:

- in the chat header near other active-tab controls
- also available in the conversation sidebar item context menu

Behavior:

1. User clicks `Hide Now`
2. Active chat immediately disappears from the active set
3. App selects a safe fallback conversation or empty state
4. A toast appears: `Chat hidden` with `Undo`
5. Collapsed label and active list update immediately

This action must be faster than opening a context menu and navigating a destructive-sounding option.

### Archive

Archive is for organization, not emergency privacy.

Recommended placement:

- conversation sidebar item context menu
- optional archive action in a row hover affordance if the visual density still feels clean

Behavior:

1. User archives a chat
2. It disappears from active list
3. It appears in `Archived`
4. Toast offers `Undo`

### Restore

Restore must be available from both `Archived` and `Hidden`.

Behavior:

- restore returns chat to `active`
- restored chat may become the active tab immediately if initiated directly from a management list

### Search

Default search behavior:

- normal message search searches only `active`

Optional explicit controls:

- `Include archived`
- `Include hidden`

Recommendation:

- Support `Include archived` in the UI
- Do **not** include hidden chats in standard search by default
- Hidden chats should require entering the dedicated hidden management surface before searching them

## Security Requirements

This feature touches persisted user-owned chat content. Security requirements are part of the implementation, not a follow-up.

Reference sources:

- [src/web/api.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/api.rs)
- [tests/auth_routing.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/tests/auth_routing.rs)
- [docs/reference/api.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/api.md)
- [docs/archive/security/20260517-security_part2.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/archive/security/20260517-security_part2.md)

### Authentication Requirements

All new visibility-related chat routes must use the same auth model as the current chat routes:

- `Authorization: Bearer <api-token>`
- `check_api_token(&auth, &cfg)` for per-route checks
- `unauthorized_api_token()` for unauthorized replies

When `api-token` is configured:

- requests without a valid bearer must be rejected

When `api-token` is not configured:

- preserve the current local-first behavior already used by chat routes

Do not:

- add unauthenticated convenience routes for archive/hide/restore
- rely only on `auth_guard` if the neighboring chat routes also use per-endpoint `check_api_token`
- create alternate token rules for hidden chats

### Required Route Protection

Any new route introduced by this feature must require `api-token` at the same level as the existing chat routes, including:

- archive
- hide
- restore
- archived-only list surfaces
- hidden-only list surfaces
- any hidden-only search flow

If the feature is implemented via expanded query params on existing routes rather than new routes, those code paths must remain behind the same existing route auth checks.

### Frontend Header / Token Handling

All new frontend fetch calls must use the existing auth header helpers.

For GET-like calls:

```js
headers: window.authHeaders ? window.authHeaders() : {}
```

For JSON requests:

```js
headers: window.authHeaders
  ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
  : { 'Content-Type': 'application/json' }
```

Do not duplicate token retrieval logic in feature code if the shared helper already covers it.

### Hidden Metadata Leakage Rules

Security here also includes reducing accidental disclosure of sensitive chat existence and content.

Hidden chats must not leak through:

- collapsed sidebar label
- active conversation list
- title filter
- default message search
- default archive surface
- default counters or summaries that expose titles/snippets in ordinary UI

If hidden chat titles or snippets are shown, that must happen only inside a deliberate hidden-management surface the user explicitly opened.

### Search Leakage Rules

The backend must enforce visibility filtering for search.

Required:

- default `GET /api/chat/search` scope is active chats only
- archived chats only appear when explicitly included
- hidden chats do not appear in standard search by default

Do not rely on frontend post-filtering after retrieving broader search results.

### Logging Rules

Do not log hidden chat names, message snippets, or full tab payloads in normal error handling.

Safe logging:

- route/action name
- tab ID
- generic error string

Avoid:

- serialized tab payloads
- full titles/content for hidden items

### API Documentation Requirement

When implementing, update `docs/reference/api.md` so all new routes or visibility query modes explicitly state:

- `Auth: api-token.`
- `Authorization: Bearer <api-token>`
- default visibility behavior
- hidden/search filtering expectations

### Auth Regression Test Requirement

Add auth-routing coverage in [tests/auth_routing.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/tests/auth_routing.rs) for every new backend route or materially new query path.

Minimum expected coverage:

- archive route requires `api-token`
- hide route requires `api-token`
- restore route requires `api-token`
- archived/hidden list route or query path remains protected
- hidden-search route or query path remains protected

## UX Architecture

## Main Surfaces

### 1. Active Sidebar

This remains the main conversation rail.

Contains:

- `New Chat`
- title filter
- `Search Messages`
- recency groups for active chats only
- optional entry points to `Archived` and `Hidden`

Recommended additions:

- small management row under `Search Messages`
- two secondary pills:
  - `Archived`
  - `Hidden`

These should show counts.

### 2. Archived Surface

Recommended form:

- inline expandable sidebar section or flyout panel

Best choice for v1:

- expandable sidebar section below active groups

Rationale:

- easier to discover
- lighter than a modal
- visually consistent with current conversation rail

Contents:

- section header `Archived`
- count badge
- restore action
- optional move to hidden
- optional delete

### 3. Hidden Surface

Recommended form:

- dedicated privacy-focused flyout or modal, not always expanded inline

Rationale:

- hidden chats should not be casually visible
- a deliberate open/reveal flow is part of the privacy promise

Recommended UX:

1. User clicks `Hidden`
2. Flyout/modal opens with a restrained privacy-oriented presentation
3. Copy explains: `Hidden chats are kept out of normal sidebar and search`
4. Chats are listed with muted metadata
5. Each row supports:
   - `Restore`
   - `Archive instead`
   - `Delete`

Optional enhancement:

- require a second click `Reveal hidden chats`
- default surface shows only count and explanation until revealed

This is not true security, but it materially reduces casual shoulder-surfing exposure.

## UX Semantics

### Archive vs Hide vs Delete Copy

Use explicit language:

- `Archive`
  - `Remove from your main chat list. You can restore it anytime.`
- `Hide`
  - `Keep this chat, but remove it from normal sidebar and search surfaces.`
- `Delete`
  - `Permanently remove this chat and its messages.`

Never use ambiguous words like:

- `Close`
- `Remove`
- `Dismiss`

for these states.

### Empty States

Archived empty state:

- `No archived chats`
- `Archived chats leave your main list without being deleted.`

Hidden empty state:

- `No hidden chats`
- `Hide chats you want out of normal view.`

### Toasts

Recommended toasts:

- `Chat archived` with `Undo`
- `Chat hidden` with `Undo`
- `Chat restored`
- `Chat moved to hidden`
- `Chat moved to archive`

### Confirmation Rules

Do **not** confirm archive/hide.

Reason:

- they are reversible
- confirmation slows down the fast path

Do confirm delete.

## Visual Design Requirements

This feature must match existing glassmorphism and token-driven chat UI patterns in [docs/reference/ui-design-patterns.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/ui-design-patterns.md).

## Styling Direction

### Archive

Archive should feel calm and organizational.

Use:

- muted elevated surfaces
- subtle cool-gray or slate gradients
- low-saturation icons

Avoid:

- warning colors
- danger affordances

### Hidden

Hidden should feel deliberate and private, but not ominous.

Use:

- darker floating surface with stronger blur
- tighter borders
- slightly more focused glow treatment
- iconography suggesting conceal/reveal, not lock/security theater

Avoid:

- bright red
- hacker/vault aesthetics
- anything implying real encryption if it does not exist

### Suggested Tokens / Visual Treatment

Prefer existing tokens from:

- `static/css/tokens.css`
- `static/css/chat.css`
- `static/css/components.css`

For new classes:

- use `--surface-card-overlay` / `--surface-card-overlay-strong`
- use `--surface-card-line` / `--surface-card-line-strong`
- use `--shadow-surface` / `--shadow-floating`
- use `--radius-base`

### Motion

Use micro-motion only:

- reveal panels with small fade + translate
- action buttons with subtle hover lift
- count pills with gentle state change

Must add:

- `@media (prefers-reduced-motion: reduce)` overrides

### Light Theme

All new surfaces require `[data-theme="light"]` overrides.

Do not ship dark-theme-only hidden/archive styling.

## Information Architecture Recommendation

Implement the sidebar management stack as:

1. Active conversations
2. Search Messages entry point
3. Management pills row
   - `Archived`
   - `Hidden`
4. Archived inline section or flyout
5. Hidden deliberate reveal surface

This gives:

- discoverable archive
- intentionally less casual hidden access
- no confusion with delete

## Backend Architecture

## Schema Changes

File:

- [src/chat_storage.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/chat_storage.rs)

### Tabs Table

Add:

```sql
visibility TEXT NOT NULL DEFAULT 'active' CHECK (visibility IN ('active','archived','hidden'))
```

Recommended optional columns:

```sql
visibility_changed_at INTEGER
```

If using one timestamp only:

- update it whenever visibility changes

Do **not** add separate tables for archived/hidden tabs. Visibility is tab metadata, not a separate entity.

## Migration Strategy

Current storage initialization relies on `CREATE TABLE IF NOT EXISTS` and does not appear to have a generalized migration framework.

Required implementation:

1. Add a migration helper in `ChatStorage::open()`
2. Detect whether `tabs.visibility` exists
3. If missing, run:

```sql
ALTER TABLE tabs ADD COLUMN visibility TEXT NOT NULL DEFAULT 'active';
```

4. Optionally validate/fix invalid legacy rows

Recommended helper shape:

- `fn run_schema_migrations(conn: &Connection) -> Result<()>`
- call after `SCHEMA_SQL`

Do not rely on rebuilding the table from scratch for existing users.

## Rust Types

Update:

- `TabMeta`
- `ChatTabRow`

Add:

```rust
pub visibility: String
```

Optional:

```rust
pub visibility_changed_at: Option<i64>
```

If a string enum is used in the API, prefer introducing a Rust enum internally for correctness:

```rust
enum TabVisibility {
    Active,
    Archived,
    Hidden,
}
```

Serialize as lowercase strings for API compatibility.

## Storage Queries

### list_tabs

Current `list_tabs()` should be extended to support filtering by visibility.

Preferred approach:

- new storage method:
  - `list_tabs_by_visibility(visibilities: &[TabVisibility])`

This will support:

- active-only sidebar load
- archived management list
- hidden management list
- future combined views

### get_tab

`get_tab(id)` should still return any tab regardless of visibility, as long as the caller is authorized and knows the ID.

### update_tab_meta

Must write `visibility`.

### search

Search must accept allowed visibilities and filter by tab visibility in SQL.

Recommended search signature:

```rust
pub fn search(&self, query: &str, limit: usize, offset: usize, visibilities: &[TabVisibility]) -> Result<SearchResultsPage>
```

SQL must join `tabs` and filter by `t.visibility IN (...)`.

## API Design

Files:

- [src/web/api.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/api.rs)
- [docs/reference/api.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/api.md)

## Recommendation

Use both:

- metadata-based visibility updates
- convenience endpoints for semantic actions

This keeps the model clean while giving the frontend obvious verbs.

### Option A: Metadata patch only

Use:

- `PATCH /api/chat/tabs/{id}/meta`

with:

```json
{
  "visibility": "archived"
}
```

Pros:

- fewer endpoints

Cons:

- less explicit product semantics
- slightly weaker readability in frontend code

### Option B: Semantic endpoints plus metadata support

Recommended.

Add:

- `POST /api/chat/tabs/{id}/archive`
- `POST /api/chat/tabs/{id}/hide`
- `POST /api/chat/tabs/{id}/restore`

Optional:

- `POST /api/chat/tabs/{id}/move-to-hidden`
- `POST /api/chat/tabs/{id}/move-to-archive`

Suggested behavior:

- archive => `visibility = 'archived'`
- hide => `visibility = 'hidden'`
- restore => `visibility = 'active'`

Reason:

- easier for agents and future maintainers to reason about
- less fragile than building all flows around generic patch payloads
- easier to secure and test explicitly in the auth matrix

### List Endpoints

Current `GET /api/chat/tabs` should default to active chats only.

Add query options:

- `visibility=active`
- `visibility=archived`
- `visibility=hidden`
- `visibility=all`

Preferred behavior:

- no query param => `active`

Alternative:

- `?include_archived=1`
- `?include_hidden=1`

Recommendation: prefer a single `visibility` query model. It is cleaner and easier to document.

### Search Endpoint

Update `GET /api/chat/search` to accept:

- `visibility=active`
- `visibility=active,archived`
- `visibility=hidden`

Default:

- `active`

Do not default to all.

## Frontend Architecture

## State Model

Files:

- [static/js/core/app-state.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/core/app-state.js)
- [static/js/features/chat-state.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-state.js)

### Tab Object

Every tab object should include:

```js
visibility: 'active' | 'archived' | 'hidden'
```

Update:

- `newChatTab()`
- `normalizeChatTab()`
- `normalizeTabForSave()`

Default:

- `visibility: 'active'`

### Local UI State

Add dedicated UI state for management surfaces, for example:

```js
chat.visibilityUi = {
  archiveOpen: false,
  hiddenOpen: false,
  hiddenRevealed: false,
  activeSearchVisibility: ['active'],
};
```

This should stay frontend-only and not pollute persisted tab data.

## State Actions

Add explicit helpers in `chat-state.js`:

- `archiveChatTab(id)`
- `hideChatTab(id)`
- `restoreChatTab(id)`
- `setChatTabVisibility(id, visibility)`

Do not reuse `closeChatTab()` for these actions.

Expected behavior:

1. optimistic local update
2. rerender sidebar/search/header
3. persist via semantic endpoint or metadata patch
4. show toast with `Undo`

Undo semantics:

- archive undo => restore to `active`
- hide undo => restore to `active`

## Sidebar Rendering

File:

- [static/js/features/chat-sessions-sidebar.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-sessions-sidebar.js)

### Active List Filtering

The main grouped conversation list should only render `visibility === 'active'`.

Current `_groupTabsByRecency()` should only receive active chats, or should internally filter.

### New Management UI

Add below search:

- `Archived` pill/button with count
- `Hidden` pill/button with count

These should open their respective surfaces.

### Archived Rendering

Preferred v1:

- inline collapsible section or adjacent flyout

Each archived row should include:

- title
- lightweight metadata
- `Restore`
- context menu with:
  - `Restore`
  - `Hide`
  - `Delete`

### Hidden Rendering

Preferred v1:

- dedicated flyout/modal

Each hidden row should include:

- title
- reduced metadata
- `Restore`
- context menu with:
  - `Restore`
  - `Archive`
  - `Delete`

### Collapsed Label Behavior

Critical privacy fix:

- if active tab becomes hidden, immediately switch away
- collapsed label must never show a hidden chat name

If no active visible tab remains:

- show empty state or create/select another active chat

Do not allow hidden chats to remain selected in the visible shell.

## Header UX

File candidates:

- [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html)
- [static/js/features/chat-render.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-render.js)
- [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css)

### Add “Hide Now”

Add a compact header action for the active chat.

Requirements:

- visible but not visually dominant over send/composer actions
- accessible tooltip: `Hide this chat from normal view`
- keyboard accessible

Recommended style:

- small pill button
- eye-off style icon
- privacy-focused but not alarmist coloring

Avoid:

- tiny icon-only affordance with no tooltip/copy
- burying the main privacy action only in the row context menu

## Search UX

File:

- [static/js/features/chat-search.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-search.js)

### Default

Search only active chats.

### Archive Search

Allow an explicit archived inclusion control.

Recommended UI:

- small filter chips near the search summary:
  - `Active`
  - `Archived`

### Hidden Search

Do not search hidden chats in the normal search flyout.

Recommended approach:

- hidden chats have their own search input inside the hidden management surface

This preserves the privacy promise and avoids surprising snippet exposure.

## Delete / Trash Interaction

Current delete/trash behavior should remain, but the role of that system must be clarified.

### Delete remains destructive

- `Delete` still calls backend delete
- undo remains a best-effort convenience

### Archive/Hidden do not use trash

Do not send archived/hidden chats through:

- `chat.tabTrash`
- `closeChatTab()`
- delete endpoints

Reason:

- that system is not durable
- it has incorrect semantics for non-destructive storage

## Implementation File Map

### Backend

- [src/chat_storage.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/chat_storage.rs)
  - schema
  - migrations
  - visibility-aware list/search/update methods

- [src/web/api.rs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/api.rs)
  - query parsing
  - new semantic endpoints
  - visibility-aware list/search behavior

### Frontend

- [static/js/features/chat-state.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-state.js)
  - tab visibility actions
  - optimistic updates
  - fallback active-tab selection

- [static/js/features/chat-sessions-sidebar.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-sessions-sidebar.js)
  - archive/hidden entry points
  - active list filtering
  - archived and hidden rendering
  - updated context menu items

- [static/js/features/chat-search.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-search.js)
  - visibility-scoped search
  - archive inclusion controls

- [static/js/core/app-state.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/core/app-state.js)
  - UI state for archive/hidden panels

- [static/index.html](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html)
  - sidebar management containers
  - hidden surface container
  - header hide action mount point if needed

- [static/css/chat.css](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/css/chat.css)
  - management pills
  - archived section styles
  - hidden flyout/modal styles
  - reduced-motion and light-theme variants

## Suggested Context Menu Structure

### Active Chat

- `Rename`
- `Pin/Unpin`
- `Archive`
- `Hide`
- separator
- `Export JSON`
- `Export Markdown`
- `Duplicate`
- separator
- `Delete`

### Archived Chat

- `Restore`
- `Hide`
- separator
- `Export JSON`
- `Export Markdown`
- `Duplicate`
- separator
- `Delete`

### Hidden Chat

- `Restore`
- `Archive`
- separator
- `Export JSON`
- `Export Markdown`
- `Duplicate`
- separator
- `Delete`

This structure should be consistent across surfaces.

## Fallback Active Tab Logic

When a currently active chat is archived or hidden:

1. try next active chat
2. else try previous active chat
3. else show empty state with `New Chat`

Do not automatically open archived or hidden chats after a privacy action.

## Data Contract Details

## `TabMeta`

Add:

```json
{
  "visibility": "active"
}
```

### `ChatTabRow`

Add:

```json
{
  "visibility": "active"
}
```

### Search Result

Optional but recommended:

```json
{
  "visibility": "active"
}
```

This is useful if the same renderer is later used for archive-aware search.

## Edge Cases

### Last Active Chat Hidden

- active view becomes empty
- empty state should not mention trash as the primary recovery mechanism
- update empty-state copy to mention archived/hidden recovery if appropriate

### Hidden Chat With Matching Search Snippet

- must not appear in default search

### Drag Reorder

- only active chats participate in normal reorder
- archived/hidden ordering can use:
  - existing `tab_order`, or
  - recency by `updated_at`

Recommendation:

- keep active reorder only
- archived/hidden can sort by latest `visibility_changed_at` or `updated_at`

### Pinning

Recommendation:

- pins matter only for `active`
- when archiving/hiding a pinned chat:
  - preserve `pinned = true` in storage
  - ignore it outside active rendering

Restoring it should return it to pinned position.

### Duplicate

Duplicating an archived/hidden chat should create a new `active` chat by default.

Reason:

- duplicate is usually an action to resume or branch work

## Accessibility

Required:

- keyboard access to all archive/hide/restore actions
- visible focus states
- reduced-motion support
- clear ARIA labels on header and management buttons
- sufficient contrast in both themes

Suggested ARIA copy:

- `Archive chat`
- `Hide chat from normal view`
- `Restore archived chat`
- `Reveal hidden chats`

## Testing Requirements

## Rust / API

Add tests for:

- migration adds `visibility` column to older DBs
- default list returns only active tabs
- archive/hide/restore endpoints update visibility correctly
- search excludes archived/hidden by default
- search includes archived when requested
- hidden tabs are excluded from standard search paths
- new visibility routes reject missing/invalid bearer when `api-token` is configured
- expanded list/search visibility query paths do not bypass route auth

## Frontend Unit/Behavior

Add or extend UI tests for:

- archiving removes row from active sidebar and shows in archived surface
- hiding removes row from active sidebar and collapsed label
- restore returns chat to active sidebar
- default message search ignores archived/hidden chats
- archived inclusion control works
- hidden chats only searchable inside hidden surface if implemented
- active-tab fallback works when current chat is hidden

Likely test file:

- [tests/ui/chat/chat-shell.spec.js](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/tests/ui/chat/chat-shell.spec.js)

Additional test coverage may warrant a new spec file such as:

- `tests/ui/chat/chat-visibility.spec.js`

## Screenshot / Docs Requirements

Because this is a user-visible chat feature, update docs in the same PR.

Must update:

- [docs/reference/chat.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/chat.md)
- [docs/reference/api.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/reference/api.md)

If visuals materially change, update screenshot harness:

- [tests/ui/capture.mjs](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/tests/ui/capture.mjs)
- [tests/ui/README.md](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/tests/ui/README.md)

Recommended screenshot coverage:

- active sidebar with archive/hidden pills
- archived section open
- hidden surface open
- header `Hide Now` action visible

## Suggested Rollout Plan

### Phase 1: Data Model and API

- add schema column and migration
- add visibility fields to Rust structs
- make `GET /api/chat/tabs` visibility-aware
- add archive/hide/restore endpoints
- make search visibility-aware

### Phase 2: Frontend Active Filtering

- teach tab model about `visibility`
- filter active sidebar to active chats
- add archive/hide/restore actions in state layer
- fix collapsed label leakage

### Phase 3: Archive Surface

- add archived UI
- add restore flow
- add archived search inclusion toggle

### Phase 4: Hidden Surface

- add `Hide Now`
- add hidden management flyout/modal
- add hidden-specific reveal/search behavior

### Phase 5: Tests, Docs, Screenshots

- add Playwright coverage
- update reference docs
- regenerate screenshots if needed

## Validation Checklist For Implementing Agent

Before finishing the PR, verify:

- hidden chats are not visible in normal sidebar
- hidden chats are not visible in collapsed label
- hidden chats are not visible in normal search
- archived chats are not visible in normal sidebar
- restore from archived/hidden returns chats to active
- delete still works and remains distinct
- pinning behavior survives restore
- light theme overrides exist
- reduced-motion overrides exist
- all new fetch calls include auth headers via existing helpers
- all new backend routes preserve `check_api_token` parity with current chat endpoints
- `tests/auth_routing.rs` covers the new route/query surface
- docs and tests are updated

## Reviewer Acceptance Checklist

Use this section during review. The feature should not be considered complete unless all items below are true.

### Product Behavior

- There are three durable visibility states in the implementation: `active`, `archived`, and `hidden`
- Archive and hidden are non-destructive and do not use the delete/trash path
- Delete remains clearly separate and destructive
- The active chat can be hidden quickly from an obvious UI action
- Archived chats are restorable from a clear management surface
- Hidden chats are restorable from a deliberate privacy-oriented surface

### Privacy / Leakage

- Hidden chats do not appear in the main active sidebar
- Hidden chats do not appear in the collapsed sidebar label
- Hidden chats do not appear in the default title filter flow
- Hidden chats do not appear in default cross-chat message search
- Archived chats do not appear in the main active sidebar
- Hidden chat titles/snippets are only shown inside an explicitly opened hidden-management surface

### API / Security

- Every new visibility-related route requires `api-token` when configured
- Every new frontend fetch path sends the existing auth headers
- No new route bypasses `check_api_token(...)`
- Visibility filtering is enforced server-side for search/listing behavior
- `docs/reference/api.md` documents the new routes/query modes and their auth requirements

### UX / Design Quality

- Archive, Hide, and Delete are clearly distinguished in copy and placement
- The new UI matches existing chat styling and token usage
- Light theme overrides exist
- Reduced-motion overrides exist
- Empty states and toasts explain where chats went and how to recover them

### Data / Restore Semantics

- Existing chats migrate safely to `visibility = 'active'`
- Restoring archived/hidden chats returns them to active visibility
- Pin state survives archive/hide/restore
- Reordering behavior remains coherent for active chats
- Duplicating an archived/hidden chat produces an active chat unless intentionally designed otherwise

### Test Coverage

- Rust tests cover migration and visibility-aware storage/search behavior
- Auth-routing tests cover every new route or expanded query path
- UI tests cover archive, hide, restore, and search filtering behavior
- Documentation updates are included in the same PR as the feature

## Recommendation

The correct architecture is **one shared visibility foundation** with three states:

- `active`
- `archived`
- `hidden`

Do not build archive first and then bolt on a second unrelated hidden system later. Model both now, even if the UI ships in phases. That avoids repeat migrations, repeated API churn, and UX inconsistency.

The premium UX target should be:

- archive is easy and organizational
- hide is fast and privacy-focused
- delete is clearly destructive
- hidden chats stay out of normal sight unless the user deliberately goes to them

That is the simplest model that actually satisfies the user problem.
