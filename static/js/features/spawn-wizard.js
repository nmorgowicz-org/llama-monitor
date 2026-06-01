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
import { switchView } from './setup-view.js';
import { setTuneConfig, showTunePanel } from './tune-panel.js';

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
function formatParams(paramB) {
  if (paramB >= 1000) return `${(paramB / 1000).toFixed(1)}T`;
  if (paramB % 1 === 0) return `${paramB}B`;
  return `${Number(paramB).toFixed(1)}B`;
}
function formatGB(bytes) {
  if (!bytes) return '0 GB';
  return (bytes / 1e9).toFixed(1) + ' GB';
}
// Use binary GiB (1024³) for system VRAM totals so "64 GiB" shows as "64 GB",
// matching Apple's marketing convention (GiB labeled as GB).
function formatVramTotal(bytes) {
  if (!bytes) return '0 GB';
  return (bytes / (1024 ** 3)).toFixed(1) + ' GB';
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

// Exposed for testing/screenshot scripts; internal state is mutable.
export const wizardState = {
  currentStep: 0,
  profile: 'balanced',
  useCase: 'general',    // 'agentic' | 'general' | 'roleplay'
  mode: 'guided',
  model: {
    source: 'local',     // 'local' | 'hf' | 'import'
    path: '',
    hfRepo: '',
    hfFile: '',
    hfTokenSet: false,
    delivery: 'local_file', // 'local_file' | 'imported_local' | 'stream_hf' | 'downloaded_hf'
    originRepo: '',
    originFile: '',
    localMeta: null,
    paramB: 0,           // estimated parameter count (from HF metadata if available)
    modelBytes: 0,       // file size in bytes once known
    nCtxTrain: 0,        // training context length from GGUF metadata (0 = unknown)
    quantFiles: [],      // GGUF files from HF repo for hardware-step quant swap
    mmprojFiles: [],     // mmproj files found in HF repo
    chatTemplatePath: null,  // local path to installed .jinja template (null = use embedded)
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
    // MTP
    mtpEnabled: true,
    mtpDraftNMax: 2,
    // Sampling (null = use llama-server default)
    temperature: null,
    topP: null,
    topK: null,
    minP: null,
    repeatPenalty: null,
    seed: null,
  },
  access: {
    port: 8001,
    bindHost: '127.0.0.1',
    apiKey: '',
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
  bindWizardHfToken();
  restoreProfile();
  applyProfileVisibility();
  renderHfDiscoverPills();          // static — no network call needed
  loadHfQuickPicks();               // pre-load author quick-picks in background
  loadCommunityPicks();             // load community-picks.json if present

  document.getElementById('btn-open-spawn-wizard')
    ?.addEventListener('click', () => openSpawnWizard());

  // HF download panel buttons
  document.getElementById('hf-dlp-download-btn')?.addEventListener('click', _startHfDownload);
  document.getElementById('hf-dlp-use-hf-btn')?.addEventListener('click', () => {
    // Keep HF source, hide panel, let user continue with Next
    hideHfDownloadPanel();
  });
  document.getElementById('hf-dlp-cancel-btn')?.addEventListener('click', _cancelHfDownload);
  document.getElementById('hf-dlp-open-settings')?.addEventListener('click', () => {
    window.openSettingsModal?.();
    setTimeout(() => {
      document.querySelector('.settings-tab[data-tab="models"]')?.click();
      document.getElementById('settings-hf-token')?.focus();
    }, 80);
  });

  // Refresh download destination when settings change (e.g., models dir updated)
  window.addEventListener('settings-applied', () => {
    refreshHfTokenState();
    const panel = document.getElementById('hf-download-panel');
    if (panel && panel.style.display !== 'none') {
      const fname = (wizardState.model?.hfFile || '').split('/').pop();
      if (fname) showHfDownloadPanel(fname);
    }
  });
}

function applyReducedMotion() {
  if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
    document.documentElement.classList.add('reduce-motion');
  }
}

export function openSpawnWizard(opts = {}) {
  if (!dom.overlay) return;
  dom.overlay.classList.add('open');
  refreshHfTokenState();

  // Check binary prereq every time wizard opens
  _checkBinaryPrereq();

  if (opts.localPath) {
    // Pre-load a local model path and jump straight to step 2 (model).
    wizardState.model.source = 'local';
    wizardState.model.path = opts.localPath;
    wizardState.model.delivery = 'local_file';
    wizardState.model.localMeta = opts.localModel || null;
    if (dom.modelPathInput) dom.modelPathInput.value = opts.localPath;
    // Select the "local" source card visually.
    dom.modelSourceCards?.forEach(c => {
      c.classList.toggle('selected', c.dataset.source === 'local');
    });
    updateModelInputVisibility();
    renderLocalModelHint();
    showStep(1); // step 1 = Model (0-indexed)
  } else {
    updateModelInputVisibility();
    renderLocalModelHint();
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
  dom.localModelHint   = document.getElementById('spawn-local-model-hint');
  dom.localModelHintTitle = document.getElementById('spawn-local-model-hint-title');
  dom.localModelHintMeta  = document.getElementById('spawn-local-model-hint-meta');
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
  dom.portInput        = document.getElementById('spawn-port');
  dom.bindHostSelect   = document.getElementById('spawn-bind-host');
  dom.apiKeyInput      = document.getElementById('spawn-api-key');

  // Step 5
  dom.spawnServerBtn = document.getElementById('spawn-server-btn');
  dom.statusText     = document.getElementById('spawn-status-text');
  dom.progressFill   = document.getElementById('spawn-progress-fill');
  dom.errorText      = document.getElementById('spawn-error-text');
  dom.successText    = document.getElementById('spawn-success-text');

  // Binary prereq banner
  dom.binaryPrereq        = document.getElementById('wizard-binary-prereq');
  dom.prereqIdle          = document.getElementById('wizard-prereq-idle');
  dom.prereqProgress      = document.getElementById('wizard-prereq-progress');
  dom.prereqSuccess       = document.getElementById('wizard-prereq-success');
  dom.prereqDownloadBtn   = document.getElementById('wizard-prereq-download-btn');
  dom.prereqSettingsBtn   = document.getElementById('wizard-prereq-settings-btn');
  dom.prereqPathRow       = document.getElementById('wizard-prereq-path-row');
  dom.prereqPath          = document.getElementById('wizard-prereq-path');
  dom.prereqBar           = document.getElementById('wizard-prereq-bar');
  dom.prereqElapsed       = document.getElementById('wizard-prereq-elapsed');
  dom.prereqSuccessText   = document.getElementById('wizard-prereq-success-text');

  // Model card panel
  dom.cardPanel           = document.getElementById('wizard-card-panel');
  dom.cardPanelTitle      = document.getElementById('wizard-card-panel-title');
  dom.cardPanelHfLink     = document.getElementById('wizard-card-panel-hf-link');
  dom.cardPanelClose      = document.getElementById('wizard-card-panel-close');
  dom.cardLoading         = document.getElementById('wizard-card-loading');
  dom.cardError           = document.getElementById('wizard-card-error');
  dom.cardFrontmatter     = document.getElementById('wizard-card-frontmatter');
  dom.cardFrontmatterPre  = document.getElementById('wizard-card-frontmatter-content');
  dom.cardContent         = document.getElementById('wizard-card-content');
}

// ── Events ────────────────────────────────────────────────────────────────────

function bindEvents() {
  dom.closeBtn?.addEventListener('click', closeSpawnWizard);
  dom.overlay?.addEventListener('click', e => { if (e.target === dom.overlay) closeSpawnWizard(); });
  bindCtxQuickPicks();
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
      if (card.dataset.source === 'local' && !wizardState.model.delivery) wizardState.model.delivery = 'local_file';
      if (card.dataset.source === 'import') wizardState.model.delivery = 'imported_local';
      if (card.dataset.source === 'hf') wizardState.model.delivery = 'stream_hf';
      dom.modelSourceCards.forEach(c => c.classList.remove('selected'));
      card.classList.add('selected');
      if (card.dataset.source !== 'hf') hideHfDownloadPanel();
      updateModelInputVisibility();
      renderLocalModelHint();
      clearValidationError();
      if (card.dataset.source === 'import') loadThirdPartyModels();
    });
    card.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); card.click(); } });
  });

  dom.browseModelBtn?.addEventListener('click', async () => {
    let defaultPath = dom.modelPathInput?.value.trim() || '';
    if (!defaultPath) {
      // Fetch the effective models directory so Browse opens there by default.
      try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const r = await fetch('/api/hf/download-dir', { headers });
        if (r.ok) {
          const d = await r.json();
          defaultPath = d.dir || '';
        }
      } catch { /* ignore — fall back to home */ }
    } else {
      // Strip the filename to get the parent directory.
      const sep = defaultPath.includes('\\') ? '\\' : '/';
      const parts = defaultPath.split(sep);
      parts.pop();
      defaultPath = parts.join(sep) || (defaultPath.includes('\\') ? 'C:\\' : '/');
    }
    openDeferredFileBrowser('spawn-model-path', 'gguf', defaultPath);
  });
  dom.importBrowseBtn?.addEventListener('click', () => openDeferredFileBrowser('spawn-import-path', 'gguf'));

  dom.modelPathInput?.addEventListener('input', () => {
    wizardState.model.path = dom.modelPathInput.value.trim();
    wizardState.model.source = 'local';
    wizardState.model.delivery = 'local_file';
    if (wizardState.model.localMeta?.path && wizardState.model.localMeta.path !== wizardState.model.path) {
      wizardState.model.localMeta = null;
    }
    onModelPathChanged();
    renderLocalModelHint();
  });

  dom.importPathInput?.addEventListener('input', () => {
    wizardState.model.path = dom.importPathInput.value.trim();
    wizardState.model.source = 'import';
    wizardState.model.delivery = 'imported_local';
    wizardState.model.localMeta = null;
    onModelPathChanged();
    renderLocalModelHint();
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
    const v = dom.specTypeSelect.value;
    if (dom.draftModelWrap) dom.draftModelWrap.style.display = v === 'draft-model' ? '' : 'none';
    _updateSpecHint(v);
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

  // Sampling fields in review step
  _bindSamplingFields();
  dom.portInput?.addEventListener('input', () => {
    const parsed = parseInt(dom.portInput.value, 10);
    wizardState.access.port = Number.isFinite(parsed) && parsed > 0 ? parsed : 8001;
  });
  dom.bindHostSelect?.addEventListener('change', () => {
    wizardState.access.bindHost = dom.bindHostSelect.value || '127.0.0.1';
  });
  dom.apiKeyInput?.addEventListener('input', () => {
    wizardState.access.apiKey = (dom.apiKeyInput.value || '').trim();
  });

  // Hardware step quant swap
  document.getElementById('hw-quant-select')?.addEventListener('change', e => {
    const fpath = e.target.value;
    const qf = wizardState.model.quantFiles?.find(q => (q.path || q.name) === fpath);
    if (qf) {
      wizardState.model.hfFile = fpath;
      if (qf.size) wizardState.model.modelBytes = Number(qf.size);
      if (detectMtpFromName(fpath) && !wizardState.arch.mtpDepth) {
        wizardState.arch.mtpDepth = 1;
        renderMtpSection();
      }
      scheduleVramUpdate();
      // Refresh the download panel if it's already visible
      const panel = document.getElementById('hf-download-panel');
      if (panel && panel.style.display !== 'none') showHfDownloadPanel(fpath);
    }
  });

  // Model card panel
  dom.cardPanelClose?.addEventListener('click', _closeCardPanel);

  // Binary prereq buttons
  dom.prereqDownloadBtn?.addEventListener('click', _downloadBinaryForWizard);
  dom.prereqSettingsBtn?.addEventListener('click', () => {
    window.openSettingsModal?.();
    setTimeout(() => {
      document.querySelector('.settings-tab[data-tab="session"]')?.click();
      document.getElementById('set-server-path')?.focus();
    }, 80);
  });
}

async function refreshHfTokenState() {
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const res = await fetch('/api/hf/token', { headers });
    if (!res.ok) return;
    const data = await res.json();
    wizardState.model.hfTokenSet = !!data.set;
    _updateWizardHfTokenUI(!!data.set);
  } catch {}
}

function _updateWizardHfTokenUI(isSet) {
  const badge      = document.getElementById('wizard-hf-token-badge');
  const inputRow   = document.getElementById('wizard-hf-token-input-row');
  const savedRow   = document.getElementById('wizard-hf-token-saved-row');
  if (badge) {
    badge.textContent = isSet ? '✓ Active' : 'Not set';
    badge.className = 'wizard-hf-token-badge ' + (isSet ? 'token-badge-ok' : 'token-badge-none');
    badge.style.display = '';
  }
  if (inputRow) inputRow.style.display = isSet ? 'none' : '';
  if (savedRow) savedRow.style.display = isSet ? ''     : 'none';
}

function bindWizardHfToken() {
  const saveBtn   = document.getElementById('wizard-hf-token-save');
  const removeBtn = document.getElementById('wizard-hf-token-remove');
  const input     = document.getElementById('wizard-hf-token-input');

  saveBtn?.addEventListener('click', async () => {
    const token = input?.value.trim() || '';
    if (!token) { input?.focus(); return; }
    const origText = saveBtn.textContent;
    saveBtn.disabled = true; saveBtn.textContent = 'Saving…';
    try {
      const headers = window.authHeaders
        ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
        : { 'Content-Type': 'application/json' };
      const res = await fetch('/api/hf/token', { method: 'PUT', headers, body: JSON.stringify({ token }) });
      const data = await res.json().catch(() => ({}));
      if (data.ok) {
        if (input) input.value = '';
        saveBtn.textContent = '✓ Saved';
        setTimeout(() => { saveBtn.textContent = origText; saveBtn.disabled = false; }, 1500);
        await refreshHfTokenState();
      } else {
        saveBtn.textContent = 'Failed'; setTimeout(() => { saveBtn.textContent = origText; saveBtn.disabled = false; }, 2000);
      }
    } catch {
      saveBtn.textContent = 'Error'; setTimeout(() => { saveBtn.textContent = origText; saveBtn.disabled = false; }, 2000);
    }
  });

  // Allow Enter key in token input to trigger save
  input?.addEventListener('keydown', e => { if (e.key === 'Enter') { e.preventDefault(); saveBtn?.click(); } });

  removeBtn?.addEventListener('click', async () => {
    try {
      const headers = window.authHeaders ? window.authHeaders() : {};
      await fetch('/api/hf/token', { method: 'DELETE', headers });
      await refreshHfTokenState();
    } catch {}
  });
}

function renderLocalModelHint() {
  if (!dom.localModelHint) return;
  const meta = wizardState.model.localMeta;
  const isLocalSource = wizardState.model.source === 'local' || wizardState.model.source === 'import';
  if (!isLocalSource || !meta) {
    dom.localModelHint.style.display = 'none';
    return;
  }
  dom.localModelHint.style.display = '';
  if (dom.localModelHintTitle) {
    dom.localModelHintTitle.textContent = meta.model_name || meta.name || meta.filename || (meta.path?.split(/[\\/]/).pop() || 'Selected model');
  }
  if (dom.localModelHintMeta) {
    const parts = [];
    if (meta.size_display) parts.push(meta.size_display);
    if (meta.quant_type) parts.push(meta.quant_type);
    if (meta.param_b != null) parts.push(formatParams(meta.param_b));
    if (meta.vram_est_gb != null) parts.push(`~${Number(meta.vram_est_gb).toFixed(0)} GB weights`);
    dom.localModelHintMeta.textContent = parts.join(' · ') || 'Opened from your local model library.';
  }
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

// ── HF download panel ────────────────────────────────────────────────────────

let _dlPollTimer = null;
let _dlCurrentId = null;

function _dlPanel(id) { return document.getElementById(id); }

async function showHfDownloadPanel(fname) {
  const panel = _dlPanel('hf-download-panel');
  if (!panel) return;
  // Reset to idle state
  _dlSetState('idle');
  panel.style.display = '';

  // Fetch effective models dir and show destination
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const res = await fetch('/api/hf/download-dir', { headers });
    const data = res.ok ? await res.json() : null;
    const dir = data?.dir || '~/.config/llama-monitor/models';
    const configured = data?.configured ?? false;
    const destPath = dir.replace(/\/$/, '') + '/' + fname.split('/').pop();
    const destEl = _dlPanel('hf-dlp-dest-path');
    if (destEl) { destEl.textContent = destPath; destEl.title = destPath; }
    const warnEl = _dlPanel('hf-dlp-no-dir-warn');
    if (warnEl) warnEl.style.display = configured ? 'none' : '';
  } catch { /* ignore */ }
}

function hideHfDownloadPanel() {
  const panel = _dlPanel('hf-download-panel');
  if (panel) panel.style.display = 'none';
  _dlCancelPoll();
}

function _dlSetState(state) {
  ['idle','progress','complete'].forEach(s => {
    const el = _dlPanel(`hf-dlp-${s}`);
    if (el) el.style.display = s === state ? '' : 'none';
  });
}

async function _startHfDownload() {
  const { hfRepo, hfFile } = wizardState.model;
  if (!hfRepo || !hfFile) { showValidationError('Select a GGUF file first.'); return; }
  const btn = _dlPanel('hf-dlp-download-btn');
  if (btn) { btn.disabled = true; btn.textContent = 'Starting…'; }
  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const res = await fetch('/api/hf/download', {
      method: 'POST',
      headers,
      body: JSON.stringify({ repo_id: hfRepo, file_path: hfFile, resume: true }),
    });
    const data = await res.json().catch(() => ({}));
    if (btn) { btn.disabled = false; btn.textContent = 'Download to models folder'; }
    if (!res.ok || !data.ok) { showValidationError(data.error || 'Download failed to start.'); return; }
    _dlCurrentId = data.download_id;
    // Show progress state
    const fileEl = _dlPanel('hf-dlp-progress-file');
    if (fileEl) fileEl.textContent = hfFile.split('/').pop();
    _dlSetState('progress');
    _dlPollStatus(data.download_id, data.local_path);
  } catch (err) {
    if (btn) { btn.disabled = false; }
    showValidationError(err.message || 'Download request failed.');
  }
}

function _dlPollStatus(downloadId, localPath) {
  _dlCancelPoll();
  const headers = window.authHeaders ? window.authHeaders() : {};
  async function poll() {
    try {
      const res = await fetch(`/api/models/download/${downloadId}/status`, { headers });
      if (!res.ok) return;
      const data = await res.json();
      const s = data.status;
      if (!s) return;
      const { status, bytes_downloaded = 0, total_bytes = 0, speed = 0, eta = 0 } = s;
      // Update bar + stats
      const pct = total_bytes > 0 ? Math.round(bytes_downloaded / total_bytes * 100) : 0;
      const bar = _dlPanel('hf-dlp-bar');
      if (bar) bar.style.width = pct + '%';
      const pctEl = _dlPanel('hf-dlp-progress-pct');
      if (pctEl) pctEl.textContent = total_bytes > 0 ? `${pct}%` : '';
      const statsEl = _dlPanel('hf-dlp-stats');
      if (statsEl) {
        const mb = (bytes_downloaded / 1_048_576).toFixed(1);
        const tot = total_bytes > 0 ? ` / ${(total_bytes / 1_048_576).toFixed(0)} MB` : '';
        const spd = speed > 0 ? ` · ${(speed / 1_048_576).toFixed(1)} MB/s` : '';
        const etaStr = eta > 0 ? ` · ETA ${eta < 60 ? eta + 's' : Math.round(eta/60) + 'm'}` : '';
        statsEl.textContent = `${mb} MB${tot}${spd}${etaStr}`;
      }

      if (status === 'completed') {
        _dlCancelPoll();
        _dlSetState('complete');
        clearValidationError();
        // Switch wizard source to local with the downloaded path
        const effectivePath = (data.status?.local_path) || localPath;
        if (effectivePath) {
          const downloadedFile = wizardState.model.hfFile || '';
          const downloadedRepo = wizardState.model.hfRepo || '';
          wizardState.model.source = 'local';
          wizardState.model.delivery = 'downloaded_hf';
          wizardState.model.path = effectivePath;
          wizardState.model.originRepo = downloadedRepo;
          wizardState.model.originFile = downloadedFile;
          wizardState.model.localMeta = {
            path: effectivePath,
            filename: effectivePath.split(/[\\/]/).pop() || effectivePath,
            size_display: wizardState.model.modelBytes ? formatBytes(wizardState.model.modelBytes) : '',
            quant_type: guessQuantFromName(downloadedFile || effectivePath),
            param_b: wizardState.model.paramB || null,
          };
          wizardState.model.hfRepo = '';
          wizardState.model.hfFile = '';
          if (dom.modelPathInput) dom.modelPathInput.value = effectivePath;
          dom.modelSourceCards?.forEach(c => {
            c.classList.toggle('selected', c.dataset.source === 'local');
          });
          updateModelInputVisibility();
          updateSelectedModelDisplay();
          renderLocalModelHint();
        }
        return;
      }
      if (status === 'failed') {
        _dlCancelPoll();
        _dlSetState('idle');
        showValidationError(s.message || 'Download failed.');
        return;
      }
      if (status === 'cancelled') {
        _dlCancelPoll();
        _dlSetState('idle');
        return;
      }
    } catch { /* network glitch — keep polling */ }
    _dlPollTimer = setTimeout(poll, 1000);
  }
  _dlPollTimer = setTimeout(poll, 800);
}

function _dlCancelPoll() {
  if (_dlPollTimer) { clearTimeout(_dlPollTimer); _dlPollTimer = null; }
}

async function _cancelHfDownload() {
  if (!_dlCurrentId) return;
  const headers = window.authHeaders ? window.authHeaders() : {};
  await fetch(`/api/models/download/${_dlCurrentId}/cancel`, { method: 'POST', headers }).catch(() => {});
  _dlCancelPoll();
  _dlCurrentId = null;
  _dlSetState('idle');
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

  // Scroll the wizard body to the top whenever changing steps
  const wizardBody = document.querySelector('.wizard-body');
  if (wizardBody) wizardBody.scrollTop = 0;

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
    // Entering hardware step — refresh VRAM, then render model context + new sections
    updateCtxModelMaxHint();
    fetchGpuVram().then(() => {
      scheduleVramUpdate();
      renderHardwareModelHeader();
    });
    renderMmprojSection();
    renderMtpSection();
    _updateSpecHint(dom.specTypeSelect?.value || '');
    // Trigger download panel now (moved from file-select to hardware step entry)
    if (wizardState.model.source === 'hf' && wizardState.model.hfFile) {
      showHfDownloadPanel(wizardState.model.hfFile);
    } else {
      hideHfDownloadPanel();
    }
  }
  if (index === 3) {
    refreshHfTokenState().finally(() => {
      fetchGpuVram().then(() => estimateVramFull().then(() => renderSummary()));
    });
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
  autoInstallChatTemplate();
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

      // Store the model's training context ceiling for UX warnings
    if (m.n_ctx_train) {
      wizardState.model.nCtxTrain = m.n_ctx_train;
      updateCtxTrainWarning();
      updateCtxModelMaxHint();
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
    // /metrics/gpu returns BTreeMap<String, GpuMetrics> (object keyed by GPU name on Mac/Linux)
    // or an array or { gpus: [...] } depending on endpoint version
    let totalVram = 0;
    const gpus = Array.isArray(data) ? data : (data.gpus ? data.gpus : Object.values(data));
    for (const g of gpus) {
      // Rust GpuMetrics struct uses `vram_total` field (value in MB); also check legacy names
      const t = g.vram_total_mb || g.total_mb || g.total_memory_mb || g.vram_total || 0;
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

  const availGb = Math.round(availVram / (1024 ** 3));
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

// ── Chat template auto-install ───────────────────────────────────────────────

// Community template registry keyed by model family
const COMMUNITY_TEMPLATES = {
  qwen: {
    name: 'qwen-fixed',
    display: "froggeric's Fixed Template",
    repo: 'froggeric/Qwen-Fixed-Chat-Templates',
    file: 'chat_template.jinja',
    description: 'Fixes tool calling, KV cache invalidation & agentic loop bugs for Qwen 3.5 / 3.6',
    hfUrl: 'https://huggingface.co/froggeric/Qwen-Fixed-Chat-Templates',
  },
};

function detectModelFamily(name) {
  const lower = (name || '').toLowerCase();
  if (lower.includes('qwen')) return 'qwen';
  if (lower.includes('llama-3') || lower.includes('llama3') || lower.match(/llama.?3/)) return 'llama3';
  if (lower.includes('gemma')) return 'gemma';
  if (lower.includes('mistral') || lower.includes('mixtral')) return 'mistral';
  return null;
}

async function autoInstallChatTemplate() {
  const { source, path, hfRepo } = wizardState.model;
  const identityName = source === 'hf' ? hfRepo : path;
  const family = detectModelFamily(identityName);
  const tpl = family ? COMMUNITY_TEMPLATES[family] : null;

  if (!tpl) {
    wizardState.model.chatTemplatePath = null;
    _renderChatTemplateStatus(family ? 'none-known' : 'no-family', family, null, null);
    return;
  }

  _renderChatTemplateStatus('installing', family, tpl, null);

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const resp = await fetch('/api/chat-template/install-hf', {
      method: 'POST', headers,
      body: JSON.stringify({ repo: tpl.repo, file: tpl.file, name: tpl.name }),
    });
    const data = resp.ok ? await resp.json() : { ok: false, error: `HTTP ${resp.status}` };
    if (data.ok && data.path) {
      wizardState.model.chatTemplatePath = data.path;
      _renderChatTemplateStatus('installed', family, tpl, data);
    } else {
      _renderChatTemplateStatus('error', family, tpl, data);
    }
  } catch (err) {
    _renderChatTemplateStatus('error', family, tpl, { error: err.message || String(err) });
  }
}

function _renderChatTemplateStatus(state, family, tpl, data) {
  const section = document.getElementById('chat-template-section');
  const statusEl = document.getElementById('ct-status');
  const bodyEl = document.getElementById('ct-body');
  if (!section) return;

  if (state === 'no-family') { section.style.display = 'none'; return; }
  section.style.display = '';

  if (state === 'none-known') {
    if (statusEl) { statusEl.textContent = 'Embedded'; statusEl.className = 'ct-status ct-neutral'; }
    if (bodyEl) bodyEl.textContent = `Using template embedded in model file. No community fix available for ${family || 'this'} yet.`;
    return;
  }

  if (state === 'installing') {
    if (statusEl) { statusEl.textContent = 'Downloading…'; statusEl.className = 'ct-status ct-installing'; }
    if (bodyEl) {
      bodyEl.textContent = '';
      const nameEl = document.createElement('span');
      nameEl.className = 'ct-name';
      nameEl.textContent = tpl.display;
      bodyEl.appendChild(nameEl);
      bodyEl.appendChild(document.createTextNode(' — downloading…'));
    }
    return;
  }

  if (state === 'installed') {
    if (statusEl) {
      statusEl.textContent = data?.already_existed ? '✓ Cached' : '✓ Installed';
      statusEl.className = 'ct-status ct-ok';
    }
    if (bodyEl) {
      bodyEl.textContent = '';
      const nameEl = document.createElement('strong');
      nameEl.textContent = tpl.display;
      const descEl = document.createElement('span');
      descEl.textContent = ` — ${tpl.description}`;
      const link = document.createElement('a');
      link.href = tpl.hfUrl; link.target = '_blank'; link.rel = 'noopener noreferrer';
      link.textContent = ' ↗'; link.className = 'ct-hf-link';
      bodyEl.appendChild(nameEl);
      bodyEl.appendChild(descEl);
      bodyEl.appendChild(link);
    }
    return;
  }

  if (state === 'error') {
    if (statusEl) { statusEl.textContent = '⚠ Failed'; statusEl.className = 'ct-status ct-error'; }
    if (bodyEl) {
      bodyEl.textContent = '';
      const msg = document.createElement('span');
      msg.className = 'ct-error-msg';
      msg.textContent = `${data?.error || 'Download failed'} — server will use embedded template.`;
      const retryBtn = document.createElement('button');
      retryBtn.type = 'button'; retryBtn.className = 'ct-retry-btn btn-wizard-tertiary';
      retryBtn.textContent = 'Retry';
      retryBtn.addEventListener('click', autoInstallChatTemplate);
      bodyEl.appendChild(msg);
      bodyEl.appendChild(document.createTextNode(' '));
      bodyEl.appendChild(retryBtn);
    }
  }
}

// ── HF discover categories ────────────────────────────────────────────────────
// Static curated categories that map to queryable HF API searches.

const HF_DISCOVER_CATEGORIES = [
  { id: 'trending',  label: 'Trending',      params: { query: '',           sort: 'trending',  limit: 30 } },
  { id: 'qwen3',     label: 'Qwen3',         params: { query: 'qwen3',      sort: 'downloads', limit: 30 } },
  { id: 'llama3',    label: 'Llama 3.x',     params: { query: 'llama-3',    sort: 'downloads', limit: 30 } },
  { id: 'mistral',   label: 'Mistral / MoE', params: { query: 'mistral',    sort: 'downloads', limit: 30 } },
  { id: 'gemma',     label: 'Gemma',         params: { query: 'gemma',      sort: 'downloads', limit: 30 } },
  { id: 'heretic',   label: 'Heretic',       params: { query: 'heretic',    sort: 'downloads', limit: 30 } },
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
      container.querySelectorAll('.hf-discover-pill').forEach(p => p.classList.remove('active', 'loading'));
      dom.hfQuickpicks?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active', 'loading'));
      pill.classList.add('active', 'loading');
      wizardState.hfBrowseAuthor = null;
      if (dom.hfRepoInput) dom.hfRepoInput.value = '';
      // When the pill's own query is empty, the pill's sort is essential for the
      // backend to return meaningful results (e.g. trending requires sort=trending).
      // Only let the sort select override when a query term anchors the search.
      const sort = cat.params.query
        ? (dom.hfSortSelect?.value || cat.params.sort)
        : cat.params.sort;
      showHfSearchResults({ ...cat.params, sort });
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
        dom.hfQuickpicks?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active', 'loading'));
        document.getElementById('hf-discover-pills')
          ?.querySelectorAll('.hf-discover-pill').forEach(p => p.classList.remove('active', 'loading'));
        btn.classList.add('active', 'loading');
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

  const clearPillLoading = () => {
    dom.hfQuickpicks?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('loading'));
    document.getElementById('hf-discover-pills')
      ?.querySelectorAll('.hf-discover-pill').forEach(p => p.classList.remove('loading'));
  };

  const scrollToResults = () => {
    const scrollParent = container.closest('.wizard-body');
    if (!scrollParent) return;
    const cRect = container.getBoundingClientRect();
    const pRect = scrollParent.getBoundingClientRect();
    scrollParent.scrollTo({ top: scrollParent.scrollTop + cRect.top - pRect.top - 12, behavior: 'smooth' });
  };

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
    if (!resp.ok) { clearPillLoading(); container.innerHTML = '<div class="hf-search-empty">Search failed.</div>'; return; }
    const data = await resp.json();
    const models = data.models || [];

    clearPillLoading();
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

      // Age badge — prefer last_modified, fall back to created_at
      const ageStr = _hfRelativeAge(m.last_modified || m.created_at || '');
      if (ageStr) {
        const age = document.createElement('span');
        age.className = 'hf-sr-age';
        age.textContent = ageStr;
        age.title = m.last_modified || m.created_at || '';
        meta.appendChild(age);
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

      // Model card button — opens in-app panel without selecting the repo
      const cardLink = document.createElement('button');
      cardLink.type = 'button';
      cardLink.className = 'hf-sr-card-link';
      cardLink.title = 'View model card';
      cardLink.setAttribute('aria-label', `View model card for ${m.id}`);
      cardLink.innerHTML = '<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>';
      cardLink.addEventListener('click', e => { e.stopPropagation(); openCardPanel(m.id); });

      row.appendChild(nameEl);
      row.appendChild(meta);
      row.appendChild(cardLink);

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
    // Scroll after rows are in the DOM so the full result height is measured correctly.
    scrollToResults();
  } catch (err) {
    clearPillLoading();
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
    } else {
      // Re-fire the active discover pill with the new sort
      const activePill = document.querySelector('#hf-discover-pills .hf-discover-pill.active');
      if (activePill) {
        const cat = HF_DISCOVER_CATEGORIES.find(c => c.id === activePill.dataset.catId);
        if (cat) showHfSearchResults({ ...cat.params, sort });
      }
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

    // Store file lists so hardware step can offer quant swap + mmproj
    wizardState.model.quantFiles = [];
    wizardState.model.mmprojFiles = [];
    files.forEach(f => {
      const fp = f.path || f.name || '';
      if (!fp) return;
      if (f.is_mmproj) wizardState.model.mmprojFiles.push(f);
      else wizardState.model.quantFiles.push(f);
    });

    dom.hfFileList.innerHTML = '';
    if (!files.length) { dom.hfFileList.innerHTML = '<div class="hf-file-empty">No GGUF files found in this repo.</div>'; return; }

    const vramGb = cachedVram / (1024 ** 3);
    let autoSelectFn = null;      // recommended quant match
    let firstSelectFn = null;     // first non-mmproj file, fallback

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
        wizardState.model.delivery = 'stream_hf';
        wizardState.model.originRepo = repo;
        wizardState.model.originFile = fname;
        wizardState.model.localMeta = null;
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
        autoInstallChatTemplate();
      };
      item.addEventListener('click', selectFile);
      item.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectFile(); } });

      if (!file.is_mmproj) {
        if (!firstSelectFn) firstSelectFn = selectFile;
        if (!autoSelectFn && file.label && vramGb > 0 && file.label === getRecommendedQuant(vramGb)) {
          autoSelectFn = selectFile;
        }
      }

      dom.hfFileList.appendChild(item);
    });

    // Auto-select the recommended file (or first file) so the user can hit Next without manually clicking
    if (!wizardState.model.hfFile) {
      const autoFn = autoSelectFn || firstSelectFn;
      if (autoFn) autoFn();
    }
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

const TOOL_ICONS = {
  'Ollama': '🦙',
  'LM Studio': '🎨',
  'Jan': '🤖',
  'GPT4All': '🌍',
  'HuggingFace': '🤗',
};

function formatBytes(bytes) {
  if (!bytes) return '';
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${Math.round(bytes / 1e6)} MB`;
  return `${Math.round(bytes / 1e3)} KB`;
}

async function loadThirdPartyModels() {
  const listWrap = document.getElementById('import-model-list-wrap');
  const listLoading = document.getElementById('import-model-list-loading');
  const listEmpty = document.getElementById('import-model-list-empty');
  const listEl = document.getElementById('import-model-list');
  if (!listEl) return;

  listLoading && (listLoading.style.display = '');
  listEmpty && (listEmpty.style.display = 'none');
  listEl.style.display = 'none';
  listEl.innerHTML = '';

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const resp = await fetch('/api/third-party-models', { method: 'POST', headers, body: JSON.stringify({}) });
    if (!resp.ok) throw new Error('fetch failed');
    const data = await resp.json();
    const models = (data.models || []).filter(Boolean);

    listLoading && (listLoading.style.display = 'none');

    if (!models.length) {
      listEmpty && (listEmpty.style.display = '');
      return;
    }

    // Group by source_tool
    const grouped = {};
    for (const m of models) {
      const tool = m.source_tool || 'Other';
      if (!grouped[tool]) grouped[tool] = [];
      grouped[tool].push(m);
    }

    for (const [tool, toolModels] of Object.entries(grouped)) {
      const icon = TOOL_ICONS[tool] || '📦';
      const groupEl = document.createElement('div');
      groupEl.className = 'import-tool-group';
      groupEl.innerHTML = `<div class="import-tool-header"><span class="import-tool-icon">${icon}</span><span class="import-tool-name">${tool}</span></div>`;

      for (const m of toolModels) {
        const itemEl = document.createElement('div');
        itemEl.className = 'import-model-item';
        itemEl.setAttribute('role', 'button');
        itemEl.setAttribute('tabindex', '0');
        itemEl.dataset.path = m.path;
        const sizeStr = formatBytes(m.size);
        itemEl.innerHTML =
          `<span class="import-model-name">${m.name}</span>` +
          (sizeStr ? `<span class="import-model-size">${sizeStr}</span>` : '');
        itemEl.addEventListener('click', () => selectImportedModel(m));
        itemEl.addEventListener('keydown', e => {
          if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectImportedModel(m); }
        });
        groupEl.appendChild(itemEl);
      }
      listEl.appendChild(groupEl);
    }

    listEl.style.display = '';
  } catch {
    listLoading && (listLoading.style.display = 'none');
    listEmpty && (listEmpty.style.display = '');
  }
}

function selectImportedModel(m) {
  wizardState.model.path = m.path;
  wizardState.model.source = 'import';
  wizardState.model.delivery = 'imported_local';
  // Pre-populate localMeta with tool display info so the hint shows the
  // human-readable name rather than the raw file/blob path.
  wizardState.model.localMeta = {
    model_name: m.name,
    size_display: formatBytes(m.size),
    source_tool: m.source_tool,
    path: m.path,
  };
  // Sync the fallback text input so validation sees the path.
  if (dom.importPathInput) dom.importPathInput.value = m.path;
  // Mark the selected card visually.
  document.querySelectorAll('.import-model-item').forEach(el => el.classList.remove('selected'));
  // Find and mark the clicked item — match by path data attribute.
  document.querySelectorAll('.import-model-item').forEach(el => {
    if (el.dataset.path === m.path) el.classList.add('selected');
  });
  // Trigger arch inference + introspection using the model name for heuristics.
  onModelPathChanged();
  renderLocalModelHint();
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

export function scheduleVramUpdate() {
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
    if (availVram > 0) dom.vramPanelTotal.textContent = formatVramTotal(availVram) + ' total';
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
    { quant: 'q8_0', kk: 'q8_0', kv: 'q8_0', desc: uc === 'agentic' ? 'Required for agentic use' : 'Balanced quality', rec: uc !== 'roleplay' },
    { quant: 'q4_0', kk: 'q4_0', kv: 'q4_0', desc: uc === 'agentic' ? 'Not recommended for agents' : 'More context, great for RP', rec: uc === 'roleplay', warnAgentic: uc === 'agentic' },
    { quant: 'f16',  kk: 'f16',  kv: 'f16',  desc: 'Full precision, VRAM-heavy', rec: false },
  ];

  dom.vramScenarios.innerHTML = '';
  const activeQuant = hw.cacheTypeK || 'q8_0';
  // Cap context to the model's training window so we never show a number the model can't actually use.
  // When VRAM is plentiful relative to the model, all three cards may hit the same cap — in that case
  // the differentiator shifts from "more context" to "better quality".
  const nCtxTrain = wizardState.model.nCtxTrain || 0;

  for (const s of scenarios) {
    const vramCtx = maxContext(modelBytes, arch, s.kk, s.kv, slots, ubatch, nCpuMoe, availVram, fitGran, 0.05);
    const cappedByModel = nCtxTrain > 0 && vramCtx > nCtxTrain;
    const ctx = cappedByModel ? nCtxTrain : vramCtx;

    const card = document.createElement('div');
    const isActive = s.kk === activeQuant;
    card.className = 'vram-scenario-card' + (s.rec ? ' scenario-rec' : '') + (isActive ? ' selected' : '');
    card.setAttribute('tabindex', '0');
    card.setAttribute('role', 'button');
    card.setAttribute('aria-label', `${s.quant} KV: ${formatCtx(ctx)} tokens — ${s.desc}`);

    const selectable = ctx > 0;

    // When model is the bottleneck (not VRAM), rewrite the quality-focused description
    // so users understand all three quants give the same context, but differ in quality.
    let desc = s.desc;
    if (cappedByModel) {
      if (s.quant === 'q8_0') desc = 'Best quality — VRAM headroom to spare';
      else if (s.quant === 'q4_0') desc = 'Good quality — most VRAM headroom';
      else if (s.quant === 'f16') desc = 'Full precision — uses most VRAM';
    }

    const limitNote = cappedByModel ? '<span class="vsc-limit-note">model max</span>' : '';

    // All values are internal constants — no user input reaches this template.
    // eslint-disable-next-line no-unsanitized/property
    card.innerHTML = `
      <div class="vsc-quant-name">${s.quant.toUpperCase()} KV</div>
      <div class="vsc-ctx-row">
        <span class="vsc-ctx">${selectable ? formatCtx(ctx) : '—'}</span>
        ${selectable ? '<span class="vsc-ctx-unit">tokens</span>' : ''}
        ${limitNote}
      </div>
      <div class="vsc-desc">${desc}</div>
      ${s.rec ? '<span class="vsc-rec-badge">★ Recommended</span>' : ''}
      ${s.warnAgentic ? '<span class="vsc-warn">⚠ Not for agents</span>' : ''}
    `;

    if (selectable) {
      const applyScenario = () => {
        wizardState.hardware.cacheTypeK = s.kk;
        wizardState.hardware.cacheTypeV = s.kv;
        wizardState.hardware.contextSize = ctx;

        if (dom.cacheTypeKSelect) dom.cacheTypeKSelect.value = s.kk;
        if (dom.cacheTypeVSelect) dom.cacheTypeVSelect.value = s.kv;
        if (dom.contextSizeInput) dom.contextSizeInput.value = ctx;

        card.querySelector('.vsc-ctx')?.classList.add('counting');
        setTimeout(() => card.querySelector('.vsc-ctx')?.classList.remove('counting'), 300);

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

// ── Hardware step: model header + quant swap ─────────────────────────────────

function renderHardwareModelHeader() {
  const header = document.getElementById('hw-model-header');
  if (!header) return;
  const { source, path, hfRepo, hfFile, quantFiles } = wizardState.model;
  if (!hfRepo && !path) { header.style.display = 'none'; return; }
  header.style.display = '';

  const repoEl = document.getElementById('hw-model-repo');
  if (repoEl) {
    const fullRepo = hfRepo || path.split(/[/\\]/).pop() || path;
    const slashIdx = fullRepo.indexOf('/');
    if (hfRepo && slashIdx > 0) {
      const author = fullRepo.slice(0, slashIdx + 1); // "owner/"
      const modelName = fullRepo.slice(slashIdx + 1);
      repoEl.textContent = '';
      const authorSpan = document.createElement('span');
      authorSpan.className = 'hw-model-author';
      authorSpan.textContent = author;
      const nameSpan = document.createElement('span');
      nameSpan.className = 'hw-model-name';
      nameSpan.textContent = modelName;
      repoEl.appendChild(authorSpan);
      repoEl.appendChild(nameSpan);
    } else {
      repoEl.textContent = fullRepo;
    }
  }

  const quantRow = document.getElementById('hw-quant-row');
  const quantSelect = document.getElementById('hw-quant-select');
  const vramGb = cachedVram / (1024 ** 3);

  if (quantSelect && quantFiles && quantFiles.length > 1) {
    quantSelect.innerHTML = '';
    quantFiles.forEach(qf => {
      const fpath = qf.path || qf.name || '';
      const fname = fpath.split('/').pop();
      if (!fname) return;
      const opt = document.createElement('option');
      opt.value = fpath;
      const dispLabel = qf.label || fname; // prefer "Q4_K_M" over full filename
      const sizeStr = qf.size ? ` · ${formatBytes(qf.size)}` : '';
      const isRec = qf.label && vramGb > 0 && qf.label === getRecommendedQuant(vramGb);
      opt.textContent = dispLabel + sizeStr + (isRec ? ' ★' : '');
      if (fpath === hfFile) opt.selected = true;
      quantSelect.appendChild(opt);
    });
    if (quantRow) quantRow.style.display = '';
  } else {
    if (quantRow) quantRow.style.display = 'none';
    const fileEl = document.getElementById('hw-model-file');
    if (fileEl) fileEl.textContent = hfFile ? hfFile.split('/').pop() : (path.split(/[/\\]/).pop() || '');
  }
}

// ── Hardware step: mmproj section ────────────────────────────────────────────

function renderMmprojSection() {
  const row = document.getElementById('hw-mmproj-row');
  if (!row) return;
  const files = wizardState.model.mmprojFiles || [];
  if (!files.length) { row.style.display = 'none'; return; }
  row.style.display = '';

  const select = document.getElementById('hw-mmproj-select');
  if (!select) return;

  if (!select.dataset.populated) {
    select.dataset.populated = '1';
    select.innerHTML = '';
    const noneOpt = document.createElement('option');
    noneOpt.value = ''; noneOpt.textContent = '(none — text-only)';
    select.appendChild(noneOpt);
    files.forEach(f => {
      const fpath = f.path || f.name || '';
      const fname = fpath.split('/').pop();
      const opt = document.createElement('option');
      opt.value = fpath;
      const sizeStr = f.size ? ` · ${formatBytes(f.size)}` : '';
      opt.textContent = fname + sizeStr;
      if (fpath === wizardState.model.mmprojHfFile) opt.selected = true;
      select.appendChild(opt);
    });
    select.addEventListener('change', () => {
      const fpath = select.value;
      wizardState.model.mmprojHfFile = fpath;
      const f = files.find(x => (x.path || x.name) === fpath);
      wizardState.arch.mmprojBytes = f?.size ? Number(f.size) : 0;
      scheduleVramUpdate();
    });
  }

  // Sync selection state
  if (wizardState.model.mmprojHfFile) {
    select.value = wizardState.model.mmprojHfFile;
  }
}

// ── Hardware step: MTP section ───────────────────────────────────────────────

function renderMtpSection() {
  const section = document.getElementById('hw-mtp-section');
  if (!section) return;
  const hasMtp = wizardState.arch.mtpDepth > 0 ||
    detectMtpFromName(wizardState.model.hfFile || wizardState.model.path || '');
  if (!hasMtp) { section.style.display = 'none'; return; }
  section.style.display = '';

  const checkbox = document.getElementById('hw-use-mtp');
  // The user-facing control is spec-draft-n-max (draft tokens per step), not "depth"
  // arch.mtpDepth = number of MTP heads built into the model (VRAM estimation only)
  const draftNMaxInput = document.getElementById('hw-mtp-depth');

  if (draftNMaxInput) {
    if (!draftNMaxInput.dataset.bound) {
      draftNMaxInput.dataset.bound = '1';
      draftNMaxInput.addEventListener('input', () => {
        const v = parseInt(draftNMaxInput.value, 10);
        if (v >= 0 && v <= 4) {
          wizardState.hardware.mtpDraftNMax = v;
        }
      });
    }
    draftNMaxInput.value = wizardState.hardware.mtpDraftNMax;
  }

  if (checkbox) {
    if (!checkbox.dataset.bound) {
      checkbox.dataset.bound = '1';
      checkbox.addEventListener('change', () => {
        wizardState.hardware.mtpEnabled = checkbox.checked;
        // MTP requires parallel=1 — update state
        if (checkbox.checked) wizardState.hardware.parallelSlots = 1;
        const depthRow = document.getElementById('hw-mtp-depth-row');
        if (depthRow) depthRow.style.display = checkbox.checked ? '' : 'none';
        scheduleVramUpdate();
      });
    }
    checkbox.checked = wizardState.hardware.mtpEnabled;
    const depthRow = document.getElementById('hw-mtp-depth-row');
    if (depthRow) depthRow.style.display = checkbox.checked ? '' : 'none';
  }
}

function _updateSpecHint(value) {
  const container = document.getElementById('spawn-spec-hint');
  if (!container) return;
  container.querySelectorAll('[data-spec]').forEach(el => {
    el.style.display = el.dataset.spec === value ? '' : 'none';
  });
}

function updateLegacyVramPill(total, avail) {
  if (dom.vramEstimateText) {
    dom.vramEstimateText.textContent = avail > 0
      ? `${formatGB(total)} / ${formatVramTotal(avail)}`
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
    updateCtxQuickPickActive();
    showCtxFitWarning(r.context_size, wizardState.useCase);

    if (r.warnings?.length) showToast('Auto-size warnings', 'warning', r.warnings[0]);
    else showToast('Auto-sized', 'success', note);
  } catch (err) {
    showToast('Auto-size error: ' + (err.message || String(err)), 'error');
  } finally {
    btn.disabled = false; btn.textContent = origText;
  }
}

// ── Context quick-picks ───────────────────────────────────────────────────────

function bindCtxQuickPicks() {
  document.querySelectorAll('.ctx-pick').forEach(btn => {
    btn.addEventListener('click', () => {
      const ctx = parseInt(btn.dataset.ctx, 10);
      if (!ctx) return;
      if (dom.contextSizeInput) dom.contextSizeInput.value = ctx;
      wizardState.hardware.contextSize = ctx;
      updateCtxQuickPickActive();
      showCtxFitWarning(ctx, wizardState.useCase, true); // manual — no fit warning
      updateCtxTrainWarning();
      updateVramDisplay();
    });
  });

  // Manual input: update state, chip highlight, and both warning types
  dom.contextSizeInput?.addEventListener('change', () => {
    const ctx = parseInt(dom.contextSizeInput.value, 10);
    if (!ctx) return;
    wizardState.hardware.contextSize = ctx;
    updateCtxQuickPickActive();
    showCtxFitWarning(ctx, wizardState.useCase, true);
    updateCtxTrainWarning();
    updateVramDisplay();
  });
}

function updateCtxQuickPickActive() {
  const current = parseInt(dom.contextSizeInput?.value || '0', 10);
  document.querySelectorAll('.ctx-pick').forEach(btn => {
    btn.classList.toggle('active', parseInt(btn.dataset.ctx, 10) === current);
  });
}

// Show the model's training context ceiling near the context size input so users
// always know the limit before accidentally exceeding it.
function updateCtxModelMaxHint() {
  const hint = document.getElementById('ctx-model-max-hint');
  const btn  = document.getElementById('ctx-pick-model-max');
  const nCtxTrain = wizardState.model.nCtxTrain || 0;
  if (!nCtxTrain) {
    if (hint) hint.style.display = 'none';
    if (btn)  btn.style.display  = 'none';
    return;
  }
  const fmtK = n => n >= 1000 ? `${Math.round(n / 1024)}k` : `${n}`;
  const label = fmtK(nCtxTrain);
  if (hint) {
    hint.textContent = `model max: ${label}`;
    hint.style.display = '';
  }
  if (btn) {
    btn.dataset.ctx = nCtxTrain;
    btn.childNodes[0].textContent = label + ' ';
    btn.style.display = '';
    // Remove duplicate if there's already a static pick with the same value
    document.querySelectorAll('.ctx-pick:not(#ctx-pick-model-max)').forEach(el => {
      if (Number(el.dataset.ctx) === nCtxTrain) btn.style.display = 'none';
    });
  }
}

// Warn when the selected context exceeds the model's training context length.
// We still allow it — users may be using RoPE/YaRN extension — but we flag it clearly.
function updateCtxTrainWarning() {
  const el = document.getElementById('ctx-train-warning');
  if (!el) return;
  const nCtxTrain = wizardState.model.nCtxTrain;
  const selected  = wizardState.hardware.contextSize;
  if (!nCtxTrain || !selected || selected <= nCtxTrain) {
    el.style.display = 'none';
    return;
  }
  const fmtK = n => n >= 1024 ? `${Math.round(n / 1024)}k` : `${n}`;
  const scale = (1 / (selected / nCtxTrain)).toFixed(3);
  el.textContent = '';
  const strong = document.createElement('strong');
  strong.textContent = `Context (${fmtK(selected)}) exceeds this model's training window (${fmtK(nCtxTrain)})`;
  const code = document.createElement('code');
  code.textContent = `--rope-scaling yarn --rope-freq-scale ${scale}`;
  el.appendChild(strong);
  el.appendChild(document.createTextNode(
    `. Generation quality degrades beyond the training limit. To extend safely, enable RoPE scaling (YaRN) — or add `
  ));
  el.appendChild(code);
  el.appendChild(document.createTextNode(' to Extra Args.'));
  el.className = 'ctx-fit-warning';
  el.style.display = '';
  updateCtxModelMaxHint();
}

// Minimum-context guidance: warn when auto-size lands below the target for the use case.
// Never warn when the user has manually typed or picked a high value — that's intentional.
const CTX_TARGETS = { agentic: 131072, general: 32768, roleplay: 65536 };

function showCtxFitWarning(ctx, useCase, manualSet = false) {
  const el = document.getElementById('ctx-fit-warning');
  if (!el) return;

  // If user explicitly chose this value (chip click or manual type), no warning
  if (manualSet) { el.style.display = 'none'; return; }

  const target = CTX_TARGETS[useCase] ?? 0;
  if (!target || ctx >= target) { el.style.display = 'none'; return; }

  const fmtCtx = c => c >= 1024 ? `${Math.round(c / 1024)}k` : `${c}`;
  const got = fmtCtx(ctx), need = fmtCtx(target);

  el.textContent = '';
  const strong = document.createElement('strong');
  if (useCase === 'agentic') {
    strong.textContent = `Can't reach ${need} for agentic work`;
    el.appendChild(strong);
    el.appendChild(document.createTextNode(
      ` — auto-sized to ${got} at q8_0 KV. Try Q4_K_M or IQ3_XXS to shrink weights, or switch to a 27B model. You can also override by typing a custom value or picking 200k/256k above.`
    ));
  } else if (useCase === 'roleplay') {
    strong.textContent = `Below ${need} RP target`;
    el.appendChild(strong);
    el.appendChild(document.createTextNode(
      ` — auto-sized to ${got}. Switch KV cache to q4_0 to halve KV memory, or use a smaller quant.`
    ));
  } else {
    el.appendChild(document.createTextNode(
      `Auto-size returned ${got} (target ${need}). Consider a smaller quantization.`
    ));
  }
  el.className = ctx < target * 0.5 ? 'ctx-fit-warning ctx-fit-error' : 'ctx-fit-warning';
  el.style.display = '';
}

// Use-case sampling defaults (temperature, top-p, min-p, repeat-penalty)
const SAMPLING_DEFAULTS = {
  agentic:  { temperature: 0.3,  topP: 0.95, minP: 0.02, topK: null, repeatPenalty: 1.05, seed: null },
  general:  { temperature: 0.7,  topP: 0.9,  minP: 0.05, topK: null, repeatPenalty: 1.05, seed: null },
  roleplay: { temperature: 1.0,  topP: 0.95, minP: 0.05, topK: null, repeatPenalty: 1.05, seed: null },
};

function applyUseCaseSamplingDefaults() {
  const def = SAMPLING_DEFAULTS[wizardState.useCase] || SAMPLING_DEFAULTS.general;
  const h = wizardState.hardware;
  // Only apply defaults if user hasn't already set explicit values
  if (h.temperature == null) h.temperature = def.temperature;
  if (h.topP == null) h.topP = def.topP;
  if (h.minP == null) h.minP = def.minP;
  if (h.topK == null && def.topK != null) h.topK = def.topK;
  if (h.repeatPenalty == null) h.repeatPenalty = def.repeatPenalty;
}

// ── Summary (Step 4) ──────────────────────────────────────────────────────────

function renderSummary() {
  if (!dom.summaryList) return;
  dom.summaryList.innerHTML = '';

  // Apply use-case sampling defaults before rendering
  applyUseCaseSamplingDefaults();
  // Sync sampling fields in the review step form
  _syncSamplingFields();

  const m = wizardState.model, hw = wizardState.hardware;
  const arch = getEffectiveArch();
  const availVram = cachedVram || wizardState.vram.available;
  const modelBytes = getModelBytes();

  const modelDisplay = m.source === 'hf'
    ? (m.hfFile ? `${m.hfRepo} / ${m.hfFile.split('/').pop()}` : m.hfRepo || '(none)')
    : (m.path ? m.path.split(/[\\/]/).pop() || m.path : '(none)');

  let acquisition = 'Local file';
  if (m.delivery === 'stream_hf' && m.originRepo) {
    acquisition = `Stream from HuggingFace · ${m.originRepo}${m.originFile ? ` / ${m.originFile.split('/').pop()}` : ''}`;
  } else if (m.delivery === 'downloaded_hf' && m.originRepo) {
    acquisition = `Downloaded from HuggingFace · ${m.originRepo}${m.originFile ? ` / ${m.originFile.split('/').pop()}` : ''}`;
  } else if (m.delivery === 'imported_local') {
    acquisition = 'Imported local file';
  }

  const ctxK = hw.cacheTypeK || 'q8_0', ctxV = hw.cacheTypeV || 'q8_0';
  const kvSize = modelBytes > 0 ? kvBytes(arch, hw.contextSize, hw.parallelSlots, ctxK, ctxV) : 0;

  const rows = [
    { label: 'Use case',      value: { agentic: 'Agentic / RAG', general: 'General chat', roleplay: 'Roleplay / creative' }[wizardState.useCase] || wizardState.useCase },
    { label: 'Profile',       value: wizardState.profile },
    { label: 'Acquisition',   value: acquisition },
    { label: 'Port',          value: String(wizardState.access.port || 8001) },
    { label: 'Model',         value: modelDisplay },
    { label: 'Bind host',     value: wizardState.access.bindHost === '0.0.0.0' ? '0.0.0.0 (LAN visible)' : '127.0.0.1 only' },
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
  if (arch.mtpDepth > 0) {
    const mtpActive = hw.mtpEnabled;
    rows.push({ label: 'MTP', value: mtpActive ? `enabled · draft ${hw.mtpDraftNMax || 2} tokens/step · --parallel 1` : 'disabled' });
  }
  if ((m.delivery === 'stream_hf' || m.delivery === 'downloaded_hf' || m.originRepo) && m.hfTokenSet != null) {
    rows.push({ label: 'HF token', value: m.hfTokenSet ? 'Saved in app settings' : 'Not saved' });
  }
  const tplPath = wizardState.model.chatTemplatePath;
  const tplFamily = detectModelFamily(m.hfRepo || m.path || '');
  if (tplPath) {
    const tplName = tplPath.split(/[/\\]/).pop() || tplPath;
    rows.push({ label: 'Chat template', value: tplName });
  } else if (tplFamily) {
    rows.push({ label: 'Chat template', value: 'Embedded (from model file)' });
  }

  const specType = dom.specTypeSelect?.value || '';
  if (specType) {
    let sv = { 'ngram-mod': 'N-gram (fast)', 'draft-model': 'Draft model' }[specType] || specType;
    if (specType === 'draft-model' && dom.draftModelInput?.value) sv += ` (${dom.draftModelInput.value.split(/[\\/]/).pop()})`;
    rows.push({ label: 'Speculative', value: sv });
  }
  if (hw.kvUnified) rows.push({ label: 'KV unified', value: 'Yes' });
  if (hw.ignoreEos) rows.push({ label: 'Ignore EOS', value: 'Yes' });
  if (wizardState.access.apiKey) rows.push({ label: 'Server API key', value: `${wizardState.access.apiKey.slice(0, 4)}…${wizardState.access.apiKey.slice(-4)}` });

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
    if (wizardState.access.bindHost === '0.0.0.0' && !wizardState.access.apiKey) warns.push('LAN-visible endpoint without a server API key. Set one unless you intentionally want an open local-network server.');
    else if (wizardState.access.bindHost === '0.0.0.0') warns.push('LAN-visible endpoint enabled. Make sure clients know the API key you set.');
    if (warns.length) {
      dom.summaryWarnings.style.display = '';
      dom.summaryWarnings.innerHTML = '';
      warns.forEach(w => { const p = document.createElement('div'); p.textContent = w; dom.summaryWarnings.appendChild(p); });
    } else {
      dom.summaryWarnings.style.display = 'none';
    }
  }
  if (dom.healthCheckBtn) dom.healthCheckBtn.style.display = '';

  // Add step shortcuts for last-minute changes
  const editRow = document.createElement('div');
  editRow.className = 'summary-edit-row';
  editRow.style.display = 'flex';
  editRow.style.gap = '8px';
  editRow.style.flexWrap = 'wrap';
  editRow.style.marginTop = '10px';

  const shortcuts = [
    { label: 'Edit model', step: 1 },
    { label: 'Edit hardware', step: 2 },
    { label: 'Edit sampling', step: 3, focusId: 'spawn-temperature' },
  ];
  shortcuts.forEach(({ label, step, focusId }) => {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'btn-wizard-tertiary';
    btn.textContent = label;
    btn.addEventListener('click', () => {
      showStep(step);
      if (focusId) {
        setTimeout(() => document.getElementById(focusId)?.focus(), 50);
      }
    });
    editRow.appendChild(btn);
  });
  dom.summaryList.appendChild(editRow);
}

// ── Sampling field sync (Review step) ────────────────────────────────────────

function _syncSamplingFields() {
  const h = wizardState.hardware;
  const setVal = (id, val) => {
    const el = document.getElementById(id);
    if (el && val != null) el.value = val;
    else if (el) el.value = '';
  };
  setVal('spawn-temperature', h.temperature);
  setVal('spawn-seed', h.seed);
  setVal('spawn-top-p', h.topP);
  setVal('spawn-min-p', h.minP);
  setVal('spawn-repeat-penalty', h.repeatPenalty);
  if (dom.bindHostSelect) dom.bindHostSelect.value = wizardState.access.bindHost || '127.0.0.1';
  if (dom.portInput) dom.portInput.value = String(wizardState.access.port || 8001);
  if (dom.apiKeyInput) dom.apiKeyInput.value = wizardState.access.apiKey || '';
}

function _bindSamplingFields() {
  const bind = (id, key, isInt = false) => {
    const el = document.getElementById(id);
    if (!el || el.dataset.bound) return;
    el.dataset.bound = '1';
    el.addEventListener('input', () => {
      const raw = el.value.trim();
      if (raw === '') { wizardState.hardware[key] = null; return; }
      const v = isInt ? parseInt(raw, 10) : parseFloat(raw);
      if (!isNaN(v)) wizardState.hardware[key] = v;
    });
  };
  bind('spawn-temperature', 'temperature');
  bind('spawn-seed', 'seed', true);
  bind('spawn-top-p', 'topP');
  bind('spawn-min-p', 'minP');
  bind('spawn-repeat-penalty', 'repeatPenalty');
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
    bind_host: wizardState.access.bindHost || '127.0.0.1',
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
    api_key: wizardState.access.apiKey || null,
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
  if (!_binaryReady) {
    showErrorText('llama.cpp binary not found. Download it using the banner above.');
    return;
  }
  wizardState.spawn.inFlight = true; wizardState.spawn.error = '';
  if (!dom.spawnServerBtn) return;
  dom.spawnServerBtn.disabled = true;
  setStatusText('Preparing configuration…'); setProgress(10); clearStatusMessages();
  try {
    const payload = buildSpawnPayload();
    setStatusText('Starting llama-server…'); setProgress(30);
    const headers = window.authHeaders ? { ...window.authHeaders(), 'Content-Type': 'application/json' } : { 'Content-Type': 'application/json' };
    const resp = await fetch('/api/sessions/spawn', { method: 'POST', headers, body: JSON.stringify(payload) });
    setProgress(60);
    if (!resp.ok) { const t = await resp.text().catch(()=>'Unknown error'); throw new Error(t || `HTTP ${resp.status}`); }
    const data = await resp.json().catch(() => null);
    if (!data?.ok) throw new Error(data?.error || 'Spawn request failed.');
    setStatusText('Server process started. Waiting for endpoint…');
    setProgress(75);
    await waitForSpawnReadiness(payload.port);
    setProgress(100); setStatusText('Server started.');
    showSuccessText('Server is running.'); showToast('Server started', 'success');
    setTuneConfig(payload);
    setTimeout(() => {
      closeSpawnWizard();
      if (document.body.classList.contains('setup-active')) {
        switchView('monitor');
      }
      showTunePanel();
      setTimeout(() => window.restorePreviousPosition?.(), 600);
    }, 1200);
  } catch (err) {
    const msg = (err.message || String(err)).split('\n')[0].trim();
    showErrorText(msg || 'Failed to start server.'); setStatusText('Spawn failed.'); wizardState.spawn.error = msg;
    showToast('Spawn failed', 'error', msg || 'Check logs.');
  } finally {
    wizardState.spawn.inFlight = false;
    if (dom.spawnServerBtn) dom.spawnServerBtn.disabled = false;
  }
}

async function waitForSpawnReadiness(port, timeoutMs = 30000) {
  const started = Date.now();
  const headers = window.authHeaders ? window.authHeaders() : {};

  while (Date.now() - started < timeoutMs) {
    try {
      const resp = await fetch('/api/sessions/active/readiness', {
        method: 'GET',
        headers,
        cache: 'no-store',
      });
      const data = await resp.json().catch(() => null);
      if (resp.ok && data?.ok && data.ready) return;
    } catch {
      // Keep polling until timeout; the backend route may lag while the process boots.
    }
    const elapsed = Date.now() - started;
    setStatusText(`Waiting for endpoint on port ${port}…`);
    setProgress(Math.min(95, 75 + Math.floor((elapsed / timeoutMs) * 20)));
    await new Promise(r => setTimeout(r, 800));
  }

  throw new Error(`llama-server started but did not become reachable on port ${port} in time.`);
}

function buildSpawnPayload() {
  const h = wizardState.hardware, m = wizardState.model;
  const arch = getEffectiveArch();
  const gpuLayers = h.gpuLayers === 'manual' ? (h.gpuLayersManual ?? -1) : (h.gpuLayers === 'all' ? -1 : null);

  // MTP: when enabled, use draft-mtp spec type and force parallel=1
  const mtpActive = arch.mtpDepth > 0 && h.mtpEnabled;
  const specType = mtpActive ? 'draft-mtp,ngram-mod' : (dom.specTypeSelect?.value || '');
  const parallelSlots = mtpActive ? 1 : h.parallelSlots;

  return {
    model_path: m.source !== 'hf' ? (m.path || null) : null,
    hf_repo: m.source === 'hf' ? (m.hfRepo || null) : null,
    hf_file: m.source === 'hf' ? (m.hfFile || null) : null,
    port: wizardState.access.port || 8001,
    bind_host: wizardState.access.bindHost || '127.0.0.1',
    gpu_layers: gpuLayers,
    context_size: h.contextSize,
    batch_size: h.batchSize,
    ubatch_size: h.ubatchSize,
    parallel_slots: parallelSlots,
    ctk: h.cacheTypeK || null,
    ctv: h.cacheTypeV || null,
    n_cpu_moe: h.nCpuMoe || null,
    tensor_split: h.tensorSplit || null,
    spec_type: specType,
    spec_draft_n_max: mtpActive ? (h.mtpDraftNMax || 2) : undefined,
    draft_model: (dom.draftModelInput?.value || '').trim() || null,
    kv_unified: h.kvUnified || null,
    ignore_eos: h.ignoreEos || null,
    fit: h.fitCtx ? 'on' : null,
    fit_ctx: h.fitCtx || null,
    // Sampling defaults (null = use llama-server built-in defaults)
    temperature: h.temperature != null ? h.temperature : null,
    top_p: h.topP != null ? h.topP : null,
    top_k: h.topK != null ? h.topK : null,
    min_p: h.minP != null ? h.minP : null,
    repeat_penalty: h.repeatPenalty != null ? h.repeatPenalty : null,
    seed: h.seed != null ? h.seed : null,
    api_key: wizardState.access.apiKey || null,
    chat_template_file: wizardState.model.chatTemplatePath || null,
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

// ── Model card panel ──────────────────────────────────────────────────────────

async function openCardPanel(repoId) {
  if (!dom.cardPanel) return;

  // Show panel in loading state
  dom.cardPanel.classList.add('open');
  dom.cardPanel.setAttribute('aria-hidden', 'false');
  if (dom.cardPanelTitle) dom.cardPanelTitle.textContent = repoId;
  if (dom.cardPanelHfLink) {
    dom.cardPanelHfLink.href = `https://huggingface.co/${repoId}`;
    dom.cardPanelHfLink.textContent = '';
    const svg = dom.cardPanelHfLink.querySelector('svg') || document.createElementNS('http://www.w3.org/2000/svg','svg');
    dom.cardPanelHfLink.appendChild(svg);
    dom.cardPanelHfLink.appendChild(document.createTextNode(' huggingface.co'));
  }
  if (dom.cardLoading)    { dom.cardLoading.style.display = ''; }
  if (dom.cardError)      { dom.cardError.style.display = 'none'; dom.cardError.textContent = ''; }
  if (dom.cardFrontmatter){ dom.cardFrontmatter.style.display = 'none'; }
  if (dom.cardContent)    { dom.cardContent.style.display = 'none'; dom.cardContent.innerHTML = ''; }

  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch(`/api/hf/card?repo=${encodeURIComponent(repoId)}`, { headers });
    const data = resp.ok ? await resp.json() : { error: `HTTP ${resp.status}` };

    if (dom.cardLoading) dom.cardLoading.style.display = 'none';

    if (data.error) {
      if (dom.cardError) { dom.cardError.textContent = data.error; dom.cardError.style.display = ''; }
      return;
    }

    const raw = data.markdown || '';

    // Split off YAML front-matter (--- ... ---)
    let frontmatter = '';
    let body = raw;
    const fmMatch = raw.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);
    if (fmMatch) {
      frontmatter = fmMatch[1];
      body = fmMatch[2];
    }

    if (frontmatter && dom.cardFrontmatter && dom.cardFrontmatterPre) {
      dom.cardFrontmatterPre.textContent = frontmatter;
      dom.cardFrontmatter.style.display = '';
    }

    if (dom.cardContent) {
      dom.cardContent.textContent = '';
      if (!body.trim()) {
        const p = document.createElement('p');
        p.style.cssText = 'color:var(--color-text-muted);font-size:var(--text-sm)';
        p.textContent = 'No model card content found.';
        dom.cardContent.appendChild(p);
      } else if (window.marked && window.DOMPurify) {
        // RETURN_DOM_FRAGMENT gives a sanitized DocumentFragment — no innerHTML needed
        const frag = window.DOMPurify.sanitize(window.marked.parse(body), { RETURN_DOM_FRAGMENT: true });
        dom.cardContent.appendChild(frag);
      } else {
        dom.cardContent.textContent = body;
      }
      dom.cardContent.style.display = '';
    }
  } catch (err) {
    if (dom.cardLoading) dom.cardLoading.style.display = 'none';
    if (dom.cardError) { dom.cardError.textContent = err.message || 'Failed to load model card.'; dom.cardError.style.display = ''; }
  }
}

function _closeCardPanel() {
  if (!dom.cardPanel) return;
  dom.cardPanel.classList.remove('open');
  dom.cardPanel.setAttribute('aria-hidden', 'true');
}

/** Convert an ISO 8601 timestamp to a human-readable relative age, e.g. "3d ago", "2mo ago". */
function _hfRelativeAge(iso) {
  if (!iso) return '';
  const ms = Date.now() - new Date(iso).getTime();
  if (isNaN(ms) || ms < 0) return '';
  const mins  = Math.floor(ms / 60_000);
  const hours = Math.floor(ms / 3_600_000);
  const days  = Math.floor(ms / 86_400_000);
  const weeks = Math.floor(days / 7);
  const months = Math.floor(days / 30);
  const years  = Math.floor(days / 365);
  if (mins  <  60)  return `${mins}m ago`;
  if (hours < 24)   return `${hours}h ago`;
  if (days  <  7)   return `${days}d ago`;
  if (weeks <  5)   return `${weeks}w ago`;
  if (months < 12)  return `${months}mo ago`;
  return `${years}y ago`;
}

// ── Binary prerequisite check & download ─────────────────────────────────────

let _binaryReady  = false;
let _platformInfo = null;   // cached result of /api/llama-binary/platform-info
let _selectedBackend = null;

async function _checkBinaryPrereq() {
  if (!dom.binaryPrereq) return;
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};

    // Fetch platform info and current version in parallel
    const [vResp, pResp] = await Promise.all([
      fetch('/api/llama-binary/version', { headers }),
      fetch('/api/llama-binary/platform-info', { headers }),
    ]);

    const vData = vResp.ok ? await vResp.json() : {};
    _platformInfo = pResp.ok ? await pResp.json() : null;

    if (_selectedBackend === null && _platformInfo) {
      _selectedBackend = _platformInfo.auto_backend;
    }

    if (vData.build) {
      _binaryReady = true;
      if (dom.binaryPrereq.style.display !== 'none') {
        _showPrereqState('success');
        if (dom.prereqSuccessText) {
          const label = _platformInfo ? _platformInfo.label : 'llama.cpp';
          dom.prereqSuccessText.textContent = `${label} b${vData.build} installed and ready.`;
        }
        setTimeout(() => { if (dom.binaryPrereq) dom.binaryPrereq.style.display = 'none'; }, 3000);
      }
      _updateSpawnBtnForPrereq();
    } else {
      _binaryReady = false;
      _showPrereqState('idle');
      dom.binaryPrereq.style.display = '';
      _renderPrereqIdle(vData, _platformInfo);
      _updateSpawnBtnForPrereq();
    }
  } catch {
    // Network error — don't block the wizard
  }
}

function _renderPrereqIdle(vData, platform) {
  // Update download button label with platform detail
  if (dom.prereqDownloadBtn && platform) {
    const label = platform.label || 'llama.cpp';
    dom.prereqDownloadBtn.textContent = `Download ${label}`;
  }

  // Show configured path if present but binary missing
  if (dom.prereqPath && dom.prereqPathRow) {
    const path = vData.path || '';
    dom.prereqPath.textContent = path || '(not configured — will use app default)';
    dom.prereqPathRow.style.display = '';
  }

  // For multi-backend platforms (Windows, Linux), inject a backend selector
  const existingPicker = document.getElementById('wizard-prereq-backend-picker');
  if (existingPicker) existingPicker.remove();

  if (platform && platform.multi_backend && platform.backends && platform.backends.length > 1) {
    const picker = document.createElement('div');
    picker.id = 'wizard-prereq-backend-picker';
    picker.className = 'wizard-prereq-backend-picker';

    const pickerLabel = document.createElement('div');
    pickerLabel.className = 'wizard-prereq-backend-label';
    pickerLabel.textContent = 'Select your GPU / backend:';
    picker.appendChild(pickerLabel);

    const select = document.createElement('select');
    select.className = 'wizard-prereq-backend-select';
    platform.backends.forEach(b => {
      const opt = document.createElement('option');
      opt.value = b.id;
      opt.textContent = b.label;
      if (b.id === _selectedBackend) opt.selected = true;
      select.appendChild(opt);
    });
    select.addEventListener('change', () => {
      _selectedBackend = select.value;
      // Update the note shown below the selector
      const selected = platform.backends.find(b => b.id === _selectedBackend);
      if (noteEl) noteEl.textContent = selected ? selected.note : '';
      // Update download button label
      if (dom.prereqDownloadBtn) {
        dom.prereqDownloadBtn.textContent = `Download llama.cpp (${select.options[select.selectedIndex].text.split(' —')[0]})`;
      }
    });
    picker.appendChild(select);

    // Note line below selector
    const noteEl = document.createElement('div');
    noteEl.className = 'wizard-prereq-backend-note';
    const currentBackend = platform.backends.find(b => b.id === _selectedBackend);
    noteEl.textContent = currentBackend ? currentBackend.note : '';
    picker.appendChild(noteEl);

    // Insert before the actions div
    const actions = dom.prereqIdle.querySelector('.wizard-prereq-actions');
    if (actions) dom.prereqIdle.insertBefore(picker, actions);

    // Update initial download button label for Windows
    if (dom.prereqDownloadBtn && currentBackend) {
      dom.prereqDownloadBtn.textContent = `Download llama.cpp (${currentBackend.label.split(' —')[0]})`;
    }
  }
}

function _showPrereqState(state) {
  if (dom.prereqIdle)     dom.prereqIdle.style.display     = state === 'idle'     ? '' : 'none';
  if (dom.prereqProgress) dom.prereqProgress.style.display = state === 'progress' ? '' : 'none';
  if (dom.prereqSuccess)  dom.prereqSuccess.style.display  = state === 'success'  ? '' : 'none';
}

function _updateSpawnBtnForPrereq() {
  if (!dom.spawnServerBtn) return;
  if (!_binaryReady) {
    dom.spawnServerBtn.disabled = true;
    dom.spawnServerBtn.title = 'llama.cpp binary required — download it above first';
  } else {
    dom.spawnServerBtn.disabled = false;
    dom.spawnServerBtn.title = '';
  }
}

async function _downloadBinaryForWizard() {
  if (!dom.binaryPrereq || !dom.prereqDownloadBtn) return;
  _showPrereqState('progress');
  if (dom.prereqDownloadBtn) dom.prereqDownloadBtn.disabled = true;

  const backend = _selectedBackend || (_platformInfo && _platformInfo.auto_backend) || null;
  const platformLabel = _platformInfo ? _platformInfo.label : 'llama.cpp';

  // Update progress description with what we're downloading
  const descEl = dom.prereqProgress?.querySelector('.wizard-prereq-desc');
  if (descEl) {
    descEl.textContent = backend && _platformInfo && _platformInfo.multi_backend
      ? `Downloading llama.cpp ${backend.toUpperCase()} build — this may take a minute…`
      : `Downloading ${platformLabel} build — this may take a minute…`;
  }

  const startTime = Date.now();
  let elapsedTimer = setInterval(() => {
    const s = Math.floor((Date.now() - startTime) / 1000);
    if (dom.prereqElapsed) dom.prereqElapsed.textContent = `${s}s elapsed…`;
    if (dom.prereqBar) {
      const pct = Math.min(90, 5 + Math.floor(s * 1.2));
      dom.prereqBar.style.width = pct + '%';
    }
  }, 1000);

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const body = backend ? { backend } : {};
    const resp = await fetch('/api/llama-binary/update', {
      method: 'POST', headers, body: JSON.stringify(body)
    });
    clearInterval(elapsedTimer);
    if (dom.prereqBar) dom.prereqBar.style.width = '100%';

    if (!resp.ok) {
      const txt = await resp.text().catch(() => `HTTP ${resp.status}`);
      throw new Error(txt);
    }
    const data = await resp.json();
    if (data.ok === false) throw new Error(data.error || 'Download failed');

    _binaryReady = true;
    _showPrereqState('success');
    if (dom.prereqSuccessText) {
      // data.version is the tag like "b5678"; data.backend is what was installed
      const ver  = data.version || 'installed';
      const back = data.backend ? ` · ${data.backend.toUpperCase()}` : '';
      dom.prereqSuccessText.textContent = `llama.cpp ${ver}${back} downloaded and ready.`;
    }
    _updateSpawnBtnForPrereq();
    setTimeout(() => { if (dom.binaryPrereq) dom.binaryPrereq.style.display = 'none'; }, 4000);
  } catch (err) {
    clearInterval(elapsedTimer);
    _showPrereqState('idle');
    _renderPrereqIdle({}, _platformInfo);
    if (dom.prereqDownloadBtn) dom.prereqDownloadBtn.disabled = false;
    showToast('Binary download failed', 'error', (err.message || 'Unknown error').split('\n')[0]);
  }
}
