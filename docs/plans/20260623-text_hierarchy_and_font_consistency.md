# Text hierarchy + font consistency pass

**Date:** 2026-06-23
**Status:** Proposed (not started)
**Author:** design/UX pass following the architecture-label PR (`feat/arch-labels-and-ux-fixes`)

## Why this doc exists

Two threads converged into one design-system task:

1. While adding the Dense/MoE architecture labels we discovered the CSS referenced
   `--color-text-tertiary`, **a token that does not exist anywhere in the project**.
   The arch labels were remapped onto existing tokens (`--color-text-muted` /
   `--color-text-secondary`) as a stop-gap. The open question was whether to
   actually introduce a richer neutral-text scale to get the "modern/premium"
   layered feel that polished apps have.
2. Testing on Windows showed the UI font rendering **noticeably smaller than on
   macOS**, to the point of needing a manual font-scale bump in settings. The
   inconsistency itself is the bug — scale should match across platforms.

The decision for **right now** is: **leave the arch labels on existing tokens; do
not ship a half-used token.** This doc is the plan for doing it properly later,
app-wide, plus a font pass to fix the cross-platform sizing.

---

## Part A — App-wide neutral text hierarchy

### Current state

Defined text tokens (`static/css/tokens.css`):

| Token | Dark (`:root`) | Light (`[data-theme=light]`) | Role today |
|-------|----------------|------------------------------|------------|
| `--color-text-primary`   | `#f9fafb` (gray-50)  | `#020817`           | Headings, key values |
| `--color-text-secondary` | `#d1d5db` (gray-300) | `#1f2937` (slate-800) | Body / labels |
| `--color-text-muted`     | `#9ca3af` (gray-400) | `#4b5563` (slate-600) | Everything de-emphasized |
| `--color-text-inverse`   | `#111827`            | (inherits)           | Text on light chips |

Palettes (`cyber-rose`, `solar-violet`, `lava-core`) override **accents only** and
inherit the neutral text colors from the dark/light base. So any new neutral text
token only needs values in **two** scopes (`:root` and `[data-theme="light"]`) and
all palettes get it for free.

### The problem with the current 3-step scale

- It is **top-heavy in dark mode**: a large jump `primary → secondary`, then
  `secondary → muted` is only one Tailwind step (300 → 400). There is almost no
  room to wedge a "tertiary" between them — it would be nearly invisible.
- There is **no faint step below `muted`**. Captions, hints, timestamps,
  metadata, placeholder text, and disabled states all collapse onto `muted`. That
  flattening is what reads as "not premium" — there is no deliberate ambient layer.

### Recommendation: add a faint step, not a middle one

Introduce **one** new token below `muted` rather than squeezing one in the middle.
This is the higher-leverage change and matches how Apple HIG (label → secondary →
tertiary → quaternary), Radix, Linear, and Geist structure neutral text.

Proposed token (name chosen to read as "fainter than muted"):

```css
/* :root (dark) */
--color-text-faint: #6b7280;   /* gray-500 — below muted */

/* [data-theme="light"] */
--color-text-faint: #64748b;   /* slate-500 — below muted */
```

Resulting 4-step ambient → emphatic scale:

```
primary  > secondary > muted > faint
(values)   (labels)    (de-emph) (ambient metadata)
```

> Optional, only if a true mid-step is later wanted: `--color-text-tertiary`
> (`#b8bec8` dark / `#374151` light). Lower priority — the gap it fills is small in
> dark mode. Do **not** ship it unless something concretely needs it.

### Semantic usage guide (the part that actually creates the premium feel)

Defining the token does nothing on its own. The win is **consistent application**.
Establish and document these roles:

| Level | Token | Use for |
|-------|-------|---------|
| Primary   | `--color-text-primary`   | Section titles, headline metrics, active values |
| Secondary | `--color-text-secondary` | Body text, control labels, emphasized inline values |
| Muted     | `--color-text-muted`     | Secondary labels, inactive tab text, card subtitles |
| **Faint** | **`--color-text-faint`** | **Field hints, helper captions, timestamps, units, architecture/metadata labels, placeholder text, disabled controls** |

### Rollout strategy (phased, reviewable)

This is app-wide and should **not** be one mega-commit. Phase by surface so each
diff is reviewable and visually verifiable:

1. **Tokens** — add `--color-text-faint` to both scopes in `tokens.css`. No usage
   yet. (1 small commit.)
2. **Setup / launch / spawn** surfaces (the area this PR already touches) — move
   field-hints, card metadata, and the new arch labels onto `--color-text-faint`.
3. **Chat** surface — message timestamps, token/sec readouts, helper text.
4. **Dashboard / system panels** — units, sublabels, idle metadata.
5. **Modals / settings** — descriptions, hint rows, disabled state text.

Each phase: grep the surface's CSS for `--color-text-muted` on caption-like rules,
decide muted-vs-faint per rule, screenshot dark + light + one palette.

### Acceptance criteria (Part A)

- [ ] `--color-text-faint` defined in `:root` **and** `[data-theme="light"]`.
- [ ] No CSS references an undefined text token (grep for `--color-text-tertiary`
      returns nothing, or it is properly defined if adopted).
- [ ] A short "text hierarchy" section added to the design/token docs with the
      usage table above.
- [ ] Contrast: `faint` on the app background passes WCAG AA for non-essential
      text (it is informational, not interactive) — verify in both themes.
- [ ] Spot-check all three palettes inherit correctly (no hardcoded overrides
      needed).

---

## Part B — Font consistency pass (Mac vs Windows)

### Root causes found

Investigation of the current setup turned up four compounding issues:

1. **Fonts are loaded from the Google Fonts CDN, not bundled.**
   `tokens.css:1` (`@import`) and `index.html:32` (`<link>`) pull Inter + Fira Code
   from `fonts.googleapis.com`. Stacks:
   - `--font-body: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif`
   - When Inter loads slowly, is blocked offline, or is cached differently by the
     desktop webview, **macOS falls back to `-apple-system` (San Francisco)** and
     **Windows falls back to `Segoe UI`**. Segoe UI has a **smaller x-height** than
     SF/Inter at the same pixel size, so text *looks* materially smaller on
     Windows even when the px is identical. **This is the most likely primary
     cause of the reported discrepancy.** It also means FOUT, offline breakage,
     and a privacy/telemetry call to Google on every launch.

2. **px/rem mix in the type scale.** ~**590** hardcoded `font-size: NNpx` rules vs
   ~**448** rem/token rules (`var(--text-*)` + `NNrem`). The user font-scale
   setting works by setting `document.documentElement.style.fontSize`
   (`user-menu.js:235`), which only scales **rem** values. So bumping the scale
   grows ~448 rules and leaves ~590 fixed — uneven, and part of why a "small bump"
   felt off/inconsistent.

3. **`html` has no explicit base `font-size`** (`base.css:9`). The rem scale
   therefore hangs off the UA/platform default, which is influenced by OS DPI and
   webview defaults that differ between WKWebView (macOS) and WebView2 (Windows).

4. **No webview zoom factor is pinned.** Windows WebView2 honors the OS
   "text size" / display-scaling setting; macOS WKWebView does not in the same way.
   Nothing in the Rust/`wry` setup normalizes this.

### Fixes, ranked by impact

1. **Self-host Inter + Fira Code (highest impact, fixes #1).**
   - Add `static/fonts/` with woff2 files for the weights actually used
     (Inter 400/500/600/700/800, Fira Code 400/500/600).
   - Replace the CDN `@import`/`<link>` with local `@font-face` declarations.
   - Keep `-apple-system`/`Segoe UI` as fallbacks **only** for the pre-paint flash;
     with the font bundled, both platforms render Inter and the x-height
     difference disappears.
   - Bonus: removes the external network dependency (works offline, no Google
     call) — aligns with this being a local-first desktop app.

2. **Pin a root font-size and finish the rem migration (fixes #2, #3).**
   - Set an explicit base on `html` (e.g. `font-size: 16px`) so rem is anchored
     identically on every platform, and the font-scale setting has a stable base.
   - Audit the 590 hardcoded `px` font-sizes; migrate **type** sizes to the
     `--text-*` token scale (or rem) so the user font-scale setting affects the
     whole UI uniformly. Borders/icon dimensions can stay px — this is about text.
   - This is the largest chunk of work; do it surface-by-surface like Part A.

3. **Normalize webview zoom on Windows (fixes #4, if still needed after 1+2).**
   - Investigate setting a consistent `zoomFactor` / disabling OS-text-scaling
     influence in the `wry`/WebView2 setup so the logical px baseline matches
     macOS. Validate this is still necessary after self-hosting fonts — it may
     not be.

4. **Make the font-scale setting honest.** Once type is rem-based, confirm the
   setting scales everything; consider exposing a small/medium/large preset in
   addition to the numeric slider.

### Testing matrix

Validate on **both** platforms — use the Windows test box (per project notes) so
this isn't guessed:

| Check | macOS (M-series) | Windows (test box) |
|-------|------------------|--------------------|
| Default launch, no font-scale change | text size baseline | **must visually match Mac** |
| Inter actually rendering (not fallback) | ✓ | ✓ (inspect computed font-family) |
| Offline launch | fonts still render | fonts still render |
| Font-scale at 0.9 / 1.0 / 1.2 | all text scales together | all text scales together |
| Dark + light + one palette | no regressions | no regressions |

### Acceptance criteria (Part B)

- [ ] Inter + Fira Code bundled locally; no `fonts.googleapis.com` references
      remain in `tokens.css` / `index.html`.
- [ ] App renders identical type at default scale on macOS and Windows
      (side-by-side screenshots in the PR).
- [ ] `html` has an explicit base `font-size`; type sizes use the rem/token scale;
      remaining hardcoded `px` font-sizes are intentional and documented.
- [ ] Font-scale setting visibly scales the **entire** UI, not a subset.
- [ ] Offline launch renders bundled fonts (no CDN dependency).

---

## Sequencing & effort

| Step | Scope | Rough effort | Risk |
|------|-------|--------------|------|
| A1 — add `--color-text-faint` token | tokens.css | XS | none |
| B1 — self-host fonts | fonts + tokens + html | S | low (visual diff) |
| B2 — pin root size, px→rem type audit | app-wide CSS | M–L | medium (broad diff) |
| A2–A5 — apply faint token per surface | app-wide CSS | M | low (visual only) |
| B3 — webview zoom normalize (if needed) | Rust/`wry` | S | medium (platform) |

Suggested order: **B1 first** (it likely resolves the most-felt pain with the
smallest, safest diff), then **A1 + A2–A5** (the premium-feel hierarchy), then
**B2** (the larger px→rem cleanup), then **B3** only if a gap remains.

## Non-goals

- Not redesigning the type scale ramp itself (sizes/line-heights) — only making it
  consistent and rem-anchored.
- Not introducing `--color-text-tertiary` unless a concrete need appears; the faint
  step is the deliberate addition.
- Not touching accent/palette colors.

## References

- Stop-gap that prompted this: arch labels remapped off the undefined
  `--color-text-tertiary` in `setup-view.css` / `spawn-wizard.css`.
- Token definitions: `static/css/tokens.css` (`:root` ~L129, `[data-theme=light]`
  ~L314).
- Font loading: `tokens.css:1`, `index.html:32`, stacks at `tokens.css:151-152`.
- Font-scale wiring: `user-menu.js:235`, `settings.js:560`.
</content>
</invoke>
