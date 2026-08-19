# Spawn Wizard — UI/UX Redesign

**Status:** Archived — Option A ("Guided") implemented and verified 2026-08-06. **State:** Done.

**Origin:** Opus sub-agent study ("Spawn wizard UI/UX redesign proposal"), dispatched 2026-08-06 to
answer "how should the spawn wizard's Step 2 (hardware/tuning) be redesigned so it stops being an
~800-line unnavigable scroll, for both a general user (sensible-defaults, collapsed) and an advanced
user (everything visible), without pages of scrolling for either." Reproduced in full below —
nothing condensed, no wireframe or table cut — per explicit instruction to preserve every detail as
a single source of truth for a fresh-context implementer.

Grounded in: `static/index.html` (3519–5012), `static/js/features/spawn-wizard-groups.js`, `-ia.js`,
`-llama-ia.js`, `-mlx-ia.js`, `spawn-wizard.js`, `spawn-wizard-vram-display.js`,
`docs/reference/{spawn-wizard,inference-tuning,rapid-mlx-runtime}.md`.

Line numbers below are as reported by the sub-agent at time of writing (2026-08-06); re-verify
against current file state before relying on them for exact edits — the codebase has continued to
change (chat-template lifecycle work, IA registry refactor) since the agent's read.

---

## 1. What's wrong today

**P1 — Step 2 is a single ~800-line vertical stack with no navigation.** `#wizard-step-2` spans
`static/index.html:3947–4759`. Its `.wizard-main` column serially contains: the Rapid panel
(`#rapid-hardware-panel`, 3949) *plus* the whole MLX field dump (`#spawn-rapid-advanced-fields`,
3957–4210), then the llama.cpp header (`#hw-model-header`, 4221), `.hardware-grid` (4252),
`.kv-inline-row` (4300), `#hw-mtp-section` (4320), three warning divs (4374–4377),
`#spawn-advanced-fields` with five generated groups + `#spawn-spec-details` (4384–4565),
`#hf-download-panel` (4568), `#spawn-hardware-summary` (4611), `#wizard-advisor` (4623), and two
`<details>` sweeps (4632, 4642). There is no way to jump to "the KV stuff" — you scroll. This *is*
the "pages and pages of scrolling."

**P2 — Tier does double duty and neither job well.** `spawn-wizard-ia.js:15` `isOpenForProfile()`
compares group tier to profile rank to decide `<details>.open`, while `spawn-wizard.js:2009–2019`
uses the *same* tier to disable+autofill. So "how expert am I" is conflated with "what am I looking
at right now." A Balanced user who wants to check threads must open a group whose tier says
Advanced; an Advanced user gets *every* group open at once, which is exactly the wall of fields (P1)
with no mitigation. Tier is an author-time taxonomy leaking into runtime layout.

**P3 — Quick disables controls with no derived answer.** `spawn-wizard-groups.js:96` exempts
`spawn-context-size` and `spawn-batch-size` from the `quickValue` requirement, and the file itself
calls this a "pre-existing, product-accepted exception." Result: a Quick user sees Context size
greyed out showing `8192` (the static `value="8192"` at `index.html:4273`) with no indication
whether that's a decision or a leftover. Disabled-without-explanation is the classic form
anti-pattern.

**P4 — A live-looking dead knob.** `#spawn-kv-cache-dtype` (`index.html:3969`) is an enabled
`<select>` whose own hint (4974) says *"This build always launches with int8 active KV, regardless
of this selection."* `spawn-wizard-groups.js:66` even tags it `effective: 'reasoning-pins-int8'` —
but nothing renders that metadata. Same for `spawn-turboquant-mode` (`effective:
'turboquant-withheld'`, hint at 4102 says "requested but not applied") and
`spawn-rapid-pflash-policy` (`effective: 'pflash-off'`). Three controls that lie by looking editable.

**P5 — The same concept appears in 2–3 places, in different collapse states.** KV precision is
`#spawn-cache-type-k/v` (4302) *and* the scenario cards (`spawn-wizard-vram-display.js:436–465`)
*and* implicitly the "Context fit modes" header (4706). Prompt cache is
`#spawn-cache-mode`/`#spawn-cache-ram` (4455) for llama.cpp and
`#spawn-rapid-cache-mode`/`#spawn-retained-cache-mib` (3977/3986) for MLX, unlabelled as siblings.
Speculative decoding is `#hw-use-mtp` (4328, a checkbox in an always-visible section) *and*
`#spawn-spec-type`'s `draft-mtp` options (4514, inside a collapsed `<details>` inside another
collapsed group). Users must reconcile two controls for one decision.

**P6 — Provenance is invisible.** `applyUseCaseKvDtype()` (`spawn-wizard.js:1489–1500`) silently
rewrites `cacheTypeK/V` from the step-0 use-case card until `kvDtypeUserSet` flips. Good behaviour,
zero surface: the user picks "Agentic" on step 0, arrives on step 2 two clicks later, and sees
`q8_0` selected with no "because you chose Agentic" and no way to see it will stop tracking once
touched. Same for `#ctx-model-max-hint` and the advisor.

**P7 — "Review" is a separate step that mostly isn't review.** `#wizard-step-3` (4761) is titled
"Review your configuration" but is actually *eight more sections of editable settings* — sampling
grid (4775), thinking (4821), response shaping (4856), network exposure (4902), extra args (4929) —
with `#spawn-summary-list` (4938) buried at the bottom. Then step 4 (`#preset-params-table`) and step
5 (`#spawn-config-card`) show the config *again*. Three summaries, one of which is a config editor.

---

## 2. Shared foundation (both options)

Both options replace the 6 steps with **3**, and both are the *same DOM* with a mode toggle —
answering "why have two different surfaces":

| Today | Proposed |
|---|---|
| 0 Profile + use-case | folded into ① header (profile becomes a **view mode**, use-case stays as an intent chip) |
| 1 Model | ① **Model** (keeps all the room it has today, plus more) |
| 2 Hardware | ② **Tune** (redesigned; this document's subject) |
| 3 Review (actually 8 editors) | its editors move *into* ② as sections; the summary becomes a persistent drawer |
| 4 Preset settings | preset name+save moves into the ③ launch bar |
| 5 Spawn | ③ **Launch** |

Three cross-cutting mechanisms, both options:

- **Sticky context bar** — model, quant, loader, use-case, and the VRAM stacked bar (`#vram-bar` +
  `.vram-legend`, `index.html:4669–4693`) pinned to the top of ② at ~96px, always visible while
  scrolling. Precedent: Figma's persistent properties header, Stripe Checkout's order summary rail.
  This is *the* fix for "I changed threads, did my memory just blow up?"
- **Effective-value readout** — any control with an `effective:` key in `spawn-wizard-groups.js`
  renders as a **locked row**: struck-through/greyed control + a `Effective: int8 · pinned by
  reasoning profile` chip + `Why?` popover quoting `docs/reference/rapid-mlx-runtime.md:76`.
  Precedent: Xcode Build Settings "Resolved" column, `kubectl` effective config, Chrome
  `chrome://policy` "managed by". Kills P4 with zero new backend.
- **Provenance chips** — every auto-seeded value gets a small `auto · Agentic` chip; touching the
  control flips it to `you`. This is literally `kvDtypeUserSet` (`spawn-wizard.js:1491`) made
  visible. Precedent: Xcode's bold-when-overridden, Lightroom's reset dots.

---

## 3. Option A — **Guided** (general user)

**Thesis:** step ② is not a settings page, it's **four decisions** with a memory budget attached.
Everything else is a drawer.

### Layout pattern

Two-pane: sticky context bar + **decision cards** (main) + **sticky budget inspector** (right rail,
reuses `.hw-vram-sidebar`). Everything not a decision goes into one **"All settings" drawer** that
opens as an overlay panel *within step ②* — no step transition (progressive disclosure without
navigation cost; precedent: macOS System Settings "Advanced…" sheets, Lightroom's collapsed panel
groups).

### Wireframe

```
┌───────────────────────────────────────────────────────────────────────────────────────────────┐
│  ① Model ──────── ② Tune ──────── ③ Launch                        View: [ Guided ▾ ]   ⌘K Find a setting…    │
├───────────────────────────────────────────────────────────────────────────────────────────────┤
│ 🧠 Qwen3.6-27B-Instruct · Q4_K_M · 15.8 GB   ⚙ llama.cpp   🤖 Agentic          [change model] [change quant]  │ sticky
│ ████████████████ weights 15.8 │████ KV 4.2│▓ 1.1│░░░░░░░░ free 26.9        21.1 / 48.0 GB   ✓ Comfortable    │
├───────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                               │  MEMORY BUDGET            48.0 GB   [Explain]│
│ ▌ 1  How much context?                        auto · Agentic  │  ┌──────────────────────────────────────┐  │
│ ▌    Longer context = more KV memory.                         │  │████████████████ ███ ▓ ░░░░░░░░░░░░░░░░░░ │  │
│   ┌───────┬───────┬──────────┬───────┬───────┬──────────┐  │  └──────────────────────────────────────┘  │
│   │  32k  │  65k  │ ● 131k     │ 160k  │ 200k  │ 262k max  │  │   Weights 15.8 · KV 4.2 · OH 1.1 · Free 26.9 │
│   │Focused│  RP   │   Agent    │ Large │ Large │ Model max │  │                                              │
│   └───────┴───────┴────────────┴───────┴───────┴──────────┘  │   Current context   131k  ✓ within trained   │
│   or type ▸ [ 131072      ]        KV cost at this size: 4.2GB│                                              │
│                                                               │   CONTEXT FIT MODES                          │
│ ▌ 2  Cache precision (KV)                     auto · Agentic  │   ┌────────────────────────────────────┐   │
│ ▌    How exactly past tokens are stored.                      │   │ ★ Reliable agents      131k tokens   │   │
│   ┌────────────────────┬──────────────────┬──────────────┐  │   │   q8_0 KV · fits, 26.9 GB free       │   │
│   │ ★ q8_0  recommended  │  q4_0  +context  │  f16 lossless│  │   ├────────────────────────────────────┤   │
│   │   for agentic work   │  ⚠ tool calls    │  most memory │  │   │   More context         131k tokens   │   │
│   │   4.2 GB KV          │  2.1 GB KV       │  8.4 GB KV   │  │   │   q4_0 KV · ⚠ agentic risk           │   │
│   └────────────────────┴──────────────────┴──────────────┘  │   ├────────────────────────────────────┤   │
│   ⓘ You picked Agentic. q4_0 KV degrades tool-call           │   │   Full precision       131k tokens   │   │
│     reliability — pick it only if you need the headroom.      │   │   f16 KV · tight, 22.7 GB free       │   │
│                                                               │   └────────────────────────────────────┘   │
│ ▌ 3  Vision                                    not detected   │                                              │
│ ▌    Multimodal projector (mmproj).                           │   [ ⚡ Auto-size for me ]                     │
│   ( none — text only ▾ )  [Browse…]                           │                                              │
│                                                               │   SYSTEM RAM  (discrete GPU only)   64 GB    │
│ ▌ 4  Speed boost                              auto · on       │   ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░         │
│ ▌    Predict tokens ahead, verify in one pass. Free speed.    │                                              │
│   ◉ On — MTP heads detected (draft-mtp + ngram-mod)           ├──────────────────────────────────────────────┤
│   ○ N-gram only     ○ Off                                     │  ⚡ ADVISOR                                   │
│   ▸ tuning: draft tokens 2, n-min —, p-min —, draft KV q8_0   │  • MoE model: try n_cpu_moe auto-tune        │
│                                                               │  • -fa on required for quantized KV ✓        │
│ ───────────────────────────────────────────────────────────  ├──────────────────────────────────────────────┤
│   Everything else is on safe defaults.                        │  ⬇ DOWNLOAD                                  │
│   [ ⚙ All settings (23) ]   [ 3 changed from default ▾ ]      │  Save to ~/models   [Download] [Stream]      │
└───────────────────────────────────────────────────────────────────────────────────────────────┘
                                       [ ◀ Model ]      [ Launch ▶ ]
```

No `<select>`. `Why?` shows the quote from `rapid_mlx_runtime.rs:749` /
`docs/reference/rapid-mlx-runtime.md:76–83`.

### Open vs collapsed by default — and why

**Always open (the four cards):** context size, KV precision, vision/mmproj, speed boost.
Rationale: these are the only four things that (a) change the VRAM bar visibly, (b) have a
*correctness* consequence the user can't recover from without restarting the server, and (c) are
use-case-dependent. Everything in `#spawn-advanced-fields` fails all three tests.

**Always visible but not a card:** the budget rail, advisor, download panel, warnings
(`#ctx-fit-warning`, `#ctx-train-warning`, `#ctx-vram-warning`) — these render *into* the affected
card, not at the bottom of the page (P1).

**Collapsed into the drawer, but never hidden:** GPU layers, MoE/tensor split, prompt cache,
auto-fit, mlock, KV unified, batching/threads/priority/flash-attn, sweeps, sampling, thinking,
response shaping, network, extra args. The button says **"All settings (23)"** with a live count and
a **"3 changed from default"** chip — so the existence of every knob is advertised even when
collapsed (requirement 5).

**Never a lie:** MLX's `spawn-kv-cache-dtype`, `spawn-turboquant-mode`, `spawn-rapid-pflash-policy`
render as locked effective-value rows in the drawer, not selects.

### Scenario walkthroughs

**S1 — llama.cpp, general/roleplay, q4_0 KV is fine.**
User picks "Roleplay" on ①. On ② card 2 is pre-seeded (`applyUseCaseKvDtype`,
`spawn-wizard.js:1493`) to `q4_0` with the chip `auto · Roleplay`, the `★ recommended` star sits on
the q4_0 tile (today `scenario.rec` already does this, `vram-display.js:453`), and the ⚠ agentic
warning text is *absent* because the copy is use-case conditional. The user does nothing. Zero
hunting, zero anxiety — the low-fidelity choice is the pre-selected, star-marked, non-warned one.
Total interaction: none.

**S2 — llama.cpp, agentic, needs q8_0.**
Use-case Agentic seeds `q8_0` (`USE_CASE_TO_KV_DTYPE`). Card 2 shows `★ q8_0 recommended for
agentic work` selected with chip `auto · Agentic`. The q4_0 tile is **not disabled** — it carries `⚠
tool calls` and, on hover/select, the line already in the codebase: *"Fits more tokens, but lower
cache precision can hurt tool-call coherence"* (`vram-display.js:452`), upgraded to name the
documented finding and link `docs/reference/inference-tuning.md`. Selecting it flips the chip to
`you` and drops a persistent amber note into the card. Steering without restriction: the default is
right, the override is one click, the consequence is named. Crucially the user *never scrolls* to
reach this — it's the second card.

**S3 — Rapid-MLX, any use-case, int8 pin.**
Card 2 has no control at all (wireframe above). The budget rail's fit modes for MLX stop being
KV-quant cards and become **context-size** cards (`renderMlxScenarioCards`, `vram-display.js:~690`
already diverges here) — correct, since KV dtype is fixed and context is the free variable
(`docs/reference/rapid-mlx-runtime.md:134–141`). Agentic and general users get identical KV; the
difference shows up in card 4 (speculation off — the hint at `index.html:4041` says Rapid 0.11.1
only engages on greedy unconstrained requests, so it's actively wrong for agentic) and in sampling
mode. The user reads one sentence and moves on instead of picking int4 and being silently
overridden.

---

## 4. Option B — **Pro** (advanced user, nothing collapsed)

**Thesis:** show everything, but never in one column and never in scroll-order. Pattern: **left-rail
settings navigator + dense multi-column panes + search filter + modified-highlighting** — i.e. VS
Code Settings / Xcode Build Settings / Ableton device rack, not an accordion.

### Wireframe

```
┌───────────────────────────────────────────────────────────────────────────────────────────────┐
│  ① Model ─── ② Tune ─── ③ Launch          View: [ Pro ▾ ]   ⌘K [ kv cache____ ]  ☐ Modified only (4)  ↺ Reset all│
├───────────────────────────────────────────────────────────────────────────────────────────────┤
│ 🧠 Qwen3.6-27B-Q4_K_M 15.8GB │ llama.cpp │ 🤖 Agentic │ ctx 131k │ KV q8_0 │ -fa on │ par 1 │ spec draft-mtp      │
│ ████████████████ 15.8 │████ 4.2│▓1.1│░░░░░░░░░ 26.9      21.1/48.0 GB  ✓ Comfortable   [Explain] [⚡Auto-size]   │
├───────────────────┬───────────────────────────────────────────────────────────────────────────┤
│ ▸ CORE           │ ▌ CONTEXT & KV CACHE          ───────────────────────────────────────────    │
│   ● Context & KV │   Context window, cache precision, unified pool, prefix cache.                │
│   ○ Placement    │                                                                               │
│   ○ Batching     │   Context size    [131072    ]  32k 65k ●131k 160k 200k 262k     ⓘ trained max 262144         │
│   ○ Speculative  │   K cache quant   ( q8_0 — recommended ▾ )   ⓘ auto·Agentic       KV @131k = 4.2 GB           │
│   ○ Vision       │   V cache quant   ( q8_0 — recommended ▾ )   ⚠ must match K                                   │
│   ○ Prompt cache │   KV unified      ( Default ▾ )              Flash attn ( on ▾ )  ⓘ required for quant KV     │
│ ▸ SAMPLING       │   Prompt cache    ( Custom ▾ )  -cram [8192 ] MiB     Auto-fit ( Default ▾ ) target [    ] MB │
│   ○ Defaults     │                                                                               │
│   ○ Thinking     │ ▌ PLACEMENT & MEMORY          ───────────────────────────────────────────    │
│   ○ Shaping      │   GPU layers ( Auto ▾ ) [   ]    n_cpu_moe [auto] [⚡Auto-tune][Verify]   Tensor split [    ]  │
│ ▸ SERVER         │   mlock ( off )    ⚠ avoid on unified memory                                  │
│   ○ Network      │   ┌ MoE expert offload ─────────────────────────────────────────────────┐                      │
│   ○ Extra args   │   │ GPU ●──────────────────────────── CPU     0 of 48 on CPU      │                      │
│ ▸ TOOLS          │   └────────────────────────────────────────────────────────────────┘                      │
│   ○ Benchmarks   │                                                                               │
│   ○ Download     │ ▌ BATCHING & THREADS          ───────────────────────────────────────────    │
│                  │   batch [2048]  ubatch [2048]  parallel [1]🔒MTP   threads [—]  tb [—]  prio ( default ▾ )    │
│ ─── ADVISOR ───  │                                                                               │
│ • -fa on ✓       │ ▌ SPECULATIVE DECODING        ───────────────────────────────────────────    │
│ • MoE: autotune  │   Mode ( draft-mtp,ngram-mod — recommended ▾ )    n-max [2]  n-min [ ]  p-min [ ]             │
│ • q8_0 KV: agent │   Draft KV  K ( q8_0 ▾ )  V ( q8_0 ▾ )     Draft model [/path/…gguf ] [Browse…]               │
│   floor ✓        │   candidates: [qwen3.6-0.6b-draft.gguf] [+2 more]      ⓘ forces --parallel 1                  │
└──────────────────┴───────────────────────────────────────────────────────────────────────────────────────────────┘
```

### How it avoids being a wall of fields

1. **Left rail = random access.** Nine to eleven anchors; clicking scroll-spies the pane. Max scroll
   inside any one pane is ~1.5 viewports, and the rail never leaves. Precedent: VS Code Settings,
   macOS System Settings, Xcode.
2. **⌘K filter across every control.** Typing `kv` hides everything else and shows matching rows
   from all sections with their section breadcrumb. This is the definitive answer to "I know the
   setting exists, I don't know where it lives" — and it makes P1's discovery problem structurally
   impossible. Precedent: VS Code settings search, Figma quick actions.
3. **"Modified only (4)" toggle + bold-when-non-default.** Xcode's build-settings trick. An expert's
   real question is "what did I change?", answered in one click without a review step.
4. **Multi-column dense rows.** Related scalars share a line (`batch / ubatch / parallel / threads /
   tb / prio` is one row, not seven stacked `.hardware-field`s). Roughly a 3× vertical compression of
   `index.html:4387–4482`.
5. **Locked rows collapse three lying selects into one bordered "effective" block** with an optional
   "Requested (for a future runtime)" line that keeps the values persistable without pretending
   they're live.
6. **Persistent budget strip + one-line config digest** in the header — the review step's job, done
   continuously (inline validation replacing deferred validation).

### Scenario walkthroughs

**S1 — llama.cpp general/roleplay, q4_0 acceptable.** Rail → *Context & KV* (already the landing
pane). K/V selects show `auto · Roleplay` = `q4_0`. The pro user either accepts or, more likely,
sets f16 at 32k per `docs/reference/inference-tuning.md:150` ("on Apple Silicon use f16 KV when it
fits") — the header bar updates KV bytes live, no scroll, no step change.

**S2 — llama.cpp agentic, q8_0.** Same pane. `q8_0` pre-selected with provenance. The advisor card
in the rail carries a permanent `q8_0 KV: agentic floor ✓` line that turns amber the moment K or V
drops to q4_0. Pro mode does **not** block the choice — it annotates it. Because K and V are
adjacent on the same pane, the "matching types or you fall off the fused kernel" rule
(`inference-tuning.md:143`) is enforceable inline (`⚠ must match K`), which today it is not.

**S3 — Rapid-MLX int8 pin.** Rail → *Cache & Perf → Active memory*. Three locked rows, one glance,
three `Why?` popovers. The pro user's real levers on this backend — context size, `max-num-seqs`,
retained cache, prefill step size — are the *only* editable things in that region, which matches the
documented reality that context is the free variable at fixed int8 KV
(`rapid-mlx-runtime.md:134`). An expert who wants int4 finds the "Requested" line, sets it, and
understands it's aspirational.

---

## 5. Comparison

| | **Option A — Guided** | **Option B — Pro** |
|---|---|---|
| Core pattern | **Progressive disclosure via task cards** + **inspector rail** (Nielsen staged disclosure; Figma/Lightroom inspector) | **Master–detail settings navigator** + **faceted search** (VS Code Settings, Xcode Build Settings) |
| Anti-scroll mechanism | Only 4 decisions on the page; everything else behind one in-place drawer with its own left rail | Left rail random access + ⌘K filter + multi-column dense rows + "Modified only" |
| Est. main-column height | ~1.3 viewports (vs ~5–6 today) | ~1.5 viewports *per pane*, never traversed linearly |
| VRAM feedback | Sticky bar + fit-mode cards in rail; each card shows its own KV delta | Sticky bar + per-field byte deltas; fit-mode cards demoted to the Context pane |
| Handles the MLX pin | Locked card, no control | Locked-row block + separate "Requested" line |
| Steering (agentic q8_0) | Star + conditional warning copy on the tile | Advisor line that turns amber + inline validation |
| Discoverability of hidden knobs | "All settings (23)" count + "3 changed" chip | Nothing hidden; count is the rail |
| **Tradeoff** | An expert resents the extra click into the drawer; card metaphor doesn't scale past ~6 decisions | Novice-hostile density; ⌘K is a learned affordance; needs scroll-spy + filter infra that doesn't exist yet |
| Cost vs today | Medium — reuses every existing DOM node, adds card shells + drawer | Higher — needs rail, scroll-spy, search index over `CONTROLS`, modified-diff |

Both delete "pages of scrolling" by *different* means: A by removing fields from the page, B by
removing linearity from the page.

---

## 6. Full inventory — nothing dropped

`✅ card` = an Option-A decision card · `📦 drawer` = Option-A "All settings" · `🔒` = locked
effective-value row · Option-B column = rail section.

### llama.cpp

| Control (DOM id) | Today | Option A | Option B pane |
|---|---|---|---|
| `hw-quant-select`, `hw-quant-local-btn`, `hw-quant-swap-actions` | step2 header | sticky bar `[change quant]` | header + Model pane |
| `hw-tags-row` / `hw-tags-add-btn` | step2 header | 📦 drawer (Library) | Model pane |
| `hw-mmproj-select`, `hw-mmproj-browse-btn` | step2 header | ✅ card 3 | Vision |
| **`modal-image-min/max-tokens`** *(exists only in preset editor, `index.html:2754/2758` — **absent from wizard today**)* | — | ✅ card 3, shown when mmproj set | Vision |
| `spawn-gpu-layers` (+`-manual`) | grid | 📦 drawer | Placement |
| `spawn-context-size` + `ctx-quick-picks` + `ctx-model-max-hint` | grid | ✅ card 1 | Context & KV |
| `spawn-kv-unified` | grid | 📦 drawer | Context & KV |
| `spawn-cache-type-k` / `-v` | kv row | ✅ card 2 | Context & KV |
| `hw-use-mtp`, `hw-mtp-draft-select`, `hw-mtp-download-btn`, `hw-mtp-depth` | MTP section | ✅ card 4 (**merged** with spec-type — P5) | Speculative |
| `spawn-spec-type`, `spawn-spec-draft-n-min`, `-p-min`, `spawn-spec-draft-type-k/v`, `spawn-draft-model`+browse+candidates | nested `<details>` ×2 | ✅ card 4 "▸ tuning" inline expander | Speculative |
| `spawn-batch-size`, `-ubatch-size`, `-parallel-slots`, `-threads`, `-threads-batch`, `-prio`, `-flash-attn` | adv group | 📦 drawer (one dense row) | Batching |
| `spawn-n-cpu-moe` + `spawn-moe-autotune-btn`/`-verify`, `moe-offload-slider` | adv + rail | 📦 drawer; slider stays in rail | Placement |
| `spawn-tensor-split` | adv | 📦 drawer | Placement |
| `spawn-cache-mode`, `spawn-cache-ram` | adv | 📦 drawer | Prompt cache |
| `spawn-fit-enable`, `spawn-fit-target` | adv | 📦 drawer | Context & KV |
| `spawn-mlock` (+`spawn-mlock-warning`) | adv | 📦 drawer | Placement |
| `ctx-fit-warning`, `ctx-train-warning`, `ctx-vram-warning` | bottom of step | **inline in the causing card** | inline in pane |
| `vram-panel`/`vram-bar`/legend, `metal-limit-row`, `ctx-rail-summary`, `vram-scenarios`, `vram-autosize-btn`, `ram-panel` | rail | rail (kept, promoted to sticky) | sticky header + Context pane |
| `spawn-hardware-summary`, `spawn-vram-pill` | bottom | **merged into sticky bar** (dedupe) | sticky bar |
| `wizard-advisor` | bottom | rail | rail |
| `wizard-batch-sweep`, `wizard-depth-sweep` | bottom `<details>` | 📦 drawer → Benchmarks | Benchmarks |
| `hf-download-panel` (+dest/progress/cancel) | mid-step | rail bottom | Download pane |

### Rapid-MLX

| Control | Option A | Option B pane |
|---|---|---|
| `spawn-rapid-reasoning-mode` ("Allow thinking") | ✅ card 4 area (Generation strip) | Thinking |
| `spawn-rapid-tool-call-parser`, `-reasoning-parser`, `-hybrid-mode` | 📦 drawer | Protocol |
| `spawn-sampling-mode` | 📦 drawer | Sampling |
| `spawn-kv-cache-dtype` | 🔒 card 2 | 🔒 Active memory |
| `spawn-turboquant-mode` | 🔒 drawer | 🔒 Active memory |
| `spawn-rapid-pflash-policy` | 🔒 drawer | 🔒 Active memory |
| `spawn-rapid-prefill-step-size` | 📦 drawer | Active memory |
| `spawn-rapid-cache-mode`, `spawn-retained-cache-mib`, `-hybrid-cache-entries` | ✅ card 2 (secondary row) | Retained cache |
| `spawn-rapid-gpu-memory-utilization`, `-max-num-seqs`, `-max-concurrent-requests`, `-prefill-batch-size`, `-completion-batch-size` | 📦 drawer | Scheduler |
| `spawn-rapid-auto-tool-choice` | 📦 drawer | Tools |
| `spawn-rapid-speculative-enabled` + `-source`, `-sidecars-list`, `-model`, `-pin-status`, `-trust-consent`, `-recheck`, `-tokens`, `-disable-auto-k` | ✅ card 4 (collapsed sub-block, warning-first) | Companions |
| `spawn-workload-scenario` (hidden select, `index.html:3963`) | stays hidden, driven by the ① intent chip | same |

### Migrated from step 3/4/5

| Control | Option A | Option B |
|---|---|---|
| `spawn-temperature/top-p/min-p/top-k/repeat-penalty/presence-penalty/max-tokens/seed`, `spawn-sampling-presets` | 📦 drawer → Sampling | Sampling pane |
| `spawn-enable-thinking`, `-preserve-thinking`, `-reasoning-mode`, `-reasoning-budget`, `-budget-message` | 📦 drawer → Thinking | Thinking pane |
| `spawn-output-mode`, `spawn-grammar`, `spawn-json-schema`, `spawn-tool-call-format` | 📦 drawer → Response shaping | Shaping pane |
| `spawn-port`, `spawn-bind-host`, `spawn-alias`, `spawn-api-key` | 📦 drawer → Network | Network pane |
| `spawn-extra-args` | 📦 drawer → Extra args | Extra args pane |
| `spawn-summary-list`, `spawn-summary-warnings`, `preset-params-table`, `spawn-config-card`, `spawn-sidebar-config` | **collapsed into one** "Full config" drawer available from ② and ③ | same |
| `spawn-preset-name-input`, `spawn-save-preset-btn` | ③ launch bar | ③ launch bar |
| `spawn-server-btn`, status/progress/error | ③ | ③ |

**Explicitly dropped:** nothing. **Explicitly demoted:** `spawn-hardware-summary` (duplicate of the
VRAM panel) and two of the three config summaries (steps 3/4/5 each render the config; one canonical
drawer replaces them). **Explicitly added:** vision min/max image tokens in the wizard (already in
the preset schema and the CLI — `--image-min-tokens`/`--image-max-tokens` — so no new backend), and
a `Why?` popover source that reads the `effective:` keys already sitting unused in
`spawn-wizard-groups.js:66/68/74`.

### Backend call-outs (not silent assumptions)

Everything above uses existing data. The only genuinely new backend surface would be **per-control
byte deltas** ("KV @131k = 4.2 GB" next to each option in card 2). `/api/vram-estimate` already
returns `total_bytes`/`headroom_bytes`/`recommendation` and `renderLlamaCppScenarioCards` already
fires three parallel estimates (`vram-display.js:471–504`) — so this is derivable client-side today,
at the cost of 3–5 concurrent estimate calls per keystroke. **Recommend debounce + a small client
cache keyed on the `buildEstimateBody()` payload**, not a new endpoint.

---

## 7. Recommendation

**Ship the hybrid: one surface, two views — Option A as the default, Option B behind a `View:
Guided / Pro` toggle in the header, both rendering from the same `CONTROLS` registry.** The registry
work from the last session isn't wasted — it becomes the data source for the rail, the ⌘K index,
the "All settings (23)" count and the locked-row rendering, which is a far better use of it than
deciding `<details>.open`. The decisive change is retiring `tier` as a *layout* input
(`isOpenForProfile`, `spawn-wizard-ia.js:15`) and replacing it with two independent axes: **view
mode** (Guided/Pro, user-chosen, persisted) and **criticality** (author-declared: is this a decision
or a default?), because "I'm an expert" and "this control matters right now" are different questions
that today share one variable. Build Option A first — it's mostly re-parenting existing DOM into
four card shells plus a drawer, and it alone fixes P1, P4, P6 and both KV scenarios; Option B's rail
+ search is the follow-on that earns its keep only once the Guided view proves the card set is
right.

### Critical files for implementation

- `static/index.html` (3519–5012 — wizard DOM; 2578–2930 — preset-editor section pattern to borrow
  accent/title/desc from)
- `static/js/features/spawn-wizard-groups.js` (registry gains `critical`, `view`, and a rendered
  `effective` contract)
- `static/js/features/spawn-wizard-ia.js` (replace `isOpenForProfile` tier-rank logic with view-mode
  + criticality; add card/drawer and rail renderers)
- `static/js/features/spawn-wizard.js` (`applyProfileVisibility` ~1997, `applyUseCaseKvDtype` ~1489
  for provenance chips, `wizardState`/`STEP_LABELS` ~181–241 for the 6→3 step collapse)
- `static/js/features/spawn-wizard-vram-display.js` (sticky-bar reuse, per-option byte deltas, MLX
  vs llama.cpp scenario divergence at 420–465)

---

## 8. Implementation notes (added post-agent, for execution)

Not part of the Opus agent's report — recorded here so this document remains the single
source of truth for a fresh-context implementer picking this up later.

- The registry refactor referenced in §7 (`critical`, `view` split off from `tier`) and the
  llama.cpp/MLX split into `spawn-wizard-llama-ia.js` / `spawn-wizard-mlx-ia.js` have already
  landed on disk as of 2026-08-06 (see memory `applyProfileVisibility delegates to
  applyLlamaTierVisibility`, `llama.cpp IA registry created in spawn-wizard-llama-ia.js`). Confirm
  current shape of these files before assuming the agent's file/line citations still match —
  re-read them first.
- Static assets are embedded in the binary: `cargo build` is required after any JS/HTML/CSS edit
  before a running dev server will reflect the change, even with no-cache headers.
- Build order per §7's recommendation: Option A (four cards + drawer + sticky bar + locked-row
  renderer) first, fully verified against both llama.cpp and Rapid-MLX scenario walkthroughs
  (S1–S3 in §3), before starting Option B's rail/search infrastructure.
