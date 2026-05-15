# llama-monitor UI Design Patterns

Practical reference for AI agents and developers.
Each section describes what actually exists in the codebase and what is expected when adding new UI.

Sources:
- static/css/tokens.css (design tokens)
- static/css/layout.css (app shell, sidebar, dashboard grid)
- static/css/chat.css (chat, session panel, modals, search)
- static/css/components.css (shared component shells)
- static/css/cards-inference.css, cards-hardware.css (dashboard cards)
- static/js/features/* (behavior, wiring)

## How to use this doc (for AI agents)

When implementing a new feature or UI change:

1) Prefer existing patterns.
   - Match existing component styles (buttons, modals, toasts, cards, panels).
   - Use tokens where they exist; do not invent new visual styles.

2) For new floating surfaces (modals, popovers, menus):
   - Use glassmorphism (radial/linear gradients + blur + subtle border + layered shadow).
   - Use micro-interactions (translateY(-1px), scale(1.05), etc.) with reduced-motion override.

3) For chat-related UI:
   - Match chat.css for messages, header buttons, session panel.
   - New chat controls should live in static/js/features/chat-*.js.

4) For dashboard UI:
   - Match cards-inference.css, cards-hardware.css for widgets and metrics.

5) For modals and forms:
   - Match .modal, .modal-field, .guided-action-btn, and input styles.

6) For dark/light themes:
   - Provide [data-theme="light"] overrides for new themed elements.
   - Keep glassmorphism and glow, but soften for light theme.

7) For motion:
   - Add @media (prefers-reduced-motion: reduce) for animations and hover transforms.
   - Most animations are CSS-only, triggered by classes; some hover effects are JS-assisted.

## 1. Design Tokens

- Tokens are the primary source of truth for colors, radii, shadows, and typography.
- A small number of hardcoded values still exist (e.g., special state colors, contrast tweaks). Avoid adding new ones.

### Color tokens
- Primary: --color-primary (#6366f1), --color-primary-light (#818cf8)
- Semantic: --color-success (#10b981), --color-warning (#f59e0b), --color-error (#f43f5e), --color-info (#06b6d4)
- Text: --color-text-primary, --color-text-secondary, --color-text-muted, --color-text-inverse
- Grays: --color-gray-1, --color-gray-2, --color-gray-3

### Surface tokens
- --color-bg, --color-bg-surface, --color-bg-elevated, --color-bg-floating
- --surface-card-base, --surface-card-elevated, --surface-card-overlay, --surface-card-overlay-strong
- --surface-card-ambient-a/b, --surface-card-glow, --surface-card-line, --surface-card-line-strong

### Typography tokens
- --font-body: 'Inter' stack
- --font-mono: 'Fira Code' stack
- Sizes: --text-3xs (0.625rem) to --text-3xl (1.875rem)

### Radius tokens
- --radius-sm: 8px (inputs, small controls)
- --radius-base: 12px (cards, panels, modals, menus)
- --radius-card: 20px (dashboard cards)
- 999px for pills, badges, tiny buttons

### Shadow tokens
- --shadow-surface: 0 8px 24px rgba(0,0,0,0.22)
- --shadow-elevated: 0 18px 48px rgba(0,0,0,0.34)
- --shadow-floating: 0 26px 72px rgba(0,0,0,0.5)

### Gradient tokens
- --gradient-primary: 160deg indigo → violet → cyan
- --gradient-success, --gradient-warning, --gradient-error

### Border tokens
- --border-subtle: rgba(255,255,255,0.07)
- --border-emphasis: rgba(255,255,255,0.14)

Rule: If a color, radius, shadow, font size, or spacing appears more than once, use a token.

## 2. Layout Patterns

### App shell
- Left icon rail: fixed 208px, collapsible to 68px (nav.js). Subtle gradient, soft right border.
- Content area: flex column, margin-left matches sidebar width.
- Page switching:
  - .page { display: none; flex-direction: column; }
  - .page.active { display: flex; }
- Only one page visible at a time; pages scroll independently.
- Main layout classes:
  - .top-bar
  - .sidebar
  - .content-area
  - .page

### Top cockpit bar
- Sticky, radial + linear gradient background, blur, soft border-bottom.
- Contains: logo/title (left), status chips + telemetry + session buttons (center), user menu (right).
- Use .nav-cockpit container with .nav-cockpit-chip for status chips.

### Chat page layout
- Row layout:
  - Session panel: .chat-sessions-panel (left, 240px when visible)
  - Chat main area: .chat-main-area (center, flex 1)
  - Context notes bar: .chat-context-bar (right, resizable)
- The session panel is controlled via:
  - .visible: panel shown
  - .collapsed: panel shrinks (or uses collapsed strip)
- See static/js/features/chat-sessions-sidebar.js.

### Dashboard
- 12-column grid:
  - .dashboard-grid { grid-template-columns: repeat(12, minmax(0,1fr)); }
- Cards span columns; header/control-bar span full width.
- Inference grid: 3-column sub-grid for speed/context/generation cards.
- Cards are CSS-structured but JS-populated (dashboard-render.js etc.).

Example:
  .page { display: none; flex-direction: column; }
  .page.active { display: flex; }
  .dashboard-grid { grid-template-columns: repeat(12, minmax(0,1fr)); gap: var(--gap-lg); }

## 3. Navigation Patterns

### Sidebar buttons
- Icon + label, hover subtle lift, active left accent bar + radial glow.
- .sidebar-btn:
  - Default: gradient(rgba(255,255,255,0.012)), color: --color-text-secondary
  - Hover: brighter gradient, translateY(-1px), inset + drop shadow
  - Active: color: --color-primary-light, radial gradient, left bar with --gradient-primary
- Behavior in static/js/features/nav.js via data-tab attributes.

Example:
  .sidebar-btn.active {
    color: var(--color-primary-light);
    background:
      radial-gradient(circle at 16% 50%, rgba(99,102,241,0.18), transparent 36%),
      linear-gradient(135deg, rgba(99,102,241,0.18), rgba(99,102,241,0.08));
    box-shadow:
      inset 0 1px 0 rgba(255,255,255,0.05),
      0 14px 28px rgba(79,70,229,0.18),
      0 0 28px rgba(99,102,241,0.1);
  }
  .sidebar-btn.active::before {
    content: '';
    position: absolute;
    top: 10px; bottom: 10px; left: 0;
    width: 3px;
    border-radius: 999px;
    background: var(--gradient-primary);
    box-shadow: 0 0 10px rgba(99,102,241,0.4);
  }

### Status badge on nav
- Small pill: .sidebar-badge, absolute top-right, red background, tiny font.
- Empty badge shrinks to 6px dot.

### Top bar chips
- Status chips: .nav-cockpit-chip, monospace, tiny, uppercase, subtle border.
- Metrics chips: .metric-badge, monospace, warning color, pill shape.
- Agent status: .agent-status, pill with hover tooltip and glow.

## 4. Chat UI Patterns

### Chat header
- Row of small buttons: Behavior, Model, Style, Compact, etc.
- .chat-header-btn: pill button, subtle border, gradient bg, hover lift.
- Active header button: cyan-tinted radial gradient, small glowing dot indicator.
- Persona row: .chat-persona-row with persona name + edit button.
- Explicit badges: small pills with glow for explicit_level.

Example:
  .chat-header-btn.active {
    border-color: var(--surface-card-line);
    background:
      radial-gradient(circle at 20% 22%, rgba(56,189,248,0.12), transparent 28%),
      linear-gradient(180deg, rgba(22,78,99,0.2), rgba(15,23,42,0.08));
    color: #b6f3f9;
  }
  .chat-header-btn.active::before {
    content: '';
    position: absolute;
    top: 4px; left: 6px;
    width: 5px; height: 5px;
    border-radius: 50%;
    background: #67e8f9;
    box-shadow: 0 0 8px rgba(34,211,238,0.45);
  }

### Chat sessions sidebar (left panel)
- Replaces horizontal tab bar (Phase 1 of chat-system-evolution).
- Managed by static/js/features/chat-sessions-sidebar.js.
- Key elements:
  - #chat-sessions-panel (.chat-sessions-panel)
  - .csp-header (Conversations header)
  - .csp-list (list of .csp-item)
  - .csp-item (individual chat session)
- Behavior:
  - showSessionPanel() / hideSessionPanel() called from nav.js.
  - renderChatSessionsSidebar() called from chat-state.js bindings.
  - Supports pin/unpin, rename, export, duplicate, delete via context menu.

### Message bubbles
- User: blue-teal gradient, right-aligned, soft border, layered shadow.
- Assistant: dark subtle bg, left-aligned, very soft border.
- Style variants via data-chat-style: rounded, compact, minimal, bubbly.
- Streaming: animated border pulse with soft cyan glow.

Example:
  .chat-message-user .chat-msg-body {
    background:
      linear-gradient(160deg, rgba(8,145,178,0.2), rgba(8,145,178,0.12));
    border: 1px solid rgba(8,145,178,0.18);
    box-shadow:
      0 6px 18px rgba(0,0,0,0.22),
      0 0 18px rgba(8,145,178,0.07),
      inset 0 1px 0 rgba(255,255,255,0.09);
  }
  .chat-message-assistant .chat-msg-body {
    background: var(--chat-bubble-assistant-bg);
    border: 1px solid rgba(255,255,255,0.06);
    box-shadow: 0 16px 34px rgba(0,0,0,0.14), inset 0 1px 0 rgba(255,255,255,0.04);
  }

### Thinking indicator
- .chat-thinking-summary: pill with purple tint, animated icon + dots.
- Token count: tiny, tabular-nums, muted.

### Telemetry inline rail
- .chat-telemetry-rail: row of chips for context, tokens, throughput.
- Context ring: .chat-telemetry-context-ring, conic-gradient radial.
- Throughput bars: mini gradient fills (prompt-bar, gen-bar).
- Live chip: .chat-telemetry-live-chip, monospace, small.

### Context pressure bar
- Thin gradient bar at top: teal → amber → orange → rose.
- .ctx-pressure-fill: width = usage %, color by level.
- Critical: pulsing animation.
- Chat input textarea also glows by pressure level.

Example:
  .ctx-pressure-fill.ctx-pressure-medium { background: rgba(234,179,8,0.6); }
  .ctx-pressure-fill.ctx-pressure-high { background: rgba(234,179,8,0.9); }
  .ctx-pressure-fill.ctx-pressure-critical {
    background: rgba(239,68,68,0.9);
    animation: ctx-pressure-pulse 1.5s ease-in-out infinite;
  }

### Chat input row
- .chat-input-row: textarea with auto-resize, send/stop buttons.
- Textarea: auto-growing, gradient bg, focus glow, soft border.
- Send button: gradient-primary, glow, subtle breathe animation.
- Stop button: red gradient.

Example:
  .btn-chat-send {
    background: var(--gradient-primary);
    color: white;
    box-shadow: 0 12px 28px rgba(99,102,241,0.3), inset 0 1px 0 rgba(255,255,255,0.18);
  }
  .btn-chat-send:hover:not(:disabled) {
    transform: translateY(-2px);
    box-shadow: 0 16px 34px rgba(99,102,241,0.4), inset 0 1px 0 rgba(255,255,255,0.22);
  }

### Message actions
- .chat-action-btn: small square, hover purple tint + glow, scale(1.05).
- Copied: green tint. Delete: red tint.

### FTS search (cross-session search)
- Implemented in static/js/features/chat-search.js.
- Triggered by a search button in .csp-header.
- Key elements:
  - .csp-search-btn (button)
  - .csp-search-input-wrap (inline input container)
  - #csp-search-input (input)
  - .csp-search-results (results list)
  - .csp-search-result (individual result card)
- Behavior:
  - openSearch() shows input, hides .csp-list, shows .csp-search-results.
  - closeSearch() restores normal list.
  - onSearchInput() debounces and calls /api/chat/search.
- Styling in chat.css under .csp-search-* rules.

## 5. Dashboard Cards

### Card shell (universal)
- Glassmorphism, radial + linear gradients, soft border, layered shadow.
- Hover: border intensifies, glow appears, subtle float animation.
- Use .widget-card as base for all metric cards.

Example:
  .widget-card {
    background:
      radial-gradient(circle at 18% 18%, var(--surface-card-ambient-a), transparent 26%),
      radial-gradient(circle at 82% 14%, var(--surface-card-ambient-b), transparent 24%),
      linear-gradient(180deg, var(--surface-card-elevated), var(--surface-card-base));
    border: 1px solid var(--surface-card-overlay-strong);
    border-radius: var(--radius-card);
    box-shadow:
      0 16px 40px rgba(0,0,0,0.36),
      0 4px 16px rgba(0,0,0,0.28),
      inset 0 1px 0 rgba(255,255,255,0.06),
      inset 0 -1px 0 rgba(0,0,0,0.2);
    transition: transform 0.25s ease, box-shadow 0.25s ease, border-color 0.25s ease;
  }

### Card live/idle states
- is-live: teal border, glow, animated edge gradient.
- is-idle: neutral, subtle border.
- is-blocked: amber tint, pulse.
- is-dormant: reduced opacity, desaturated.

### Metrics typography
- Numbers: --font-mono, tabular-nums, bold weights.
- Labels: uppercase, letter-spacing, muted color.
- Values: --color-text-primary, large font for key metrics.

### Sparklines
- Gradient fills with smooth curves.
- Subtle grid, sheen animation, current-dot glow.
- Use semantic colors: teal/blue for prompt, green for generation.

### Hardware cards
- .widget-hardware: same glassmorphism base.
- Metric blocks: .hw-metric-block, soft bg, hover lift + glow.
- Bars: gradient fill with shimmer, cap dot with glow.
- Ring viz: .hw-ring-viz, conic-gradient, ambient glow.

## 6. Modals and Overlays

### Modal shell
- Glassmorphism, deep shadow, radial gradients, blur.
- Max width 760px, max height 86vh.
- Header and footer with subtle gradient, content scrollable.
- Base classes in components.css:
  - .modal-backdrop
  - .modal
  - .modal-header
  - .modal-body
  - .modal-footer

Example:
  .modal {
    width: min(760px, 96vw);
    max-height: min(86vh, 900px);
    background:
      radial-gradient(circle at 18% 16%, rgba(99,102,241,0.12), transparent 28%),
      radial-gradient(circle at 84% 18%, rgba(34,211,238,0.08), transparent 24%),
      linear-gradient(160deg, rgba(40,48,58,0.94), rgba(28,34,42,0.97));
    border: 1px solid rgba(255,255,255,0.1);
    box-shadow: 0 12px 48px rgba(0,0,0,0.48), 0 2px 12px rgba(0,0,0,0.32), inset 0 1px 0 rgba(255,255,255,0.07);
    backdrop-filter: blur(22px) saturate(1.15);
  }

### Modals and ARIA (current state)
- Many modals use aria-modal, aria-label, and Escape-to-close.
- Focus trapping is implemented in several modals but not uniformly.
- When adding a new modal:
  - Use aria-modal="true"
  - Add aria-label
  - Close on Escape and backdrop click
  - Implement focus trap if feasible; treat as a goal, not an assumption.

### Form fields
- Inputs: gradient bg, subtle border, focus glow.
- Focus: --color-primary border, 3px soft ring.
- Hover: subtle lift.

Example:
  .modal-field input:focus {
    border-color: rgba(99,102,241,0.64);
    box-shadow: 0 0 0 3px rgba(99,102,241,0.16);
  }

### Toggle switches
- Track: pill, subtle bg, inner shadow.
- Thumb: white gradient, shadow, translateX on active.
- Active: teal/cyan track with gradient thumb.

### Select dropdowns
- Styled with tokens, background matches inputs.
- Option bg: --color-bg, text: --color-text-primary.

### Buttons (modal)
- .btn-modal-save: primary style (teal gradient, white text).
- .btn-modal-cancel: ghost style, transparent bg.

### Settings modal specifics
- Implemented in static/js/features/settings.js.
- Tracks dirty state for unsaved changes.
- Shows warning before closing if dirty.
- Uses tabs internally for sections.

## 7. Premium Buttons

### Pill buttons (guided actions, key actions)
- Radial gradient, glow, translate on hover.
- Active: stronger gradient, box-shadow glow.

Example:
  .guided-action-btn {
    border-radius: 14px;
    background:
      radial-gradient(circle at top, rgba(99,102,241,0.14), transparent 60%),
      linear-gradient(180deg, rgba(255,255,255,0.06), rgba(255,255,255,0.02));
    border: 1px solid rgba(255,255,255,0.08);
    box-shadow: 0 12px 24px rgba(2,6,23,0.16), inset 0 1px 0 rgba(255,255,255,0.04);
  }
  .guided-action-btn:hover {
    transform: translateY(-1px);
    border-color: rgba(129,140,248,0.28);
    box-shadow: 0 16px 30px rgba(2,6,23,0.2), 0 0 0 1px rgba(99,102,241,0.1);
  }

### Ghost buttons
- Transparent bg, subtle border, hover fill.
- Use for secondary actions, close buttons, menu items.

Example:
  .btn-modal-cancel {
    background: rgba(255,255,255,0.04);
  }

### Icon buttons
- Small, square, hover background, scale.
- .chat-action-btn, .viz-gear-btn, etc.

Example:
  .chat-action-btn {
    width: 24px; height: 24px;
    border-radius: 6px;
    background: linear-gradient(180deg, rgba(255,255,255,0.04), rgba(255,255,255,0.02));
    border: 1px solid rgba(255,255,255,0.08);
  }
  .chat-action-btn:hover {
    background: linear-gradient(180deg, rgba(99,102,241,0.1), rgba(99,102,241,0.05));
    box-shadow: 0 0 8px rgba(99,102,241,0.15);
    transform: scale(1.05);
  }

### Active/pressed
- Primary buttons: intensify gradient + shadow.
- Pressed: scale(0.95), reduce shadow.

## 8. Glassmorphism

Used on: modals, popovers, context menus, hovercards, panels, cards.

Pattern:
- Radial gradients for ambient color.
- backdrop-filter: blur(14-22px) saturate(1.1-1.2).
- Subtle border: rgba(255,255,255,0.08-0.1).
- Soft layered shadow.

Example:
  .glass-panel {
    background:
      radial-gradient(circle at 12% 10%, rgba(99,102,241,0.08), transparent 24%),
      linear-gradient(180deg, rgba(28,34,44,0.98), rgba(18,22,30,0.98));
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 12px;
    box-shadow: 0 18px 42px rgba(0,0,0,0.34), inset 0 1px 0 rgba(255,255,255,0.05);
    backdrop-filter: blur(14px);
  }

Rule: Always pair glassmorphism with a shadow and a faint border. Never use plain semi-transparent bg alone.

## 9. Soft Borders with Hover Glow

Base:
- border: 1px solid rgba(255,255,255,0.07) (or --border-subtle)

Hover:
- Intensify border color (e.g., rgba(255,255,255,0.12))
- Add glow via box-shadow

Example:
  .soft-card {
    border: 1px solid var(--border-subtle);
  }
  .soft-card:hover {
    border-color: var(--surface-card-line);
    box-shadow: 0 0 18px rgba(99,102,241,0.1), var(--shadow-surface);
  }

Rule: Hover glow color should relate to element purpose (primary actions → indigo, telemetry → cyan, error → rose).

## 10. Box Shadows

Layered approach: outer depth + inner highlight.

- Card: 0 16px 40px rgba(0,0,0,0.36) + inset 0 1px 0 rgba(255,255,255,0.06)
- Elevated: 0 18px 48px rgba(0,0,0,0.34)
- Floating: 0 26px 72px rgba(0,0,0,0.5)

Rule:
- Use token when available.
- Always add inset 0 1px 0 rgba(255,255,255,0.04-0.08) for depth.
- Hover: add color-tinted glow shadow.

## 11. Gradient Text

For section headers, labels, badges.

Pattern:
- Subtle, cool-toned gradients (indigo → violet → cyan).
- Use background-clip: text.

Example:
  .gradient-header {
    background: linear-gradient(105deg, #a5b4fc 0%, #c4b5fd 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

Use sparingly: section headers, sidebar headings, widget labels.

## 12. Micro-interactions

Small, fast transforms on hover. Durations: 120-240ms.

- Buttons/tabs: translateY(-1px)
- Icons: scale(1.15)
- Tooltips: fade + translateY(-4px)
- Cards: translateY(-2px) on hover
- Popovers: opacity + translateY(8px → 0)

Example:
  .interactive {
    transition: transform 160ms ease, box-shadow 160ms ease, border-color 160ms ease;
  }
  .interactive:hover {
    transform: translateY(-1px);
  }

Rule: Never use more than one transform property per hover (e.g., don't combine scale + rotate).

## 13. Scrollbars

Thin, indigo-tinted, intensify on hover.
Use for: session lists, modals, long panels.

Example:
  .scrollable::-webkit-scrollbar { width: 3px; }
  .scrollable::-webkit-scrollbar-track { background: transparent; }
  .scrollable::-webkit-scrollbar-thumb {
    background: rgba(99,102,241,0.3);
    border-radius: 2px;
  }
  .scrollable::-webkit-scrollbar-thumb:hover {
    background: rgba(99,102,241,0.5);
  }

## 14. Status Indicators

Small colored dot with glow, optional pulse.
Use: live/active indicators, telemetry, connection status.

Example:
  .status-dot {
    width: 6px; height: 6px;
    border-radius: 999px;
    background: var(--color-text-muted);
  }
  .status-dot.running {
    background: var(--color-success);
    box-shadow: 0 0 18px rgba(16,185,129,0.46);
    animation: status-pulse 2.4s ease-in-out infinite;
  }

Rule:
- Use semantic colors: success (green), warning (amber), error (rose).
- Pulse animation only for actively live states.

## 15. Rounded Corners

- 999px: pills, badges, tiny buttons (status chips, context pills, slot pills)
- 12-14px: cards, panels, modals, menus
- 8-10px: inputs, small controls, inline buttons

Rule: Higher radius = more "friendly" element; use 999px for anything pill-shaped.

## 16. Toasts and Notifications

- Container: fixed bottom-right, stack vertically.
- Toast: glassmorphism, slide-in animation, icon + title + message.
- Variants: toast-success (green), toast-error (red), toast-warning (amber), toast-info (cyan).
- Explicit mode: toast-explicit, levels 0/1/2 with distinct tints.

Example:
  .toast {
    padding: 16px 18px;
    border-radius: 12px;
    background: linear-gradient(145deg, rgba(40,48,58,0.95), rgba(30,36,43,0.98));
    border: 1px solid rgba(255,255,255,0.08);
    box-shadow: 0 8px 30px rgba(0,0,0,0.4), 0 2px 10px rgba(0,0,0,0.3);
    animation: toast-slide-in 0.35s cubic-bezier(0.16,1,0.3,1);
    backdrop-filter: blur(20px);
  }
  .toast.toast-success { border-color: rgba(163,190,140,0.4); }
  .toast.toast-error { border-color: rgba(191,97,106,0.4); }

Rule: Use toasts for transient feedback only (success, error, warning, info). Don't use for persistent info.

## 17. Typography

### Hierarchy
- 3xl (1.875rem): major page titles
- 2xl (1.5rem): section headers
- xl (1.25rem): card titles
- lg (1.125rem): prominent labels
- base (1rem): body text, chat message text
- sm (0.875rem): secondary text, form labels
- xs (0.75rem): metadata, hints
- 2xs/3xs (0.6875rem/0.625rem): tiny labels, telemetry, badges

### Uppercase labels
- Use for: section headers, status labels, badges, metric labels.
- Style: uppercase, letter-spacing (0.04-0.09em), muted color.

Example:
  .section-label {
    font-size: var(--text-2xs);
    font-weight: 800;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--color-text-muted);
  }

### Muted text
- Use --color-text-muted for secondary info, hints, placeholders.
- Never use pure gray (#808080) — always a token.

## 18. Dark Theme Base

- Deep navy with radial/linear gradients.
- Background: --color-bg (#0f1115), --color-bg-surface (#16191e)
- Avoid flat colors; always layer gradients and subtle overlays.
- Use --surface-card-* tokens for cards.

Example:
  body {
    background: radial-gradient(circle at top center, rgba(56,189,248,0.055), transparent 36%),
                var(--color-bg);
  }

Rule: Any new surface should use a gradient + token, not a single hex.

## 19. Light Theme Overrides

- Softer backgrounds, reduced contrast.
- Cool gray shadows instead of pure black.
- Keep glassmorphism and glow, but soften.
- Dynamically rendered content (chat messages, tool outputs) may not be fully adapted.

Example:
  [data-theme="light"] {
    --color-bg: #f5f7fb;
    --color-bg-surface: #ffffff;
    --shadow-surface: 0 8px 24px rgba(15,23,42,0.1);
    --shadow-elevated: 0 18px 48px rgba(15,23,42,0.16);
  }

Rule:
- Always provide [data-theme="light"] overrides for new themed elements.
- Reduce glow intensity by 20-40% in light theme.
- Use rgba(15,23,42,0.1-0.16) for shadows instead of black.

## 20. prefers-reduced-motion

Required for all animations and hover transforms.
Currently respected in many places, but not all. Treat as a hard rule for new code.

Rules:
- Disable all keyframe animations.
- Remove transform-based hover effects (translateY, scale, rotate).
- Keep basic color/background transitions (160-240ms).
- Use !important sparingly to override; target specifically.

Example:
  @media (prefers-reduced-motion: reduce) {
    *, *::before, *::after {
      animation-duration: 0.01ms !important;
      animation-iteration-count: 1 !important;
    }
    .interactive:hover {
      transform: none !important;
    }
  }

Rule: Every @keyframes used in the app must have a corresponding reduced-motion override.

## 21. Accessibility (goals and current state)

This section describes what is in place and what is expected, not what is guaranteed everywhere.

### Current state (partial)
- Some components are keyboard-focusable and show visible focus outlines.
- Some modals and menus use ARIA attributes (aria-expanded, aria-label, role="menu", aria-modal).
- Escape-to-close is implemented in several modals and menus.
- Many interactive elements are keyboard-accessible, but not all.

### Requirements for new components
Any new interactive component should:
- Be keyboard-focusable (tabindex or native focus).
- Show visible focus styles.
- Support Enter/Space activation for buttons.
- Use ARIA attributes where they improve semantics (labels, states).
- Respect prefers-reduced-motion.

### Focus styles
- Interactive elements must show focus.
- Use outline or border-color with a soft ring.

Example:
  button:focus-visible,
  input:focus-visible,
  [tabindex]:focus-visible {
    outline: none;
    border-color: rgba(99,102,241,0.64);
    box-shadow: 0 0 0 3px rgba(99,102,241,0.16);
  }

### ARIA attributes
- Menus: aria-expanded on triggers, role="menu" on containers.
- Buttons with icons: aria-label.
- Panels: aria-label describing purpose.

### Keyboard activation
- Buttons: Enter/Space to activate.
- Menus: arrow key navigation where applicable.

Rule: Treat accessibility as an incremental requirement. New code should be better than the average of the existing codebase.
