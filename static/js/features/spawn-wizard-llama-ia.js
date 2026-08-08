// llama.cpp information architecture for the Spawn Wizard hardware step
// (plan §5 Phase 4 item 4). Retires the hand-written #spawn-advanced-fields /
// #spawn-spec-details <details> blocks: the flat field dump inside
// #spawn-advanced-fields is hidden and relocated into registry-generated,
// tier-driven groups by the shared engine in spawn-wizard-ia.js — the same
// mechanism spawn-wizard-mlx-ia.js uses for Rapid-MLX. Existing DOM ids
// remain the serialization contract; #spawn-advanced-fields itself is now a
// plain wrapper div (JS anchor), not a <details>.

import { createWizardIA } from './spawn-wizard-ia.js';

const SUPERSECTIONS = [
  {
    id: 'advanced-tuning',
    title: 'Advanced tuning',
    description: 'Safe defaults for most setups. Change only if you have a specific reason.',
  },
];

const GROUPS = [
  {
    supersection: 'advanced-tuning', id: 'batching-threads', title: 'Batching & threads',
    description: 'Prompt/micro-batch sizing, flash attention, and CPU thread allocation.',
    critical: true, view: 'both',
    controls: [
      'spawn-batch-size', 'spawn-parallel-slots', 'spawn-ubatch-size', 'spawn-flash-attn',
      'spawn-prio', 'spawn-threads', 'spawn-threads-batch',
    ],
  },
  {
    supersection: 'advanced-tuning', id: 'moe-multigpu', title: 'MoE & multi-GPU',
    description: 'Mixture-of-Experts CPU offload and tensor-split placement across GPUs.',
    critical: false, view: 'both',
    controls: ['spawn-n-cpu-moe', 'spawn-tensor-split'],
  },
  {
    supersection: 'advanced-tuning', id: 'prompt-cache', title: 'Prompt cache',
    description: 'Persistent KV prefix-cache mode and size bound.',
    critical: false, view: 'both',
    controls: ['spawn-cache-mode', 'spawn-cache-ram'],
  },
  {
    supersection: 'advanced-tuning', id: 'fit-memory', title: 'Auto-fit & memory',
    description: 'Shrink context to fit a memory budget; pin the model in RAM.',
    critical: false, view: 'both',
    controls: ['spawn-fit-enable', 'spawn-fit-target', 'spawn-mlock'],
  },
  {
    // Relocates the existing #spawn-spec-details <details> as-is (plan §2.8:
    // Advanced, nested — the llama.cpp peer of MLX's 'companions' group) so
    // its internal conditional wiring (draft-KV rows, draft-model path) isn't
    // re-derived.
    supersection: 'advanced-tuning', id: 'speculative-decoding', title: 'Speculative decoding',
    prebuiltId: 'spawn-spec-details',
    critical: false, view: 'both',
  },
];

const ia = createWizardIA({
  groupClassName: 'mlx-native-group',
  rowClassName: 'llama-wiz-row',
  originAnchorComment: 'llama-wiz-origin',
});

export function configureLlamaWizardIA(root, enabled, profile = 'balanced') {
  ia.configure(root, enabled, profile, GROUPS, SUPERSECTIONS, '#spawn-advanced-fields');
  window._llamaGroups = GROUPS;
  window._llamaSupersections = SUPERSECTIONS;
}

export function applyLlamaTierVisibility(root, profile) {
  ia.applyTierVisibility(root, profile);
}
