// MLX-family information architecture for the Spawn Wizard hardware step.
// Presentation-only: existing DOM ids remain the serialization contract. This
// module only regroups the flat Rapid-MLX field dump into progressive,
// labelled sections at runtime — mirroring preset-editor-mlx.js's pattern —
// so the wizard and the preset editor share the same MLX section vocabulary
// (Generation, Cache & Performance, Server & Safety).

const SUPERSECTIONS = [
  {
    id: 'generation',
    title: 'Generation & Runtime',
    description: 'Thinking, sampling, and model protocol defaults.',
  },
  {
    id: 'cache-performance',
    title: 'Cache & Performance',
    description: 'Active KV precision, retained prompts, and scheduling.',
  },
  {
    id: 'server-safety',
    title: 'Server & Safety',
    description: 'Tool integration and qualified companions.',
  },
];

const GROUPS = [
  {
    supersection: 'generation', id: 'thinking', title: 'Thinking & reasoning',
    description: 'Control model reasoning behavior independently from sampling.',
    controls: ['spawn-rapid-reasoning-mode'],
  },
  {
    supersection: 'generation', id: 'protocol', title: 'Model protocol',
    description: 'Keep automatic detection unless a modified finetune requires an override.',
    controls: ['spawn-rapid-tool-call-parser', 'spawn-rapid-reasoning-parser', 'spawn-rapid-hybrid-mode'],
  },
  {
    supersection: 'generation', id: 'sampling', title: 'Sampling defaults',
    description: 'Server-level sampling defaults; explicit client parameters always win.',
    controls: ['spawn-sampling-mode'],
  },
  {
    supersection: 'cache-performance', id: 'active-memory', title: 'Active memory',
    description: 'Precision and prefill choices that affect live unified-memory pressure.',
    controls: ['spawn-kv-cache-dtype', 'spawn-rapid-prefill-step-size', 'spawn-turboquant-mode'],
  },
  {
    supersection: 'cache-performance', id: 'retained-cache', title: 'Retained prompt cache',
    description: 'Bound reusable prompt snapshots by both memory and working-set size.',
    controls: ['spawn-retained-cache-mib', 'spawn-rapid-hybrid-cache-entries'],
  },
  {
    supersection: 'cache-performance', id: 'scheduler', title: 'Scheduler & throughput',
    description: 'Advanced batching and admission limits; defaults suit one interactive user.',
    controls: [
      'spawn-rapid-gpu-memory-utilization', 'spawn-rapid-max-num-seqs',
      'spawn-rapid-max-concurrent-requests', 'spawn-rapid-pflash-policy',
      'spawn-rapid-prefill-batch-size', 'spawn-rapid-completion-batch-size',
    ],
  },
  {
    supersection: 'server-safety', id: 'tool-integration', title: 'Tool integration',
    description: 'Enable only for models with a compatible tool-call parser.',
    controls: ['spawn-rapid-auto-tool-choice'],
  },
  {
    supersection: 'server-safety', id: 'companions', title: 'Companions & experimental acceleration',
    description: 'Only qualified local companions belong here; unsupported remote launches fail closed.',
    collapsible: true,
    controls: [
      'spawn-rapid-speculative-enabled', 'spawn-rapid-speculative-mode-wrap',
      'spawn-rapid-speculative-sidecars-wrap', 'spawn-rapid-speculative-model-wrap',
      'spawn-rapid-speculative-pin-status-wrap', 'spawn-rapid-speculative-trust-wrap',
      'spawn-rapid-speculative-tokens-wrap', 'spawn-rapid-speculative-auto-k-wrap',
    ],
  },
];

const originalPositions = new Map();
let iaContainer = null;
let hiddenSources = [];

function rowForControl(id) {
  const el = document.getElementById(id);
  if (!el) return null;
  if (el.classList.contains('hardware-field') || el.classList.contains('kv-inline-row')) return el;
  return el.closest('.hardware-field, .kv-inline-row') || el;
}

function rememberPosition(row) {
  if (originalPositions.has(row) || !row.parentNode) return;
  const anchor = document.createComment('mlx-wiz-origin');
  row.parentNode.insertBefore(anchor, row);
  originalPositions.set(row, anchor);
}

function restorePositions() {
  originalPositions.forEach((anchor, row) => {
    if (anchor.parentNode) anchor.parentNode.insertBefore(row, anchor.nextSibling);
    row.classList.remove('mlx-wiz-row');
  });
  originalPositions.clear();
}

function buildGroup(group) {
  const container = document.createElement(group.collapsible ? 'details' : 'section');
  container.className = 'mlx-native-group mlx-wiz-group';
  container.dataset.mlxWizGroup = group.id;
  const header = document.createElement(group.collapsible ? 'summary' : 'div');
  header.className = 'mlx-native-group-header';
  const title = document.createElement('h4');
  title.className = 'mlx-native-group-title';
  title.textContent = group.title;
  const description = document.createElement('p');
  description.className = 'mlx-native-group-description';
  description.textContent = group.description;
  header.append(title, description);
  if (group.collapsible) {
    const badge = document.createElement('span');
    badge.className = 'mlx-native-group-badge';
    badge.textContent = 'Advanced';
    header.appendChild(badge);
  }
  container.appendChild(header);
  let any = false;
  group.controls.forEach(id => {
    const row = rowForControl(id);
    if (!row) return;
    rememberPosition(row);
    row.classList.add('mlx-wiz-row');
    container.appendChild(row);
    any = true;
  });
  return any ? container : null;
}

function hideSource(el) {
  if (!el || el.dataset.mlxWizHidden) return;
  el.dataset.mlxWizHidden = el.style.display || '(default)';
  el.style.display = 'none';
  hiddenSources.push(el);
}

function restoreSources() {
  hiddenSources.forEach(el => {
    const prev = el.dataset.mlxWizHidden;
    el.style.display = prev === '(default)' ? '' : prev;
    delete el.dataset.mlxWizHidden;
  });
  hiddenSources = [];
}

export function configureMlxWizardIA(root, enabled) {
  const advancedFields = (root || document).querySelector('#spawn-rapid-advanced-fields');
  if (!advancedFields) return;

  if (!enabled) {
    if (iaContainer) {
      iaContainer.remove();
      iaContainer = null;
    }
    restorePositions();
    restoreSources();
    return;
  }

  if (iaContainer) return; // already built for this activation

  // Hide the original flat dump (intro title/hint + the grid + the trailing
  // reasoning row) — its fields are about to be relocated into grouped
  // sections below. Anything not covered by a group stays where it is.
  const introTitle = advancedFields.querySelector('.wizard-section-title');
  const introHint = advancedFields.querySelector('.field-hint');
  hideSource(introTitle);
  hideSource(introHint);
  advancedFields.querySelectorAll('.hardware-grid').forEach(hideSource);

  iaContainer = document.createElement('div');
  iaContainer.className = 'mlx-wiz-ia';

  SUPERSECTIONS.forEach(super_ => {
    const groupsForSuper = GROUPS.filter(g => g.supersection === super_.id);
    const builtGroups = groupsForSuper.map(buildGroup).filter(Boolean);
    if (!builtGroups.length) return;
    const section = document.createElement('section');
    section.className = 'mlx-wiz-supersection';
    section.dataset.mlxWizSuper = super_.id;
    const heading = document.createElement('h3');
    heading.className = 'mlx-wiz-supersection-title';
    heading.textContent = super_.title;
    const desc = document.createElement('p');
    desc.className = 'mlx-wiz-supersection-desc';
    desc.textContent = super_.description;
    section.append(heading, desc, ...builtGroups);
    iaContainer.appendChild(section);
  });

  advancedFields.appendChild(iaContainer);
}
