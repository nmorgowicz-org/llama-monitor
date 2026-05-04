# Context Window Card For Chat Plan

Date: 2026-05-02

## Purpose

This document defines a follow-up plan for the inference metrics context window card so it better reflects the current product reality:

1. The app now has a functional multi-tab chat system.
2. For sessions launched outside the app, we often cannot display true live context usage from the model runtime.
3. The current dashboard card still assumes a llama-server-centric interpretation of context and over-emphasizes metrics like "Peak observed" that are no longer useful.

The goal of this work is to redesign the context card so it can surface chat-relevant context information in a premium, modern, flexible way without pretending that unavailable runtime metrics are available.

This document is intended for a future AI agent implementing the next feature branch, alongside the broader architecture work in [`docs/architecture/20260502-window_architecture_cleanup_plan.md`](../architecture/20260502-window_architecture_cleanup_plan.md).

## Product Problem

The current card is optimized for a narrow interpretation of context:

- it shows a live usage rail when `llama-server` exposes current context tokens
- otherwise it falls back to "peak observed only"
- it uses a second rail for "Peak observed"

Current implementation:

- HTML card shell in [`static/index.html`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html:337)
- update logic in [`static/js/features/dashboard-ws.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/dashboard-ws.js:503)

Problems with the current design:

1. "Peak observed" is low-value and not actionable.
2. When live context is unavailable, the card becomes a weak fallback rather than a meaningful product surface.
3. The card does not leverage the app’s strongest internal source of context-related information: chat tabs, message token metadata, compaction state, and context pressure inside the chat experience.
4. The current card is designed around one runtime stream, not around `0..N` chats.

## Current Relevant Code Reality

### Runtime context data

The dashboard currently reads:

- `context_capacity_tokens`
- `context_live_tokens`
- `context_live_tokens_available`
- `context_high_water_tokens`

These come from llama metrics and slot polling in Rust:

- [`src/llama/poller.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/llama/poller.rs:206)
- [`src/llama/metrics.rs`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/llama/metrics.rs:26)

### Chat-side context data

The chat system already tracks useful information per message/tab:

- `input_tokens`
- `output_tokens`
- per-message `ctx_pct`
- per-tab `lastCtxPct`
- compaction tombstones with `ctx_pct_before`
- auto-compact settings and thresholds

Relevant files:

- [`static/js/features/chat-transport.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-transport.js:292)
- [`static/js/features/chat-render.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-render.js:267)
- [`static/js/features/chat-params.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-params.js:160)

### Chat tab count

There does not appear to be an explicit hard limit on chat tabs today.

New tabs are added by appending to `window.chatTabs`:

- [`static/js/features/chat-state.js`](/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/chat-state.js:84)

That means this card should be designed for `0..N` tabs and should not assume only one or two concurrent chats.

## Design Goals

The redesigned card should:

1. remain useful whether or not live runtime context is exposed
2. reflect chat context pressure as a first-class app concept
3. work with zero chats, one chat, or many chats
4. stay premium and visually rich, not become a plain diagnostic table
5. preserve at-a-glance scanning value on desktop
6. avoid lying about unavailable runtime information

## Non-Goals

This work is not:

- a chat-tab management overhaul
- a full dashboard redesign
- a rewrite of all context metric plumbing
- a requirement to compute exact token truth for externally launched sessions

## Core Product Decision

The context card should become a hybrid "Context Intelligence" card rather than a narrow "runtime context rail."

The card should prefer the best available source in this order:

1. true live runtime context from llama metrics when available
2. chat-derived context pressure from active/persisted chat tabs
3. capacity-only fallback when only model context size is known
4. empty/educational state when none of the above are available

This is more honest and more useful than the current "peak observed only" fallback.

## Proposed Naming

Recommended card title options:

- `CONTEXT INTELLIGENCE`
- `CONTEXT PRESSURE`
- `CONTEXT WINDOW`

Recommendation:

- Keep the user-facing label as `CONTEXT WINDOW` for continuity.
- Internally treat it as a richer context-intelligence card.

## Data Model Recommendation

Introduce a small derived frontend view-model for the card, separate from raw llama metrics.

Suggested shape:

```js
{
  mode: 'live-runtime' | 'chat-derived' | 'capacity-only' | 'empty',
  viewMode: 'gauge' | 'fleet',       // persisted user preference
  capacityTokens: number,
  runtimeLiveTokens: number | null,
  runtimeLivePct: number | null,
  activeChatCount: number,
  pressuredChatCount: number,
  busiestChat: {
    id: string,
    name: string,
    ctxPct: number | null,
    inputTokens: number,
    outputTokens: number,
    autoCompact: boolean,
    lastMessageTimestamp: number,     // epoch ms, for recency sorting
  } | null,
  chatSummaries: Array<{              // sorted by lastMessageTimestamp desc
    id: string,
    name: string,
    ctxPct: number | null,
    state: 'idle' | 'warm' | 'warning' | 'critical' | 'unknown',
    lastMessageTimestamp: number,     // epoch ms
    isStale: boolean,                 // true if lastMessageTimestamp > 7 days ago
  }>,
  staleChatCount: number,             // count of chats > 7 days old
  aggregateChatPressure: {
    avgPct: number | null,
    maxPct: number | null,
  },
  note: string | null,
}
```

This model should be derived in JS from:

- current llama metrics
- `window.chatTabs`
- active tab metadata
- compaction configuration

## Two UI Options

## Option A: Hero Gauge + Chat Strip

### Summary

This option keeps one dominant "hero" readout for the single most important context number, with a secondary strip for chat distribution.

### Visual Structure

Top:

- oversized percentage or token figure
- compact state chip: `live`, `derived`, `capacity only`, `idle`
- subtle radial or arc gauge behind the number

Middle:

- one-line explanatory subtitle
- runtime line when available: `42K / 128K live`
- otherwise derived line: `3 chats tracked · 1 under pressure`

Bottom:

- horizontal mini-strip of chat pills or tiny bars
- each pill represents a tab
- color-coded by context pressure
- if too many tabs exist, show first N plus `+X more`

### Best Use Case

This option is best if:

- you want the card to stay dashboard-like
- you want maximum legibility at a glance
- you want a premium "single hero metric" look

### Motion / Animation

When live runtime context is available:

- slow breathing glow on the gauge ring
- animated gradient fill along the arc
- micro pulse when thresholds are crossed

When chat-derived:

- subtle wave shimmer across active chat pills
- warning tabs get a restrained ember-like inner glow

Idle state:

- low-energy glass card with faint ambient gradient
- no spinner-style motion

### Strengths

- strongest at-a-glance readability
- easiest transition from current card
- works well even with many chats via summarized strip

### Weaknesses

- less detail-rich for comparing many chats
- chat distribution is secondary rather than primary

## Option B: Context Fleet Board

### Summary

This option turns the card into a compact multi-chat control surface. Instead of one big rail, it shows chat-aware context cards inside the dashboard card itself.

### Visual Structure

Top:

- card title
- chip showing mode: `chat-derived` or `live-runtime`
- summary text such as `4 chats · 1 critical · 2 auto-compact`

Body:

- a compact stack or grid of mini rows, one per top-priority chat
- each row contains:
  - tab name
  - miniature pressure bar
  - context percentage or `unknown`
  - compaction badge if enabled

Footer:

- one aggregate line:
  - `active chat pressure avg 61%`
  - or `runtime live 42K / 128K`

If there are more chats than fit:

- show top 4 by pressure
- final row becomes `+7 more chats`

### Best Use Case

This option is best if:

- you want the card to reflect the chat system more explicitly
- you expect users to run multiple parallel conversations
- you want the dashboard to feel more like a coordination cockpit

### Motion / Animation

- each mini row can softly animate its pressure fill
- active chat rows can have a low-amplitude "signal scan" gradient
- warning/critical rows can use restrained edge glow rather than hard blinking

### Strengths

- best expression of multi-chat context pressure
- future-friendly if chat count becomes more important
- easier to grow into richer tab management later

### Weaknesses

- less clean as a single-metric hero card
- more complex visually
- higher implementation complexity

## Recommendation

Recommended direction: **Both Option A and Option B, with user-toggle**

The card should support both views, with a segmented control toggle in the card header. See the **Dual View Mode** section above for details.

Default view: **Option A (Gauge)** — it preserves strong dashboard scanability and is the easiest transition from the current card. But users who prefer the fleet-level view can switch to Option B (Fleet) at any time.

Reasoning:

1. Option A preserves strong dashboard scanability and works as a single-metric hero card.
2. Option B best expresses multi-chat context pressure and is future-friendly.
3. Different users have different mental models — the toggle lets each user pick their preference.
4. The preference persists across reloads so the user never has to re-toggle.

## Recommended Final Behavior

### Mode 1: Live Runtime Context Available

Display:

- hero number = live percentage
- subtitle = `NNK / CCCK live`
- bottom chat strip = optional chat context pills if chat tabs exist

Interpretation:

- runtime remains authoritative
- chat is supporting context, not the primary metric

### Mode 2: Runtime Context Unavailable, Chat Context Available

Display:

- hero number = busiest chat `ctx_pct` or aggregate derived percentage
- chip = `derived`
- subtitle = `Based on tracked chat conversations`
- bottom strip = per-chat pills/bars

Interpretation:

- the card is still useful without pretending to know server truth

### Mode 3: Capacity Only

Display:

- hero number = capacity tokens only, or a softer `—`
- subtitle = `Context size known, live usage unavailable`
- show one understated capacity bar
- do not show "peak observed"

### Mode 4: No Active Chats, No Runtime Context

Display:

- educational empty state
- example copy:
  - `Start a chat or attach to a server to track context pressure`

## What To Remove

Remove from the current card:

1. the `Peak observed` rail
2. the phrase `peak observed only`
3. the notion that the fallback state is still primarily about runtime historical peaks

The card should become clearer and more intentional.

## Visual Language Guidance

This work should feel premium and modern, not utilitarian.

### Styling Direction

- layered glass surface
- richer gradient depth than the current plain rails
- restrained neon accents only at warning thresholds
- clean typography hierarchy
- animated fills that feel ambient, not arcade-like

### Color Semantics

- `idle`: cool slate / muted cyan
- `warm`: blue-green
- `warning`: amber-gold
- `critical`: ember red
- `derived`: use a distinct but calm tint, such as electric teal

### Animation Guidance

Avoid:

- hard blinking
- loading-spinner energy
- overly noisy particle effects

Prefer:

- breathing glows
- slow gradient drift
- lightweight pressure-fill shimmer
- threshold crossing pulses

## Dual View Mode: Toggle Between Option A and Option B

Both Option A (Hero Gauge + Chat Strip) and Option B (Context Fleet Board) should be available to the user, with an easy way to swap between them.

### Why

Different users have different mental models. Some prefer a single dominant metric with supporting context (Option A). Others prefer a fleet-level view that treats each chat as equally important (Option B). Rather than choosing one, the card should let the user pick.

### Toggle Control

Use a **segmented control** in the card header, positioned to the right of the card title:

```
┌──────────────────────────────────────────┐
│  CONTEXT WINDOW    ┌───────┬───────┐    │
│                    │ Gauge │ Fleet │    │
│                    └───────┴───────┘    │
└──────────────────────────────────────────┘
```

### Segmented Control Design

- Two segments: `Gauge` and `Fleet`
- Active segment: filled with a subtle gradient fill matching the card's accent color
- Inactive segment: transparent background, muted text
- Smooth transition between views (200ms crossfade, no jarring layout shift)
- Minimum width: ~140px total, each segment ~60px
- Touch-friendly: 44px minimum tap target height

### View Transition Behavior

- Switching views should feel like a mode change, not a page reload
- Use a quick crossfade: outgoing view fades to 0 opacity, incoming view fades from 0 to 1
- Maintain card height during transition to avoid layout shift — both views should reserve similar vertical space
- If one view is significantly shorter, add subtle bottom padding to match the taller view's height

### Persistence

The user's view preference should persist across page reloads:

- Save to `ui-settings.json` under a new key: `contextCardView: 'gauge' | 'fleet'`
- Load on bootstrap, apply before first render to avoid flash of wrong view
- Default to `'gauge'` (Option A) if no preference is set

### Implementation Note

Both views should render their DOM simultaneously but toggle visibility via CSS classes. This avoids re-rendering cost on toggle and enables smooth crossfade transitions.

```css
.context-card-view {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    transition: opacity 200ms ease;
}

.context-card-view.hidden {
    opacity: 0;
    pointer-events: none;
}

.context-card-view.active {
    opacity: 1;
}
```

## Chat Ordering: Recency-First

Chats should be ordered by **date of last message** (most recent first), not by context pressure.

### Why

Recency is a better signal of what the user cares about right now. A chat with 90% context pressure from 10 days ago is less relevant than a chat with 20% pressure that was just active.

### Rules

1. Sort all chats by `lastMessageTimestamp` descending.
2. Chats with no messages yet (empty tabs) sort to the end.
3. Chats older than **7 days** are considered "stale" and hidden behind a collapsible `+X more` overflow.
4. The **most recent chat** (even if stale) always appears as the primary context presentation — never hide the freshest chat.
5. Within the `+X more` overflow, stale chats remain sorted by recency.

### Overflow Behavior

- Show the top N non-stale chats (see N below).
- If there are stale chats, append a `+X more` pill/row that expands inline when clicked.
- Clicking `+X more` reveals the stale chats in a compact inline expansion.
- The expansion should collapse when the user clicks elsewhere or clicks the pill again.

### Suggested N

- desktop: 4 to 6 summary items (before overflow)
- narrow layouts: 3 to 4 summary items (before overflow)

## Handling Many Chats

The card should plan for large tab counts even if the rest of the chat UI does not yet fully optimize for them.

Recommended rules:

1. Sort chats by recency (see Chat Ordering above).
2. Surface only the top N non-stale chats in-card.
3. Summarize stale chats as `+X more` (click to expand).
4. Never let the card height explode because of tab count.

## Architecture Guidance

This feature should be implemented with the architecture cleanup work in mind, not against it.

Recommended ownership:

1. Derive context-card data in a dedicated context module, not directly inside `dashboard-ws.js`.
2. Keep raw runtime metric reading separate from chat-derived context aggregation.
3. Keep rendering in the dashboard render layer or a context-card-specific render helper.
4. Avoid adding more `window.*` coupling to ship this feature.

Suggested future modules:

- `context-card-state.js`
- `context-card-derive.js`
- `context-card-render.js`

These names are examples, not requirements.

## Implementation Plan

## Phase 1: Inventory Current Inputs

Tasks:

1. List current runtime fields used by the card.
2. List current chat-tab and message fields that can support derived context.
3. Confirm whether `ctx_pct` is present consistently enough across chat flows to support card rendering.

Exit Criteria:

- the derived data sources are documented

## Phase 2: Define Derived Context Model

Tasks:

1. Create a derived JS model for the context card.
2. Define mode-selection logic:
   - live-runtime
   - chat-derived
   - capacity-only
   - empty
3. Decide what aggregate chat number to surface:
   - busiest chat
   - average across chats
   - weighted active-chat signal

Recommendation:

- surface busiest-chat `ctx_pct` as the hero when runtime is unavailable
- use aggregate counts and secondary pills for the rest

## Phase 3: Redesign Card Markup (Both Views + Toggle)

Tasks:

1. Replace the current dual-rail markup.
2. Add structure for **both** Option A and Option B views, stacked with CSS toggle classes.
3. Add segmented control in card header (`Gauge` / `Fleet`).
4. Keep IDs and render hooks coherent and explicit.
5. Wire toggle to save `contextCardView` to `ui-settings.json` on change.
6. Load `contextCardView` from `ui-settings.json` on bootstrap, apply before first render.

Important:

- do not grow the dashboard row height excessively
- preserve responsive behavior
- both views should reserve similar vertical space to avoid layout shift on toggle

## Phase 4: Build New Rendering Logic

Tasks:

1. Move context-card-specific rendering out of the current inlined `updateContextMetrics()` shape if practical.
2. Render states cleanly for all four modes.
3. Add premium animation classes based on severity and data mode.
4. Implement recency-based sort: sort `chatSummaries` by `lastMessageTimestamp` descending.
5. Implement stale chat overflow: chats > 7 days old hidden behind `+X more` expandable pill.
6. Implement toggle handler: switch `viewMode`, crossfade views, persist to `ui-settings.json`.
7. Ensure both views render simultaneously (CSS visibility toggle) for smooth transitions.

## Phase 5: Manual And E2E Validation

Scenarios:

1. Local spawned session with live context available
2. Attached session with capacity but no live context
3. No chats, no live runtime context
4. One chat with low pressure
5. One chat near compaction threshold
6. Many chats with mixed pressure
7. Toggle between Gauge and Fleet views — verify smooth crossfade and no layout shift
8. Verify view preference persists across page reload
9. Chats with messages > 7 days old — verify they are hidden behind `+X more`
10. Click `+X more` — verify stale chats expand inline, sorted by recency
11. Mixed stale and fresh chats — verify fresh chats appear first, stale in overflow

## Open Product Questions

These do not block writing the feature branch, but they should be consciously answered during implementation:

1. Should the hero derived number represent:
   - most recent tab (recency-based, since we sort by last message)
   - busiest tab
   - aggregate across tabs
2. Should chats with unknown `ctx_pct` still appear in the summary strip as `unknown`?
3. Should the card count only tabs with non-system messages, or all tabs?
4. Should auto-compact-enabled chats get a reassuring badge to signal they are self-managing?
5. Should the `+X more` overflow expand inline within the card, or as a popover/dropdown?

Recommendation:

1. Hero = most recent tab when runtime unavailable (aligns with recency-first ordering)
2. Unknown tabs can appear, but sorted last
3. Count only chats with actual conversational content
4. Yes, show an understated auto-compact badge
5. Inline expansion within the card — keeps context local, no popover drift

## Success Criteria

This work is successful if:

1. the context card is still valuable when runtime live context is unavailable
2. the card reflects the existence of multiple chat conversations
3. the card looks premium and intentional
4. the fallback state is honest and not confusing
5. the implementation does not deepen the existing `window.*` architecture debt

The target is a context card that feels like a modern product feature, not a degraded metric placeholder.
