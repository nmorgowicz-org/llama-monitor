// ── Spawn Wizard Module ───────────────────────────────────────────────────────
// Spawn Llama-Server V2 — complete guided wizard.
//
// Key features:
//  - Use-case selector (agentic / general / roleplay)
//  - Pre-download quant advisor with size + max-context table
//  - Architecture-aware VRAM breakdown (live animated bar)
//  - Scenario cards (q8_0 KV / q4_0 KV / f16 KV max context)
//  - MoE expert offload slider with live feedback
//  - Auto-size button pulls backend recommendation
//  - Step validation before advancing

import { openDeferredFileBrowser } from './file-browser-launcher.js';
import { showToast } from './toast.js';

// ── VRAM math (client-side, for instant slider feedback) ──────────────────────

const KV_BPE = {
  f32:2.0*2, f16:2.0, bf16:2.0,
  q8_0:1.0, q5_0:0.625, q5_1:0.625,
  q4_k_m:0.5, q4_k_s:0.5, q4_0:0.5, q4_1:0.5, iq4_xs:0.5, iq4_nl:0.5,
  q3_k_m:0.375, q3_k_s:0.375, q3_k_l:0.375, iq3_m:0.375, iq3_s:0.375,
  q3_k:0.375, iq3_xs:0.375, iq3_xxs:0.375,
  q2_k:0.25, iq2_m:0.25, iq2_s:0.25, iq2_xs:0.25, iq2_xxs:0.25,
  iq1_m:0.125, iq1_s:0.125,
};
function kvBpe(quant) { return KV_BPE[quant] ?? 1.0; }

// Compute KV cache bytes for standard full-attention models.
// For Gemma-style models with local attention we simplify:
//   global_frac = n_global_layers / n_layers
//   effective_ctx = global_frac * ctx + (1-global_frac) * min(ctx, window)
function kvBytes(arch, ctx, slots, ctk, ctv) {
  const s = Math.max(slots, 1);
  const k = kvBpe(ctk), v = kvBpe(ctv);
  // For hybrid DeltaNet models: only nAttnLayers use KV cache (not all nLayers).
  const effectiveLayers = (arch.nAttnLayers && arch.nAttnLayers < arch.nLayers)
    ? arch.nAttnLayers : arch.nLayers;
  if (arch.localAttnWindow > 0 && arch.nGlobalAttnLayers < arch.nLayers) {
    const globalL = arch.nGlobalAttnLayers;
    const localL  = arch.nLayers - globalL;
    const hd = arch.headDim, window = arch.localAttnWindow;
    const gkv = arch.nKvHeads, lkv = arch.localKvHeads || 1;
    const gCtx = ctx * s;
    const lCtx = Math.min(ctx, window) * s;
    return (globalL * gkv * hd * gCtx * (k + v)) +
           (localL  * lkv * hd * lCtx * (k + v));
  }
  return effectiveLayers * arch.nKvHeads * arch.headDim * ctx * s * (k + v);
}

function moeWeightSplit(modelBytes, arch, nCpuMoe) {
  if (!arch.nExperts || nCpuMoe <= 0) return { vram: modelBytes, ram: 0 };
  const cpuRatio = Math.min(nCpuMoe, arch.nExperts) / arch.nExperts;
  const expertFrac = arch.expertFraction ?? 0.65;
  const ram = Math.round(modelBytes * expertFrac * cpuRatio);
  return { vram: modelBytes - ram, ram };
}

function mtpBytes(modelBytes, mtp) { return mtp > 0 ? Math.round(modelBytes * 0.015 * mtp) : 0; }
function gpuOverheadBytes(ubatch) { return (300 + Math.max(0, (ubatch - 512)) * 0.15) * 1024 * 1024; }

function maxContext(modelBytes, arch, ctk, ctv, slots, ubatch, nCpuMoe, availVram, fitGran, headroom) {
  if (!availVram) return 0;
  const { vram: wv } = moeWeightSplit(modelBytes, arch, nCpuMoe);
  const mmproj      = arch.mmprojBytes || 0;
  const mtp         = mtpBytes(modelBytes, arch.mtpDepth || 0);
  const linearState = arch.linearAttnStateBytes || 0; // constant; doesn't scale with context
  const oh          = gpuOverheadBytes(ubatch);
  const fixed       = wv + mmproj + mtp + linearState + oh;
  const usable = availVram * (1 - headroom);
  if (fixed >= usable) return 0;
  const kvBudget = usable - fixed;

  // Standard full-attention: solve directly
  if (!arch.localAttnWindow) {
    const s  = Math.max(slots, 1);
    const kv = arch.nLayers * arch.nKvHeads * arch.headDim * s * (kvBpe(ctk) + kvBpe(ctv));
    if (kv <= 0) return 0;
    const ctx = Math.floor(kvBudget / kv);
    const g   = fitGran || 1024;
    return Math.floor(ctx / g) * g;
  }

  // Sliding-window (Gemma): binary search
  let lo = 512, hi = 2_097_152;
  while (lo + 1 < hi) {
    const mid = lo + Math.floor((hi - lo) / 2);
    if (kvBytes(arch, mid, slots, ctk, ctv) <= kvBudget) lo = mid;
    else hi = mid;
  }
  const g = fitGran || 1024;
  return Math.floor(lo / g) * g;
}

function formatCtx(n) {
  if (!n) return '—';
  if (n >= 1_000_000) return (n / 1e6).toFixed(1) + 'M';
  if (n >= 1000) return (n / 1000).toFixed(0) + 'K';
  return String(n);
}
function formatGB(bytes) {
  if (!bytes) return '0 GB';
  return (bytes / 1e9).toFixed(1) + ' GB';
}
function formatBytes(bytes) {
  if (!bytes) return '';
  const b = Number(bytes);
  if (!isFinite(b)) return '';
  if (b >= 1e9) return (b / 1e9).toFixed(1) + ' GB';
  if (b >= 1e6) return (b / 1e6).toFixed(1) + ' MB';
  if (b >= 1e3) return (b / 1e3).toFixed(0) + ' KB';
  return b + ' B';
}
function formatSpeed(bps) {
  if (bps >= 1e9) return (bps / 1e9).toFixed(1) + ' GB/s';
  if (bps >= 1e6) return (bps / 1e6).toFixed(1) + ' MB/s';
  return (bps / 1e3).toFixed(0) + ' KB/s';
}

// ── State ─────────────────────────────────────────────────────────────────────

const STEP_LABELS = ['Profile', 'Model', 'Hardware', 'Summary', 'Spawn'];

const wizardState = {
  currentStep: 0,
  profile: 'balanced',
  useCase: 'general',    // 'agentic' | 'general' | 'roleplay'
  mode: 'guided',
  model: {
    source: 'local',     // 'local' | 'hf' | 'import'
    path: '',
    hfRepo: '',
    hfFile: '',
    paramB: 0,           // estimated parameter count (from HF metadata if available)
    modelBytes: 0,       // file size in bytes once known
  },
  // Architecture from introspection (or heuristic)
  arch: {
    nLayers: 0, nKvHeads: 0, headDim: 0,
    nGlobalAttnLayers: 0, localAttnWindow: 0, localKvHeads: 1,
    nExperts: 0, nExpertsUsed: 0, expertFraction: 0.65,
    mtpDepth: 0, mmprojBytes: 0, paramB: 0,
  },
  hardware: {
    gpuLayers: 'auto', gpuLayersManual: null,
    contextSize: 8192,
    batchSize: 2048, ubatchSize: 512,
    parallelSlots: 1,
    cacheTypeK: 'q8_0', cacheTypeV: 'q8_0',
    nCpuMoe: 0,
    tensorSplit: '',
    fitCtx: 1024,
    kvUnified: false, ignoreEos: false,
  },
  vram: { available: 0 },
  spawn: { inFlight: false, error: '' },
};

// ── DOM refs ──────────────────────────────────────────────────────────────────

let dom = {};

// ── Public API ────────────────────────────────────────────────────────────────

export function initSpawnWizard() {
  cacheDom();
  applyReducedMotion();
  bindEvents();
  bindHfSortSelect();
  bindQuantizerEditor();
  restoreProfile();
  applyProfileVisibility();
  renderHfDiscoverPills();          // static — no network call needed
  loadHfQuickPicks();               // pre-load author quick-picks in background
  loadCommunityPicks();             // load community-picks.json if present

  document.getElementById('btn-open-spawn-wizard')
    ?.addEventListener('click', () => openSpawnWizard());
}

function applyReducedMotion() {
  if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
    document.documentElement.classList.add('reduce-motion');
  }
}

export function openSpawnWizard(opts = {}) {
  if (!dom.overlay) return;
  dom.overlay.classList.add('open');

  if (opts.localPath) {
    // Pre-load a local model path and jump straight to step 2 (model).
    wizardState.model.source = 'local';
    wizardState.model.path = opts.localPath;
    if (dom.modelPathInput) dom.modelPathInput.value = opts.localPath;
    // Select the "local" source card visually.
    dom.modelSourceCards?.forEach(c => {
      c.classList.toggle('selected', c.dataset.source === 'local');
    });
    updateModelInputVisibility();
    showStep(1); // step 1 = Model (0-indexed)
  } else {
    updateModelInputVisibility();
    showStep(0);
  }
}

export function closeSpawnWizard() {
  if (!dom.overlay) return;
  dom.overlay.classList.remove('open');
  resetSpawnStatus();
}

// ── DOM caching ───────────────────────────────────────────────────────────────

function cacheDom() {
  dom.overlay  = document.getElementById('spawn-wizard-overlay');
  dom.closeBtn = document.getElementById('spawn-wizard-close');
  dom.stepLabel  = document.getElementById('wizard-step-label');
  dom.stepBadges = dom.overlay?.querySelectorAll('.step-badge[data-step]');
  dom.steps      = dom.overlay?.querySelectorAll('.wizard-step[id^="wizard-step-"]');
  dom.backBtn  = document.getElementById('wizard-back-btn');
  dom.nextBtn  = document.getElementById('wizard-next-btn');

  // Step 1
  dom.profileCards  = dom.overlay?.querySelectorAll('.profile-card[data-profile]');
  dom.usecaseCards  = dom.overlay?.querySelectorAll('.usecase-card[data-usecase]');

  // Step 2
  dom.modelSourceCards = dom.overlay?.querySelectorAll('.model-source-card[data-source]');
  dom.modelInputLocal  = document.getElementById('model-input-local');
  dom.modelInputHf     = document.getElementById('model-input-hf');
  dom.modelInputImport = document.getElementById('model-input-import');
  dom.modelPathInput   = document.getElementById('spawn-model-path');
  dom.hfRepoInput       = document.getElementById('spawn-hf-repo');
  dom.hfSortSelect      = document.getElementById('spawn-hf-sort');
  dom.hfQuickpicks      = document.getElementById('hf-quickpicks');
  dom.hfSearchResults   = document.getElementById('hf-search-results');
  dom.importPathInput   = document.getElementById('spawn-import-path');
  dom.browseModelBtn   = document.getElementById('spawn-browse-model-btn');
  dom.importBrowseBtn  = document.getElementById('spawn-import-browse-btn');
  dom.selectedModel     = document.getElementById('spawn-selected-model');
  dom.selectedModelName = document.getElementById('spawn-selected-model-name');
  dom.selectedModelMeta = document.getElementById('spawn-selected-model-meta');
  dom.hfFileList       = document.getElementById('spawn-hf-file-list');
  dom.quantAdvisor     = document.getElementById('quant-advisor');
  dom.quantAdvisorTable  = document.getElementById('quant-advisor-table');
  dom.quantAdvisorSubtitle = document.getElementById('quant-advisor-subtitle');

  // Step 3
  dom.vramPanel       = document.getElementById('vram-panel');
  dom.vramPanelTotal  = document.getElementById('vram-panel-total');
  dom.vramBar         = document.getElementById('vram-bar');
  dom.vSegWeights  = document.getElementById('vseg-weights');
  dom.vSegKv       = document.getElementById('vseg-kv');
  dom.vSegMmproj   = document.getElementById('vseg-mmproj');
  dom.vSegMtp      = document.getElementById('vseg-mtp');
  dom.vSegOverhead = document.getElementById('vseg-overhead');
  dom.vSegFree     = document.getElementById('vseg-free');
  dom.vLegWeightsLabel  = document.getElementById('vleg-weights-label');
  dom.vLegKvLabel       = document.getElementById('vleg-kv-label');
  dom.vLegMmprojItem    = document.getElementById('vleg-mmproj');
  dom.vLegMmprojLabel   = document.getElementById('vleg-mmproj-label');
  dom.vLegMtpItem       = document.getElementById('vleg-mtp');
  dom.vLegMtpLabel      = document.getElementById('vleg-mtp-label');
  dom.vLegOverheadLabel = document.getElementById('vleg-overhead-label');
  dom.vLegFreeLabel     = document.getElementById('vleg-free-label');
  dom.vLegFreeDot       = document.getElementById('vleg-free-dot');
  dom.vramScenarios   = document.getElementById('vram-scenarios');
  dom.moeOffloadPanel   = document.getElementById('moe-offload-panel');
  dom.moeOffloadSlider  = document.getElementById('moe-offload-slider');
  dom.moeOffloadSubtitle= document.getElementById('moe-offload-subtitle');
  dom.moeOffloadHint    = document.getElementById('moe-offload-hint');
  dom.vramAutosizeBtn  = document.getElementById('vram-autosize-btn');
  dom.vramAutosizeNote = document.getElementById('vram-autosize-note');

  dom.modeGuidedBtn = document.getElementById('spawn-mode-guided');
  dom.modeRawBtn    = document.getElementById('spawn-mode-raw');
  dom.rawCodeArea   = document.getElementById('spawn-raw-script');
  dom.wizardGuidedSection = document.getElementById('spawn-wizard-guided-section');
  dom.wizardRawSection    = document.getElementById('spawn-wizard-raw-section');
  dom.gpuLayersSelect      = document.getElementById('spawn-gpu-layers');
  dom.gpuLayersManualWrap  = document.getElementById('spawn-gpu-layers-manual-wrap');
  dom.gpuLayersManualInput = document.getElementById('spawn-gpu-layers-manual');
  dom.contextSizeInput   = document.getElementById('spawn-context-size');
  dom.batchSizeInput     = document.getElementById('spawn-batch-size');
  dom.advancedFields     = document.getElementById('spawn-advanced-fields');
  dom.ubatchSizeInput    = document.getElementById('spawn-ubatch-size');
  dom.parallelSlotsInput = document.getElementById('spawn-parallel-slots');
  dom.cacheTypeKSelect   = document.getElementById('spawn-cache-type-k');
  dom.cacheTypeVSelect   = document.getElementById('spawn-cache-type-v');
  dom.nCpuMoeInput       = document.getElementById('spawn-n-cpu-moe');
  dom.tensorSplitInput   = document.getElementById('spawn-tensor-split');
  dom.fitCtxInput        = document.getElementById('spawn-fit-ctx');
  dom.moeNote            = document.getElementById('spawn-moe-note');
  // Legacy VRAM pill (kept for backward compat if HTML still has it)
  dom.vramEstimateText = document.getElementById('spawn-vram-estimate-text');
  dom.vramPill         = document.getElementById('spawn-vram-pill');
  dom.specTypeSelect  = document.getElementById('spawn-spec-type');
  dom.draftModelWrap  = document.getElementById('spawn-draft-model-wrap');
  dom.draftModelInput = document.getElementById('spawn-draft-model');
  dom.kvUnifiedCheck  = document.getElementById('spawn-kv-unified');
  dom.ignoreEosCheck  = document.getElementById('spawn-ignore-eos');

  // Step 4
  dom.summaryList      = document.getElementById('spawn-summary-list');
  dom.summaryWarnings  = document.getElementById('spawn-summary-warnings');
  dom.savePresetBtn    = document.getElementById('spawn-save-preset-btn');
  dom.healthCheckBtn   = document.getElementById('spawn-health-check-btn');

  // Step 5
  dom.spawnServerBtn = document.getElementById('spawn-server-btn');
  dom.statusText     = document.getElementById('spawn-status-text');
  dom.progressFill   = document.getElementById('spawn-progress-fill');
  dom.errorText      = document.getElementById('spawn-error-text');
  dom.successText    = document.getElementById('spawn-success-text');
}

// ── Events ────────────────────────────────────────────────────────────────────

function bindEvents() {
  dom.closeBtn?.addEventListener('click', closeSpawnWizard);
  dom.overlay?.addEventListener('click', e => { if (e.target === dom.overlay) closeSpawnWizard(); });
  document.addEventListener('keydown', e => {
    if (!dom.overlay?.classList.contains('open')) return;
    if (e.key === 'Escape') closeSpawnWizard();
  });

  dom.backBtn?.addEventListener('click', () => {
    if (wizardState.currentStep > 0) showStep(wizardState.currentStep - 1);
  });
  dom.nextBtn?.addEventListener('click', () => {
    const next = wizardState.currentStep + 1;
    if (next < STEP_LABELS.length) {
      if (!validateStep(wizardState.currentStep)) return;
      showStep(next);
    }
  });

  // Profile cards
  dom.profileCards?.forEach(card => {
    card.setAttribute('tabindex', '0'); card.setAttribute('role', 'button');
    card.addEventListener('click', () => {
      wizardState.profile = card.dataset.profile;
      dom.profileCards.forEach(c => c.classList.remove('selected'));
      card.classList.add('selected');
      persistProfile(); applyProfileVisibility();
    });
    card.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); card.click(); } });
  });

  // Use-case cards
  dom.usecaseCards?.forEach(card => {
    card.addEventListener('click', () => {
      wizardState.useCase = card.dataset.usecase;
      dom.usecaseCards.forEach(c => c.classList.remove('selected'));
      card.classList.add('selected');
      updateVramDisplay();
    });
    card.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); card.click(); } });
  });

  // Model source cards
  dom.modelSourceCards?.forEach(card => {
    card.setAttribute('tabindex', '0'); card.setAttribute('role', 'button');
    card.addEventListener('click', () => {
      wizardState.model.source = card.dataset.source;
      dom.modelSourceCards.forEach(c => c.classList.remove('selected'));
      card.classList.add('selected');
      updateModelInputVisibility();
      clearValidationError();
      if (card.dataset.source === 'import') loadThirdPartyModels();
    });
    card.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); card.click(); } });
  });

  dom.browseModelBtn?.addEventListener('click', () => openDeferredFileBrowser('spawn-model-path', 'gguf'));
  dom.importBrowseBtn?.addEventListener('click', () => openDeferredFileBrowser('spawn-import-path', 'gguf'));

  dom.modelPathInput?.addEventListener('input', () => {
    wizardState.model.path = dom.modelPathInput.value.trim();
    wizardState.model.source = 'local';
    onModelPathChanged();
  });

  dom.importPathInput?.addEventListener('input', () => {
    wizardState.model.path = dom.importPathInput.value.trim();
    wizardState.model.source = 'import';
    onModelPathChanged();
  });

  dom.hfRepoInput?.addEventListener('blur', () => triggerHfFileFetch());
  dom.hfRepoInput?.addEventListener('keydown', e => {
    if (e.key === 'Enter') { e.preventDefault(); triggerHfFileFetch(); }
  });

  // Hardware fields
  [
    dom.gpuLayersSelect, dom.gpuLayersManualInput, dom.contextSizeInput,
    dom.batchSizeInput, dom.ubatchSizeInput, dom.parallelSlotsInput,
    dom.cacheTypeKSelect, dom.cacheTypeVSelect, dom.nCpuMoeInput,
    dom.tensorSplitInput, dom.specTypeSelect, dom.draftModelInput,
    dom.kvUnifiedCheck, dom.ignoreEosCheck, dom.fitCtxInput,
  ].forEach(el => {
    el?.addEventListener('input', onHardwareChange);
    el?.addEventListener('change', onHardwareChange);
  });

  dom.gpuLayersSelect?.addEventListener('change', () => {
    wizardState.hardware.gpuLayers = dom.gpuLayersSelect.value;
    if (dom.gpuLayersManualWrap) dom.gpuLayersManualWrap.style.display = dom.gpuLayersSelect.value === 'manual' ? '' : 'none';
  });
  dom.specTypeSelect?.addEventListener('change', () => {
    if (dom.draftModelWrap) dom.draftModelWrap.style.display = dom.specTypeSelect.value === 'draft-model' ? '' : 'none';
  });

  // MoE slider
  dom.moeOffloadSlider?.addEventListener('input', () => {
    wizardState.hardware.nCpuMoe = Number(dom.moeOffloadSlider.value);
    if (dom.nCpuMoeInput) dom.nCpuMoeInput.value = wizardState.hardware.nCpuMoe;
    updateMoeSliderVisuals();
    scheduleVramUpdate();
  });

  // Auto-size button
  dom.vramAutosizeBtn?.addEventListener('click', triggerAutoSize);

  dom.savePresetBtn?.addEventListener('click', saveAsPreset);
  dom.healthCheckBtn?.addEventListener('click', runHealthCheck);
  dom.spawnServerBtn?.addEventListener('click', spawnServer);
  dom.modeGuidedBtn?.addEventListener('click', () => setMode('guided'));
  dom.modeRawBtn?.addEventListener('click', () => setMode('raw'));
  dom.rawCodeArea?.addEventListener('input', onRawCodeChange);
}

// ── Validation ────────────────────────────────────────────────────────────────

function validateStep(step) {
  if (step === 1) {
    const { source, path, hfRepo, hfFile } = wizardState.model;
    if (source === 'local' || source === 'import') {
      if (!path) { showValidationError('Select or enter a model path.'); return false; }
    } else if (source === 'hf') {
      if (!hfRepo) { showValidationError('Enter a HuggingFace repo ID (e.g. bartowski/Llama-3.3-70B-…).'); return false; }
      if (!hfFile) { showValidationError('Select a GGUF file from the list.'); return false; }
    }
  }
  clearValidationError();
  return true;
}

function showValidationError(msg) {
  const stepEl = document.getElementById(`wizard-step-${wizardState.currentStep}`);
  if (!stepEl) return;
  let el = stepEl.querySelector('.wizard-validation-error');
  if (!el) {
    el = document.createElement('div');
    el.className = 'wizard-validation-error';
    el.setAttribute('role', 'alert');
    stepEl.querySelector('.wizard-main')?.prepend(el);
  }
  el.textContent = msg;
  el.style.display = '';
  el.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
}

function clearValidationError() {
  dom.overlay?.querySelectorAll('.wizard-validation-error').forEach(el => { el.style.display = 'none'; });
}

// ── Mode toggle ───────────────────────────────────────────────────────────────

function setMode(mode) {
  wizardState.mode = mode;
  dom.wizardGuidedSection?.classList.toggle('hidden', mode !== 'guided');
  dom.wizardRawSection?.classList.toggle('hidden', mode !== 'raw');
  dom.modeGuidedBtn?.classList.toggle('active', mode === 'guided');
  dom.modeRawBtn?.classList.toggle('active', mode !== 'guided');
  if (mode === 'raw') updateRawScript();
}

function updateRawScript() {
  if (!dom.rawCodeArea) return;
  const s = wizardState, hw = s.hardware, m = s.model;
  const args = [];
  if (m.source === 'hf' && m.hfRepo) {
    args.push('-hf', m.hfRepo);
    if (m.hfFile) args.push('--hf-file', m.hfFile);
  } else if (m.path) {
    args.push('-m', `"${m.path}"`);
  }
  if (hw.gpuLayers === 'manual' && hw.gpuLayersManual != null) args.push('-ngl', String(hw.gpuLayersManual));
  else args.push('-ngl', '9999');
  if (hw.contextSize) args.push('-c', String(hw.contextSize));
  if (hw.batchSize) args.push('-b', String(hw.batchSize));
  if (hw.ubatchSize) args.push('-ub', String(hw.ubatchSize));
  if (hw.parallelSlots > 1) args.push('--parallel', String(hw.parallelSlots));
  if (hw.cacheTypeK) args.push('-ctk', hw.cacheTypeK);
  if (hw.cacheTypeV) args.push('-ctv', hw.cacheTypeV);
  if (hw.nCpuMoe > 0) args.push('--n-cpu-moe', String(hw.nCpuMoe));
  if (hw.tensorSplit) args.push('--tensor-split', hw.tensorSplit);
  if (hw.fitCtx) { args.push('--fit', 'on'); args.push('--fit-ctx', String(hw.fitCtx)); }
  const specType = dom.specTypeSelect?.value || '';
  if (specType) { args.push('--spec-type', specType); if (specType === 'draft-model' && dom.draftModelInput?.value) args.push('-md', `"${dom.draftModelInput.value}"`); }
  if (hw.kvUnified) args.push('--kv-unified');
  if (hw.ignoreEos) args.push('--ignore-eos');
  dom.rawCodeArea.textContent = 'llama-server \\\n  ' + args.join(' \\\n  ');
}

function onRawCodeChange() {
  const text = dom.rawCodeArea?.textContent || '';
  const hw = wizardState.hardware;
  const rf = flag => {
    const idx = text.indexOf(flag + ' ');
    if (idx === -1) return null;
    return text.slice(idx + flag.length).trim().split(/[\s\\]+/)[0] || null;
  };
  const ctx = rf('-c') || rf('--ctx-size'); if (ctx) { const n = Number(ctx); if (n > 0) hw.contextSize = n; }
  const ngl = rf('-ngl'); if (ngl) { const n = Number(ngl); if (!isNaN(n)) { hw.gpuLayersManual = n; hw.gpuLayers = 'manual'; } }
  const bs = rf('-b') || rf('--batch-size'); if (bs) { const n = Number(bs); if (n > 0) hw.batchSize = n; }
  const ub = rf('-ub') || rf('--ubatch-size'); if (ub) { const n = Number(ub); if (n > 0) hw.ubatchSize = n; }
  const moe = rf('--n-cpu-moe'); if (moe) { const n = Number(moe); if (!isNaN(n) && n >= 0) hw.nCpuMoe = n; }
}

// ── Step management ───────────────────────────────────────────────────────────

function showStep(index) {
  wizardState.currentStep = index;
  clearValidationError();

  dom.steps?.forEach(s => s.classList.remove('active'));
  document.getElementById(`wizard-step-${index}`)?.classList.add('active');

  dom.stepBadges?.forEach(b => {
    const s = Number(b.dataset.step);
    b.classList.remove('active', 'completed');
    if (s === index) b.classList.add('active');
    else if (s < index) b.classList.add('completed');
  });

  if (dom.stepLabel) dom.stepLabel.textContent = STEP_LABELS[index] || '';
  if (dom.backBtn) dom.backBtn.style.display = index === 0 ? 'none' : '';
  if (dom.nextBtn) dom.nextBtn.style.display = index === STEP_LABELS.length - 1 ? 'none' : '';

  if (index === 2) {
    // Entering hardware step — refresh VRAM and fetch GPU info
    fetchGpuVram().then(() => scheduleVramUpdate());
  }
  if (index === 3) {
    fetchGpuVram().then(() => estimateVramFull().then(() => renderSummary()));
  }
}

// ── Profile & use-case ────────────────────────────────────────────────────────

function persistProfile() { try { localStorage.setItem('spawn_wizard_profile', wizardState.profile); } catch {} }
function restoreProfile() {
  try {
    const s = localStorage.getItem('spawn_wizard_profile');
    if (s && ['quick','balanced','advanced'].includes(s)) wizardState.profile = s;
  } catch {}
  dom.profileCards?.forEach(c => c.classList.toggle('selected', c.dataset.profile === wizardState.profile));
}
function applyProfileVisibility() {
  const isAdv = wizardState.profile === 'advanced';
  const isQ   = wizardState.profile === 'quick';
  if (dom.advancedFields) dom.advancedFields.classList.toggle('visible', isAdv);
  if (isQ) {
    if (dom.contextSizeInput) dom.contextSizeInput.disabled = true;
    if (dom.batchSizeInput) dom.batchSizeInput.disabled = true;
    if (dom.gpuLayersSelect) { dom.gpuLayersSelect.value = 'auto'; dom.gpuLayersSelect.disabled = true; }
  } else {
    if (dom.contextSizeInput) dom.contextSizeInput.disabled = false;
    if (dom.batchSizeInput) dom.batchSizeInput.disabled = false;
    if (dom.gpuLayersSelect) dom.gpuLayersSelect.disabled = false;
  }
}

// ── Model source visibility ───────────────────────────────────────────────────

function updateModelInputVisibility() {
  const src = wizardState.model.source;
  dom.modelInputLocal?.classList.toggle('visible', src === 'local');
  dom.modelInputHf?.classList.toggle('visible', src === 'hf');
  dom.modelInputImport?.classList.toggle('visible', src === 'import');
}

function updateSelectedModelDisplay() {
  const { source, path, hfRepo, hfFile } = wizardState.model;
  let name = '', meta = '';
  if (source === 'hf' && hfRepo) {
    name = hfFile ? hfFile.split('/').pop() : hfRepo;
    meta = hfFile ? `${hfRepo}  ·  HuggingFace` : 'HuggingFace repo';
  } else if (path) {
    name = path.split(/[\\/]/).pop() || path;
    meta = path;
  }
  if (!name) { dom.selectedModel?.classList.remove('visible'); return; }
  dom.selectedModel?.classList.add('visible');
  if (dom.selectedModelName) dom.selectedModelName.textContent = name;
  if (dom.selectedModelMeta) dom.selectedModelMeta.textContent = meta;
}

// ── Model path changed ────────────────────────────────────────────────────────

function onModelPathChanged() {
  updateSelectedModelDisplay();
  clearValidationError();

  const path = wizardState.model.path;
  if (path) {
    const name = path.split(/[/\\]/).pop() || path;

    // Infer total param count
    const inferredParams = inferParamBFromName(name);
    if (inferredParams > 0) wizardState.model.paramB = inferredParams;

    // Detect MoE from "NB-AMB" suffix (e.g. 35B-A3B, 26B-A4B, 122B-A10B)
    const moeInfo = parseMoeSuffix(name);
    if (moeInfo && !wizardState.arch.nExperts) {
      // We don't know exact expert count without introspection, but we know it's MoE
      // Set a flag so the MoE panel shows up; introspection will fill exact count
      wizardState.arch._isMoePending = true;
      // Rough expert count: assume Qwen3/Gemma4 style ~128 experts for large MoE
      // Will be overridden by introspection
      const totalB = moeInfo.total, activeB = moeInfo.active;
      if (totalB > 20) {
        // Likely many experts (128+)
        wizardState.arch.nExperts = totalB > 100 ? 128 : (totalB > 30 ? 64 : 8);
        wizardState.arch.nExpertsUsed = Math.round(activeB);
      }
    }

    // Detect MTP from filename
    if (detectMtpFromName(name) && !wizardState.arch.mtpDepth) {
      wizardState.arch.mtpDepth = 1; // conservative default; introspection will refine
    }

    // Try introspection (will refine all arch values)
    tryIntrospectModel(path);
  }

  // Update quant advisor if we have param count
  if (wizardState.model.paramB > 0) triggerQuantAdvisor();
  scheduleVramUpdate();
}

function inferParamBFromName(name) {
  // Match patterns like "27B", "7b", "70B", "235b", "3.5b", "122B"
  // Prefer the first large number (total params) not the "active" suffix
  const matches = [...name.matchAll(/(\d+(?:\.\d+)?)\s*[Bb]/gi)];
  if (!matches.length) return 0;
  // If there's a pattern like "35B-A3B" or "122B-A10B", take the larger (total) param count
  const values = matches.map(m => parseFloat(m[1]));
  return Math.max(...values);
}

/// Parse MoE "AXB" active-parameter suffix from a filename.
/// "35B-A3B" → { total: 35, active: 3 }
/// "26B-A4B" → { total: 26, active: 4 }
function parseMoeSuffix(name) {
  // Match "NB-AMB" or "NB_AMB" patterns (N total, M active)
  const m = name.match(/(\d+(?:\.\d+)?)[Bb][-_][Aa](\d+(?:\.\d+)?)[Bb]/i);
  if (!m) return null;
  return { total: parseFloat(m[1]), active: parseFloat(m[2]) };
}

/// Detect if a filename indicates MTP (multi-token prediction) heads.
function detectMtpFromName(name) {
  const lower = name.toLowerCase();
  return lower.includes('mtp') || lower.includes('multi-token') || lower.includes('multitokenprediction');
}

/// Detect mmproj filename for a model, given its path.
/// Looks for a file matching common mmproj naming patterns in the same directory.
function inferMmprojPath(modelPath) {
  if (!modelPath) return null;
  const dir = modelPath.replace(/[/\\][^/\\]+$/, '');
  const basename = modelPath.split(/[/\\]/).pop() || '';

  // Common pattern: strip quant suffix and add mmproj variants
  // e.g. "Qwen3.6-27B-...-Q4_K_S.gguf" → "Qwen3.6-27B-UD-mmproj-BF16.gguf"
  // We can't auto-locate without a dir scan, but we can suggest the pattern.
  const stem = basename.replace(/-?(Q\d|IQ\d|F16|BF16|q\d)[^.]*\.gguf$/i, '');
  return { dir, stem };
}

// ── Introspection ─────────────────────────────────────────────────────────────

let introspectDebounce = null;

function tryIntrospectModel(path) {
  if (!path || !path.toLowerCase().endsWith('.gguf')) return;
  if (introspectDebounce) clearTimeout(introspectDebounce);
  introspectDebounce = setTimeout(() => doIntrospect(path), 1200);
}

async function doIntrospect(path) {
  try {
    const headers = window.authHeaders ? { ...window.authHeaders(), 'Content-Type': 'application/json' } : { 'Content-Type': 'application/json' };
    const resp = await fetch('/api/model/introspect', { method: 'POST', headers, body: JSON.stringify({ model_path: path }) });
    if (!resp.ok) return;
    const data = await resp.json();
    if (!data.ok || !data.metadata) return;

    const m = data.metadata;
    // Merge into arch state
    if (m.n_layers)      wizardState.arch.nLayers    = m.n_layers;
    if (m.n_kv_heads)    wizardState.arch.nKvHeads   = m.n_kv_heads;
    if (m.head_dim)      wizardState.arch.headDim     = m.head_dim;
    if (m.n_experts)     wizardState.arch.nExperts    = m.n_experts;
    if (m.n_experts_used) wizardState.arch.nExpertsUsed = m.n_experts_used;
    if (m.mtp_depth)     wizardState.arch.mtpDepth    = m.mtp_depth;

    // Update MoE slider max if we got expert count
    if (wizardState.arch.nExperts > 0 && dom.moeOffloadSlider) {
      dom.moeOffloadSlider.max = wizardState.arch.nExperts;
    }

    // Detect training context for display
    if (m.n_ctx_train) {
      // Suggest this as a nice upper bound
    }

    scheduleVramUpdate();
    if (wizardState.model.paramB > 0) triggerQuantAdvisor();
  } catch {}
}

// ── GPU VRAM query ────────────────────────────────────────────────────────────

let cachedVram = 0;

async function fetchGpuVram() {
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch('/metrics/gpu', { headers });
    if (!resp.ok) return;
    const data = await resp.json();
    // Look for total VRAM across all GPUs (or unified memory)
    let totalVram = 0;
    const gpus = Array.isArray(data) ? data : (data.gpus || [data]);
    for (const g of gpus) {
      const t = g.vram_total_mb || g.total_mb || g.total_memory_mb || 0;
      totalVram += t * 1024 * 1024;
    }
    if (totalVram > 0) {
      cachedVram = totalVram;
      wizardState.vram.available = totalVram;
    }
  } catch {}
}

// ── Quant advisor (pre-download) ──────────────────────────────────────────────

let quantAdvisorDebounce = null;

function triggerQuantAdvisor() {
  if (quantAdvisorDebounce) clearTimeout(quantAdvisorDebounce);
  quantAdvisorDebounce = setTimeout(loadQuantAdvisor, 600);
}

async function loadQuantAdvisor() {
  const paramB = wizardState.model.paramB;
  if (!paramB || paramB <= 0) return;

  const availVram = cachedVram || wizardState.vram.available;
  if (!availVram) return; // need VRAM to give useful numbers

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };

    // Pass arch info if we have it
    const body = {
      param_b: paramB,
      model_name: wizardState.model.path || wizardState.model.hfRepo || '',
      available_vram_bytes: availVram,
      use_case: wizardState.useCase,
      parallel_slots: wizardState.hardware.parallelSlots,
      n_layers: wizardState.arch.nLayers || undefined,
      n_kv_heads: wizardState.arch.nKvHeads || undefined,
      head_dim: wizardState.arch.headDim || undefined,
      n_experts: wizardState.arch.nExperts || undefined,
      mtp_depth: wizardState.arch.mtpDepth || undefined,
    };

    const resp = await fetch('/api/vram/quant-compare', { method: 'POST', headers, body: JSON.stringify(body) });
    if (!resp.ok) return;
    const data = await resp.json();
    if (!data.ok || !data.quants) return;

    renderQuantAdvisor(data.quants, availVram);
  } catch {}
}

function renderQuantAdvisor(quants, availVram) {
  if (!dom.quantAdvisor || !dom.quantAdvisorTable) return;
  if (!quants || quants.length === 0) { dom.quantAdvisor.style.display = 'none'; return; }

  const availGb = (availVram / 1e9).toFixed(0);
  if (dom.quantAdvisorSubtitle) dom.quantAdvisorSubtitle.textContent = `Estimated VRAM available: ${availGb} GB`;

  const table = document.createElement('table');
  table.className = 'qa-table';

  const thead = table.createTHead();
  const hrow = thead.insertRow();
  ['', 'Quant', 'Size', 'Max ctx (q8_0)', 'Max ctx (q4_0)', 'Quality'].forEach(h => {
    const th = document.createElement('th');
    th.textContent = h;
    hrow.appendChild(th);
  });

  const tbody = table.createTBody();
  for (const q of quants) {
    const tr = tbody.insertRow();
    if (q.recommended) tr.className = 'qa-row-rec';
    if (!q.fits_vram) tr.className = (tr.className + ' qa-row-nofit').trim();

    // Fit dot
    const dotTd = tr.insertCell();
    const dot = document.createElement('span');
    dot.className = 'qa-fit-dot ' + (q.fits_vram ? 'fits' : 'nofit');
    dotTd.appendChild(dot);

    // Quant name + rec badge
    const nameTd = tr.insertCell();
    const nameSpan = document.createElement('span');
    nameSpan.style.fontWeight = '600';
    nameSpan.textContent = q.label;
    nameTd.appendChild(nameSpan);
    if (q.recommended) {
      const badge = document.createElement('span');
      badge.className = 'qa-badge-rec';
      badge.textContent = '★ Rec';
      badge.style.marginLeft = '6px';
      nameTd.appendChild(badge);
    }
    if (q.is_imatrix) {
      const im = document.createElement('span');
      im.style.cssText = 'margin-left:4px; font-size:10px; color:#94a3b8;';
      im.textContent = 'imatrix';
      nameTd.appendChild(im);
    }

    // Size
    const sizeTd = tr.insertCell();
    sizeTd.textContent = q.model_size_gb.toFixed(1) + ' GB';
    sizeTd.style.color = '#94a3b8';

    // Max ctx q8_0
    const ctxQ8Td = tr.insertCell();
    ctxQ8Td.className = 'qa-ctx';
    if (q.max_ctx_q8 > 0) {
      ctxQ8Td.textContent = formatCtx(q.max_ctx_q8);
      ctxQ8Td.classList.add('qa-ctx-q8');
    } else {
      ctxQ8Td.textContent = '—'; ctxQ8Td.classList.add('qa-ctx-na');
    }

    // Max ctx q4_0
    const ctxQ4Td = tr.insertCell();
    ctxQ4Td.className = 'qa-ctx';
    if (q.max_ctx_q4 > 0) {
      ctxQ4Td.textContent = formatCtx(q.max_ctx_q4);
      ctxQ4Td.classList.add('qa-ctx-q4');
    } else {
      ctxQ4Td.textContent = '—'; ctxQ4Td.classList.add('qa-ctx-na');
    }

    // Quality badge
    const qualTd = tr.insertCell();
    const qualBadge = document.createElement('span');
    const qClass = 'qa-quality-' + (q.quality || '').toLowerCase();
    qualBadge.className = `qa-quality-badge ${qClass}`;
    qualBadge.textContent = q.quality_label || q.quality;
    qualTd.appendChild(qualBadge);
  }

  dom.quantAdvisorTable.innerHTML = '';
  dom.quantAdvisorTable.appendChild(table);
  dom.quantAdvisor.style.display = '';
}

// ── HF discover categories ────────────────────────────────────────────────────
// Static curated categories that map to queryable HF API searches.

const HF_DISCOVER_CATEGORIES = [
  { id: 'trending',  label: 'Trending',      params: { query: '',           sort: 'trending',  limit: 30 } },
  { id: 'qwen3',     label: 'Qwen3',         params: { query: 'qwen3',      sort: 'downloads', limit: 30 } },
  { id: 'llama3',    label: 'Llama 3.x',     params: { query: 'llama-3',    sort: 'downloads', limit: 30 } },
  { id: 'mistral',   label: 'Mistral / MoE', params: { query: 'mistral',    sort: 'downloads', limit: 30 } },
  { id: 'deepseek',  label: 'DeepSeek',      params: { query: 'deepseek',   sort: 'downloads', limit: 30 } },
  { id: 'gemma',     label: 'Gemma',         params: { query: 'gemma',      sort: 'downloads', limit: 30 } },
  { id: 'phi4',      label: 'Phi-4',         params: { query: 'phi-4',      sort: 'downloads', limit: 30 } },
  { id: 'command',   label: 'Command R',     params: { query: 'command-r',  sort: 'downloads', limit: 30 } },
];

function renderHfDiscoverPills() {
  const container = document.getElementById('hf-discover-pills');
  if (!container) return;
  container.innerHTML = '';
  for (const cat of HF_DISCOVER_CATEGORIES) {
    const pill = document.createElement('button');
    pill.type = 'button';
    pill.className = 'hf-discover-pill';
    pill.textContent = cat.label;
    pill.dataset.catId = cat.id;
    pill.addEventListener('click', () => {
      // Deactivate all discover + quantizer pills
      container.querySelectorAll('.hf-discover-pill').forEach(p => p.classList.remove('active'));
      dom.hfQuickpicks?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active'));
      pill.classList.add('active');
      wizardState.hfBrowseAuthor = null;
      if (dom.hfRepoInput) dom.hfRepoInput.value = '';
      showHfSearchResults(cat.params);
    });
    container.appendChild(pill);
  }
}

// ── Community picks ───────────────────────────────────────────────────────────

let communityPicksData = null;
let communityPicksActiveCat = 0;

async function loadCommunityPicks() {
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch('/api/hf/community-picks', { headers });
    if (!resp.ok) return;
    const json = await resp.json();
    if (!json.ok || !json.data) return;

    communityPicksData = json.data;
    const panel = document.getElementById('hf-community-picks');
    if (!panel) return;

    const cats = communityPicksData.categories || [];
    const totalModels = cats.reduce((s, c) => s + (c.models?.length || 0), 0);
    const meta = document.getElementById('hf-cp-toggle-meta');
    if (meta) {
      const gen = communityPicksData.generated_at
        ? new Date(communityPicksData.generated_at).toLocaleDateString()
        : '';
      meta.textContent = `${totalModels} models${gen ? ' · ' + gen : ''}`;
    }

    panel.style.display = '';
    renderCommunityPicksTabs(cats);
    renderCommunityPicksList(cats[0]);

    document.getElementById('hf-cp-toggle')?.addEventListener('click', () => {
      const body = document.getElementById('hf-cp-body');
      const toggle = document.getElementById('hf-cp-toggle');
      if (!body || !toggle) return;
      const open = toggle.getAttribute('aria-expanded') === 'true';
      toggle.setAttribute('aria-expanded', String(!open));
      body.style.display = open ? 'none' : '';
    });
  } catch {}
}

function renderCommunityPicksTabs(cats) {
  const tabs = document.getElementById('hf-cp-tabs');
  if (!tabs) return;
  tabs.innerHTML = '';
  cats.forEach((cat, i) => {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'hf-cp-tab' + (i === communityPicksActiveCat ? ' active' : '');
    btn.textContent = cat.label;
    btn.title = cat.description || '';
    btn.addEventListener('click', () => {
      communityPicksActiveCat = i;
      tabs.querySelectorAll('.hf-cp-tab').forEach((t, j) =>
        t.classList.toggle('active', j === i)
      );
      renderCommunityPicksList(cat);
    });
    tabs.appendChild(btn);
  });
}

function renderCommunityPicksList(cat) {
  const list = document.getElementById('hf-cp-list');
  if (!list) return;
  const models = cat?.models || [];

  if (!models.length) {
    const emptyDiv = document.createElement('div');
    emptyDiv.className = 'hf-cp-empty';
    const strong = document.createElement('strong');
    strong.textContent = 'No picks in this category yet';
    emptyDiv.appendChild(strong);
    emptyDiv.appendChild(document.createTextNode('Run the Hermes community-picks cron to populate it.'));
    list.appendChild(emptyDiv);
    return;
  }

  list.innerHTML = '';
  for (const m of models) {
    const item = document.createElement('div');
    item.className = 'hf-cp-item';
    item.tabIndex = 0;
    item.setAttribute('role', 'button');

    const sizeLabel = m.param_b > 0
      ? (m.param_b >= 1000 ? (m.param_b / 1000).toFixed(1) + 'T' : m.param_b + 'B')
      : '';

    const mainDiv = document.createElement('div');
    mainDiv.className = 'hf-cp-item-main';
    const nameDiv = document.createElement('div');
    nameDiv.className = 'hf-cp-name';
    nameDiv.textContent = m.display_name || m.hf_repo;
    mainDiv.appendChild(nameDiv);
    if (m.why) {
      const whyDiv = document.createElement('div');
      whyDiv.className = 'hf-cp-why';
      whyDiv.textContent = m.why;
      mainDiv.appendChild(whyDiv);
    }
    item.appendChild(mainDiv);

    const metaDiv = document.createElement('div');
    metaDiv.className = 'hf-cp-meta';
    const mkBadge = (cls, text) => {
      const s = document.createElement('span');
      s.className = `hf-cp-badge ${cls}`;
      s.textContent = text;
      return s;
    };
    if (sizeLabel)       metaDiv.appendChild(mkBadge('hf-cp-badge-size', sizeLabel));
    if (m.quant_rec)     metaDiv.appendChild(mkBadge('hf-cp-badge-quant', m.quant_rec));
    if (m.is_moe)        metaDiv.appendChild(mkBadge('hf-cp-badge-moe', 'MoE'));
    if (m.mention_count > 0) {
      const mentions = document.createElement('span');
      mentions.className = 'hf-cp-mentions';
      mentions.textContent = `${m.mention_count} mentions`;
      metaDiv.appendChild(mentions);
    }
    item.appendChild(metaDiv);

    const loadPick = () => {
      // Deactivate discover/quantizer pills
      document.getElementById('hf-discover-pills')
        ?.querySelectorAll('.hf-discover-pill').forEach(p => p.classList.remove('active'));
      dom.hfQuickpicks?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active'));
      // Pre-fill repo input and load files
      if (dom.hfRepoInput) dom.hfRepoInput.value = m.hf_repo;
      wizardState.model.hfRepo = m.hf_repo;
      if (m.param_b > 0) wizardState.model.paramB = m.param_b;
      if (dom.hfSearchResults) dom.hfSearchResults.style.display = 'none';
      fetchHfFiles(m.hf_repo);
      if (m.param_b > 0) triggerQuantAdvisor();
      clearValidationError();
    };
    item.addEventListener('click', loadPick);
    item.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); loadPick(); } });

    list.appendChild(item);
  }
}

function escHtml(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

// ── HF quick-picks ────────────────────────────────────────────────────────────

async function loadHfQuickPicks() {
  if (!dom.hfQuickpicks) return;
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch('/api/hf/quantizers', { headers });
    if (!resp.ok) return;
    const data = await resp.json();
    if (!data.ok || !data.quantizers) return;

    dom.hfQuickpicks.innerHTML = '';
    for (const q of data.quantizers) {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'hf-qp-btn';
      if (q.quant_style === 'imatrix') btn.classList.add('hf-qp-imatrix');
      if (q.quant_style === 'ud')      btn.classList.add('hf-qp-ud');
      btn.textContent = q.display_name;
      btn.title = q.description + (q.note ? `\n\n${q.note}` : '');
      btn.dataset.author = q.username;
      btn.addEventListener('click', () => {
        dom.hfQuickpicks?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active'));
        document.getElementById('hf-discover-pills')
          ?.querySelectorAll('.hf-discover-pill').forEach(p => p.classList.remove('active'));
        btn.classList.add('active');
        // Clear the repo input and show author models
        if (dom.hfRepoInput) dom.hfRepoInput.value = '';
        browseHfAuthor(q.username);
      });
      dom.hfQuickpicks.appendChild(btn);
    }
  } catch {}
}

// ── Quantizer editor ──────────────────────────────────────────────────────────

let quantizerEditorList = []; // live copy while editor is open

function bindQuantizerEditor() {
  const editBtn = document.getElementById('hf-qp-edit-btn');
  const editor  = document.getElementById('hf-qp-editor');
  if (!editBtn || !editor) return;

  editBtn.addEventListener('click', () => {
    const open = editBtn.getAttribute('aria-expanded') === 'true';
    editBtn.setAttribute('aria-expanded', String(!open));
    editor.style.display = open ? 'none' : '';
    if (!open) openQuantizerEditor();
  });

  document.getElementById('hf-qp-editor-add-btn')?.addEventListener('click', () => {
    const usernameEl = document.getElementById('hf-qp-editor-username');
    const displayEl  = document.getElementById('hf-qp-editor-displayname');
    const username = usernameEl?.value.trim();
    if (!username) return;
    const display_name = displayEl?.value.trim() || username;
    quantizerEditorList.push({ username, display_name, description: '', quant_style: 'standard', note: null });
    renderEditorList();
    if (usernameEl) usernameEl.value = '';
    if (displayEl)  displayEl.value = '';
  });

  document.getElementById('hf-qp-editor-save-btn')?.addEventListener('click', saveQuantizerEdits);
  document.getElementById('hf-qp-editor-reset-btn')?.addEventListener('click', resetQuantizersToDefaults);
}

async function openQuantizerEditor() {
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch('/api/hf/quantizers', { headers });
    const data = await resp.json();
    if (data.ok && data.quantizers) {
      quantizerEditorList = data.quantizers.map(q => ({ ...q }));
      renderEditorList();
    }
  } catch {}
}

function renderEditorList() {
  const container = document.getElementById('hf-qp-editor-list');
  if (!container) return;
  container.innerHTML = '';
  for (let i = 0; i < quantizerEditorList.length; i++) {
    const q = quantizerEditorList[i];
    const row = document.createElement('div');
    row.className = 'hf-qp-editor-row';

    const styleClass = q.quant_style === 'imatrix' ? 'hf-qp-imatrix'
                     : q.quant_style === 'ud'       ? 'hf-qp-ud' : '';
    const label = document.createElement('span');
    label.className = `hf-qp-editor-name ${styleClass}`;
    label.textContent = q.display_name || q.username;
    label.title = q.username + (q.description ? '\n' + q.description : '');

    const removeBtn = document.createElement('button');
    removeBtn.type = 'button';
    removeBtn.className = 'hf-qp-editor-remove';
    removeBtn.textContent = '×';
    removeBtn.title = `Remove ${q.username}`;
    removeBtn.addEventListener('click', () => {
      quantizerEditorList.splice(i, 1);
      renderEditorList();
    });

    row.appendChild(label);
    row.appendChild(removeBtn);
    container.appendChild(row);
  }
}

async function saveQuantizerEdits() {
  const headers = window.authHeaders
    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
    : { 'Content-Type': 'application/json' };
  try {
    const resp = await fetch('/api/hf/quantizers', {
      method: 'PUT',
      headers,
      body: JSON.stringify(quantizerEditorList),
    });
    const data = await resp.json();
    if (data.ok) {
      // Close editor and reload quick-picks
      document.getElementById('hf-qp-edit-btn')?.setAttribute('aria-expanded', 'false');
      document.getElementById('hf-qp-editor')?.style && (document.getElementById('hf-qp-editor').style.display = 'none');
      loadHfQuickPicks();
    }
  } catch {}
}

async function resetQuantizersToDefaults() {
  const headers = window.authHeaders
    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
    : { 'Content-Type': 'application/json' };
  try {
    await fetch('/api/hf/quantizers', { method: 'PUT', headers, body: '[]' });
    // Reload defaults into editor
    await openQuantizerEditor();
    loadHfQuickPicks();
  } catch {}
}

async function browseHfAuthor(author) {
  const sort = dom.hfSortSelect?.value || 'downloads';
  wizardState.hfBrowseAuthor = author;
  await showHfSearchResults({ author, sort, limit: 40 });
}

/// Show HF model search / author browse results.
async function showHfSearchResults({ query, author, sort, limit }) {
  const container = dom.hfSearchResults;
  if (!container) return;

  container.innerHTML = '<div class="hf-search-loading">Searching HuggingFace…</div>';
  container.style.display = '';

  // Hide file list when showing search results
  if (dom.hfFileList) { dom.hfFileList.innerHTML = ''; dom.hfFileList.classList.remove('visible'); }

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };

    const body = {
      query: query || '',
      author: author || undefined,
      sort: sort || 'downloads',
      limit: limit || 20,
    };

    const resp = await fetch('/api/hf/search', { method: 'POST', headers, body: JSON.stringify(body) });
    if (!resp.ok) { container.innerHTML = '<div class="hf-search-empty">Search failed.</div>'; return; }
    const data = await resp.json();
    const models = data.models || [];

    container.innerHTML = '';
    if (!models.length) {
      container.innerHTML = '<div class="hf-search-empty">No models found.</div>';
      return;
    }

    models.forEach(m => {
      const row = document.createElement('div');
      row.className = 'hf-search-result';
      row.setAttribute('tabindex', '0');
      row.setAttribute('role', 'button');

      const nameEl = document.createElement('span');
      nameEl.className = 'hf-sr-name';
      nameEl.textContent = m.id || '';

      const meta = document.createElement('span');
      meta.className = 'hf-sr-meta';

      // Downloads badge
      if (m.downloads > 0) {
        const dl = document.createElement('span');
        dl.textContent = m.downloads >= 1000 ? `${(m.downloads/1000).toFixed(0)}k↓` : `${m.downloads}↓`;
        meta.appendChild(dl);
      }

      // Provider/type badges
      if (m.has_imatrix) {
        const b = document.createElement('span');
        b.className = 'hf-sr-badge hf-sr-badge-imatrix';
        b.textContent = 'imatrix';
        meta.appendChild(b);
      } else if ((m.quant_provider || '').toLowerCase() === 'unsloth') {
        const b = document.createElement('span');
        b.className = 'hf-sr-badge hf-sr-badge-ud';
        b.textContent = 'UD';
        meta.appendChild(b);
      }
      if (m.gated) {
        const b = document.createElement('span');
        b.className = 'hf-sr-badge hf-sr-badge-gated';
        b.textContent = 'gated';
        meta.appendChild(b);
      }
      const lowerTags = (m.tags || []).map(t => t.toLowerCase());
      if (lowerTags.some(t => t.includes('moe'))) {
        const b = document.createElement('span');
        b.className = 'hf-sr-badge hf-sr-badge-moe';
        b.textContent = 'MoE';
        meta.appendChild(b);
      }

      row.appendChild(nameEl);
      row.appendChild(meta);

      const selectRepo = () => {
        wizardState.model.hfRepo = m.id;
        if (dom.hfRepoInput) dom.hfRepoInput.value = m.id;
        if (m.param_b > 0) wizardState.model.paramB = m.param_b;
        // Hide search results, load files
        container.style.display = 'none';
        // Clear quick-pick active state
        dom.hfQuickpicks?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active'));
        fetchHfFiles(m.id);
        if (m.param_b > 0) triggerQuantAdvisor();
        clearValidationError();
      };
      row.addEventListener('click', selectRepo);
      row.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectRepo(); } });

      container.appendChild(row);
    });
  } catch (err) {
    const errEl = document.createElement('div');
    errEl.className = 'hf-search-empty';
    errEl.textContent = 'Error: ' + (err.message || String(err));
    container.appendChild(errEl);
  }
}

// ── HF file listing ───────────────────────────────────────────────────────────

function triggerHfFileFetch() {
  const input = dom.hfRepoInput?.value.trim();
  if (!input) return;

  // Detect: "user/repo" = direct repo ID → load files
  //         anything else = keyword search
  const isRepoId = input.includes('/') && !input.includes(' ');

  if (isRepoId) {
    wizardState.model.hfRepo = input;
    // Hide search results if open
    if (dom.hfSearchResults) dom.hfSearchResults.style.display = 'none';
    // Clear quick-pick active
    dom.hfQuickpicks?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active'));
    const inferredP = inferParamBFromName(input);
    if (inferredP > 0) wizardState.model.paramB = inferredP;
    fetchHfFiles(input);
  } else {
    // Keyword search across all GGUF models
    const sort = dom.hfSortSelect?.value || 'downloads';
    showHfSearchResults({ query: input, sort, limit: 20 });
  }
}

// Sort select triggers a re-search
function bindHfSortSelect() {
  dom.hfSortSelect?.addEventListener('change', () => {
    const author = wizardState.hfBrowseAuthor;
    const query  = dom.hfRepoInput?.value.trim() || '';
    const sort   = dom.hfSortSelect.value;
    if (author) {
      browseHfAuthor(author);
    } else if (query && !query.includes('/')) {
      showHfSearchResults({ query, sort, limit: 20 });
    }
  });
}

async function fetchHfFiles(repo) {
  if (!dom.hfFileList) return;
  dom.hfFileList.innerHTML = '<div class="hf-file-loading">Loading GGUF files…</div>';
  dom.hfFileList.classList.add('visible');

  // Also fetch VRAM so quant advisor has numbers
  if (!cachedVram) await fetchGpuVram();

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const resp = await fetch('/api/hf/files', { method: 'POST', headers, body: JSON.stringify({ repo_id: repo }) });
    if (!resp.ok) { dom.hfFileList.innerHTML = '<div class="hf-file-empty">Failed to load files. Check the repo ID.</div>'; return; }
    const data = await resp.json();
    const files = (data.files || []).filter(Boolean);

    dom.hfFileList.innerHTML = '';
    if (!files.length) { dom.hfFileList.innerHTML = '<div class="hf-file-empty">No GGUF files found in this repo.</div>'; return; }

    const vramGb = cachedVram / 1e9;

    files.forEach(file => {
      const fname = file.path || file.name || '';
      if (!fname) return;
      const item = document.createElement('div');
      item.className = 'hf-file-item';
      item.setAttribute('tabindex', '0'); item.setAttribute('role', 'button');
      item.dataset.filename = fname;
      item.dataset.size = file.size || '';

      const nameSpan = document.createElement('span');
      nameSpan.className = 'hf-file-name';
      nameSpan.textContent = fname.split('/').pop() || fname;

      const metaSpan = document.createElement('span');
      metaSpan.className = 'hf-file-size';
      const parts = [];
      if (file.size) parts.push(formatBytes(file.size));
      if (file.label) {
        parts.push(file.label);
        if (vramGb > 0 && file.label === getRecommendedQuant(vramGb)) parts.push('✓ Recommended');
      }
      metaSpan.textContent = parts.join(' · ');

      // Quant type and mmproj badges
      const qt = file.quant_type || '';
      if (qt === 'imatrix' || file.is_imatrix) {
        const b = document.createElement('span');
        b.className = 'hf-file-badge hf-file-badge-imatrix';
        b.textContent = 'imatrix';
        b.title = 'Importance-matrix calibrated — better quality at same bpw (mradermacher style)';
        nameSpan.appendChild(b);
      } else if (qt === 'unsloth_dynamic') {
        const b = document.createElement('span');
        b.className = 'hf-file-badge hf-file-badge-ud';
        b.textContent = 'UD';
        b.title = 'Unsloth Dynamic — mixed bits per layer, excellent quality/size tradeoff';
        nameSpan.appendChild(b);
      }
      if (file.is_mmproj) {
        const b = document.createElement('span');
        b.className = 'hf-file-badge hf-file-badge-mmproj';
        b.textContent = 'mmproj';
        b.title = 'Vision projector — load alongside the main model for multimodal inference';
        nameSpan.appendChild(b);
      }

      item.appendChild(nameSpan);
      item.appendChild(metaSpan);

      const selectFile = () => {
        if (file.is_mmproj) {
          // Selecting an mmproj file — store as companion, don't change main model
          wizardState.model.mmprojPath = fname;
          wizardState.model.mmprojHfFile = fname;
          // Estimate mmproj size for VRAM
          if (file.size) wizardState.arch.mmprojBytes = Number(file.size);
          showToast('mmproj selected', 'success', fname.split('/').pop());
          dom.hfFileList.querySelectorAll('.hf-file-item.selected[data-mmproj]').forEach(el => el.classList.remove('selected'));
          item.classList.add('selected');
          item.dataset.mmproj = '1';
          scheduleVramUpdate();
          return;
        }

        dom.hfFileList.querySelectorAll('.hf-file-item.selected:not([data-mmproj])').forEach(el => el.classList.remove('selected'));
        item.classList.add('selected');
        wizardState.model.hfFile = fname;
        wizardState.model.path = ''; // not a local path
        if (file.size) wizardState.model.modelBytes = Number(file.size);

        // Infer param count from filename if not yet known
        if (!wizardState.model.paramB) wizardState.model.paramB = inferParamBFromName(fname) || inferParamBFromName(repo);

        // Detect MTP from filename
        if (detectMtpFromName(fname) && !wizardState.arch.mtpDepth) {
          wizardState.arch.mtpDepth = 1;
        }

        updateSelectedModelDisplay();
        clearValidationError();
        if (wizardState.model.paramB > 0) triggerQuantAdvisor();
        scheduleVramUpdate();
      };
      item.addEventListener('click', selectFile);
      item.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectFile(); } });

      dom.hfFileList.appendChild(item);
    });
  } catch {
    dom.hfFileList.innerHTML = '<div class="hf-file-empty">Error loading files. Check the repo ID and your HF token.</div>';
  }
}

function getRecommendedQuant(vramGb) {
  if (vramGb < 8)  return 'Q4_K_M';
  if (vramGb <= 16) return 'Q5_K_M';
  if (vramGb <= 24) return 'Q5_K_M';
  return 'Q8_0';
}

// ── Third-party model import ──────────────────────────────────────────────────

async function loadThirdPartyModels() {
  if (!dom.importPathInput) return;
  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const resp = await fetch('/api/third-party-models', { method: 'POST', headers, body: JSON.stringify({ include_subdirs: true }) });
    if (!resp.ok) return;
    const data = await resp.json();
    const models = (data.models || []).filter(Boolean);
    if (!models.length) return;

    const listId = 'spawn-import-datalist';
    let dl = document.getElementById(listId);
    if (!dl) {
      dl = document.createElement('datalist');
      dl.id = listId;
      dom.importPathInput.setAttribute('list', listId);
      dom.importPathInput.parentNode.appendChild(dl);
    }
    dl.innerHTML = '';
    models.forEach(m => {
      const opt = document.createElement('option');
      opt.value = m.path;
      dl.appendChild(opt);
    });
  } catch {}
}

// ── Hardware change ───────────────────────────────────────────────────────────

let vramDebounce = null;

function onHardwareChange() {
  readHardwareState();
  scheduleVramUpdate();
}

function readHardwareState() {
  const h = wizardState.hardware;
  h.gpuLayers = dom.gpuLayersSelect?.value ?? 'auto';
  if (dom.gpuLayersManualInput) {
    const v = dom.gpuLayersManualInput.value;
    h.gpuLayersManual = v !== '' ? Number(v) : null;
  }
  if (dom.contextSizeInput) { const v = Number(dom.contextSizeInput.value); h.contextSize = v > 0 ? v : 8192; }
  if (dom.batchSizeInput)   { const v = Number(dom.batchSizeInput.value);   h.batchSize   = v > 0 ? v : 2048; }
  if (dom.ubatchSizeInput)  { const v = Number(dom.ubatchSizeInput.value);  h.ubatchSize  = v > 0 ? v : 512;  }
  if (dom.parallelSlotsInput) { const v = Number(dom.parallelSlotsInput.value); h.parallelSlots = v > 0 ? v : 1; }
  if (dom.cacheTypeKSelect) h.cacheTypeK = dom.cacheTypeKSelect.value || 'q8_0';
  if (dom.cacheTypeVSelect) h.cacheTypeV = dom.cacheTypeVSelect.value || 'q8_0';
  if (dom.nCpuMoeInput) { const v = dom.nCpuMoeInput.value; h.nCpuMoe = v !== '' ? Number(v) : 0; }
  if (dom.tensorSplitInput) h.tensorSplit = dom.tensorSplitInput.value.trim() || '';
  if (dom.fitCtxInput) { const v = Number(dom.fitCtxInput.value); h.fitCtx = v > 0 ? v : 1024; }
  if (dom.kvUnifiedCheck) h.kvUnified = dom.kvUnifiedCheck.checked;
  if (dom.ignoreEosCheck) h.ignoreEos = dom.ignoreEosCheck.checked;
}

function scheduleVramUpdate() {
  if (vramDebounce) clearTimeout(vramDebounce);
  vramDebounce = setTimeout(updateVramDisplay, 250);
}

// ── Animated VRAM display ─────────────────────────────────────────────────────

function getEffectiveArch() {
  const a = wizardState.arch;
  // If we don't have introspection data, build heuristic from param count
  if (!a.nLayers && wizardState.model.paramB > 0) {
    return buildHeuristicArch(wizardState.model.path || wizardState.model.hfRepo, wizardState.model.paramB);
  }
  return a;
}

function buildHeuristicArch(name, paramB) {
  const lower = (name || '').toLowerCase();

  // ── Qwen3-Coder-Next: hybrid DeltaNet + MoE ──────────────────────────────
  if (lower.includes('coder-next') || lower.includes('qwen3-coder-next')) {
    // 48 layers (12 attn + 36 DeltaNet), 512 experts / 11 active, head_dim 256
    return {
      nLayers: 48, nKvHeads: 2, headDim: 256,
      nAttnLayers: 12, // only these 12 use KV cache
      linearAttnStateBytes: 36 * 32 * 128 * 128 * 2, // ~38 MB (negligible)
      nGlobalAttnLayers: 0, localAttnWindow: 0, localKvHeads: 1,
      nExperts: 512, nExpertsUsed: 11, expertFraction: 0.92,
      mtpDepth: wizardState.arch.mtpDepth || 0,
      mmprojBytes: wizardState.arch.mmprojBytes || 0,
    };
  }

  // ── Qwen3.6 family: hybrid DeltaNet, 1/4 attn layers ─────────────────────
  // Covers: Qwen3.6-27B (dense), Qwen3.6-35B-A3B (MoE), davidau 40B expansion,
  // and all finetunes/distillations that mention Qwen3.6 in the name.
  if (lower.includes('qwen3.6') || lower.includes('qwen3-6')) {
    const nLayers = paramB > 35 ? 96 : 64;
    const nAttnLayers = Math.floor(nLayers / 4); // exactly 1:3 attn:deltanet ratio
    const nDeltanet = nLayers - nAttnLayers;
    const linearState = nDeltanet * 48 * 128 * 128 * 2; // ~76 MB for 27B
    const isMoe = parseMoeSuffix(name) !== null || lower.includes('a3b');
    return {
      nLayers, nKvHeads: 4, headDim: 256,
      nAttnLayers, linearAttnStateBytes: linearState,
      nGlobalAttnLayers: 0, localAttnWindow: 0, localKvHeads: 1,
      nExperts: isMoe ? 64 : 0,
      nExpertsUsed: isMoe ? 3 : 0,
      expertFraction: isMoe ? 0.80 : 0.65,
      mtpDepth: wizardState.arch.mtpDepth || (detectMtpFromName(name) ? 1 : 0),
      mmprojBytes: wizardState.arch.mmprojBytes || 0,
      paramB,
    };
  }

  // Gemma-4 MoE (e.g. Gemma-4-26B-A4B) is both Gemma alternating attention AND MoE
  const isGemma = lower.includes('gemma-3') || lower.includes('gemma3') ||
                  lower.includes('gemma-4') || lower.includes('gemma4');

  if (isGemma) {
    const n = paramB < 5 ? [34, 4, 256] : paramB < 14 ? [52, 8, 256] : [62, 16, 256];
    const globalL = Math.round(n[0] / 6);
    return {
      nLayers: n[0], nKvHeads: n[1], headDim: n[2],
      nGlobalAttnLayers: globalL, localAttnWindow: 512, localKvHeads: 1,
      // Inherit MoE state if already detected (e.g. Gemma-4-26B-A4B)
      nExperts: wizardState.arch.nExperts || 0,
      nExpertsUsed: wizardState.arch.nExpertsUsed || 0,
      expertFraction: 0.65,
      mtpDepth: wizardState.arch.mtpDepth || 0,
      mmprojBytes: wizardState.arch.mmprojBytes || 0,
    };
  }

  // Standard heuristic
  let nl, nkv, hd;
  if (paramB < 2)       { nl=22;  nkv=4;  hd=64;  }
  else if (paramB < 5)  { nl=28;  nkv=4;  hd=128; }
  else if (paramB < 10) { nl=32;  nkv=8;  hd=128; }
  else if (paramB < 18) { nl=40;  nkv=8;  hd=128; }
  else if (paramB < 35) { nl=40;  nkv=8;  hd=128; }
  else if (paramB < 55) { nl=60;  nkv=8;  hd=128; }
  else                  { nl=80;  nkv=8;  hd=128; }

  return {
    nLayers: nl, nKvHeads: nkv, headDim: hd,
    nGlobalAttnLayers: 0, localAttnWindow: 0, localKvHeads: 1,
    nExperts: wizardState.arch.nExperts || 0,
    nExpertsUsed: wizardState.arch.nExpertsUsed || 0,
    expertFraction: wizardState.arch.expertFraction || 0.65,
    mtpDepth: wizardState.arch.mtpDepth || 0,
    mmprojBytes: wizardState.arch.mmprojBytes || 0,
    paramB,
  };
}

async function estimateVramFull() {
  // Called from JS math; no server round-trip needed for the breakdown
  updateVramDisplay();
}

function getModelBytes() {
  if (wizardState.model.modelBytes > 0) return wizardState.model.modelBytes;
  // Estimate from file size stat (local) or param count + quant
  const path = wizardState.model.path;
  if (!path) {
    // HF: estimate from param count + selected quant
    const paramB = wizardState.model.paramB;
    if (!paramB) return 0;
    // Guess quant from filename of selected HF file
    const fname = (wizardState.model.hfFile || '').toLowerCase();
    const quant = guessQuantFromName(fname);
    const BPW = { f16:16, q8_0:8.5, q6_k:6.5625, q5_k_m:5.69, q5_k_s:5.52, q4_k_m:4.85, q4_k_s:4.58, q4_0:4.55, iq4_xs:4.25, q3_k_m:3.875, q2_k:2.625, iq2_xxs:2.0625, iq1_m:1.75 };
    const bpw = BPW[quant] ?? 4.85;
    return Math.round(paramB * 1e9 * bpw / 8);
  }
  return 0; // will be 0 until file stat succeeds
}

function guessQuantFromName(name) {
  const lower = name.toLowerCase();
  if (lower.includes('q8_0')) return 'q8_0';
  if (lower.includes('q6_k')) return 'q6_k';
  if (lower.includes('q5_k_m')) return 'q5_k_m';
  if (lower.includes('q5_k_s')) return 'q5_k_s';
  if (lower.includes('q4_k_m')) return 'q4_k_m';
  if (lower.includes('q4_k_s')) return 'q4_k_s';
  if (lower.includes('iq4_xs')) return 'iq4_xs';
  if (lower.includes('q4_0')) return 'q4_0';
  if (lower.includes('q3_k_m')) return 'q3_k_m';
  if (lower.includes('q2_k')) return 'q2_k';
  if (lower.includes('iq2_xxs')) return 'iq2_xxs';
  if (lower.includes('f16') || lower.includes('bf16')) return 'f16';
  return 'q4_k_m'; // reasonable default
}

function updateVramDisplay() {
  const availVram = cachedVram || wizardState.vram.available;
  if (!dom.vramPanel) return;

  const hw = wizardState.hardware;
  const arch = getEffectiveArch();
  const modelBytes = getModelBytes();

  // Compute breakdown
  const nCpuMoe = hw.nCpuMoe || 0;
  const expertFrac = arch.expertFraction || 0.65;
  const nExperts = arch.nExperts || 0;
  const cpuRatio = nExperts > 0 ? Math.min(nCpuMoe, nExperts) / nExperts : 0;
  const ramBytes = Math.round(modelBytes * expertFrac * cpuRatio);
  const weightVram = modelBytes - ramBytes;
  const kv          = kvBytes(arch, hw.contextSize, hw.parallelSlots, hw.cacheTypeK, hw.cacheTypeV);
  const mmproj      = arch.mmprojBytes || 0;
  const mtp         = mtpBytes(modelBytes, arch.mtpDepth || 0);
  const linearState = arch.linearAttnStateBytes || 0;
  const oh          = gpuOverheadBytes(hw.ubatchSize);
  const total       = weightVram + kv + linearState + mmproj + mtp + oh;
  const free = availVram - total;

  // Update total label
  if (dom.vramPanelTotal) {
    if (availVram > 0) dom.vramPanelTotal.textContent = formatGB(availVram) + ' total';
    else dom.vramPanelTotal.textContent = 'GPU VRAM unknown';
  }

  // Update bar segments (width as % of availVram or total, whichever is larger)
  const denom = availVram > 0 ? availVram : total;
  if (denom > 0) {
    setSegWidth(dom.vSegWeights,  weightVram / denom);
    setSegWidth(dom.vSegKv,       kv / denom);
    setSegWidth(dom.vSegMmproj,   mmproj / denom);
    setSegWidth(dom.vSegMtp,      mtp / denom);
    setSegWidth(dom.vSegOverhead, oh / denom);
    setSegWidth(dom.vSegFree,     Math.max(0, free) / denom);
    if (dom.vSegFree) dom.vSegFree.classList.toggle('over-budget', free < 0);
  }

  // Bar state class
  if (dom.vramBar) {
    const ratio = availVram > 0 ? total / availVram : 0;
    dom.vramBar.classList.toggle('tight', ratio >= 0.88 && ratio < 1.0);
    dom.vramBar.classList.toggle('over', ratio >= 1.0);
  }

  // Update legend labels
  if (dom.vLegWeightsLabel) dom.vLegWeightsLabel.textContent = `Weights ${formatGB(weightVram)}`;
  if (dom.vLegKvLabel)       dom.vLegKvLabel.textContent       = `KV ${formatGB(kv)}`;
  if (mmproj > 0) {
    if (dom.vLegMmprojItem)  dom.vLegMmprojItem.style.display = '';
    if (dom.vLegMmprojLabel) dom.vLegMmprojLabel.textContent  = `mmproj ${formatGB(mmproj)}`;
  } else {
    if (dom.vLegMmprojItem) dom.vLegMmprojItem.style.display = 'none';
  }
  if (mtp > 0) {
    if (dom.vLegMtpItem)  dom.vLegMtpItem.style.display = '';
    if (dom.vLegMtpLabel) dom.vLegMtpLabel.textContent  = `MTP ${formatGB(mtp)}`;
  } else {
    if (dom.vLegMtpItem) dom.vLegMtpItem.style.display = 'none';
  }
  if (dom.vLegOverheadLabel) dom.vLegOverheadLabel.textContent = `OH ${formatGB(oh)}`;
  if (dom.vLegFreeLabel) {
    const freeAbs = Math.abs(free);
    dom.vLegFreeLabel.textContent = free >= 0 ? `Free ${formatGB(free)}` : `Over ${formatGB(freeAbs)}`;
    if (dom.vLegFreeDot) dom.vLegFreeDot.style.background = free >= 0 ? '' : '#ef4444';
  }

  // Show/hide MoE panel
  if (arch.nExperts > 1) {
    if (dom.moeOffloadPanel) dom.moeOffloadPanel.style.display = '';
    if (dom.moeOffloadSlider) {
      dom.moeOffloadSlider.max = arch.nExperts;
      dom.moeOffloadSlider.value = nCpuMoe;
    }
    updateMoeSliderVisuals();
  } else {
    if (dom.moeOffloadPanel) dom.moeOffloadPanel.style.display = 'none';
  }

  // Render scenario cards
  renderScenarioCards(modelBytes, arch, availVram);

  // Legacy VRAM pill (backward compat)
  if (dom.vramPill || dom.vramEstimateText) {
    updateLegacyVramPill(total, availVram);
  }
}

function setSegWidth(el, frac) {
  if (!el) return;
  const pct = Math.max(0, Math.min(1, frac)) * 100;
  el.style.width = pct.toFixed(2) + '%';
  el.style.display = pct < 0.3 ? 'none' : '';
}

function updateMoeSliderVisuals() {
  const arch = getEffectiveArch();
  const n = arch.nExperts || 0;
  if (!n) return;
  const cpu = wizardState.hardware.nCpuMoe || 0;
  const gpu = n - cpu;
  const pct = cpu / n * 100;

  if (dom.moeOffloadSlider) {
    dom.moeOffloadSlider.style.background = `linear-gradient(90deg, #7c3aed ${pct.toFixed(1)}%, rgba(255,255,255,0.1) ${pct.toFixed(1)}%)`;
  }
  if (dom.moeOffloadSubtitle) {
    dom.moeOffloadSubtitle.textContent = `${cpu} of ${n} experts on CPU · ${gpu} in VRAM`;
  }
  if (dom.moeOffloadHint) {
    if (cpu === 0) {
      dom.moeOffloadHint.textContent = 'All experts in VRAM — fastest generation.';
    } else if (cpu >= n) {
      dom.moeOffloadHint.textContent = 'All experts on CPU — slowest generation. Only use if VRAM is very tight.';
    } else {
      const speedPenalty = Math.round((cpu / n) * 60);
      dom.moeOffloadHint.textContent = `~${speedPenalty}% generation speed reduction. More context available.`;
    }
  }
}

// ── Scenario cards ────────────────────────────────────────────────────────────

function renderScenarioCards(modelBytes, arch, availVram) {
  if (!dom.vramScenarios || !availVram || !modelBytes) return;

  const hw = wizardState.hardware;
  const fitGran = hw.fitCtx || 1024;
  const slots = hw.parallelSlots || 1;
  const ubatch = hw.ubatchSize || 512;
  const nCpuMoe = hw.nCpuMoe || 0;
  const uc = wizardState.useCase;

  const scenarios = [
    { label: 'Max coherence', kk: 'q8_0', kv: 'q8_0', note: 'q8_0 KV — min for agentic', rec: uc !== 'roleplay' },
    { label: 'Max context',   kk: 'q4_0', kv: 'q4_0', note: 'q4_0 KV — roleplay OK, agentic ⚠', rec: uc === 'roleplay', warnAgentic: uc === 'agentic' },
    { label: 'Reference',     kk: 'f16',  kv: 'f16',  note: 'f16 KV — full precision', rec: false },
  ];

  dom.vramScenarios.innerHTML = '';

  for (const s of scenarios) {
    const ctx = maxContext(modelBytes, arch, s.kk, s.kv, slots, ubatch, nCpuMoe, availVram, fitGran, 0.05);
    const card = document.createElement('div');
    card.className = 'vram-scenario-card' + (s.rec ? ' scenario-rec' : '');
    card.setAttribute('tabindex', '0');
    card.setAttribute('role', 'button');
    card.setAttribute('aria-label', `${s.label}: ${formatCtx(ctx)} tokens`);

    const selectable = ctx > 0;

    // All values are internal constants — no user input reaches this template.
    // eslint-disable-next-line no-unsanitized/property
    card.innerHTML = `
      <div class="vsc-label">${s.label}</div>
      <div class="vsc-ctx">${selectable ? formatCtx(ctx) : '—'}</div>
      <div class="vsc-kv">${s.kk.toUpperCase()} KV</div>
      ${s.rec ? '<span class="vsc-rec-badge">★ Recommended</span>' : ''}
      ${s.warnAgentic ? '<span class="vsc-warn">⚠ Not for agents</span>' : ''}
    `;

    if (selectable) {
      const applyScenario = () => {
        // Apply this KV quant and update context
        wizardState.hardware.cacheTypeK = s.kk;
        wizardState.hardware.cacheTypeV = s.kv;
        wizardState.hardware.contextSize = ctx;

        // Sync to form fields
        if (dom.cacheTypeKSelect) dom.cacheTypeKSelect.value = s.kk;
        if (dom.cacheTypeVSelect) dom.cacheTypeVSelect.value = s.kv;
        if (dom.contextSizeInput) dom.contextSizeInput.value = ctx;

        // Animate the context value
        card.querySelector('.vsc-ctx')?.classList.add('counting');
        setTimeout(() => card.querySelector('.vsc-ctx')?.classList.remove('counting'), 300);

        // Update selected state
        dom.vramScenarios.querySelectorAll('.vram-scenario-card').forEach(c => c.classList.remove('selected'));
        card.classList.add('selected');

        updateVramDisplay();
      };
      card.addEventListener('click', applyScenario);
      card.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); applyScenario(); } });
    }

    dom.vramScenarios.appendChild(card);
  }
}

function updateLegacyVramPill(total, avail) {
  if (dom.vramEstimateText) {
    dom.vramEstimateText.textContent = avail > 0
      ? `${formatGB(total)} / ${formatGB(avail)}`
      : formatGB(total);
  }
  if (!dom.vramPill) return;
  const ratio = avail > 0 ? total / avail : 0;
  const map = [
    [0.82, 'fit', 'Fits'],
    [1.00, 'tight', 'Tight'],
    [1.20, 'risk', 'At risk'],
    [Infinity, 'wont-fit', "Won't fit"],
  ];
  let cls = '', lbl = '';
  for (const [thr, c, l] of map) { if (ratio <= thr) { cls = c; lbl = l; break; } }
  dom.vramPill.className = cls ? `vram-pill-${cls}` : '';
  dom.vramPill.textContent = lbl;
}

// ── Auto-size (server-side recommendation) ────────────────────────────────────

async function triggerAutoSize() {
  if (!dom.vramAutosizeBtn) return;
  const btn = dom.vramAutosizeBtn;
  const origText = btn.textContent;
  btn.disabled = true; btn.textContent = 'Sizing…';
  if (dom.vramAutosizeNote) dom.vramAutosizeNote.textContent = '';

  try {
    const availVram = cachedVram || wizardState.vram.available;
    const modelBytes = getModelBytes();
    const arch = getEffectiveArch();

    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };

    const body = {
      model_path: wizardState.model.path || undefined,
      model_size_bytes: modelBytes || undefined,
      param_b: wizardState.model.paramB || undefined,
      model_name: wizardState.model.path || wizardState.model.hfRepo || '',
      available_vram_bytes: availVram,
      use_case: wizardState.useCase,
      parallel_slots: wizardState.hardware.parallelSlots,
      fit_granularity: wizardState.hardware.fitCtx || 1024,
      quant: guessQuantFromName(wizardState.model.hfFile || wizardState.model.path || ''),
      n_layers:    arch.nLayers    || undefined,
      n_kv_heads:  arch.nKvHeads   || undefined,
      head_dim:    arch.headDim    || undefined,
      n_experts:   arch.nExperts   || undefined,
      mtp_depth:   arch.mtpDepth   || undefined,
      mmproj_bytes: arch.mmprojBytes || undefined,
    };

    const resp = await fetch('/api/vram/auto-size', { method: 'POST', headers, body: JSON.stringify(body) });
    if (!resp.ok) { showToast('Auto-size failed', 'error'); return; }
    const data = await resp.json();
    if (!data.ok || !data.result) { showToast('Auto-size: no result', 'warning'); return; }

    const r = data.result;

    // Apply recommended settings
    wizardState.hardware.contextSize = r.context_size;
    wizardState.hardware.cacheTypeK  = r.kv_quant_k;
    wizardState.hardware.cacheTypeV  = r.kv_quant_v;
    wizardState.hardware.ubatchSize  = r.ubatch_size;
    wizardState.hardware.fitCtx      = r.fit_ctx;
    if (r.n_cpu_moe != null) wizardState.hardware.nCpuMoe = r.n_cpu_moe;

    // Sync form fields
    if (dom.contextSizeInput) dom.contextSizeInput.value = r.context_size;
    if (dom.cacheTypeKSelect) dom.cacheTypeKSelect.value  = r.kv_quant_k;
    if (dom.cacheTypeVSelect) dom.cacheTypeVSelect.value  = r.kv_quant_v;
    if (dom.ubatchSizeInput)  dom.ubatchSizeInput.value   = r.ubatch_size;
    if (dom.fitCtxInput)      dom.fitCtxInput.value       = r.fit_ctx;
    if (r.n_cpu_moe != null && dom.nCpuMoeInput) dom.nCpuMoeInput.value = r.n_cpu_moe;
    if (r.n_cpu_moe != null && dom.moeOffloadSlider) dom.moeOffloadSlider.value = r.n_cpu_moe;

    const note = `Set: ${formatCtx(r.context_size)} ctx · ${r.kv_quant_k.toUpperCase()} KV · ubatch ${r.ubatch_size}`;
    if (dom.vramAutosizeNote) dom.vramAutosizeNote.textContent = note;

    updateVramDisplay();

    if (r.warnings?.length) showToast('Auto-size warnings', 'warning', r.warnings[0]);
    else showToast('Auto-sized', 'success', note);
  } catch (err) {
    showToast('Auto-size error: ' + (err.message || String(err)), 'error');
  } finally {
    btn.disabled = false; btn.textContent = origText;
  }
}

// ── Summary (Step 4) ──────────────────────────────────────────────────────────

function renderSummary() {
  if (!dom.summaryList) return;
  dom.summaryList.innerHTML = '';

  const m = wizardState.model, hw = wizardState.hardware;
  const arch = getEffectiveArch();
  const availVram = cachedVram || wizardState.vram.available;
  const modelBytes = getModelBytes();

  const modelDisplay = m.source === 'hf'
    ? (m.hfFile ? `${m.hfRepo} / ${m.hfFile.split('/').pop()}` : m.hfRepo || '(none)')
    : (m.path ? m.path.split(/[\\/]/).pop() || m.path : '(none)');

  const ctxK = hw.cacheTypeK || 'q8_0', ctxV = hw.cacheTypeV || 'q8_0';
  const kvSize = modelBytes > 0 ? kvBytes(arch, hw.contextSize, hw.parallelSlots, ctxK, ctxV) : 0;

  const rows = [
    { label: 'Use case',      value: { agentic: 'Agentic / RAG', general: 'General chat', roleplay: 'Roleplay / creative' }[wizardState.useCase] || wizardState.useCase },
    { label: 'Profile',       value: wizardState.profile },
    { label: 'Model',         value: modelDisplay },
    { label: 'Context size',  value: `${hw.contextSize.toLocaleString()} tokens` },
    { label: 'GPU layers',    value: hw.gpuLayers === 'manual' ? String(hw.gpuLayersManual ?? '—') : hw.gpuLayers },
    { label: 'KV quant (K/V)', value: `${ctxK.toUpperCase()} / ${ctxV.toUpperCase()}` },
    { label: 'KV cache',      value: kvSize > 0 ? formatGB(kvSize) : '—' },
    { label: 'Batch / ubatch', value: `${hw.batchSize} / ${hw.ubatchSize}` },
    { label: '--fit-ctx',     value: String(hw.fitCtx || 1024) },
  ];
  if (hw.nCpuMoe > 0 && arch.nExperts > 0) rows.push({ label: 'MoE CPU offload', value: `${hw.nCpuMoe} of ${arch.nExperts} experts` });
  if (hw.tensorSplit) rows.push({ label: 'Tensor split', value: hw.tensorSplit });
  if (arch.mmprojBytes > 0) rows.push({ label: 'mmproj', value: formatGB(arch.mmprojBytes) });
  if (arch.mtpDepth > 0) rows.push({ label: 'MTP depth', value: String(arch.mtpDepth) });

  const specType = dom.specTypeSelect?.value || '';
  if (specType) {
    let sv = { 'ngram-mod': 'N-gram (fast)', 'draft-model': 'Draft model' }[specType] || specType;
    if (specType === 'draft-model' && dom.draftModelInput?.value) sv += ` (${dom.draftModelInput.value.split(/[\\/]/).pop()})`;
    rows.push({ label: 'Speculative', value: sv });
  }
  if (hw.kvUnified) rows.push({ label: 'KV unified', value: 'Yes' });
  if (hw.ignoreEos) rows.push({ label: 'Ignore EOS', value: 'Yes' });

  rows.forEach(r => {
    const row = document.createElement('div');
    row.className = 'summary-row';
    const lbl = document.createElement('span'); lbl.className = 'summary-label'; lbl.textContent = r.label;
    const val = document.createElement('span'); val.className = 'summary-value'; val.textContent = r.value;
    row.appendChild(lbl); row.appendChild(val);
    dom.summaryList.appendChild(row);
  });

  // Warnings
  if (dom.summaryWarnings) {
    const warns = [];
    if (!modelDisplay || modelDisplay === '(none)') warns.push('No model selected.');
    const ratio = availVram > 0 && modelBytes > 0 ? (modelBytes + kvSize) / availVram : 0;
    if (ratio > 1.2) warns.push("Configuration likely exceeds VRAM. Reduce context size or KV quant.");
    else if (ratio > 1.0) warns.push("VRAM is at risk. Consider reducing context or using KV quantization.");
    else if (ratio > 0.88) warns.push("VRAM is tight. Monitor for OOM errors.");
    if (wizardState.useCase === 'agentic' && kvBpe(ctxK) < 1.0) warns.push("⚠ q4_0 KV not recommended for agentic workflows — reduces tool-call coherence.");
    if (warns.length) {
      dom.summaryWarnings.style.display = '';
      dom.summaryWarnings.innerHTML = '';
      warns.forEach(w => { const p = document.createElement('div'); p.textContent = w; dom.summaryWarnings.appendChild(p); });
    } else {
      dom.summaryWarnings.style.display = 'none';
    }
  }
  if (dom.healthCheckBtn) dom.healthCheckBtn.style.display = '';
}

// ── Save as preset ────────────────────────────────────────────────────────────

async function saveAsPreset() {
  const name = window.prompt('Preset name:', 'Spawn Wizard Preset');
  if (!name) return;
  const payload = buildPresetPayload(); payload.name = name;
  try {
    const headers = window.authHeaders ? { ...window.authHeaders(), 'Content-Type': 'application/json' } : { 'Content-Type': 'application/json' };
    const resp = await fetch('/api/presets', { method: 'POST', headers, body: JSON.stringify(payload) });
    if (!resp.ok) { showToast('Save preset failed: ' + await resp.text().catch(()=>''), 'error'); return; }
    showToast(`Preset "${name}" saved`, 'success');
  } catch (err) { showToast('Save preset failed: ' + (err.message || String(err)), 'error'); }
}

function buildPresetPayload() {
  const h = wizardState.hardware, m = wizardState.model;
  const gpuLayers = h.gpuLayers === 'manual' ? (h.gpuLayersManual ?? -1) : (h.gpuLayers === 'all' ? -1 : null);
  return {
    name: 'Spawn Wizard Preset',
    model_path: m.source !== 'hf' ? (m.path || '') : '',
    hf_repo: m.source === 'hf' ? (m.hfRepo || null) : null,
    gpu_layers: gpuLayers,
    context_size: h.contextSize,
    batch_size: h.batchSize,
    ubatch_size: h.ubatchSize,
    parallel_slots: h.parallelSlots,
    ctk: h.cacheTypeK || '',
    ctv: h.cacheTypeV || '',
    n_cpu_moe: h.nCpuMoe || null,
    tensor_split: h.tensorSplit || '',
    spec_type: dom.specTypeSelect?.value || '',
    draft_model: (dom.draftModelInput?.value || '').trim() || '',
    kv_unified: h.kvUnified || false,
    ignore_eos: h.ignoreEos || false,
  };
}

// ── Health check ──────────────────────────────────────────────────────────────

async function runHealthCheck() {
  if (!dom.healthCheckBtn) return;
  const btn = dom.healthCheckBtn, orig = btn.textContent;
  btn.disabled = true; btn.textContent = 'Running…';
  try {
    const headers = window.authHeaders ? { ...window.authHeaders(), 'Content-Type': 'application/json' } : { 'Content-Type': 'application/json' };
    const resp = await fetch('/api/benchmark', { method: 'POST', headers });
    if (!resp.ok) { showToast('Health check failed: ' + await resp.text().catch(()=>''), 'error'); return; }
    const data = await resp.json();
    const verdict = (data.verdict || '').toLowerCase();
    const details = [
      data.prompt_tokens_per_second ? `Prompt: ${data.prompt_tokens_per_second.toFixed(1)} t/s` : '',
      data.gen_tokens_per_second ? `Gen: ${data.gen_tokens_per_second.toFixed(1)} t/s` : '',
      data.time_to_first_token_ms ? `TTFT: ${data.time_to_first_token_ms.toFixed(0)} ms` : '',
    ].filter(Boolean).join(' · ');
    showToast(`Health: ${verdict || 'complete'}`, verdict === 'good' ? 'success' : verdict === 'poor' ? 'error' : 'warning', details || (data.hints?.[0] ?? ''));
  } catch (err) { showToast('Health check error', 'error', err.message || String(err)); }
  finally { btn.disabled = false; btn.textContent = orig; }
}

// ── Spawn server ──────────────────────────────────────────────────────────────

async function spawnServer() {
  if (wizardState.spawn.inFlight) return;
  wizardState.spawn.inFlight = true; wizardState.spawn.error = '';
  if (!dom.spawnServerBtn) return;
  dom.spawnServerBtn.disabled = true;
  setStatusText('Preparing configuration…'); setProgress(10); clearStatusMessages();
  try {
    const payload = buildSpawnPayload();
    setStatusText('Starting llama-server…'); setProgress(30);
    const headers = window.authHeaders ? { ...window.authHeaders(), 'Content-Type': 'application/json' } : { 'Content-Type': 'application/json' };
    let resp;
    try { resp = await fetch('/api/sessions/spawn', { method: 'POST', headers, body: JSON.stringify(payload) }); }
    catch { resp = await fetch('/api/start', { method: 'POST', headers, body: JSON.stringify(payload) }); }
    setProgress(60);
    if (!resp.ok) { const t = await resp.text().catch(()=>'Unknown error'); throw new Error(t || `HTTP ${resp.status}`); }
    setProgress(90); setStatusText('Server is starting up…');
    await new Promise(r => setTimeout(r, 1500));
    setProgress(100); setStatusText('Server started.');
    showSuccessText('Server is running.'); showToast('Server started', 'success');
    setTimeout(() => closeSpawnWizard(), 1200);
  } catch (err) {
    const msg = (err.message || String(err)).split('\n')[0].trim();
    showErrorText(msg || 'Failed to start server.'); setStatusText('Spawn failed.'); wizardState.spawn.error = msg;
    showToast('Spawn failed', 'error', msg || 'Check logs.');
  } finally {
    wizardState.spawn.inFlight = false;
    if (dom.spawnServerBtn) dom.spawnServerBtn.disabled = false;
  }
}

function buildSpawnPayload() {
  const h = wizardState.hardware, m = wizardState.model;
  const gpuLayers = h.gpuLayers === 'manual' ? (h.gpuLayersManual ?? -1) : (h.gpuLayers === 'all' ? -1 : null);
  return {
    model_path: m.source !== 'hf' ? (m.path || null) : null,
    hf_repo: m.source === 'hf' ? (m.hfRepo || null) : null,
    hf_file: m.source === 'hf' ? (m.hfFile || null) : null,
    gpu_layers: gpuLayers,
    context_size: h.contextSize,
    batch_size: h.batchSize,
    ubatch_size: h.ubatchSize,
    parallel_slots: h.parallelSlots,
    ctk: h.cacheTypeK || null,
    ctv: h.cacheTypeV || null,
    n_cpu_moe: h.nCpuMoe || null,
    tensor_split: h.tensorSplit || null,
    spec_type: dom.specTypeSelect?.value || '',
    draft_model: (dom.draftModelInput?.value || '').trim() || null,
    kv_unified: h.kvUnified || null,
    ignore_eos: h.ignoreEos || null,
    fit: h.fitCtx ? 'on' : null,
    fit_ctx: h.fitCtx || null,
    profile: wizardState.profile,
    use_case: wizardState.useCase,
  };
}

// ── Status helpers ────────────────────────────────────────────────────────────

function setStatusText(t) { if (dom.statusText) dom.statusText.textContent = t; }
function setProgress(p) { if (dom.progressFill) dom.progressFill.style.width = Math.min(100, Math.max(0, p)) + '%'; }
function showErrorText(t) { if (dom.errorText) dom.errorText.textContent = t || ''; }
function showSuccessText(t) { if (dom.successText) dom.successText.textContent = t || ''; }
function clearStatusMessages() { if (dom.errorText) dom.errorText.textContent = ''; if (dom.successText) dom.successText.textContent = ''; }
function resetSpawnStatus() { wizardState.spawn = { inFlight:false, error:'' }; setStatusText('Ready to spawn.'); setProgress(0); clearStatusMessages(); }
