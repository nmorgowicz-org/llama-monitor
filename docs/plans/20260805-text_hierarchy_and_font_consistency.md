# Text hierarchy + font consistency pass

**Date:** 2026-06-23 (source-audited 2026-08-13)
**Status:** 2.0 critical subset proposed; implementation not started
**Author:** design/UX pass following the architecture-label PR (`feat/arch-labels-and-ux-fixes`)

## 2.0 decision and scope

The Windows/macOS parity work moved the deterministic-font portion of this plan
onto the 2.0 critical path. The release should not accept screenshots whose
typeface depends on whether Google Fonts was reachable or happened to finish
loading before capture.

Gate 2.0 on the following subset:

1. **B0 — deterministic evidence:** capture waits for fonts, asserts the intended
   faces are loaded, and records computed typography and layout boxes for the
   high-impact comparison surfaces.
2. **B1 — local fonts:** Inter and Fira Code are bundled with their licenses;
   production makes no Google Fonts request.
3. **B2a — stable root contract:** default root size is explicitly `16px`, saved
   font scale is applied from that baseline, and an automated check covers the
   default plus at least `0.9`, `1.0`, and `1.2`.
4. **Visual gate:** fresh, same-commit Windows/macOS captures have no hierarchy,
   wrapping, clipping, overflow, or control-reachability defect on the critical
   surfaces listed below.

The complete px-to-rem migration (B2b) and neutral hierarchy rollout (Part A)
remain phased work. Pull a surface into the 2.0 subset when the visual gate or
font-scale check finds a real defect; do not hold 2.0 merely to mechanically
replace every intentional pixel value.

Evidence and pair classifications belong in
`docs/plans/evidence/20260811-local-llm-foundry/phase-13/windows-macos-visual-parity-ledger.md`.

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

No value is approved yet. The original candidates were:

```css
/* :root (dark) */
--color-text-faint: #6b7280;   /* gray-500 — below muted */

/* [data-theme="light"] */
--color-text-faint: #64748b;   /* slate-500 — below muted */
```

Those candidates do **not** meet WCAG AA for normal text even on the base
surfaces: `#6b7280` against `#0f1115` is about 3.91:1, and `#64748b` against
`#e9eef6` is about 4.08:1. The dark candidate loses still more contrast on
elevated surfaces such as `#2a2f3a`. A dark value around `#929baa` is required
to clear 4.5:1 on that elevated surface, which leaves little visual distance
from the current `#9ca3af` muted token. Part A therefore needs a surface-aware
contrast study before any token lands; the old hex values must not be copied
into production.

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
- [ ] Contrast: normal-size informational `faint` text passes WCAG AA against
      every surface where it is used; interactive text does not use a faint value
      that compromises its required contrast. Pure decoration is the only
      contrast-exempt case.
- [ ] Spot-check all three palettes inherit correctly (no hardcoded overrides
      needed).

---

## Part B — Font consistency pass (Mac vs Windows)

### Current source audit (2026-08-13)

The source still has the deterministic-font defects described below, with a
larger fixed-size footprint than the earlier estimate:

- `static/css/tokens.css` imports the Google stylesheet and `static/index.html`
  links it again. The same font request is declared twice.
- There are no font files under `static/`. CSP in `src/web/mod.rs` still permits
  `fonts.googleapis.com` and `fonts.gstatic.com` specifically for this path.
- The capture harness does not wait for `document.fonts.ready` or assert that
  Inter and Fira Code loaded. A successful PNG can therefore contain fallback
  text and still look like valid evidence.
- Current CSS contains 783 hardcoded `font-size: ...px` declarations and 493
  token/rem declarations. The three spawn-wizard stylesheets account for 281
  hardcoded declarations, including 42 at `9px` and 86 at `10px`. These counts
  are inventory, not a command to convert layout dimensions or every value
  mechanically.
- `static/css/base.css` does not set a root size. The saved appearance preference
  applies `fontScale * 16px` from JavaScript, but the default path has no explicit
  CSS baseline.

### Confirmed causes versus hypotheses

Investigation of the current setup turned up four compounding issues:

1. **Confirmed: fonts are loaded from the Google Fonts CDN, not bundled.**
   `static/css/tokens.css` (`@import`) and `static/index.html` (`<link>`) pull
   Inter + Fira Code from `fonts.googleapis.com`. Stacks:
   - `--font-body: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif`
   - When Inter loads slowly, is blocked offline, or is cached differently by the
     desktop webview, **macOS falls back to `-apple-system` (San Francisco)** and
     **Windows falls back to `Segoe UI`**. Segoe UI has a **smaller x-height** than
     SF/Inter at the same pixel size, so text *looks* materially smaller on
     Windows even when the px is identical. **This is the most likely primary
     cause of the reported discrepancy.** It also means FOUT, offline breakage,
     and a privacy/telemetry call to Google on every launch.

2. **Confirmed: px/rem mix in the type scale.** There are currently **783**
   hardcoded `font-size: NNpx` declarations vs **493** rem/token declarations
   (`var(--text-*)` + `NNrem`). The user font-scale
   setting works by setting `document.documentElement.style.fontSize`
   (`static/js/features/user-menu.js`), which only scales **rem** values. So
   bumping the scale grows the 493 token/rem declarations while leaving 783
   fixed-px declarations unchanged—uneven, and part of why a "small bump" felt
   off/inconsistent.

3. **Confirmed source gap, unconfirmed visual cause: `html` has no explicit base
   `font-size`** (`static/css/base.css`). Browsers normally default to `16px`, so
   this omission by itself does not prove the observed Windows difference. Pin it
   to make the contract explicit, then record the computed root size on both
   platforms rather than inferring it from a screenshot.

4. **Hypothesis: native webview zoom or OS text scaling.** Nothing currently
   proves that WebView2 zoom caused the discrepancy. Browser-harness captures and
   the packaged desktop webview are separate evidence classes. Measure
   `devicePixelRatio`, `visualViewport.scale`, computed root size, and native
   WebView2 zoom before considering a Rust-side normalization.

### Settings-modal 15px observation (source-confirmed 2026-08-13)

The isolated, same-viewport browser probe confirmed that the apparent Windows
split is a native-scrollbar layout difference, not a font metric or grid-track
change:

- `static/css/settings-modal.css` fixes the first grid track at `184px` and uses
  `minmax(0, 1fr)` for content.
- macOS (overlay scrollbars): body `clientWidth` is `958px`, tabs start at
  `x=241`, and content starts at `x=425`.
- Windows (classic scrollbars): body `clientWidth` is `928px` despite a `958px`
  border box; the reserved scrollbar gutters move tabs to `x=256` and content
  to `x=440`. The computed grid track remains `184px` on both platforms.
- The original Windows batch inherited real application state and remains
  invalid parity input; these values came from an isolated, seeded probe after
  the entrance animation settled.

The accepted fix changes only the body overflow to `hidden` and overrides the
inherited `scrollbar-gutter: stable both-edges` with `scrollbar-gutter: auto`;
the tabs and content panes retain their own scrolling. A native Windows
release rebuild and recapture now show `clientWidth=958px`, tabs at `x=241`,
and content at `x=425`, matching macOS at the shared viewport. No clipping or
control-reachability regression was observed. If glyphs still differ on other
surfaces, classify font loading and rasterization separately.

### Fixes, ranked by impact

1. **Self-host Inter + Fira Code (highest impact, fixes #1).**
   - Add `static/fonts/` with woff2 files for the weights actually used
     (Inter 400/500/600/700/800, Fira Code 400/500/600).
   - Include the upstream license files and record the exact font versions and
     source hashes in the dependency notice.
   - Replace both the CDN `@import` and `<link>` with local `@font-face`
     declarations. Remove the now-unused Google font allowances from the
     production CSP and update `docs/reference/security.md`.
   - Preload the body and mono faces needed for first paint. Keep an explicit
     fallback stack for load failure, but do not treat fallback rendering as a
     passing capture.
   - Run a release build so `build.rs` registers the new static assets, then
     verify each font route returns the expected WOFF2 content type and bytes.
   - Keep `-apple-system`/`Segoe UI` as fallbacks **only** for the pre-paint flash;
     with the font bundled, both platforms render Inter and the x-height
     difference disappears.
   - Bonus: removes the external network dependency (works offline, no Google
     call) — aligns with this being a local-first desktop app.

2. **Pin a root font-size, then phase the rem migration (fixes #2, #3).**
   - Set an explicit base on `html` (e.g. `font-size: 16px`) so rem is anchored
     identically on every platform, and the font-scale setting has a stable base.
   - Audit the 783 hardcoded `px` font-sizes; migrate **type** sizes to the
     `--text-*` token scale (or rem) so the user font-scale setting affects the
     whole UI uniformly. Borders/icon dimensions can stay px — this is about text.
   - This is the largest chunk of work; do it surface-by-surface like Part A.

3. **Normalize webview zoom on Windows only if native evidence requires it.**
   - Investigate setting a consistent `zoomFactor` / disabling OS-text-scaling
     influence in the `wry`/WebView2 setup so the logical px baseline matches
     macOS. Validate this is still necessary after self-hosting fonts — it may
     not be.

4. **Make the font-scale setting honest.** Once type is rem-based, confirm the
   setting scales everything; consider exposing a small/medium/large preset in
   addition to the numeric slider.

### Re-confirmed 2026-08-13 (spawn-wizard scope)

A cold-investigation subagent (looking into a Windows "UI too small to read"
report, unrelated in origin to this doc) independently re-found the exact
problem this doc's Part B covers. The wizard CSS has since been split across
`static/css/spawn-wizard-base.css`, `static/css/spawn-wizard-theme.css`, and
`static/css/spawn-wizard-pro.css`; together they currently contain 281 fixed-px
font-size declarations, including 128 at 9–10px. This remains systemic B2 work,
not a one-line fix, and needs the surface-by-surface rollout plus visual
verification described here.

### B0 — deterministic capture and diagnostic contract

Before accepting any cross-platform pair:

1. Run the release-built app from a disposable, identically seeded application
   home at the same commit, viewport, device scale factor, theme, palette,
   font-scale, and reduced-motion state.
2. Explicitly load the required Inter and Fira Code weights through the
   FontFaceSet API, await `document.fonts.ready`, require
   `document.fonts.status === "loaded"`, and verify matching faces report a
   loaded status. Computed `font-family` alone is not proof because it still
   reports the requested family when fallback glyphs render.
3. Record computed `font-family`, `font-size`, `line-height`, `font-weight`,
   bounding box, scroll dimensions, `devicePixelRatio`, and
   `visualViewport.scale` for the compared landmarks.
4. Fail closed on an external font request, fallback face, clipped/overflowing
   text, missing landmark, or mismatched fixture state.
5. Compare browser-harness screenshots separately from packaged WKWebView/
   WebView2 screenshots. Browser parity cannot close the native-webview gate.

Critical 2.0 landmarks: welcome/launch actions, navigation, settings tabs and
appearance controls, preset editor, wizard review, dashboard headings/metrics,
chat messages/composer/sidebar, migration copy, and updater/restart messaging.

### Testing matrix

Validate on **both** platforms — use the Windows test box (per project notes) so
this isn't guessed:

| Check | macOS (M-series) | Windows (test box) |
|-------|------------------|--------------------|
| Default launch, no font-scale change | text size baseline | **must visually match Mac** |
| Inter actually rendering (not fallback) | FontFaceSet assertion + computed style | FontFaceSet assertion + computed style |
| Offline launch | local fonts render; zero external font requests | local fonts render; zero external font requests |
| Font-scale at 0.9 / 1.0 / 1.2 | all text scales together | all text scales together |
| Dark + light + one palette | no regressions | no regressions |

### Acceptance criteria (Part B)

- [ ] Inter + Fira Code bundled locally; no Google Fonts references remain in
      production `static/` or `src/` code.
- [ ] App renders equivalent type metrics and hierarchy at default scale on
      macOS and Windows (side-by-side screenshots in the PR); rasterizer-level
      antialiasing differences are allowed when wrapping and geometry match.
- [ ] `html` has an explicit base `font-size`; type sizes use the rem/token scale;
      remaining hardcoded `px` font-sizes are intentional and documented.
- [ ] Font-scale setting visibly scales the **entire** UI, not a subset.
- [ ] Offline launch renders bundled fonts (no CDN dependency).
- [ ] Capture harness waits for fonts and fails closed on fallback/external font
      traffic; receipts include computed root/font/layout diagnostics.
- [x] The settings body/tabs/content boxes prove the `184px` split within 1 CSS
      px on both platforms after the documented scrollbar-gutter fix and
      release-built recapture.
- [ ] Critical browser captures and packaged native-webview captures are both
      accepted; neither is used as a substitute for the other.

### 2.0 pass gate

The typography subset is green for 2.0 only when:

- [ ] B0, B1, and B2a are complete on the same release candidate commit.
- [ ] Release-built font routes, offline launch, CSP removal, licenses, and
      request-level no-external-font checks pass.
- [ ] Default and scaled critical landmarks have no wrapping, clipping,
      overflow, hierarchy, focus, or reachability regression.
- [x] The provisional settings-modal 15px observation is replaced by paired
      computed-box evidence and a final scrollbar-gutter classification.
- [ ] Browser-harness and packaged native-webview receipts exist for macOS and
      Windows; expected rasterization differences are distinguished from layout
      differences.
- [ ] Any B2b work deferred from 2.0 has an explicit inventory and is not hiding
      a failed critical-surface check.

Stop the 2.0 cutover if required fonts fall back, external font traffic occurs,
fixture state differs, the compared commit/viewports differ, a critical landmark
is missing, or a functional layout/accessibility defect remains. Fix and
recapture; do not waive or hand-edit screenshot evidence.

---

## Sequencing & effort

| Step | Scope | Rough effort | Risk |
|------|-------|--------------|------|
| A1 — add `--color-text-faint` token | tokens.css | XS | none |
| B0 — deterministic font/layout receipts | capture harness + evidence | S | low |
| B1 — self-host fonts | fonts + tokens + html + CSP/docs | S | low (visual diff) |
| B2a — pin root size + scale contract | base CSS + settings tests | S | low–medium |
| B2b — px→rem type audit | app-wide CSS | M–L | medium (broad diff) |
| A2–A5 — apply faint token per surface | app-wide CSS | M | low (visual only) |
| B3 — webview zoom normalize (if needed) | Rust/`wry` | S | medium (platform) |

2.0 order: **B0 baseline → B1 → B2a → B0 recapture/acceptance**. Pull only failing
critical surfaces forward from B2b. Continue **A1 + A2–A5** and the remaining
B2b migration as reviewable surface phases; attempt B3 only if packaged native
evidence still shows a platform gap after deterministic fonts and root sizing.

## Non-goals

- Not redesigning the type scale ramp itself (sizes/line-heights) — only making it
  consistent and rem-anchored.
- Not introducing `--color-text-tertiary` without a concrete need. A faint step
  also remains contingent on the surface-aware contrast study above.
- Not touching accent/palette colors.

## References

- Stop-gap that prompted this: arch labels remapped off the undefined
  `--color-text-tertiary` in `static/css/setup-view.css` and the split
  `static/css/spawn-wizard-*.css` stylesheets.
- Token definitions and font stacks: `static/css/tokens.css`.
- Font loading: `static/css/tokens.css` and `static/index.html`.
- Font-scale wiring: `static/js/features/user-menu.js` and
  `static/js/features/settings.js`.
