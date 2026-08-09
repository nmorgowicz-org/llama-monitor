// Unified control registry (plan §2.8, §3.1): single source of truth for
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

// critical/view (Phase 10a Milestone 1, added 2026-08-07; retired `tier` in
// Milestone 2): two independent axes replacing the old single `tier` overload
// (plan's P2 — "tier does double duty and neither job well").
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
//   both axes.
export const CONTROLS = [
  // ── llama.cpp: Quick-tier (disabled at Quick profile) ─────────────────
  { id: 'spawn-context-size', loaders: ['llama_cpp'], critical: true, view: 'card' },
  { id: 'hw-mtp-depth', loaders: ['llama_cpp'], critical: false, view: 'both' },
  { id: 'spawn-batch-size', loaders: ['llama_cpp'], critical: true, view: 'both' },
  { id: 'spawn-gpu-layers', loaders: ['llama_cpp'], disableOnQuick: true, quickValue: 'auto', critical: true, view: 'both' },

  // ── llama.cpp: Balanced ────────────────────────────────────────────────
  { id: 'spawn-cache-type-k', loaders: ['llama_cpp'], critical: true, view: 'card' },
  { id: 'spawn-cache-type-v', loaders: ['llama_cpp'], critical: true, view: 'card' },
  { id: 'spawn-kv-unified', loaders: ['llama_cpp'], critical: true, view: 'both' },
  { id: 'hw-quant-select', loaders: ['llama_cpp'], critical: true, view: 'both' },
  { id: 'hw-mmproj-select', loaders: ['llama_cpp'], critical: true, view: 'card' },
  { id: 'hw-use-mtp', loaders: ['llama_cpp'], critical: true, view: 'card' },
  // Promoted from Advanced for symmetry with spawn-rapid-max-num-seqs, which
  // is a §2.6 scenario axis and must be Balanced on the MLX side (plan §2.8
  // cross-cutting note).
  { id: 'spawn-parallel-slots', loaders: ['llama_cpp'], critical: true, view: 'both' },

  // Generation, structured output, access, and escape-hatch controls live in
  // the same Tune step and are relocated into Pro's canonical pane.
  ...[
    'spawn-temperature', 'spawn-top-p', 'spawn-min-p', 'spawn-repeat-penalty',
    'spawn-presence-penalty', 'spawn-top-k', 'spawn-max-tokens', 'spawn-seed',
    'spawn-enable-thinking', 'spawn-preserve-thinking', 'spawn-reasoning-mode',
    'spawn-reasoning-budget',
  ].map(id => ({ id, loaders: ['llama_cpp'], critical: false, view: 'both' })),
  ...[
    'spawn-output-mode', 'spawn-grammar', 'spawn-json-schema', 'spawn-tool-call-format',
  ].map(id => ({ id, loaders: ['llama_cpp'], critical: false, view: 'both' })),
  ...[
    'spawn-port', 'spawn-bind-host', 'spawn-alias', 'spawn-api-key',
  ].map(id => ({ id, loaders: ['llama_cpp'], critical: false, view: 'both' })),
  { id: 'spawn-extra-args', loaders: ['llama_cpp'], critical: false, view: 'both' },

  // ── llama.cpp: Advanced (#spawn-advanced-fields) ──────────────────────
  { id: 'spawn-ubatch-size', loaders: ['llama_cpp'], critical: false, view: 'both' },
  { id: 'spawn-flash-attn', loaders: ['llama_cpp'], critical: false, view: 'both' },
  { id: 'spawn-prio', loaders: ['llama_cpp'], critical: false, view: 'both' },
  { id: 'spawn-threads', loaders: ['llama_cpp'], critical: false, view: 'both' },
  { id: 'spawn-threads-batch', loaders: ['llama_cpp'], critical: false, view: 'both' },
  { id: 'spawn-n-cpu-moe', loaders: ['llama_cpp'], critical: false, view: 'both' },
  { id: 'spawn-tensor-split', loaders: ['llama_cpp'], critical: false, view: 'both' },
  { id: 'spawn-cache-mode', loaders: ['llama_cpp'], critical: false, view: 'both' },
  { id: 'spawn-cache-ram', loaders: ['llama_cpp'], critical: false, view: 'both' },
  { id: 'spawn-fit-enable', loaders: ['llama_cpp'], critical: false, view: 'both' },
  { id: 'spawn-fit-target', loaders: ['llama_cpp'], critical: false, view: 'both' },
  { id: 'spawn-mlock', loaders: ['llama_cpp'], critical: false, view: 'both' },

  // ── llama.cpp: Advanced, nested collapse (#spawn-spec-details) ────────
  { id: 'spawn-spec-type', loaders: ['llama_cpp'], nested: 'spec', critical: false, view: 'both' },
  { id: 'spawn-spec-draft-type-k', loaders: ['llama_cpp'], nested: 'spec', critical: false, view: 'both' },
  { id: 'spawn-spec-draft-type-v', loaders: ['llama_cpp'], nested: 'spec', critical: false, view: 'both' },
  { id: 'spawn-draft-model', loaders: ['llama_cpp'], nested: 'spec', critical: false, view: 'both' },
  { id: 'spawn-spec-draft-n-min', loaders: ['llama_cpp'], nested: 'spec', critical: false, view: 'both' },
  { id: 'spawn-spec-draft-p-min', loaders: ['llama_cpp'], nested: 'spec', critical: false, view: 'both' },

  // ── Rapid-MLX (mirrors spawn-wizard-mlx-ia.js GROUPS; see note below) ──
  { id: 'spawn-rapid-reasoning-mode', loaders: ['rapid_mlx'], disableOnQuick: true, quickValue: 'on', group: 'thinking', critical: true, view: 'card' },
  { id: 'spawn-rapid-tool-call-parser', loaders: ['rapid_mlx'], group: 'protocol', critical: true, view: 'both' },
  { id: 'spawn-rapid-reasoning-parser', loaders: ['rapid_mlx'], group: 'protocol', critical: true, view: 'both' },
  { id: 'spawn-rapid-hybrid-mode', loaders: ['rapid_mlx'], group: 'protocol', critical: false, view: 'both' },
  { id: 'spawn-sampling-mode', loaders: ['rapid_mlx'], group: 'sampling', critical: true, view: 'both' },
  { id: 'spawn-kv-cache-dtype', loaders: ['rapid_mlx'], quickValue: 'int8', group: 'active-memory', effective: 'reasoning-pins-int8', critical: false, view: 'both' },
  { id: 'spawn-rapid-prefill-step-size', loaders: ['rapid_mlx'], group: 'active-memory', critical: false, view: 'both' },
  { id: 'spawn-turboquant-mode', loaders: ['rapid_mlx'], group: 'active-memory', effective: 'turboquant-withheld', critical: false, view: 'both' },
  { id: 'spawn-retained-cache-mib', loaders: ['rapid_mlx'], group: 'retained-cache', critical: true, view: 'both' },
  { id: 'spawn-rapid-hybrid-cache-entries', loaders: ['rapid_mlx'], group: 'retained-cache', critical: false, view: 'both' },
  { id: 'spawn-rapid-gpu-memory-utilization', loaders: ['rapid_mlx'], group: 'scheduler', critical: false, view: 'both' },
  { id: 'spawn-rapid-max-num-seqs', loaders: ['rapid_mlx'], group: 'scheduler', critical: true, view: 'both' },
  { id: 'spawn-rapid-max-concurrent-requests', loaders: ['rapid_mlx'], group: 'scheduler', critical: false, view: 'both' },
  { id: 'spawn-rapid-pflash-policy', loaders: ['rapid_mlx'], group: 'scheduler', effective: 'pflash-off', critical: false, view: 'both' },
  { id: 'spawn-rapid-prefill-batch-size', loaders: ['rapid_mlx'], group: 'scheduler', critical: false, view: 'both' },
  { id: 'spawn-rapid-completion-batch-size', loaders: ['rapid_mlx'], group: 'scheduler', critical: false, view: 'both' },
  { id: 'spawn-rapid-auto-tool-choice', loaders: ['rapid_mlx'], group: 'tool-integration', critical: true, view: 'both' },
  { id: 'spawn-rapid-speculative-enabled', loaders: ['rapid_mlx'], group: 'companions', nested: 'companions', critical: false, view: 'both' },
];

// Phase 4 presentation descriptors are derived once from the canonical
// control registry. They describe placement/search/ownership only; payload,
// defaults, capability, and effective-value semantics remain in the backend
// adapters and wizardState. A descriptor's mountId is stable across view
// changes, so relocation can move the owner without creating a second control.
const RAPID_GROUP_CATEGORY = {
  'active-memory': 'Memory & context',
  'retained-cache': 'Memory & context',
  scheduler: 'Performance',
  sampling: 'Generation & reasoning',
  protocol: 'Tools & conversation formatting',
  'tool-integration': 'Tools & conversation formatting',
  companions: 'Advanced',
  thinking: 'Generation & reasoning',
};

function llamaCategory(id) {
  if (id === 'spawn-context-size' || id.startsWith('spawn-cache-type') || id === 'spawn-kv-unified' || id === 'spawn-fit-enable' || id === 'spawn-fit-target' || id === 'spawn-cache-mode' || id === 'spawn-cache-ram') return 'Memory & context';
  if (id === 'hw-quant-select' || id === 'hw-mmproj-select') return 'Model & compatibility';
  if (id.startsWith('spawn-temperature') || id.startsWith('spawn-top-') || id.startsWith('spawn-min-p') || id.startsWith('spawn-repeat-') || id.startsWith('spawn-presence-') || id.startsWith('spawn-max-tokens') || id.startsWith('spawn-seed') || id.startsWith('spawn-enable-thinking') || id.startsWith('spawn-preserve-thinking') || id.startsWith('spawn-reasoning-')) return 'Generation & reasoning';
  if (id === 'spawn-output-mode' || id === 'spawn-grammar' || id === 'spawn-json-schema' || id === 'spawn-tool-call-format') return 'Tools & conversation formatting';
  if (id === 'spawn-port' || id === 'spawn-bind-host' || id === 'spawn-alias' || id === 'spawn-api-key') return 'Network & observability';
  if (id === 'spawn-extra-args') return 'Advanced';
  if (id.includes('spec') || id.includes('draft') || id === 'hw-mtp-depth' || id === 'spawn-gpu-layers' || id.includes('batch') || id.includes('thread') || id.includes('flash') || id.includes('tensor') || id === 'spawn-prio' || id === 'spawn-parallel-slots' || id === 'spawn-n-cpu-moe') return 'Performance';
  if (id === 'spawn-mlock') return 'Advanced';
  return 'Advanced';
}

function descriptorForControl(control) {
  const loader = control.loaders[0];
  const category = loader === 'rapid_mlx' ? (RAPID_GROUP_CATEGORY[control.group] || 'Advanced') : llamaCategory(control.id);
  const labelWords = control.id.replace(/^hw-/, '').replace(/^spawn-/, '').split('-');
  const aliases = [control.id, ...labelWords, control.group].filter(Boolean);
  const mountKind = control.effective ? 'read-only-status' : 'setting-control';
  return Object.freeze({
    ...control,
    semanticId: control.id,
    mountId: `${loader}:${control.id}`,
    mountKind,
    guidedPlacement: control.view === 'card' ? 'decision' : 'drawer',
    proCategory: category,
    aliases: [...new Set(aliases)],
    searchText: aliases.join(' '),
  });
}

export const PRESENTATION_CONTROLS = Object.freeze(CONTROLS.map(descriptorForControl));

export function validatePresentationDescriptors(loader) {
  const controls = PRESENTATION_CONTROLS.filter(c => c.loaders.includes(loader));
  const errors = [];
  const semanticIds = new Set();
  const mountIds = new Set();
  controls.forEach(control => {
    if (semanticIds.has(control.semanticId)) errors.push(`duplicate semanticId: ${control.semanticId}`);
    if (mountIds.has(control.mountId)) errors.push(`duplicate mountId: ${control.mountId}`);
    semanticIds.add(control.semanticId);
    mountIds.add(control.mountId);
    if (!control.mountKind || !['setting-control', 'read-only-status'].includes(control.mountKind)) errors.push(`invalid mountKind: ${control.id}`);
    if (!control.guidedPlacement || !['decision', 'drawer'].includes(control.guidedPlacement)) errors.push(`invalid Guided placement: ${control.id}`);
    if (!control.proCategory || !control.aliases?.length || !control.searchText) errors.push(`incomplete descriptor: ${control.id}`);
  });
  return { ok: errors.length === 0, controls, errors };
}

export function controlsForView(loader, view) {
 return PRESENTATION_CONTROLS.filter(c => c.loaders.includes(loader) && (view === 'pro' || c.view === view || c.view === 'both'));
}

export function controlsForLoader(loader) {
 return PRESENTATION_CONTROLS.filter(c => c.loaders.includes(loader));
}

// I2 lint: every Quick-tier control must carry a quickValue, or it can never
// have been legitimately Quick-disabled (plan §2.8 note under the tier table:
// "Anything we cannot derive must not be Quick-disabled"). llama.cpp's
// spawn-context-size/spawn-batch-size are pre-existing, product-accepted
// exceptions (disabled without a derived value) and are intentionally
// excluded here rather than silently satisfied.
const QUICK_VALUE_EXEMPT = new Set(['spawn-context-size', 'spawn-batch-size']);

export function assertQuickValueCoverage() {
  const missing = CONTROLS.filter(c => c.disableOnQuick && !c.quickValue && !QUICK_VALUE_EXEMPT.has(c.id));
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
