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

export const CONTROLS = [
  // ── llama.cpp: Quick (disabled at Quick; gpu-layers writes 'auto') ────────
  { id: 'spawn-context-size', loaders: ['llama_cpp'], tier: 'quick' },
  { id: 'spawn-batch-size', loaders: ['llama_cpp'], tier: 'quick' },
  { id: 'spawn-gpu-layers', loaders: ['llama_cpp'], tier: 'quick', quickValue: 'auto' },

  // ── llama.cpp: Balanced ────────────────────────────────────────────────
  { id: 'spawn-cache-type-k', loaders: ['llama_cpp'], tier: 'balanced' },
  { id: 'spawn-cache-type-v', loaders: ['llama_cpp'], tier: 'balanced' },
  { id: 'spawn-kv-unified', loaders: ['llama_cpp'], tier: 'balanced' },
  { id: 'hw-quant-select', loaders: ['llama_cpp'], tier: 'balanced' },
  { id: 'hw-mmproj-select', loaders: ['llama_cpp'], tier: 'balanced' },
  { id: 'hw-use-mtp', loaders: ['llama_cpp'], tier: 'balanced' },
  // Promoted from Advanced for symmetry with spawn-rapid-max-num-seqs, which
  // is a §2.6 scenario axis and must be Balanced on the MLX side (plan §2.8
  // cross-cutting note).
  { id: 'spawn-parallel-slots', loaders: ['llama_cpp'], tier: 'balanced' },

  // ── llama.cpp: Advanced (#spawn-advanced-fields) ──────────────────────
  { id: 'spawn-ubatch-size', loaders: ['llama_cpp'], tier: 'advanced' },
  { id: 'spawn-flash-attn', loaders: ['llama_cpp'], tier: 'advanced' },
  { id: 'spawn-prio', loaders: ['llama_cpp'], tier: 'advanced' },
  { id: 'spawn-threads', loaders: ['llama_cpp'], tier: 'advanced' },
  { id: 'spawn-threads-batch', loaders: ['llama_cpp'], tier: 'advanced' },
  { id: 'spawn-n-cpu-moe', loaders: ['llama_cpp'], tier: 'advanced' },
  { id: 'spawn-tensor-split', loaders: ['llama_cpp'], tier: 'advanced' },
  { id: 'spawn-cache-mode', loaders: ['llama_cpp'], tier: 'advanced' },
  { id: 'spawn-cache-ram', loaders: ['llama_cpp'], tier: 'advanced' },
  { id: 'spawn-fit-enable', loaders: ['llama_cpp'], tier: 'advanced' },
  { id: 'spawn-fit-target', loaders: ['llama_cpp'], tier: 'advanced' },
  { id: 'spawn-mlock', loaders: ['llama_cpp'], tier: 'advanced' },

  // ── llama.cpp: Advanced, nested collapse (#spawn-spec-details) ────────
  { id: 'spawn-spec-type', loaders: ['llama_cpp'], tier: 'advanced', nested: 'spec' },
  { id: 'spawn-spec-draft-type-k', loaders: ['llama_cpp'], tier: 'advanced', nested: 'spec' },
  { id: 'spawn-spec-draft-type-v', loaders: ['llama_cpp'], tier: 'advanced', nested: 'spec' },
  { id: 'spawn-draft-model', loaders: ['llama_cpp'], tier: 'advanced', nested: 'spec' },
  { id: 'spawn-spec-draft-n-min', loaders: ['llama_cpp'], tier: 'advanced', nested: 'spec' },
  { id: 'spawn-spec-draft-p-min', loaders: ['llama_cpp'], tier: 'advanced', nested: 'spec' },

  // ── Rapid-MLX (mirrors spawn-wizard-mlx-ia.js GROUPS; see note below) ──
  { id: 'spawn-rapid-reasoning-mode', loaders: ['rapid_mlx'], tier: 'quick', quickValue: 'on', group: 'thinking' },
  { id: 'spawn-rapid-tool-call-parser', loaders: ['rapid_mlx'], tier: 'balanced', group: 'protocol' },
  { id: 'spawn-rapid-reasoning-parser', loaders: ['rapid_mlx'], tier: 'balanced', group: 'protocol' },
  { id: 'spawn-rapid-hybrid-mode', loaders: ['rapid_mlx'], tier: 'advanced', group: 'protocol' },
  { id: 'spawn-sampling-mode', loaders: ['rapid_mlx'], tier: 'balanced', group: 'sampling' },
  { id: 'spawn-kv-cache-dtype', loaders: ['rapid_mlx'], tier: 'advanced', quickValue: 'int8', group: 'active-memory', effective: 'reasoning-pins-int8' },
  { id: 'spawn-rapid-prefill-step-size', loaders: ['rapid_mlx'], tier: 'advanced', group: 'active-memory' },
  { id: 'spawn-turboquant-mode', loaders: ['rapid_mlx'], tier: 'advanced', group: 'active-memory', effective: 'turboquant-withheld' },
  { id: 'spawn-retained-cache-mib', loaders: ['rapid_mlx'], tier: 'balanced', group: 'retained-cache' },
  { id: 'spawn-rapid-hybrid-cache-entries', loaders: ['rapid_mlx'], tier: 'advanced', group: 'retained-cache' },
  { id: 'spawn-rapid-gpu-memory-utilization', loaders: ['rapid_mlx'], tier: 'advanced', group: 'scheduler' },
  { id: 'spawn-rapid-max-num-seqs', loaders: ['rapid_mlx'], tier: 'balanced', group: 'scheduler' },
  { id: 'spawn-rapid-max-concurrent-requests', loaders: ['rapid_mlx'], tier: 'advanced', group: 'scheduler' },
  { id: 'spawn-rapid-pflash-policy', loaders: ['rapid_mlx'], tier: 'advanced', group: 'scheduler', effective: 'pflash-off' },
  { id: 'spawn-rapid-prefill-batch-size', loaders: ['rapid_mlx'], tier: 'advanced', group: 'scheduler' },
  { id: 'spawn-rapid-completion-batch-size', loaders: ['rapid_mlx'], tier: 'advanced', group: 'scheduler' },
  { id: 'spawn-rapid-auto-tool-choice', loaders: ['rapid_mlx'], tier: 'balanced', group: 'tool-integration' },
  { id: 'spawn-rapid-speculative-enabled', loaders: ['rapid_mlx'], tier: 'advanced', group: 'companions', nested: 'companions' },
];

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
