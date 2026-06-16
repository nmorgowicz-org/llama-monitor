# Multi-Theme System Implementation Plan

## 1. Overview
The goal is to transition `llama-monitor` from a single-color-scheme application to a multi-theme application. The system should allow users to choose between different dark theme color palettes (e.g., "Carbon Mint", "Cyber Rose", "Solar Violet") through the settings interface. 

The target is to maintain the "modern" feel of the app—high-contrast gradients, subtle glows, and high-quality shadows—while making these elements fully dynamic and theme-aware.

## 2. Target Outcome
A robust system where the entire UI (including gradients, status indicators, and decorative elements) reacts to a top-level `data-theme` attribute on the `<html>` element.

## 3. Technical Architecture

### 3.1 Semantic Tokenization
The core of this project is moving from **color-based** variable names to **intent-based (semantic)** variable names. 

| Old Name (Avoid) | New Semantic Name (Target) | Purpose |
|------------------|----------------------------|---------|
| `--color-blue`   | `--color-accent`           | Primary branding and interactive elements |
| `--color-teal`   | `--color-success`          | Positive status, success badges, completions |
| `--color-red`    | `--color-error`            | Critical errors, deletion, warnings |
| `--color-yellow` | `--color-warning`          | Cautionary notes, pending states |
| `--color-indigo` | `--color-info`             | Informational tooltips, help text |

### 3.2 Implementation Pattern
All themes will be defined in `static/css/tokens.css` using the `[data-theme="name"]` selector pattern.

```css
/* static/css/tokens.css */

:root {
  /* Default (Carbon Mint) */
  --color-accent: rgba(45, 212, 191, 1);
  --color-accent-glow: rgba(45, 212, 191, 0.3);
  /* ... */
}

[data-theme="cyber-rose"] {
  --color-accent: rgba(244, 114, 182, 1); /* Pink/Magenta */
  --color-accent-glow: rgba(244, 114, 182, 0.3);
  /* ... */
}

[data-theme="solar-violet"] {
  --color-accent: rgba(167, 139, 250, 1); /* Bright Violet */
  --color-accent-glow: rgba(167, 139, 250, 0.3);
  /* ... */
}
```

## 4. Implementation Roadmap

### Phase 1: The "Great Tokenization" (Cleanup)
The most intensive phase. You must find every instance of hardcoded color values and replace them with semantic tokens.

**Areas of Interest:**
* **CSS Files**: Audit `static/css/spawn-wizard.css`, `static/css/chat-guided-generation.css`, and any component-specific CSS.
* **JavaScript Logic**: Audit `static/js/features/dashboard-render.js`, `static/js/core/format.js`, and `static/js/features/lhm.js`.
* **Gradients**: Do not just replace colors; replace the *logic*. Instead of `linear-gradient(..., rgba(99, 102, 241, 0.5), ...)`, use `linear-gradient(..., var(--color-accent-alpha-50), ...)`.

### Phase 2: Theme Definitions
Once the CSS/JS is fully tokenized, add the new theme blocks to `static/css/tokens.css`.

### Phase 3: User Interface Integration
1.  Modify `static/js/features/settings-modal.js` to include a theme selection dropdown.
2.  Update the persistence logic to save the chosen theme in `ui-settings.json`.
3.  Ensure the theme is applied to the `<html>` element on app startup.

## 5. Reference Palettes for 2026 Aesthetic

Use these as inspiration for the theme definitions.

| Theme Name | Mood | Primary Accent (Approx) |
|------------|------|--------------------------|
| `carbon-mint` | Professional, calm, technical | `#2dd4bf` (Teal/Emerald) |
| `cyber-rose` | High-energy, futuristic, Y2K | `#f472b6` (Pink/Magenta) |
| `solar-violet`| Deep space, cosmic, vibrant | `#a78bfa` (Bright Violet) |
| `lava-core`   | Intense, powerful, warning | `#ef4444` (Deep Red/Orange) |

## 6. Verification Checklist for AI Agents

- [ ] **No Hardcoded Colors**: Run a grep search for hex codes (`#...`) and `rgba(...)` in the `static/` directory. Most should only exist within the `tokens.css` file or as a very small set of functional constants.
- [ ] **Gradient Integrity**: Check that all "modern" gradients still look premium. They should use variable-driven alpha channels (e.g., `rgba(var(--accent-rgb), 0.2)`).
- [ ] **Theme Switching**: Verify that changing the theme in settings instantly updates the entire UI without a page reload.
- [ ] **Visual Regression**: Ensure that different themes don't break the readability of text or the visibility of important status indicators.
