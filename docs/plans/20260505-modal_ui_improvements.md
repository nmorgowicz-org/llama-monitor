# Modal UI Premium Overhaul — 2026-05-05

**Date:** 2026-05-05
**Status:** In Progress (settings modal upgraded, agent modal polished, remaining modals queued)
**Author:** AI Agent Session

---

## Problem Statement

The app has 12+ modals/popups with inconsistent visual quality. The settings modal and agent setup modal have premium glass-morphism styling with idle animations, gradient borders, and hover micro-interactions. However, 10 other modals fall through to bare base styles from `components.css` with:

- **No glass morphism** — flat backgrounds, no `backdrop-filter`, no inset shadows
- **No ambient glow orbs** — missing the radial gradient glow behind the modal
- **No entrance/exit animations** — instant open/close with no scale/translate transition
- **No breathing gradient borders** — missing the animated `::after` mask-composite border glow
- **No internal section card treatment** — `.modal-section` / `details` panels are flat containers
- **No form field elevation** — `.modal-field` elements lack widget-card depth
- **No idle animations** — no breathing, pulse, or ambient drift on non-interactive elements
- **No hover micro-interactions** — no transform elevation, border glow, or shadow deepening

This creates a jarring experience when users navigate between the premium dashboard/chat areas and the flat utility modals.

---

## Design Language Reference

The premium design language is defined by three authoritative sources:

| Source | File | Key Patterns |
|--------|------|--------------|
| **Widget Card** | `static/css/cards-inference.css` `.widget-card` | 3D depth, radial gradient bg, gradient top-line `::before`, gradient border glow `::after`, hover elevation, idle `card-float` |
| **Agent Modal Sections** | `static/css/agent-modal.css` `.agent-setup-section` / `.agent-setup-hero` | Staggered entrance, per-section accents, hero icon pulse, gradient border on hover |
| **Settings Modal Shell** | `static/css/settings-modal.css` `#settings-modal .modal` | Glass morphism, ambient glow orb, breathing gradient border, entrance/exit scale |

---

## Completed Work (2026-05-05)

### Settings Modal (`#settings-modal`)

| Change | Status | Details |
|--------|--------|---------|
| Widget-card for ALL `.modal-field` elements | DONE | Previously only Performance tab had card treatment. Now all panes (Session, GPU, Models, Appearance, Advanced) get elevated cards |
| Per-pane accent colors | DONE | Each pane's fields have unique top-line gradient: Session=teal, GPU=green, Models=teal, Appearance=purple, Performance=indigo, Advanced=yellow |
| Idle float animation | DONE | Staggered `settings-field-float` with per-field delay (0s, 0.4s, 0.8s, 1.2s, 1.6s) |
| `.modal-section` premium upgrade | DONE | Runtime Config card now has widget-card treatment with amber accent, hover elevation, idle float |
| Elevated form controls | DONE | Inputs/selects inside widget-card fields have gradient bg, hover glow, focus ring |
| Pane entrance animation | DONE | `settings-pane-entrance` with 300ms cubic-bezier |

### Remote Agent Setup Modal (`#remote-agent-setup-modal`)

| Change | Status | Details |
|--------|--------|---------|
| Existing premium styling | AUDITED | Already has widget-card sections, hero pulse, staggered entrances, per-section accents |
| Idle animations | PRESENT | `hero-icon-pulse`, `hero-sweep`, `progress-shimmer`, `section-fade-in` |
| Version cards | PREMIUM | Widget-card treatment with hover elevation |
| Progress section | PREMIUM | Gradient bg, shimmer animation, status badges |

---

## Remaining Modals — Priority Queue

### Phase 1: High-Impact Modals (Frequently Used)

#### 1. Config Modal (`#config-modal`)
**Frequency:** High — opened from Settings > Runtime Configuration
**Current State:** Bare `.modal` shell, flat `details.modal-section` panels

**Tasks:**
- [ ] Apply glass shell: `#config-modal .modal` gets gradient bg, backdrop-filter, inset shadows
- [ ] Add ambient glow orb: `#config-modal::before`
- [ ] Add breathing gradient border: `#config-modal .modal::after`
- [ ] Entrance/exit animations: scale + translate
- [ ] Header gradient title: `#config-modal .modal-header h2`
- [ ] Widget-card for `details.modal-section` panels (3 sections)
- [ ] Per-section accent colors via `nth-child`
- [ ] Elevated form controls inside modal fields
- [ ] `.remote-agent-status-panel` widget-card treatment
- [ ] `.remote-agent-guide` widget-card treatment
- [ ] Version info boxes elevated

**Estimated effort:** ~200 lines CSS

#### 2. Preset Modal (`#preset-modal`)
**Frequency:** High — opened frequently for model configuration
**Current State:** Bare `.modal` shell, 9 flat `details.modal-section` panels

**Tasks:**
- [ ] Apply glass shell: `#preset-modal .modal`
- [ ] Add ambient glow orb: `#preset-modal::before`
- [ ] Add breathing gradient border: `#preset-modal .modal::after`
- [ ] Entrance/exit animations
- [ ] Header gradient title
- [ ] Widget-card for all 9 `details.modal-section` panels:
  - Model & Memory
  - Context & KV Cache
  - Batching & Slots
  - Generation
  - GPU Distribution
  - Threading
  - Rope Scaling
  - Speculative Decoding
  - Advanced
- [ ] Per-section accent colors (9 unique accents)
- [ ] Staggered section entrance animations
- [ ] Elevated form controls inside modal fields
- [ ] Idle breathing on sections

**Estimated effort:** ~250 lines CSS

#### 3. Session Modal (`#session-modal`)
**Frequency:** High — opened when managing sessions
**Current State:** Bare `.modal` shell, flat `.session-item` cards

**Tasks:**
- [ ] Apply glass shell: `#session-modal .modal`
- [ ] Add ambient glow orb: `#session-modal::before`
- [ ] Add breathing gradient border: `#session-modal .modal::after`
- [ ] Entrance/exit animations
- [ ] Header gradient title
- [ ] Widget-card for `.session-item` elements
- [ ] Hover elevation on session items
- [ ] Elevated form controls inside modal fields
- [ ] Session list staggered entrance

**Estimated effort:** ~150 lines CSS

---

### Phase 2: Medium-Impact Modals

#### 4. Models Modal (`#models-modal`)
**Tasks:**
- [ ] Glass shell + ambient glow + breathing border
- [ ] Widget-card for `.model-item` elements
- [ ] Staggered grid entrance for model list
- [ ] Hover elevation on model items

**Estimated effort:** ~100 lines CSS

#### 5. Export Modal (`#export-modal`)
**Tasks:**
- [ ] Glass shell + ambient glow + breathing border
- [ ] Widget-card for `.export-panel` sections
- [ ] Card treatment for `.export-option` items
- [ ] Custom radio button styling

**Estimated effort:** ~100 lines CSS

#### 6. User Preferences Modal (`#user-preferences-modal`)
**Tasks:**
- [ ] Glass shell + ambient glow + breathing border
- [ ] Widget-card for `.personalization-section` panels
- [ ] Elevated form controls
- [ ] Custom range slider styling

**Estimated effort:** ~120 lines CSS

#### 7. Template Manager Modal (`#template-manager-modal`)
**Tasks:**
- [ ] Glass shell + ambient glow + breathing border
- [ ] Sidebar panel elevation
- [ ] Preview panel elevation
- [ ] `.explicit-policy-section .modal-section` widget-card

**Estimated effort:** ~100 lines CSS

---

### Phase 3: Lower-Impact Modals

#### 8. File Browser Modal (`#file-browser-modal`)
**Tasks:**
- [ ] Glass shell + ambient glow + breathing border
- [ ] `.file-browser-bar` elevated
- [ ] File entries card treatment

**Estimated effort:** ~80 lines CSS

#### 9. Keyboard Shortcuts Modal (`.shortcuts-modal`)
**Tasks:**
- [ ] Glass shell + ambient glow + breathing border
- [ ] `.shortcuts-section` panels elevated
- [ ] `.shortcut-item` hover states
- [ ] `.shortcut-key-combo` glow on hover

**Estimated effort:** ~80 lines CSS

#### 10. Analytics Modal (`#analytics-modal`)
**Tasks:**
- [ ] Glass shell + ambient glow + breathing border
- [ ] `.analytics-metric` / `.analytics-stat-item` card treatment

**Estimated effort:** ~80 lines CSS

#### 11. Release Notes Panel (`.slide-panel`)
**Tasks:**
- [ ] Entirely new CSS — no rules exist
- [ ] Glass shell, slide-in animation, header/body/footer layout
- [ ] Content readability styling

**Estimated effort:** ~100 lines CSS

---

## Universal CSS Patterns to Apply

### Pattern 1: Modal Shell Glass Treatment

```css
/* Apply to each modal's .modal element */
#<modal-id> .modal {
  position: relative;
  background: linear-gradient(160deg, rgba(40, 48, 58, 0.88), rgba(28, 34, 42, 0.95));
  border: 1px solid rgba(255, 255, 255, 0.09);
  border-radius: var(--radius-card);
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.45),
    0 2px 12px rgba(0, 0, 0, 0.35),
    inset 0 1px 0 rgba(255, 255, 255, 0.07),
    inset 0 -1px 0 rgba(0, 0, 0, 0.25);
  backdrop-filter: blur(24px) saturate(1.2);
  overflow: hidden;
  animation: modal-entrance 350ms cubic-bezier(0.16, 1, 0.3, 1) both;
}

#<modal-id> .modal::before {
  content: '';
  position: absolute;
  top: 0; left: 0; right: 0;
  height: 1px;
  background: linear-gradient(90deg, transparent 5%, rgba(255, 255, 255, 0.25) 50%, transparent 95%);
  z-index: 2;
}

#<modal-id> .modal::after {
  content: '';
  position: absolute;
  inset: 0;
  border-radius: var(--radius-card);
  padding: 1px;
  background: linear-gradient(160deg, rgba(99, 102, 241, 0.35), transparent 35%, transparent 65%, rgba(6, 182, 212, 0.2));
  -webkit-mask: linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
  -webkit-mask-composite: xor;
  mask-composite: exclude;
  pointer-events: none;
  z-index: 1;
  opacity: 0.6;
  transition: opacity 400ms ease;
}

#<modal-id>.open .modal::after,
#<modal-id>.active .modal::after {
  opacity: 1;
  animation: modal-border-breathe 3s ease-in-out infinite;
}
```

### Pattern 2: Ambient Glow Orb

```css
#<modal-id>::before {
  content: '';
  position: absolute;
  top: 50%; left: 50%;
  width: 500px; height: 500px;
  transform: translate(-50%, -50%);
  background: radial-gradient(circle, rgba(99, 102, 241, 0.18) 0%, rgba(6, 182, 212, 0.08) 40%, transparent 70%);
  filter: blur(60px);
  pointer-events: none;
  animation: modal-ambient-drift 8s ease-in-out infinite;
  z-index: 0;
}
```

### Pattern 3: Internal Section Widget-Card

```css
#<modal-id> <section-selector> {
  position: relative;
  padding: 18px 22px;
  background:
    radial-gradient(circle at top right, rgba(255, 255, 255, 0.04), transparent 22%),
    linear-gradient(145deg, rgba(40, 48, 58, 0.94), rgba(27, 33, 41, 0.99));
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: var(--radius-card);
  box-shadow:
    0 16px 40px rgba(0, 0, 0, 0.36),
    0 4px 16px rgba(0, 0, 0, 0.28),
    inset 0 1px 0 rgba(255, 255, 255, 0.06),
    inset 0 -1px 0 rgba(0, 0, 0, 0.2),
    inset 0 36px 80px -38px rgba(255, 255, 255, 0.05);
  backdrop-filter: blur(12px);
  overflow: hidden;
  transition: border-color 250ms ease, box-shadow 250ms ease, transform 300ms cubic-bezier(0.16, 1, 0.3, 1);
}

#<modal-id> <section-selector>::before {
  content: '';
  position: absolute;
  top: 0; left: 0; right: 0;
  height: 1px;
  background: linear-gradient(90deg, transparent, rgba(136, 192, 209, 0.4), transparent);
}

#<modal-id> <section-selector>::after {
  content: '';
  position: absolute;
  inset: 0;
  border-radius: var(--radius-card);
  padding: 1px;
  background: linear-gradient(135deg, rgba(99, 102, 241, 0.3), transparent 40%, transparent 60%, rgba(99, 102, 241, 0.15));
  -webkit-mask: linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
  -webkit-mask-composite: xor;
  mask-composite: exclude;
  opacity: 0;
  transition: opacity 250ms ease;
  pointer-events: none;
}

#<modal-id> <section-selector>:hover::after {
  opacity: 1;
}

#<modal-id> <section-selector>:hover {
  border-color: rgba(99, 102, 241, 0.45);
  box-shadow:
    0 18px 42px rgba(0, 0, 0, 0.45),
    0 4px 16px rgba(0, 0, 0, 0.32),
    0 0 40px rgba(99, 102, 241, 0.12),
    inset 0 1px 0 rgba(255, 255, 255, 0.08),
    inset 0 -1px 0 rgba(0, 0, 0, 0.25);
  transform: translateY(-2px) scale(1.005);
}
```

### Pattern 4: Idle Animations

```css
/* Breathing border */
@keyframes modal-border-breathe {
  0%, 100% { opacity: 0.6; }
  50% { opacity: 1; }
}

/* Ambient glow drift */
@keyframes modal-ambient-drift {
  0%, 100% { transform: translate(-50%, -50%) scale(1); opacity: 0.7; }
  33% { transform: translate(-45%, -55%) scale(1.1); opacity: 1; }
  66% { transform: translate(-55%, -45%) scale(0.95); opacity: 0.8; }
}

/* Section float */
@keyframes section-float {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-1.5px); }
}

/* Entrance */
@keyframes modal-entrance {
  0% { opacity: 0; transform: scale(0.95) translateY(12px); }
  100% { opacity: 1; transform: scale(1) translateY(0); }
}

/* Exit */
@keyframes modal-exit {
  0% { opacity: 1; transform: scale(1) translateY(0); }
  100% { opacity: 0; transform: scale(0.95) translateY(12px); }
}
```

---

## File Organization

### Current CSS Files
| File | Purpose |
|------|---------|
| `static/css/settings-modal.css` | Settings modal (UPGRADED) |
| `static/css/agent-modal.css` | Remote agent setup modal (AUDITED, premium) |
| `static/css/chat.css` | Chat, toasts, compact markers |
| `static/css/cards-inference.css` | Widget card base patterns |
| `static/css/components.css` | Base modal shell (bare), `.modal-section` (flat) |
| `static/css/setup-view.css` | Setup view, export options, session items, shortcuts |

### Recommended: New File
Create `static/css/modal-premium.css` for shared modal premium patterns:
- Universal modal shell glass treatment
- Ambient glow orbs
- Shared animations (entrance, exit, breathe, drift, float)
- Section widget-card base styles
- Form control elevation patterns

This avoids duplicating the same patterns across 10+ modals and makes future updates centralized.

---

## Light Theme Considerations

All premium styling must include `[data-theme="light"]` overrides. The settings modal already has these (line 995-1056). New patterns need:

```css
[data-theme="light"] #<modal-id> .modal {
  background: linear-gradient(160deg, rgba(255, 255, 255, 0.88), rgba(240, 244, 248, 0.94));
  border-color: rgba(17, 24, 39, 0.1);
  box-shadow:
    0 8px 32px rgba(15, 23, 42, 0.12),
    0 2px 12px rgba(15, 23, 42, 0.08),
    inset 0 1px 0 rgba(255, 255, 255, 0.8);
}

/* Similar overrides for sections, fields, inputs */
```

---

## Reduced Motion Considerations

All animations must respect `prefers-reduced-motion`:

```css
@media (prefers-reduced-motion: reduce) {
  #<modal-id> .modal,
  #<modal-id> .modal::after,
  #<modal-id> <section-selector>,
  #<modal-id> <section-selector>::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

---

## Estimated Total Effort

| Phase | Modals | Lines CSS | Priority |
|-------|--------|-----------|----------|
| Phase 1 | Config, Preset, Session | ~600 | High |
| Phase 2 | Models, Export, Preferences, Templates | ~400 | Medium |
| Phase 3 | File Browser, Shortcuts, Analytics, Release Notes | ~360 | Lower |
| **Total** | **11 modals** | **~1360 lines** | |

---

## Execution Strategy for Future Agents

1. **Create `modal-premium.css`** with shared patterns (entrance, exit, breathe, drift, float, glass shell, section card)
2. **Phase 1 first** — Config, Preset, Session are highest-frequency modals
3. **Each modal gets:**
   - Glass shell + ambient glow + breathing border
   - Header gradient title
   - Internal sections elevated to widget-card
   - Per-section accent colors
   - Staggered entrance animations
   - Form control elevation
   - Light theme overrides
   - Reduced motion support
4. **Verify** each modal opens/closes smoothly, hover states work, focus states are visible
5. **Run** `npm run lint` and `./scripts/validate-js.sh` after any JS changes
6. **Test** in both dark and light themes

---

## Related Files

| File | Relevance |
|------|-----------|
| `static/css/settings-modal.css` | Reference for premium patterns |
| `static/css/agent-modal.css` | Reference for section cards, animations |
| `static/css/cards-inference.css` | Widget card gold standard |
| `static/css/components.css` | Base styles to override |
| `static/css/setup-view.css` | Export, session, shortcuts styles |
| `static/index.html` | Modal HTML structure |
| `static/js/features/settings.js` | Settings modal JS |
| `static/js/features/remote-agent.js` | Agent modal JS |

---

## Light Theme Audit — 2026-05-05

**Status:** Critical gap. Light theme toggle exists in JS but only ~3% of CSS selectors have light theme overrides. App is effectively dark-only.

### Root Cause

`tokens.css` defines a complete CSS variable system with light/dark variants, but **~97% of selectors bypass it** by using hardcoded dark rgba/hex values instead of `var(--*)`. The 55 light-theme rules that exist only cover settings-modal and agent-modal partial areas.

### Coverage Table

| File | Lines | Selectors | Light Rules | Coverage |
|------|-------|-----------|-------------|----------|
| `tokens.css` | 70 | 1 | 2 | 100% (defines all vars) |
| `settings-modal.css` | 1327 | 160 | 29 | 18% |
| `agent-modal.css` | 1065 | 147 | 13 | 9% |
| `chat.css` | 3176 | 522 | 11 | 2% |
| `base.css` | 85 | 13 | 0 | **0%** |
| `layout.css` | 1008 | 130 | 0 | **0%** |
| `components.css` | 462 | 56 | 0 | **0%** |
| `cards-inference.css` | 1682 | 266 | 0 | **0%** |
| `cards-hardware.css` | 1733 | 241 | 0 | **0%** |
| `setup-view.css` | 1000 | 170 | 0 | **0%** |
| `logs.css` | 270 | 39 | 0 | **0%** |
| **TOTAL** | **11878** | **~1745** | **55** | **~3%** |

### JS Theme Toggle — Working

- `static/js/features/user-menu.js` — `applyThemePreference()` correctly sets `document.documentElement.dataset.theme`
- `toggleTheme()` toggles between `'light'` and `'dark'`
- Default theme: `'dark'`
- Persisted to `localStorage` under `uiSettings.theme`

### Top 20 Most Critical Missing Overrides

| # | File:Line | Element | Hardcoded Value | Should Use |
|---|-----------|---------|-----------------|------------|
| 1 | `base.css:26-38` | `body::before` grid | `mix-blend-mode: screen` + white grid | Invisible on light bg |
| 2 | `layout.css` | `.nav-sidebar` bg | `rgba(15, 17, 21, 0.92)` | `var(--color-bg-surface)` |
| 3 | `layout.css` | `.topbar` bg | hardcoded dark | `var(--color-bg-surface)` |
| 4 | `layout.css` | `.tab-bar` bg | hardcoded dark | `var(--color-bg-surface)` |
| 5 | `cards-inference.css` | `.dashboard-header` bg | `rgba(28, 33, 42, 0.86)` | `var(--color-bg-surface)` |
| 6 | `cards-hardware.css` | `.hw-card` bg | hardcoded dark | `var(--color-bg-surface)` |
| 7 | `chat.css` | `.chat-container` bg | hardcoded dark | `var(--color-bg)` |
| 8 | `chat.css` | `.message-assistant` bg | `rgba(255,255,255,0.03)` | `var(--color-bg-surface)` |
| 9 | `chat.css` | `.message-user` bg | hardcoded dark | `var(--color-bg-elevated)` |
| 10 | `chat.css` | `.chat-input` bg | `rgba(255,255,255,0.05)` | `var(--color-bg-elevated)` |
| 11 | `setup-view.css` | `.setup-container` bg | `rgba(10, 14, 24, 0.82)` | `var(--color-bg)` |
| 12 | `setup-view.css` | `.setup-step` bg | `rgba(0,0,0,0.62)` | `var(--color-bg-surface)` |
| 13 | `agent-modal.css` | `.agent-modal` bg | `rgba(32, 40, 50, 0.97)` | `var(--color-bg-floating)` |
| 14 | `logs.css` | `.logs-container` bg | `rgba(10, 14, 24, 0.82)` | `var(--color-bg-surface)` |
| 15 | `components.css` | `.btn-secondary` bg | `rgba(255,255,255,0.08)` | `var(--color-bg-elevated)` |
| 16 | `components.css` | `.input` bg | `rgba(255,255,255,0.04)` | `var(--color-bg-elevated)` |
| 17 | `cards-inference.css` | `.card` bg | `rgba(255,255,255,0.045)` | `var(--color-bg-surface)` |
| 18 | `cards-hardware.css` | `.bar-fill` bg | hardcoded dark | `var(--color-primary)` |
| 19 | `layout.css` | `.scrollbar` thumb | `rgba(255,255,255,0.1)` | `rgba(17,24,39,0.15)` |
| 20 | `chat.css` | `.code-block` bg | hardcoded dark | `var(--color-bg-code)` |

### Implementation Strategy

#### Phase 1: Structural Containers (Highest Impact)

Replace hardcoded dark backgrounds on these elements with CSS variables:

```css
/* layout.css */
[data-theme="light"] .nav-sidebar { background: var(--color-bg-surface); }
[data-theme="light"] .topbar { background: var(--color-bg-surface); }
[data-theme="light"] .tab-bar { background: var(--color-bg-surface); }

/* base.css */
[data-theme="light"] body::before { mix-blend-mode: multiply; }
```

This alone would make ~60% of the UI respond to theme toggle.

#### Phase 2: Cards and Components

Replace hardcoded `rgba(255,255,255,0.X)` backgrounds with `var(--color-bg-surface)` and `var(--color-bg-elevated)`.

#### Phase 3: Chat and Logs

Chat messages, code blocks, and log output need comprehensive overrides for readability.

#### Phase 4: Modals

All modal shells and internal sections need light theme overrides (already started in settings-modal.css and agent-modal.css).

### Estimated Effort

| Phase | Files | Lines CSS | Priority |
|-------|-------|-----------|----------|
| Phase 1 | base.css, layout.css | ~50 | Critical |
| Phase 2 | components.css, cards-inference.css, cards-hardware.css | ~200 | High |
| Phase 3 | chat.css, logs.css, setup-view.css | ~300 | High |
| Phase 4 | All modal CSS files | ~150 | Medium |
| **Total** | **11 files** | **~700 lines** | |

### Notes for Future Agents

- The `tokens.css` variable system is complete and correct — the problem is adoption, not definition
- Every new CSS rule should use `var(--*)` instead of hardcoded colors
- Light theme overrides should be added at the END of each CSS file, grouped under `[data-theme="light"]` blocks
- Test light theme by toggling in the UI or setting `document.documentElement.dataset.theme = 'light'` in the browser console
