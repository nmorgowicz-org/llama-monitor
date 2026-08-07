// Unified control-tier registry (plan §2.8, §3.1): single source of truth for
// which profile (Quick/Balanced/Advanced) each control is reachable/editable
// at, across both loaders. Presentation only — DOM ids, wizardState shape,
// buildSpawnPayload()/buildRapidMlxConfig() are untouched by this module.
//
// I1 — Tier never hides. Every control is reachable at every tier; tier only
//      controls editability (Quick disables) and default disclosure
//      (Advanced-tier groups auto-open).
// I2 — Quick means "the wizard already decided." A Quick-tier control must
//      carry a quickValue the wizard writes before disabling — see the lint
//      test in spawn-wizard-groups.test below (enforced at runtime by
//      assertQuickValueCoverage(), called from the Playwright/Node capture
//      harness, not on every page load).
//
// llama.cpp entries mirror the existing hand-placed markup 1:1 (index.html
// step 2 fields + #spawn-advanced-fields + #spawn-spec-details, nested).
// Rapid-MLX entries mirror spawn-wizard-mlx-ia.js's GROUPS (kept in sync by
// hand for now — see note at bottom).

// critical/view (Phase 10a Milestone 1, added 2026-08-07): two independent
// axes replacing the old single `tier` overload (plan's P2 — "tier does
// double duty and neither job well"). `tier` itself is left in place for
// now (the disclosure engine in spawn-wizard-ia.js still reads it —
// retired in Milestone 2) but is no longer the source of truth for either
// question below; critical/view are.
//   critical — can this be safely touched without expert knowledge? Decides
//     inline-editable-in-Guided (true) vs. drawer/Pro-only (false). Derived
//     1:1 from the old quick/balanced vs. advanced split: nothing here
//     reclassifies a control's actual expert-knowledge requirement, it just
//     names the axis that was already implicit in `tier`.
//   view — which view(s) render this control outside the drawer:
//     'card'   — one of the four Guided decision cards (archived doc §3
//                "always open" list: context size, KV precision, vision,
//                speed boost) — visible with no drawer/Pro interaction.
//     'both'   — reachable inline in Pro's dense layout and via Guided's
//                "All settings" drawer (I1: never hidden, just not a card).
//   Every control keeps view:'both' minimum — I1 (never hidden) applies to
//   both axes, not just the old tier.
export const CONTROLS = [
  // ── llama.cpp: Quick (disabled at Quick; gpu-layers writes 'auto') ────────
  { id: 'spawn-context-size', loaders: ['llama_cpp'], tier: 'quick', critical: true, view: 'card' },
  { id: 'spawn-batch-size', loaders: ['llama_cpp'], tier: 'quick', critical: true, view: 'both' },
  { id: 'spawn-gpu-layers', loaders: ['llama_cpp'], tier: 'quick', quickValue: 'auto', critical: true, view: 'both' },

  // ── llama.cpp: Balanced ────────────────────────────────────────────────
  { id: 'spawn-cache-type-k', loaders: ['llama_cpp'], tier: 'balanced', critical: true, view: 'card' },
  { id: 'spawn-cache-type-v', loaders: ['llama_cpp'], tier: 'balanced', critical: true, view: 'card' },
  { id: 'spawn-kv-unified', loaders: ['llama_cpp'], tier: 'balanced', critical: true, view: 'both' },
  { id: 'hw-quant-select', loaders: ['llama_cpp'], tier: 'balanced', critical: true, view: 'both' },
  { id: 'hw-mmproj-select', loaders: ['llama_cpp'], tier: 'balanced', critical: true, view: 'card' },
  { id: 'hw-use-mtp', loaders: ['llama_cpp'], tier: 'balanced', critical: true, view: 'card' },
  // Promoted from Advanced for symmetry with spawn-rapid-max-num-seqs, which
  // is a §2.6 scenario axis and must be Balanced on the MLX side (plan §2.8
  // cross-cutting note).
  { id: 'spawn-parallel-slots', loaders: ['llama_cpp'], tier: 'balanced', critical: true, view: 'both' },

  // ── llama.cpp: Advanced (#spawn-advanced-fields) ──────────────────────
  { id: 'spawn-ubatch-size', loaders: ['llama_cpp'], tier: 'advanced', critical: false, view: 'both' },
  { id: 'spawn-flash-attn', loaders: ['llama_cpp'], tier: 'advanced', critical: false, view: 'both' },
  { id: 'spawn-prio', loaders: ['llama_cpp'], tier: 'advanced', critical: false, view: 'both' },
  { id: 'spawn-threads', loaders: ['llama_cpp'], tier: 'advanced', critical: false, view: 'both' },
  { id: 'spawn-threads-batch', loaders: ['llama_cpp'], tier: 'advanced', critical: false, view: 'both' },
  { id: 'spawn-n-cpu-moe', loaders: ['llama_cpp'], tier: 'advanced', critical: false, view: 'both' },
  { id: 'spawn-tensor-split', loaders: ['llama_cpp'], tier: 'advanced', critical: false, view: 'both' },
  { id: 'spawn-cache-mode', loaders: ['llama_cpp'], tier: 'advanced', critical: false, view: 'both' },
  { id: 'spawn-cache-ram', loaders: ['llama_cpp'], tier: 'advanced', critical: false, view: 'both' },
  { id: 'spawn-fit-enable', loaders: ['llama_cpp'], tier: 'advanced', critical: false, view: 'both' },
  { id: 'spawn-fit-target', loaders: ['llama_cpp'], tier: 'advanced', critical: false, view: 'both' },
  { id: 'spawn-mlock', loaders: ['llama_cpp'], tier: 'advanced', critical: false, view: 'both' },

  // ── llama.cpp: Advanced, nested collapse (#spawn-spec-details) ────────
  { id: 'spawn-spec-type', loaders: ['llama_cpp'], tier: 'advanced', nested: 'spec', critical: false, view: 'both' },
  { id: 'spawn-spec-draft-type-k', loaders: ['llama_cpp'], tier: 'advanced', nested: 'spec', critical: false, view: 'both' },
  { id: 'spawn-spec-draft-type-v', loaders: ['llama_cpp'], tier: 'advanced', nested: 'spec', critical: false, view: 'both' },
  { id: 'spawn-draft-model', loaders: ['llama_cpp'], tier: 'advanced', nested: 'spec', critical: false, view: 'both' },
  { id: 'spawn-spec-draft-n-min', loaders: ['llama_cpp'], tier: 'advanced', nested: 'spec', critical: false, view: 'both' },
  { id: 'spawn-spec-draft-p-min', loaders: ['llama_cpp'], tier: 'advanced', nested: 'spec', critical: false, view: 'both' },

  // ── Rapid-MLX (mirrors spawn-wizard-mlx-ia.js GROUPS; see note below) ──
  { id: 'spawn-rapid-reasoning-mode', loaders: ['rapid_mlx'], tier: 'quick', quickValue: 'on', group: 'thinking', critical: true, view: 'card' },
  { id: 'spawn-rapid-tool-call-parser', loaders: ['rapid_mlx'], tier: 'balanced', group: 'protocol', critical: true, view: 'both' },
  { id: 'spawn-rapid-reasoning-parser', loaders: ['rapid_mlx'], tier: 'balanced', group: 'protocol', critical: true, view: 'both' },
  { id: 'spawn-rapid-hybrid-mode', loaders: ['rapid_mlx'], tier: 'advanced', group: 'protocol', critical: false, view: 'both' },
  { id: 'spawn-sampling-mode', loaders: ['rapid_mlx'], tier: 'balanced', group: 'sampling', critical: true, view: 'both' },
  { id: 'spawn-kv-cache-dtype', loaders: ['rapid_mlx'], tier: 'advanced', quickValue: 'int8', group: 'active-memory', effective: 'reasoning-pins-int8', critical: false, view: 'both' },
  { id: 'spawn-rapid-prefill-step-size', loaders: ['rapid_mlx'], tier: 'advanced', group: 'active-memory', critical: false, view: 'both' },
  { id: 'spawn-turboquant-mode', loaders: ['rapid_mlx'], tier: 'advanced', group: 'active-memory', effective: 'turboquant-withheld', critical: false, view: 'both' },
  { id: 'spawn-retained-cache-mib', loaders: ['rapid_mlx'], tier: 'balanced', group: 'retained-cache', critical: true, view: 'both' },
  { id: 'spawn-rapid-hybrid-cache-entries', loaders: ['rapid_mlx'], tier: 'advanced', group: 'retained-cache', critical: false, view: 'both' },
  { id: 'spawn-rapid-gpu-memory-utilization', loaders: ['rapid_mlx'], tier: 'advanced', group: 'scheduler', critical: false, view: 'both' },
  { id: 'spawn-rapid-max-num-seqs', loaders: ['rapid_mlx'], tier: 'balanced', group: 'scheduler', critical: true, view: 'both' },
  { id: 'spawn-rapid-max-concurrent-requests', loaders: ['rapid_mlx'], tier: 'advanced', group: 'scheduler', critical: false, view: 'both' },
  { id: 'spawn-rapid-pflash-policy', loaders: ['rapid_mlx'], tier: 'advanced', group: 'scheduler', effective: 'pflash-off', critical: false, view: 'both' },
  { id: 'spawn-rapid-prefill-batch-size', loaders: ['rapid_mlx'], tier: 'advanced', group: 'scheduler', critical: false, view: 'both' },
  { id: 'spawn-rapid-completion-batch-size', loaders: ['rapid_mlx'], tier: 'advanced', group: 'scheduler', critical: false, view: 'both' },
  { id: 'spawn-rapid-auto-tool-choice', loaders: ['rapid_mlx'], tier: 'balanced', group: 'tool-integration', critical: true, view: 'both' },
  { id: 'spawn-rapid-speculative-enabled', loaders: ['rapid_mlx'], tier: 'advanced', group: 'companions', nested: 'companions', critical: false, view: 'both' },
];

export function controlsForView(loader, view) {
  return CONTROLS.filter(c => c.loaders.includes(loader) && (view === 'pro' || c.view === view || c.view === 'both'));
}

export function controlsForLoader(loader) {
  return CONTROLS.filter(c => c.loaders.includes(loader));
}

export function tierOf(id, loader) {
  const c = CONTROLS.find(c => c.id === id && c.loaders.includes(loader));
  return c ? c.tier : null;
}

// I2 lint: every Quick-tier control must carry a quickValue, or it can never
// have been legitimately Quick-disabled (plan §2.8 note under the tier table:
// "Anything we cannot derive must not be Quick-disabled"). llama.cpp's
// spawn-context-size/spawn-batch-size are pre-existing, product-accepted
// exceptions (disabled without a derived value) and are intentionally
// excluded here rather than silently satisfied.
const QUICK_VALUE_EXEMPT = new Set(['spawn-context-size', 'spawn-batch-size']);

export function assertQuickValueCoverage() {
  const missing = CONTROLS.filter(c => c.tier === 'quick' && !c.quickValue && !QUICK_VALUE_EXEMPT.has(c.id));
  if (missing.length) {
    throw new Error(`Quick-tier controls missing quickValue (I2): ${missing.map(c => c.id).join(', ')}`);
  }
}

// Human-readable copy for each `effective:` tag (plan §6/P4 — the control's
// selection is accepted by the UI/backend but a runtime constraint pins the
// actual launch behavior regardless of what's picked here).
const EFFECTIVE_COPY = {
  'reasoning-pins-int8': {
    value: 'int8',
    why: "Rapid-MLX's --reasoning flag pins active KV to int8 unconditionally on this build. Kept visible so a future runtime that respects it doesn't need new UI.",
  },
  'turboquant-withheld': {
    value: 'Standard — int4 retained storage',
    why: 'Requested but not applied at launch: the server always starts with standard int4 retained storage until a per-model qualification receipt exists. K8V4 also measured 40–100% slower TTFT in Phase 6 benchmarks.',
  },
  'pflash-off': {
    value: 'Off — qualified default',
    why: 'A 2026-07-24 benchmark measured recall collapsing to 0.0–0.4 (vs 1.0 with PFlash off) at long context — the compressed region is dropped, not lossily retained. Silent failure mode in an agentic coding loop.',
  },
};

// P4 fix: render a locked-row treatment (dimmed control + "Effective: X"
// chip + "Why?" popover) for every control whose registry entry carries an
// `effective` tag, instead of leaving the selection live-looking while the
// runtime silently overrides it.
export function applyEffectiveLocks(root) {
  if (!root) return;
  for (const c of CONTROLS) {
    if (!c.effective) continue;
    const copy = EFFECTIVE_COPY[c.effective];
    if (!copy) continue;
    const field = root.querySelector(`#${c.id}`)?.closest('.hardware-field');
    if (!field || field.dataset.effectiveLocked === '1') continue;
    field.dataset.effectiveLocked = '1';
    field.classList.add('field-effective-locked');
    const label = field.querySelector('label');
    if (label && !label.querySelector('.effective-chip')) {
      const chip = document.createElement('span');
      chip.className = 'effective-chip';
      chip.textContent = `Effective: ${copy.value}`;
      chip.title = copy.why;
      label.appendChild(document.createTextNode(' '));
      label.appendChild(chip);
    }
    const btn = field.querySelector('button.effective-why-btn') || (() => {
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'effective-why-btn';
      b.textContent = 'Why?';
      b.setAttribute('aria-label', `Why is ${c.id} locked to ${copy.value}?`);
      b.addEventListener('click', () => {
        b.setAttribute('aria-expanded', String(b.getAttribute('aria-expanded') !== 'true'));
        const hint = field.querySelector('.field-hint');
        if (hint) hint.classList.toggle('effective-why-open');
      });
      const hintEl = field.querySelector('.field-hint');
      if (hintEl) field.insertBefore(b, hintEl);
      else field.appendChild(b);
      return b;
    })();
    void btn;
  }
}
