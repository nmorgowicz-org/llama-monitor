# Modern UI Reset Reference

**Date:** 2026-04-20  
**Status:** Active reference for the rebuilt web UI stylesheet  
**Primary files:** `static/index.html`, `static/style.css`, `static/app.js`

## Why This Reset Exists

The previous stylesheet had accumulated several partially implemented UI phases, duplicated legacy compatibility blocks, conflicting selectors, and generated fragments. The result was a mixed cascade where some modern card styles applied while navigation, sidebar, and dashboard layout fell back to browser defaults.

The reset removes that historical layering and establishes one coherent baseline for a modern desktop-class monitoring app.

## Product Direction

Llama Monitor should feel like a premium 2026 local infrastructure console:

- Dense enough for operators and model runners who need live metrics.
- Polished enough to feel like a native desktop app rather than a debug page.
- Dark by default, with glassy surfaces, crisp borders, and restrained gradients.
- Explicit about local vs. remote mode and what metrics are available.
- Calm when idle; high-contrast only for actionable state changes.

## Visual System

### Palette

- Base background: near-black graphite, `#0f1115`.
- Surface: layered charcoal, `#16191e`, `#1f232a`, `#2a2f3a`.
- Primary accent: indigo to violet to cyan gradient.
- Success: emerald/teal.
- Warning: amber/orange.
- Error: rose/red.

The UI should not become a one-hue purple app. Primary gradients are reserved for active actions, selected states, progress, and subtle focus treatments.

### Typography

- Body/display: Inter.
- Metrics/logs: Fira Code.
- Labels use uppercase sparingly for metric labels and table headings.
- Letter spacing stays neutral or mildly positive for labels only.

### Shape And Depth

- Controls: 8px radius.
- Panels and cards: 12-20px radius.
- Cards use subtle gradient surfaces and soft depth.
- Avoid nested visual cards unless the inner element is a repeated item or modal content.

## Layout Model

The app shell is:

1. Endpoint health strip.
2. Top navigation bar.
3. Fixed desktop sidebar.
4. Main content area offset from the sidebar.

The main server page uses:

- A full-width dashboard header with endpoint attach controls.
- A full-width control bar for preset actions.
- Full-width metric sections.
- Metric cards in responsive grids.
- Tables inside `table-wrap` containers for horizontal overflow.

On mobile/tablet, the sidebar becomes a horizontal nav and content returns to one column.

## CSS Structure

`static/style.css` is intentionally organized in this order:

1. Design tokens.
2. Base/reset styles.
3. Endpoint strip and top navigation.
4. Sidebar.
5. Page and dashboard layout.
6. Cards, metric widgets, and tables.
7. Buttons and controls.
8. Modals and settings.
9. Chat, logs, toasts.
10. Secondary panels: analytics, export, personalization, shortcuts.
11. State colors.
12. Responsive rules.

Future UI work should extend these sections rather than reintroducing broad legacy compatibility blocks.

## Interaction Rules

- App startup must not probe or attach to saved endpoints.
- Endpoint connection attempts happen only after explicit user action:
  - Start a model preset.
  - Attach to an endpoint.
- Remote host metrics are optional capability upgrades, not assumed.
- Capability-aware empty states should hide unavailable host metric sections instead of showing broken tables.

## Implementation Notes

- `index.html` must not contain duplicate IDs.
- `app.js` should preserve current class names when updating metric state.
- Sidebar navigation uses `.sidebar-btn`, not the old `.tab-btn` system.
- The Rust app embeds static assets at compile time, so UI CSS/HTML changes require rebuilding/restarting the app before browser validation.

## Future Improvements

- Split CSS into smaller source files if a build step is introduced.
- Add Playwright visual regression coverage for desktop and mobile.
- Add explicit no-session/idle dashboard empty state.
- Convert text+emoji buttons to a consistent icon system.
- Add a theme preference layer only after the core dark theme is stable.

## Implementation Guide For Future Agents

This section is the practical contract for future UI work. Read it before changing `static/index.html`, `static/style.css`, or `static/app.js`.

### Current Architecture

The UI is intentionally plain HTML/CSS/JS. There is no frontend build step, component compiler, or CSS preprocessor. The Rust backend embeds static assets at compile time through `src/web/static_assets.rs`, so any static asset change requires rebuilding or restarting the Rust app before browser validation.

The app shell is structural, not decorative:

```html
<div class="endpoint-health-strip">...</div>
<div class="top-nav-bar">...</div>
<div class="sidebar-nav">...</div>
<main class="content-area">
  <div class="page active" id="page-server">...</div>
  <div class="page" id="page-chat">...</div>
  <div class="page" id="page-logs">...</div>
</main>
```

Do not add another wrapper around the entire app unless the layout model is deliberately changed. Do not put `.content-area` inside `.sidebar-nav` or vice versa.

### Navigation And Tabs

Yes, the app has tabs, but they are page tabs controlled by the sidebar, not a separate old tab bar.

Canonical tab model:

- Tab buttons use `.sidebar-btn`.
- Tab pages use `.page`.
- Page IDs follow `page-<name>`.
- `switchTab(name)` in `static/app.js` owns page switching.
- The active tab gets `.active` on both the `.page` and the matching `.sidebar-btn`.

Current top-level pages:

- `server`: main attach/start/metrics dashboard.
- `chat`: chat interface.
- `logs`: server log stream.

The sidebar also has modal launchers for Sessions, Models, and Settings. Those are commands, not page tabs, unless a future change intentionally promotes them into pages.

If adding a true page tab:

1. Add a sidebar button with `onclick="switchTab('newname')"`.
2. Add `<div class="page" id="page-newname">...</div>` under `.content-area`.
3. Do not create `id="tab-newname"` or `.tab-btn`; those belong to the removed legacy system.
4. Check that no duplicate IDs exist:

```bash
rg -o 'id="[^"]+"' static/index.html | sort | uniq -c | awk '$1 > 1'
```

### Secondary Tabs

Secondary tabs are allowed inside modals or panels, such as Settings tabs. They should not reuse `.page` or `.sidebar-btn`.

Settings tabs use:

- `.settings-tabs`
- `.settings-tab`
- `.settings-pane`

If adding another secondary tab group, use a unique prefix, for example:

- `.models-tab`
- `.models-pane`

Do not create broad global `.tab` styles that affect unrelated controls.

### Animation Policy

Animations should be present but restrained. This is a monitoring app; animation should clarify state, not compete with metrics.

Allowed animation categories:

- Hover lift on cards/buttons: 1-2px translate, 120-180ms.
- Focus glow on inputs: subtle shadow, 120-180ms.
- Modal entry: opacity and slight scale/translate, 140-220ms.
- Toast entry/exit: translate from bottom/right and fade, 160-240ms.
- Progress bars: width transition, 200-400ms.
- Status pulse: only for active remote-agent or connection activity.

Avoid:

- Infinite decorative background animation.
- Large page transitions.
- Text sliding around while metrics update.
- Animating table row layout on every websocket update.
- Heavy blur or shadow transitions on large scrolling containers.

Recommended future token additions:

```css
:root {
  --motion-fast: 140ms;
  --motion-base: 180ms;
  --motion-slow: 260ms;
  --ease-standard: cubic-bezier(0.2, 0, 0, 1);
  --ease-emphasized: cubic-bezier(0.16, 1, 0.3, 1);
}
```

If adding animations, define keyframes near the component section that uses them or in a dedicated `Motion` section near the design tokens. Keep names specific, such as `toast-in`, `modal-in`, or `status-pulse`.

### Component Conventions

#### Buttons

Use existing button primitives:

- `.btn`: normal button.
- `.btn-sm`: smaller inline action.
- `.btn-primary`: primary command.
- `.btn-start`: primary start action.
- `.btn-stop`: stop action.
- `.btn-attach`: attach/connect action.
- `.btn-kill`: destructive kill action.

Buttons should be real `<button>` elements unless they navigate to a URL.

Future icon work should replace emoji with a small inline SVG/icon system. Until then, emoji is tolerated for existing controls but should not spread into dense metric tables.

#### Cards

Use `.widget-card` for metric cards and table panels. Avoid nested `.widget-card` inside `.widget-card`.

Metric cards use:

```html
<div class="widget-card widget-metric">
  <div class="widget-metric-label">Prompt Speed</div>
  <div class="widget-metric-value" id="m-prompt">—</div>
</div>
```

JavaScript must preserve `widget-metric-value` when changing classes.

#### Tables

Tables should be wrapped:

```html
<div class="table-wrap widget-card">
  <table class="gpu-table">...</table>
</div>
```

Use table layout for dense comparable metrics. Do not replace GPU/system tables with individual cards unless the data shape changes substantially.

#### Modals

Modals use:

- `.modal-overlay`
- `.modal`
- `.modal-header`
- `.modal-body`
- `.modal-buttons`

Modal overlays may be shown by adding `.active` or by existing JS setting `display: block`. The stylesheet supports both for compatibility.

Do not put top-level pages inside modals. Do not put modals inside `.content-area`; they should remain body-level siblings so fixed positioning works.

#### Toasts

Toasts live in the single `#toast-container`. There must only be one toast container.

Toasts should communicate completed actions, failures, or next steps. Avoid using toasts for constantly updating metrics.

### State And Capability Rendering

The UI is capability-aware. This matters for local vs remote workflows.

Core states:

- No active session: idle dashboard, no endpoint probing.
- Local spawn: inference + host metrics available.
- Local attach: inference available; host metrics may be limited.
- Remote attach: inference available; host metrics require remote agent.

Startup must be passive. The app may show persisted settings, but must not connect, probe, or autostart a remote agent until the user explicitly starts or attaches.

User-triggered connection points:

- `doStart()` -> `POST /api/start`.
- `doAttach()` -> `POST /api/attach`.

Backend pollers are gated behind `llama_poll_notify`; do not remove that gate unless replacing it with a more explicit user-action state machine.

When capabilities say a metric section is unavailable, hide or replace the whole `.metric-section`. Do not leave empty broken tables visible.

### CSS Editing Rules

The CSS reset deliberately removed broad legacy compatibility blocks. Do not recreate them.

Good additions:

- Add a small selector in the correct section.
- Add a new component section with narrow class names.
- Reuse tokens from `:root`.
- Add responsive behavior in the final media query section.

Risky additions:

- Reintroducing generic `.card`, `.tab`, `.panel`, or `.value` rules that apply everywhere.
- Adding inline `style` attributes instead of CSS classes.
- Adding duplicated design tokens later in the file.
- Styling by element alone, such as `button { ... }`, beyond the base reset.
- Adding another hidden compatibility block at the bottom.

Before finishing CSS work:

```bash
cargo fmt -- --check
cargo check
rg -o 'id="[^"]+"' static/index.html | sort | uniq -c | awk '$1 > 1'
cargo run -- --headless --port 7778
npx playwright screenshot --wait-for-timeout 1000 http://127.0.0.1:7778 /tmp/llama-monitor-desktop.png
npx playwright screenshot --viewport-size 390,844 --wait-for-timeout 1000 http://127.0.0.1:7778 /tmp/llama-monitor-mobile.png
```

Stop the smoke-test server after validation.

### Proposed Near-Term Roadmap

1. Idle state polish
   - Add an explicit no-session/idle panel.
   - Explain that no endpoint is contacted until Start or Attach.

2. Tabs and pages
   - Keep Server, Chat, and Logs as top-level sidebar tabs.
   - Decide whether Sessions and Models should become full pages or remain modals.
   - If Sessions becomes a page, use `page-sessions` and `switchTab('sessions')`; do not open a modal from that sidebar item.

3. Motion pass
   - Add motion tokens.
   - Add modal/toast entry animations.
   - Add subtle selected-sidebar indicator transition.
   - Add `prefers-reduced-motion` handling.

4. Icon pass
   - Replace emoji icons with a consistent SVG icon set.
   - Keep buttons compact and recognizable.
   - Add accessible labels/tooltips where icon-only controls appear.

5. Visual QA
   - Add Playwright screenshots for desktop and mobile.
   - Add a basic check that `.top-nav-bar`, `.sidebar-nav`, and `.content-area` computed layouts are not browser defaults.

### `prefers-reduced-motion`

If animations are added, include:

```css
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 1ms !important;
    animation-iteration-count: 1 !important;
    scroll-behavior: auto !important;
    transition-duration: 1ms !important;
  }
}
```

This should live near the responsive rules.

### Definition Of Done For UI Changes

A UI change is not done until:

- No duplicate IDs are present.
- The app shell renders as a modern layout at desktop width.
- The mobile layout is usable at roughly 390px wide.
- `cargo check` passes.
- The Rust app was restarted after static asset edits.
- A screenshot was reviewed.
- Any new interaction is documented here if it changes navigation, state, or component conventions.
