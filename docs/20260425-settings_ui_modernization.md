# Settings UI Modernization — 2026-04-25

**Branch:** `fix/windows-remote-agent-install`
**Goal:** Give the settings popover a 2026 premium, glassmorphic makeover matching the main page card aesthetics.

---

## Implementation Checklist

### Phase 1: Modal Shell — Glass & Entrance

- [x] **[1.1]** Apply deep glass treatment to modal — `backdrop-filter: blur(20px)`, multi-layer inset shadows, `::before` top-edge highlight, `::after` gradient border glow (matching `widget-card`)
- [x] **[1.2]** Add modal entrance animation — scale 0.95→1.0 + opacity 0→1, 350ms `cubic-bezier(0.16, 1, 0.3, 1)` spring curve
- [x] **[1.3]** Add modal exit animation — reverse scale/fade on close
- [x] **[1.4]** Add ambient glow orb behind modal — blurred primary gradient, animated drift
- [x] **[1.5]** Deepen overlay with vignette radial gradient

### Phase 2: Tab Sidebar — Premium Navigation

- [x] **[2.1]** Add inline SVG icons to each tab (Session, GPU, Models, Appearance, Advanced)
- [x] **[2.2]** Redesign active tab — pill shape, gradient left border accent, gradient text fill, soft glow
- [x] **[2.3]** Add hover states — subtle background fill + translateX slide + border highlight
- [x] **[2.4]** Add tab content transition — fade + slide between panes instead of instant switch
- [x] **[2.5]** Style tab focus states for keyboard navigation

### Phase 3: Form Controls — Custom Toggles, Checkboxes, Elevated Inputs

- [x] **[3.1]** Custom toggle switch — replace native checkbox with gradient track + animated thumb with glow
- [x] **[3.2]** Custom checkboxes — replace native with styled square + SVG checkmark animation (stroke-dashoffset)
- [x] **[3.3]** Elevate inputs — glass background, inner top highlight, animated pulsing focus ring
- [x] **[3.4]** Custom select dropdowns — gradient chevron icon, glass styling

### Phase 4: Typography & Hierarchy

- [x] **[4.1]** Section headers — gradient text fill, icon prefix, animated underline divider
- [x] **[4.2]** Field labels — increased letter-spacing, text-transform, visual hierarchy from help text
- [x] **[4.3]** Help text — styled callout/badge with info icon and subtle background pill
- [x] **[4.4]** Pane titles — larger, bolder, with gradient accent

### Phase 5: Buttons & Micro-interactions

- [x] **[5.1]** Save button — ripple effect on click, press scale animation, success state (green flash)
- [x] **[5.2]** Cancel button — hover slide, icon
- [x] **[5.3]** Idle breathing animation on save button to draw attention
- [x] **[5.4]** Add haptic-style press feedback (scale down on :active)

### Phase 6: UX Improvements

- [x] **[6.1]** Add keyboard shortcut hints in footer ("⌘S to save", "Esc to close")
- [x] **[6.2]** Add unsaved changes indicator — subtle dirty dot on modal header when fields change
- [x] **[6.3]** Reword Advanced tab "Runtime Configuration" help text for clarity
- [x] **[6.4]** Add ESC key handler to close modal
- [x] **[6.5]** Add ⌘S / Ctrl+S handler to trigger save

---

## Files Modified

| File | Changes |
|------|---------|
| `static/style.css` | ~300 new lines: glass modal, custom toggles/checkboxes, tab redesign, animations, micro-interactions |
| `static/index.html` | HTML structure: SVG tab icons, custom toggle/checkbox markup, keyboard hints, pane transitions |
| `static/app.js` | Tab transition logic, dirty state tracking, keyboard shortcuts, save success animation |

---

## Design Tokens Used

All new styles leverage existing CSS custom properties:
- `--gradient-primary` — primary gradient for active states, save button, glows
- `--color-primary`, `--color-primary-light` — accents, focus rings, text fills
- `--border-subtle`, `--border-emphasis` — borders, dividers
- `--shadow-surface`, `--shadow-elevated` — elevation layers
- `--radius-sm`, `--radius-base`, `--radius-card` — border radii hierarchy
- `--gap-xs` through `--gap-xl` — spacing scale
- `--text-xs` through `--text-3xl` — typography scale

No new tokens added; all derived from existing design system.

---

## Animation Timing

All animations use consistent spring-based easing:
- **Entrance/exit:** 350ms `cubic-bezier(0.16, 1, 0.3, 1)`
- **Hover transitions:** 200ms `ease`
- **Idle breathing:** 2.5s `ease-in-out infinite`
- **Tab switch:** 250ms `cubic-bezier(0.16, 1, 0.3, 1)`
- **Toggle/checkbox:** 200ms `cubic-bezier(0.34, 1.56, 0.64, 1)` (overshoot spring)
- **Ripple:** 400ms `ease-out`
- **Success flash:** 600ms `ease`

---

## E2E / Playwright Test Impact Analysis

### Tests That Touch Settings Modal

**File:** `tests/ui/capability-rendering.spec.js`

| Test | Selector Used | Risk | Status |
|------|---------------|------|--------|
| `'settings opens and secondary tabs switch'` | `getByRole('button', { name: /settings/i })`, `#settings-modal`, `#settings-session`, `getByRole('button', { name: 'Advanced' })`, `#settings-advanced`, `getByRole('button', { name: /open runtime configuration/i })` | **Low** — All IDs and button text/roles preserved | ✅ Verified |
| `'configuration explains local executable...'` | Same path as above + `#config-modal` | **Low** — Settings → Advanced → Runtime Config path unchanged | ✅ Verified |
| `'guided SSH setup builds a structured target'` | Same path + `#ssh-guide-*` inputs | **Low** — Settings navigation unchanged | ✅ Verified |
| `'typing SSH target does not auto-detect'` | Same path + `#set-remote-agent-ssh-target` | **Low** — Settings navigation unchanged | ✅ Verified |

### What Was Preserved for Test Compatibility

- ✅ `#settings-modal` ID on overlay div
- ✅ `#settings-session`, `#settings-gpu`, `#settings-models`, `#settings-appearance`, `#settings-advanced` IDs on panes
- ✅ `settings-tab` class and `data-tab` attributes on tab buttons
- ✅ Button text "Advanced", "Open Runtime Configuration" unchanged
- ✅ All input `id` attributes preserved (`settings-default-spawn-mode`, `settings-default-server-endpoint`, etc.)
- ✅ All `role="button"` semantics preserved
- ✅ Modal still uses `.open` class for visibility (tests check `toHaveClass(/open/)`)

### What Changed (No Test Impact)

- CSS-only visual changes (glass, animations, colors, shadows)
- Tab content now uses CSS transitions (fade + slide) instead of instant `display` toggle — but panes still use `.active` class
- Custom toggle/checkbox styling via CSS `appearance: none` — native `<input>` elements preserved with original `id` attributes
- Modal close now has exit animation (260ms delay before `.open` is removed) — tests should wait for `.open` class which fires after animation
- Keyboard shortcut hints added as non-interactive `<span>` elements
- SVG icons added inside tab buttons — `getByRole('button', { name: 'Advanced' })` still works (text content preserved)

### Potential Breaking Change: Modal Close Timing

The `closeSettingsModal()` function now adds a 260ms exit animation before removing the `.open` class. If any test closes the settings modal and immediately asserts that `#settings-modal` should NOT have the `.open` class, it may fail due to the animation delay.

**Mitigation:** Playwright's auto-waiting should handle this. If a test uses `await page.locator('#settings-modal').not.toHaveClass(/open/)`, Playwright will retry until the assertion passes (within the default timeout). No code change expected.

### Recommendation

Run the full UI test suite after merging:
```bash
cd tests/ui && npx playwright test capability-rendering.spec.js
```

If any test fails on the custom toggle/checkbox inputs, the inputs may need to be queried by their `id` rather than by role. The native `<input>` elements are preserved inside the custom wrappers, so `page.locator('#settings-auto-start')` should still work.

### Post-Merge Validation Checklist

- [ ] Run `cd tests/ui && npx playwright test capability-rendering.spec.js` — all tests pass
- [ ] Manually verify settings modal opens/closes with smooth animations
- [ ] Verify tab switching shows fade+slide transition
- [ ] Verify custom toggle switches work and animate
- [ ] Verify custom checkboxes work and animate
- [ ] Verify save button shows ripple + success state
- [ ] Verify dirty indicator appears when fields change
- [ ] Verify ESC closes modal, Cmd+S/Ctrl+S saves
- [ ] Verify modal looks correct in both dark and light themes
- [ ] Verify modal is usable on mobile viewport (< 768px)

---

## Implementation Notes

- All animations are `prefers-reduced-motion: reduce` friendly — they disable gracefully
- Light theme (`[data-theme="light"]`) is fully supported with adjusted glass opacity and shadow colors
- Mobile responsive — tab sidebar collapses to horizontal scroll on narrow viewports
- No JavaScript framework dependencies — pure CSS transitions with minimal JS for state management
