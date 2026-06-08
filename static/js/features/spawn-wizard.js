// ── Spawn Wizard Module ───────────────────────────────────────────────────────
// Spawn Llama-Server V2 — complete guided wizard.
//
// Key features:
//  - Use-case selector (agentic / general / roleplay)
//  - Pre-download quant advisor with size + max-context table
//  - Architecture-aware VRAM breakdown (live animated bar)
//  - Context fit modes that translate KV cache precision into plain-language outcomes
//  - MoE expert offload slider with live feedback
//  - Auto-size button pulls backend recommendation
//  - Step validation before advancing

import { openDeferredFileBrowser, openChatTemplateLibraryBrowser, uploadChatTemplateFromBrowser } from './file-browser-launcher.js';
import { showToast } from './toast.js';
import { switchView } from './setup-view.js';
import { setTuneConfig, showTunePanel } from './tune-panel.js';
import { renderSuggestionCards, suggestionPatch, requestNcpuMoeTune, requestDepthSweep, renderDepthSweep } from './tuning-cards.js';
import { setHeaderMode } from './attach-detach.js';
import { lastCapabilities, lastSystemMetrics } from '../core/app-state.js';
import {
  HF_DISCOVER_CATEGORIES,
  hfSearch,
  hfListFiles,
  hfStartDownload,
  hfPollDownload,
  hfCancelDownload,
  hfShowDownloadPanel,
  hfHideDownloadPanel,
  hfRenderDiscoverPills,
  hfLoadQuickPicks,
} from './hf-browse.js';

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
    const localHd = arch.headDim;
    const globalHd = arch.globalHeadDim || localHd;
    const window = arch.localAttnWindow;
    const gkv = arch.nKvHeads, lkv = arch.localKvHeads || 1;
    const gCtx = ctx * s;
    const lCtx = Math.min(ctx, window) * s;
    return (globalL * gkv * globalHd * gCtx * (k + v)) +
           (localL  * lkv * localHd * lCtx * (k + v));
  }
  return effectiveLayers * arch.nKvHeads * arch.headDim * ctx * s * (k + v);
}

function moeWeightSplit(modelBytes, arch, nCpuMoe) {
  // --n-cpu-moe N offloads the experts of N transformer layers, so the offload
  // fraction is N / (layer count) — not N / (experts per layer). Mirrors
  // moe_weight_split() in vram_estimator.rs.
  const moeLayers = arch.nLayers || 0;
  if (!arch.nExperts || moeLayers <= 0 || nCpuMoe <= 0) return { vram: modelBytes, ram: 0 };
  const cpuRatio = Math.min(nCpuMoe, moeLayers) / moeLayers;
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

  // Hybrid DeltaNet: only nAttnLayers use KV cache (not all nLayers).
  // kvBytes() already does this; mirror the same logic here so maxContext
  // and the VRAM bar agree on how many layers store KV.
  const effectiveLayers = (arch.nAttnLayers && arch.nAttnLayers < arch.nLayers)
    ? arch.nAttnLayers : arch.nLayers;

  // Standard full-attention: solve directly
  if (!arch.localAttnWindow) {
    const s  = Math.max(slots, 1);
    const kv = effectiveLayers * arch.nKvHeads * arch.headDim * s * (kvBpe(ctk) + kvBpe(ctv));
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

const STEP_LABELS = ['Profile', 'Model', 'Hardware', 'Summary', 'Parameters', 'Spawn'];

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
    mmprojHfRepo: '',    // HF repo owning the selected projector
    draftCandidates: [], // assistant/draft files detected near this model
    selectedDraftPath: '', // path of chosen assistant for MTP
    chatTemplatePath: null,  // local path to installed .jinja template (null = use embedded)
    chatTemplateMode: 'auto', // 'auto' | 'custom' | 'embedded'
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
    batchSize: 2048, ubatchSize: 2048,
    parallelSlots: 1,
    cacheTypeK: 'q8_0', cacheTypeV: 'q8_0',
    nCpuMoe: 0,
    tensorSplit: '',
    fitTarget: '',
    cacheRam: null,
    kvUnified: true,
    flashAttn: 'on',
    mlock: false,
    prio: null,
    threads: null,
    threadsBatch: null,
    // MTP
    mtpEnabled: true,
    mtpDraftNMax: 2,
    mtpDraftNMin: null,
    mtpDraftPMin: null,
    // Sampling (null = use llama-server default)
    temperature: null,
    topP: null,
    topK: null,
    minP: null,
    repeatPenalty: null,
    presencePenalty: null,
    maxTokens: null,
    seed: null,
    outputMode: '',
    enableThinking: null,
    preserveThinking: null,
    reasoningBudget: null,
    reasoningMode: null,
    reasoningBudgetMessage: null,
    grammar: '',
    jsonSchema: '',
    alias: '',
    extraArgs: '',
  },
  access: {
    port: 8001,
    bindHost: '127.0.0.1',
    apiKey: '',
  },
  vram: { available: 0 },
  spawn: { inFlight: false, error: '' },
  savedPresetId: null, // ID of preset saved from this wizard run (to avoid duplicates)
};

// ── DOM refs ──────────────────────────────────────────────────────────────────

let dom = {};
let pendingHardwareScrollReset = false;
let pendingHardwareScrollRestore = null;

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
  updateProfileHint();

  // HF discover pills
  const discoverPillsEl = document.getElementById('hf-discover-pills');
  const quickpicksEl = dom.hfQuickpicks;

  hfRenderDiscoverPills({
    container: discoverPillsEl,
    quickpicksContainer: quickpicksEl,
    onPillClick: (cat, pillEl) => {
      wizardState.hfBrowseAuthor = null;
      if (dom.hfRepoInput) dom.hfRepoInput.value = '';
      const sort = cat.params.query
        ? (dom.hfSortSelect?.value || cat.params.sort)
        : cat.params.sort;
      hfSearchForWizard({ ...cat.params, sort });
    },
  });

  // HF quick-picks
  hfLoadQuickPicks({
    container: quickpicksEl,
    discoverPillsContainerId: 'hf-discover-pills',
    onAuthorClick: (author) => {
      browseHfAuthor(author);
    },
  });

  // HF download panel buttons
  const dlPanel = document.getElementById('hf-download-panel');

  document.getElementById('hf-dlp-download-btn')?.addEventListener('click', () => {
    const { hfRepo, hfFile, mmprojHfFile, mmprojHfRepo } = wizardState.model;
    if (!hfRepo || !hfFile) return;
    hfStartDownload({
      repoId: hfRepo,
      filePath: hfFile,
      panelEl: dlPanel,
      onComplete: (downloadId, localPath) => {
        onHfDownloadComplete(downloadId, localPath);
      },
      onValidationError: (msg) => {
        showValidationError(msg);
      },
      onClearValidationError: () => {
        clearValidationError();
      },
    });

    // Companion mmproj download (bypasses cooldown)
    if (mmprojHfFile) {
      _startCompanionMmprojDownload(mmprojHfRepo || hfRepo, mmprojHfFile, hfFile);
    }
  });

  document.getElementById('hf-dlp-use-hf-btn')?.addEventListener('click', () => {
    hfHideDownloadPanel(dlPanel);
  });

  document.getElementById('hf-dlp-cancel-btn')?.addEventListener('click', () => {
    hfCancelDownload({
      downloadId: _dlCurrentId,
      panelEl: dlPanel,
    });
    _dlCurrentId = null;
  });

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
    if (dlPanel && dlPanel.style.display !== 'none') {
      const fname = (wizardState.model?.hfFile || '').split('/').pop();
      if (fname) hfShowDownloadPanel(dlPanel, fname);
    }
  });

  loadCommunityPicks();
}

function applyReducedMotion() {
  if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
    document.documentElement.classList.add('reduce-motion');
  }
}

export function openSpawnWizard(opts = {}) {
  if (!dom.overlay) return;
  resetWizardState();
  document.getElementById('models-modal')?.classList.remove('open');
  window.closePresetsPanel?.();
  dom.overlay.classList.add('open');
  refreshHfTokenState();

  // Check binary prereq every time wizard opens
  _checkBinaryPrereq();

  if (opts.localPath) {
    // Pre-load a local model path and jump straight to step 2 (model).
    wizardState.model.source = 'local';
    wizardState.model.path = opts.localPath;
    wizardState.model.hfRepo = '';
    wizardState.model.hfFile = '';
    wizardState.model.delivery = 'local_file';
    wizardState.model.localMeta = opts.localModel || null;
    if (dom.modelPathInput) dom.modelPathInput.value = opts.localPath;
    if (dom.hfRepoInput) dom.hfRepoInput.value = '';
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

// Clear all wizard state for a fresh start
function resetWizardState() {
  // Reset model state
  wizardState.model.source = '';
  wizardState.model.path = '';
  wizardState.model.hfRepo = '';
  wizardState.model.hfFile = '';
  wizardState.model.mmprojPath = '';
  wizardState.model.mmprojHfFile = '';
  wizardState.model.mmprojHfRepo = '';
  wizardState.model.originRepo = '';
  wizardState.model.originFile = '';
  wizardState.model.delivery = '';
  wizardState.model.cardUrl = '';
  wizardState.model.family = '';
  wizardState.model.paramB = 0;
  wizardState.model.modelBytes = 0;
  wizardState.model.nCtxTrain = 0;
  wizardState.model.chatTemplatePath = '';
  wizardState.model.localMeta = null;
  wizardState.model.mmprojFiles = [];
  wizardState.model.quantFiles = [];
  wizardState.model.hfTokenSet = false;

  // Reset hardware state
  wizardState.hardware.gpuLayers = '';
  wizardState.hardware.contextSize = 0;
  wizardState.hardware.batchSize = 0;
  wizardState.hardware.ubatchSize = 0;
  wizardState.hardware.parallelSlots = 1;
  wizardState.hardware.cacheTypeK = '';
  wizardState.hardware.cacheTypeV = '';
  wizardState.hardware.flashAttn = '';
  wizardState.hardware.kvUnified = false;
  wizardState.hardware.mlock = false;
  wizardState.hardware.prio = null;
  wizardState.hardware.nCpuMoe = 0;
  wizardState.hardware.tensorSplit = '';
  wizardState.hardware.fitTarget = null;
  wizardState.hardware.cacheRam = null;
  wizardState.hardware.temperature = null;
  wizardState.hardware.topP = null;
  wizardState.hardware.topK = null;
  wizardState.hardware.minP = null;
  wizardState.hardware.repeatPenalty = null;
  wizardState.hardware.presencePenalty = null;
  wizardState.hardware.maxTokens = null;
  wizardState.hardware.seed = null;
  wizardState.hardware.mtpEnabled = false;
  wizardState.hardware.mtpDraftNMax = 2;
  wizardState.hardware.enableThinking = null;
  wizardState.hardware.preserveThinking = null;
  wizardState.hardware.reasoningMode = null;
  wizardState.hardware.reasoningBudget = null;
  wizardState.hardware.reasoningBudgetMessage = null;
  wizardState.hardware.specType = '';
  wizardState.hardware.draftModelPath = '';
  wizardState.hardware.grammar = '';
  wizardState.hardware.jsonSchema = '';

  // Reset architecture
  wizardState.arch.nLayers = 0;
  wizardState.arch.nKvHeads = 0;
  wizardState.arch.headDim = 0;
  wizardState.arch.globalHeadDim = 0;
  wizardState.arch.nGlobalAttnLayers = 0;
  wizardState.arch.localAttnWindow = 0;
  wizardState.arch.localKvHeads = 1;
  wizardState.arch.nExperts = 0;
  wizardState.arch.nExpertsUsed = 0;
  wizardState.arch.expertFraction = 0.65;
  wizardState.arch.mtpDepth = 0;
  wizardState.arch.mmprojBytes = 0;
  wizardState.arch.linearAttnStateBytes = 0;
  wizardState.arch.isHybridAttn = false;

  // Reset UI state
  wizardState.currentStep = 0;
  wizardState.useCase = 'general';
  wizardState.profile = 'balanced';
  wizardState.mode = 'guided';
  wizardState.model.chatTemplateMode = 'auto';
  wizardState.access.port = 8001;
  wizardState.access.bindHost = '127.0.0.1';
  wizardState.access.apiKey = '';
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
  dom.footerHint = document.getElementById('wizard-footer-hint');

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
  dom.hfMinSize         = document.getElementById('spawn-hf-min-size');
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
  dom.ctxRailSummaryValue  = document.getElementById('ctx-rail-summary-value');
  dom.ctxRailSummaryStatus = document.getElementById('ctx-rail-summary-status');
  dom.ctxRailSummaryNote   = document.getElementById('ctx-rail-summary-note');
  dom.moeOffloadPanel   = document.getElementById('moe-offload-panel');
  dom.moeOffloadSlider  = document.getElementById('moe-offload-slider');
  dom.moeOffloadSubtitle= document.getElementById('moe-offload-subtitle');
  dom.moeOffloadHint    = document.getElementById('moe-offload-hint');
  dom.vramAutosizeBtn  = document.getElementById('vram-autosize-btn');
  dom.vramAutosizeNote = document.getElementById('vram-autosize-note');
  dom.vramPanelLabel   = document.getElementById('vram-panel-label');
  dom.metalLimitRow    = document.getElementById('metal-limit-row');
  dom.metalLimitText   = document.getElementById('metal-limit-text');
  dom.metalLimitBtn    = document.getElementById('metal-limit-btn');
  dom.ramPanel         = document.getElementById('ram-panel');
  dom.ramPanelTotal    = document.getElementById('ram-panel-total');
  dom.rSegUsed  = document.getElementById('rseg-used');
  dom.rSegMoe   = document.getElementById('rseg-moe');
  dom.rSegCram  = document.getElementById('rseg-cram');
  dom.rSegFree  = document.getElementById('rseg-free');
  dom.rLegUsed  = document.getElementById('rleg-used-label');
  dom.rLegMoeItem = document.getElementById('rleg-moe-item');
  dom.rLegMoe   = document.getElementById('rleg-moe-label');
  dom.rLegCram  = document.getElementById('rleg-cram-label');
  dom.rLegFree  = document.getElementById('rleg-free-label');


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
  dom.fitTargetInput     = document.getElementById('spawn-fit-target');

  // Legacy VRAM pill (kept for backward compat if HTML still has it)
  dom.vramEstimateText = document.getElementById('spawn-vram-estimate-text');
  dom.vramPill         = document.getElementById('spawn-vram-pill');
  dom.specTypeSelect     = document.getElementById('spawn-spec-type');
   dom.mtpAssistantSection = document.getElementById('hw-mtp-assistant-section');
   dom.mtpAssistantSelect  = document.getElementById('hw-mtp-assistant-select');
  dom.draftModelWrap     = document.getElementById('spawn-draft-model-wrap');
  dom.draftModelInput    = document.getElementById('spawn-draft-model');
  dom.specNgramWrap      = document.getElementById('spawn-spec-ngram-wrap');
  dom.specNgramSizeInput = document.getElementById('spawn-spec-ngram-size');
  dom.draftMinInput      = document.getElementById('spawn-draft-min');
  dom.draftMaxInput      = document.getElementById('spawn-draft-max');
  dom.kvUnifiedCheck  = document.getElementById('spawn-kv-unified');
  dom.kvUnifiedLabel  = dom.kvUnifiedCheck?.closest('label');
  dom.flashAttnSelect    = document.getElementById('spawn-flash-attn');
  dom.mlockCheck         = document.getElementById('spawn-mlock');
  dom.mlockLabel         = dom.mlockCheck?.closest('label');
  dom.prioSelect         = document.getElementById('spawn-prio');
  dom.threadsInput       = document.getElementById('spawn-threads');
  dom.threadsBatchInput  = document.getElementById('spawn-threads-batch');
  dom.specDraftNMinInput = document.getElementById('spawn-spec-draft-n-min');
  dom.specDraftPMinInput = document.getElementById('spawn-spec-draft-p-min');
  dom.fitEnableCheck  = document.getElementById('spawn-fit-enable');
  dom.fitEnableLabel  = dom.fitEnableCheck?.closest('label');
  dom.fitTargetWrap   = document.getElementById('spawn-fit-target-wrap');
  dom.cacheRamInput   = document.getElementById('spawn-cache-ram');

  // Step 4 (Summary)
  dom.summaryList      = document.getElementById('spawn-summary-list');
  dom.summaryWarnings  = document.getElementById('spawn-summary-warnings');
  dom.topKInput        = document.getElementById('spawn-top-k');
  dom.maxTokensInput   = document.getElementById('spawn-max-tokens');
  dom.outputModeSelect = document.getElementById('spawn-output-mode');
  dom.grammarWrap      = document.getElementById('spawn-grammar-wrap');
  dom.grammarInput     = document.getElementById('spawn-grammar');
  dom.jsonSchemaWrap   = document.getElementById('spawn-json-schema-wrap');
  dom.jsonSchemaInput  = document.getElementById('spawn-json-schema');
  // Step 5 (Preset Parameters)
  dom.presetParamsTable  = document.getElementById('preset-params-table');
  dom.savePresetBtn      = document.getElementById('spawn-save-preset-btn');
  dom.savedPresetName    = document.getElementById('spawn-saved-preset-name');
  dom.presetNameInput    = document.getElementById('spawn-preset-name-input');
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
   bindCtxQuickPicks();
   bindSectionToggles();
   document.addEventListener('keydown', e => {
    if (!dom.overlay?.classList.contains('open')) return;
    if (e.key === 'Escape') {
      // Close any open browse dropdown first; only close wizard if none were open
      const anyOpen = ['spawn-browse-dropdown', 'spawn-import-browse-dropdown']
        .some(id => document.getElementById(id)?.style.display !== 'none');
      if (anyOpen) { _closeBrowseDropdowns(); return; }
      closeSpawnWizard();
    }
  });

  // Close browse dropdowns when clicking outside them
  document.addEventListener('click', e => {
    if (!e.target.closest('.browse-split')) _closeBrowseDropdowns();
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

  // Profile cards (segmented control)
  dom.profileCards?.forEach(card => {
    card.setAttribute('tabindex', '0'); card.setAttribute('role', 'button');
    card.addEventListener('click', () => {
      wizardState.profile = card.dataset.profile;
      dom.profileCards.forEach(c => c.classList.remove('selected'));
      card.classList.add('selected');
      updateProfileHint();
      persistProfile(); applyProfileVisibility();
      refreshStepGuardrails();
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
      refreshStepGuardrails();
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
      if (card.dataset.source !== 'hf') hfHideDownloadPanel(document.getElementById('hf-download-panel'));
      updateModelInputVisibility();
      renderLocalModelHint();
      clearValidationError();
      if (card.dataset.source === 'import') loadThirdPartyModels();
      refreshStepGuardrails();
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
    openDeferredFileBrowser('spawn-model-path', 'gguf', defaultPath, 'model');
  });
  dom.importBrowseBtn?.addEventListener('click', () => openDeferredFileBrowser('spawn-import-path', 'gguf'));

  dom.modelPathInput?.addEventListener('input', () => {
    wizardState.model.path = dom.modelPathInput.value.trim();
    wizardState.model.source = 'local';
    wizardState.model.delivery = 'local_file';
    if (wizardState.model.localMeta?.path && wizardState.model.localMeta.path !== wizardState.model.path) {
      wizardState.model.localMeta = null;
    }
    wizardState.model.quantFiles = [];
    wizardState.model._quantSwapRepo = '';
    _lastQuantSearchFile = '';
    _tagsRowOrigin = '';
    _originResolverPromise = null; // reset in-flight resolver
    onModelPathChanged();
    renderLocalModelHint();
    // Start origin resolver immediately so autoInstallChatTemplate can await it.
    // loadLocalModel also triggers one, but _autoResolveHfOrigin checks for
    // originRepo and is idempotent — the first call wins, second is a no-op.
    if (!wizardState.model.originRepo) {
      _originResolverPromise = _autoResolveHfOrigin();
    }
  });

  dom.importPathInput?.addEventListener('input', () => {
    wizardState.model.path = dom.importPathInput.value.trim();
    wizardState.model.source = 'import';
    wizardState.model.delivery = 'imported_local';
    wizardState.model.localMeta = null;
    _originResolverPromise = null; // reset in-flight resolver
    onModelPathChanged();
    renderLocalModelHint();
    // Start origin resolver immediately so autoInstallChatTemplate can await it.
    if (!wizardState.model.originRepo) {
      _originResolverPromise = _autoResolveHfOrigin();
    }
  });

  dom.hfRepoInput?.addEventListener('input', () => {
    wizardState.model.hfRepo = dom.hfRepoInput.value.trim();
    if (!wizardState.model.hfRepo) wizardState.model.hfFile = '';
    refreshStepGuardrails();
  });
  dom.hfRepoInput?.addEventListener('blur', () => triggerHfFileFetch());
  dom.hfRepoInput?.addEventListener('keydown', e => {
    if (e.key === 'Enter') { e.preventDefault(); triggerHfFileFetch(); }
  });

  document.getElementById('spawn-chat-template-path')?.addEventListener('input', (e) => {
    const value = e.target.value.trim();
    if (!value) return;
    _applyCustomChatTemplate(value);
  });

  // Hardware fields
  [
    dom.gpuLayersSelect, dom.gpuLayersManualInput, dom.contextSizeInput,
    dom.batchSizeInput, dom.ubatchSizeInput, dom.parallelSlotsInput,
    dom.cacheTypeKSelect, dom.cacheTypeVSelect, dom.nCpuMoeInput,
    dom.tensorSplitInput, dom.specTypeSelect, dom.draftModelInput,
    dom.kvUnifiedCheck, dom.flashAttnSelect, dom.mlockCheck, dom.prioSelect,
    dom.threadsInput, dom.threadsBatchInput,
    dom.fitEnableCheck, dom.fitTargetInput, dom.cacheRamInput,
    dom.specDraftNMinInput, dom.specDraftPMinInput,
  ].forEach(el => {
    el?.addEventListener('input', onHardwareChange);
    el?.addEventListener('change', onHardwareChange);
  });

  // mmproj "Browse" button: open file browser for mmproj projectors
  const mmprojBrowseBtn = document.querySelector('#hw-mmproj-browse-btn');
  if (mmprojBrowseBtn) {
    mmprojBrowseBtn.addEventListener('click', () => {
      const row = document.getElementById('hw-mmproj-row');
      const select = document.getElementById('hw-mmproj-select');
      const hfPanel = row?.querySelector('.hw-mmproj-hf-panel');
      // When HF download panel is shown, clear it and restore the select
      if (hfPanel) hfPanel.remove();
      if (select && select.style.display === 'none') select.style.display = '';
      openDeferredFileBrowser('hw-mmproj-select', 'gguf', '', 'mmproj');
    });
  }

  // Ensure mmproj select always updates wizardState on change, even when
  // file was chosen via Browse (and no auto-detected local mmprojs existed).
  const mmprojSelect = document.getElementById('hw-mmproj-select');
  if (mmprojSelect && !mmprojSelect.dataset.boundGlobal) {
    mmprojSelect.dataset.boundGlobal = '1';
    mmprojSelect.addEventListener('change', () => {
      const fpath = mmprojSelect.value || '';
      wizardState.model.mmprojPath = fpath || '';
      wizardState.model.mmprojHfFile = fpath || '';
      wizardState.arch.mmprojBytes = 0;
      scheduleVramUpdate();
    });
  }

  // draft-model "Browse" button: open file browser for draft model
  const draftBrowseBtn = document.querySelector('#spawn-draft-browse-btn');
  if (draftBrowseBtn) {
    draftBrowseBtn.addEventListener('click', () => {
      openDeferredFileBrowser('spawn-draft-model', 'gguf', '', 'draft-model');
    });
  }

  bindHardwareToggleSwitch(dom.kvUnifiedLabel, dom.kvUnifiedCheck);
  bindHardwareToggleSwitch(dom.mlockLabel, dom.mlockCheck);
  bindHardwareToggleSwitch(dom.fitEnableLabel, dom.fitEnableCheck);

  dom.gpuLayersSelect?.addEventListener('change', () => {
    wizardState.hardware.gpuLayers = dom.gpuLayersSelect.value;
    if (dom.gpuLayersManualWrap) dom.gpuLayersManualWrap.style.display = dom.gpuLayersSelect.value === 'manual' ? '' : 'none';
    refreshStepGuardrails();
  });
  dom.specTypeSelect?.addEventListener('change', () => {
    const v = dom.specTypeSelect.value;
    const isNgram = v && (v.includes('ngram') || v === 'ngram');
    const isDraftMtp = v && v.includes('draft-mtp');
    const isDraftModel = v === 'draft-model';

    // Show/hide draft-model path input - now visible for both draft-model and MTP modes
    // (since MTP may require a separate draft model file)
    if (dom.draftModelWrap) {
      dom.draftModelWrap.style.display = (isDraftModel || isDraftMtp) ? '' : 'none';
    }

    // For draft-mtp: ensure MTP assistant section is visible and auto-populate
    // from candidates if not already set.
    if (isDraftMtp) {
      renderMtpSection();
      // Also auto-populate the draft model input from candidates
      const candidates = wizardState.model.draftCandidates || [];
      const existing = (dom.draftModelInput?.value || '').trim();
      if (!existing && candidates.length > 0) {
        const best = _bestDraftForModel(
          (wizardState.model.path || wizardState.model.hfFile || '').split(/[\\/]/).pop() || '',
          candidates,
        );
        if (best && dom.draftModelInput) {
          dom.draftModelInput.value = best.path;
        }
      }
      // Sync from selectedDraftPath if set (e.g. from MTP assistant dropdown)
      const selectedPath = wizardState.model.selectedDraftPath || '';
      if (selectedPath && dom.draftModelInput && !dom.draftModelInput.value) {
        dom.draftModelInput.value = selectedPath;
      }
    }

    // Auto-populate draft-model input when mode matches and candidates exist
    if (isDraftModel) {
      const candidates = wizardState.model.draftCandidates || [];
      const existing = (dom.draftModelInput?.value || '').trim();
      if (!existing && candidates.length > 0) {
        const best = _bestDraftForModel(
          (wizardState.model.path || wizardState.model.hfFile || '').split(/[\\/]/).pop() || '',
          candidates,
        );
        if (best && dom.draftModelInput) {
          dom.draftModelInput.value = best.path;
        }
      }
    }

    if (dom.specNgramWrap) dom.specNgramWrap.style.display = isNgram ? '' : 'none';
    _updateSpecHint(v);
    refreshStepGuardrails();
  });

  // MoE slider
  dom.moeOffloadSlider?.addEventListener('input', () => {
    wizardState.hardware.nCpuMoe = Number(dom.moeOffloadSlider.value);
    if (dom.nCpuMoeInput) dom.nCpuMoeInput.value = wizardState.hardware.nCpuMoe;
    updateMoeSliderVisuals();
    scheduleVramUpdate();
  });

  // MoE offload auto-tuner
  document.getElementById('spawn-moe-autotune-btn')?.addEventListener('click', () => autoTuneWizard(false));
  document.getElementById('spawn-moe-autotune-verify')?.addEventListener('click', () => autoTuneWizard(true));

  // Depth sweep
  document.getElementById('wizard-depth-sweep-btn')?.addEventListener('click', runDepthSweep);

  // Auto-size button
  dom.vramAutosizeBtn?.addEventListener('click', triggerAutoSize);

  dom.savePresetBtn?.addEventListener('click', saveAsPreset);
  dom.spawnServerBtn?.addEventListener('click', spawnServer);



  // Sampling fields in review step
  _bindSamplingFields();
  dom.portInput?.addEventListener('input', () => {
    const parsed = parseInt(dom.portInput.value, 10);
    wizardState.access.port = Number.isFinite(parsed) && parsed > 0 ? parsed : 8001;
    if (wizardState.currentStep === 3) renderSummary();
    refreshStepGuardrails();
  });
  dom.bindHostSelect?.addEventListener('change', () => {
    wizardState.access.bindHost = dom.bindHostSelect.value || '127.0.0.1';
    if (wizardState.currentStep === 3) renderSummary();
    refreshStepGuardrails();
  });
  dom.apiKeyInput?.addEventListener('input', () => {
    wizardState.access.apiKey = (dom.apiKeyInput.value || '').trim();
    if (wizardState.currentStep === 3) renderSummary();
    refreshStepGuardrails();
  });

  // Hardware step quant swap (HF models and local-model quant discovery)
  document.getElementById('hw-quant-select')?.addEventListener('change', e => {
    const fpath = e.target.value;
    const qf = wizardState.model.quantFiles?.find(q => (q.path || q.name) === fpath);
    if (qf) {
      wizardState.model.hfFile = fpath;
      // Always reset modelBytes so getModelBytes() re-estimates from the new filename
      // if the file size is unknown — stale size from a different quant would corrupt the math.
      wizardState.model.modelBytes = Number(qf.size) || 0;
      if (detectMtpFromName(fpath) && !wizardState.arch.mtpDepth) {
        wizardState.arch.mtpDepth = 1;
        renderMtpSection();
      }
      scheduleVramUpdate();
      // For local models with quant-swap repo: show download / stream actions.
      const isLocalSwap = (wizardState.model.source === 'local' || wizardState.model.source === 'import')
        && wizardState.model._quantSwapRepo;
      if (isLocalSwap) {
        // Only show actions when the user picks a quant that differs from their current local file.
        const currentFile = (wizardState.model.path || '').split(/[\\/]/).pop() || '';
        const pickedFile = fpath.split('/').pop() || fpath;
        if (pickedFile.toLowerCase() !== currentFile.toLowerCase()) {
          _renderQuantSwapActions(fpath, wizardState.model._quantSwapRepo);
        } else {
          const actionsRow = document.getElementById('hw-quant-swap-actions');
          if (actionsRow) actionsRow.style.display = 'none';
        }
      } else {
        const panel = document.getElementById('hf-download-panel');
        if (panel && panel.style.display !== 'none') hfShowDownloadPanel(panel, fpath.split('/').pop());
      }
      refreshStepGuardrails();
    }
  });

  // Local-model quant swap trigger
  document.getElementById('hw-quant-local-btn')?.addEventListener('click', () => {
    _lastQuantSearchFile = ''; // reset cache so a re-click re-searches
    _autoDiscoverLocalModelQuants();
  });

  // Library tag picker trigger
  document.getElementById('hw-tags-add-btn')?.addEventListener('click', e => {
    _openHwTagPicker(
      e.currentTarget,
      wizardState.model.path || '',
      wizardState.model.originRepo || '',
    );
  });

  // Model card panel
  dom.cardPanelClose?.addEventListener('click', _closeCardPanel);

  // "Settings → Models" link inside the Import card description
  document.querySelector('.wizard-settings-link[data-open-settings="models"]')?.addEventListener('click', e => {
    e.preventDefault();
    window.openSettingsModal?.();
    setTimeout(() => document.querySelector('.settings-tab[data-tab="models"]')?.click(), 80);
  });

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
  refreshStepGuardrails();
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

function getStepGuardState(step = wizardState.currentStep) {
  const info = (message) => ({ canProceed: true, tone: 'info', message, focusEl: null });
  const error = (message, focusEl = null) => ({ canProceed: false, tone: 'error', message, focusEl });
  const warning = (message) => ({ canProceed: true, tone: 'warning', message, focusEl: null });

  if (step === 0) {
    return info('Choose a profile and use case. You can change both later before launching.');
  }

  if (step === 1) {
    const { source, path, hfRepo, hfFile } = wizardState.model;
    if (source === 'local') {
      return path
        ? info('Local model selected. Continue to tune hardware and context settings.')
        : error('Choose a local GGUF file to continue.', dom.modelPathInput);
    }
    if (source === 'import') {
      return path
        ? info('Imported model selected. Continue to tune hardware and context settings.')
        : error('Pick an imported model or paste a GGUF path to continue.', dom.importPathInput);
    }
    if (!hfRepo) {
      return error('Enter a Hugging Face repo ID or pick a discover result to continue.', dom.hfRepoInput);
    }
    if (!hfFile) {
      return error('Choose a GGUF file from the selected Hugging Face repo to continue.', dom.hfFileList || dom.hfRepoInput);
    }
    return info('Hugging Face model selected. Continue to review its hardware fit.');
  }

  if (step === 2) {
    if (wizardState.hardware.gpuLayers === 'manual' && wizardState.hardware.gpuLayersManual == null) {
      return error('Enter a GPU layer count or switch GPU layers back to Auto.', dom.gpuLayersManualInput);
    }
    if (dom.fitEnableCheck?.checked && !dom.fitTargetInput?.value.trim()) {
      return error('Enter a fit target in MB or turn Auto-fit context to memory off.', dom.fitTargetInput);
    }
    // Validate draft model input when shown (for draft-model or MTP modes with external assistant)
    const specType = dom.specTypeSelect?.value || '';
    const showDraftInput = specType === 'draft-model' || specType.includes('draft-mtp');
    if (showDraftInput && !dom.draftModelInput?.value.trim()) {
      return error('Enter a draft model path for speculative decoding.', dom.draftModelInput);
    }
    return info('Review the VRAM estimate and adjust context, KV cache, or auto-size before continuing.');
  }

  if (step === 3) {
    if (wizardState.access.bindHost === '0.0.0.0' && !wizardState.access.apiKey) {
      return warning('This server will be LAN-visible without an API key. Add one unless you intentionally want an open endpoint.');
    }
    return info('Review defaults and network exposure. Continue when this matches how you want the server to start.');
  }

  if (step === 4) {
    return info('Saving a preset is optional. Click Next when you are ready to launch.');
  }

  if (step === 5) {
    return info('Spawn starts the server with the configuration shown above.');
  }

  return info('');
}

function refreshStepGuardrails() {
  const state = getStepGuardState();
  if (dom.nextBtn) {
    const isFinalStep = wizardState.currentStep >= STEP_LABELS.length - 1;
    dom.nextBtn.disabled = !isFinalStep && !state.canProceed;
    dom.nextBtn.title = !state.canProceed ? state.message : '';
    dom.nextBtn.setAttribute('aria-disabled', String(dom.nextBtn.disabled));
  }
  if (dom.footerHint) {
    dom.footerHint.textContent = state.message || '';
    dom.footerHint.classList.remove('is-warning', 'is-error');
    if (state.tone === 'warning') dom.footerHint.classList.add('is-warning');
    if (state.tone === 'error') dom.footerHint.classList.add('is-error');
  }
}

// ── Validation ────────────────────────────────────────────────────────────────

function validateStep(step) {
  const state = getStepGuardState(step);
  if (!state.canProceed) {
    showValidationError(state.message, state.focusEl);
    refreshStepGuardrails();
    return false;
  }
  clearValidationError();
  refreshStepGuardrails();
  return true;
}

function showValidationError(msg, focusEl = null) {
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
  focusEl?.focus?.();
  el.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
}

function clearValidationError() {
  dom.overlay?.querySelectorAll('.wizard-validation-error').forEach(el => { el.style.display = 'none'; });
  refreshStepGuardrails();
}

// ── HF download panel (wizard-specific wrappers) ─────────────────────────────

let _dlCurrentId = null;
let _mmprojCompanionId = null;
let _mmprojCompanionLocalPath = null;
let _originResolverPromise = null; // in-flight origin resolver to prevent double-fire

function getAuthHeaders() {
  return window.authHeaders ? window.authHeaders() : {};
}

function getRecommendedQuant(vramGb) {
  if (vramGb < 8)  return 'Q4_K_M';
  if (vramGb <= 16) return 'Q5_K_M';
  if (vramGb <= 24) return 'Q5_K_M';
  return 'Q8_0';
}

// Attach HF origin + family tags for a local model (used by auto-resolve and
// the suggestion picker).  Replaces any stale origin/family tags.
async function _attachOriginTags(localPath, repoId, family) {
  if (!localPath || !repoId) return;
  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const getResp = await fetch('/api/models/tags', { headers });
    const existing = getResp.ok
      ? ((await getResp.json().catch(() => ({}))).tags?.[localPath] || [])
      : [];
    const merged = [
      ...existing.filter(t => !t.startsWith('hf_origin:') && !t.startsWith('family:')),
      `hf_origin:${repoId}`,
    ];
    if (family) merged.push(`family:${family}`);
    await fetch('/api/models/tags', {
      method: 'PUT',
      headers,
      body: JSON.stringify({ model_path: localPath, tags: merged }),
    });
  } catch { /* non-fatal */ }
}

// Auto-resolve the HF origin of a local model from its filename.
// Fires silently; if confident, sets originRepo and family, persists tags,
// and refreshes the UI.  If ambiguous, shows a suggestion row.
// Returns a promise so callers can await the resolution instead of polling.
// The module-level _originResolverPromise prevents double-fire.
async function _autoResolveHfOrigin() {
  const { source, path, modelBytes } = wizardState.model;
  if (source !== 'local' && source !== 'import') return;
  if (wizardState.model.originRepo) return; // already known
  const filename = (path || '').split(/[\\/]/).pop() || '';
  if (!filename || filename.length < 8) return;

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const res = await fetch('/api/hf/resolve-origin', {
      method: 'POST',
      headers,
      body: JSON.stringify({ filename, size_bytes: modelBytes || 0 }),
    });
    if (!res.ok) return;
    const data = await res.json();
    if (!data.ok || !data.candidates || !data.candidates.length) return;

    if (data.confident) {
       const top = data.candidates[0];
       wizardState.model.originRepo = top.repoId;
       wizardState.model.family = top.family || '';
       wizardState.model.cardUrl = top.cardUrl || `https://huggingface.co/${top.repoId}`;
       await _attachOriginTags(path, top.repoId, top.family);
       _tagsRowOrigin = '';
       _refreshHwTagsRow();
       _removeInlineOriginHint();
     } else if (data.candidates && data.candidates.length > 0) {
       // Show a compact "Suggested HF origin" hint next to the repo line
       _showInlineOriginHint(data.candidates.slice(0, 2), path);
     }
  } catch { /* non-fatal — resolution is a nice-to-have */ }
}

// Infer model family slug from a name string (mirrors backend heuristics).
function _inferFamilyFromName(name) {
  const lower = name.toLowerCase();
  if (lower.includes('qwen3.6') || lower.includes('qwen3_6') || lower.includes('qwen36')) return 'qwen3.6';
  if (lower.includes('qwen3.5') || lower.includes('qwen3_5') || lower.includes('qwen35')) return 'qwen3.5';
  if (lower.includes('qwen3') || lower.includes('qwen-3') || lower.includes('qwen_3')) return 'qwen3';
  if (lower.includes('qwen2.5') || lower.includes('qwen2_5') || lower.includes('qwen25')) return 'qwen2.5';
  if (lower.includes('qwen')) return 'qwen';
  if (lower.includes('gemma-4') || lower.includes('gemma_4') || lower.includes('gemma4')) return 'gemma4';
  if (lower.includes('gemma-3') || lower.includes('gemma_3') || lower.includes('gemma3')) return 'gemma3';
  if (lower.includes('gemma-2') || lower.includes('gemma_2') || lower.includes('gemma2')) return 'gemma2';
  if (lower.includes('gemma')) return 'gemma';
  if (lower.includes('llama-3.3') || lower.includes('llama3_3') || lower.includes('llama33')) return 'llama3.3';
  if (lower.includes('llama-3.1') || lower.includes('llama3_1') || lower.includes('llama31')) return 'llama3.1';
  if (lower.includes('llama-3') || lower.includes('llama3')) return 'llama3';
  if (lower.includes('llama')) return 'llama';
  if (lower.includes('mistral-large')) return 'mistral-large';
  if (lower.includes('mistral-nemo') || lower.includes('nemo')) return 'mistral-nemo';
  if (lower.includes('mistral') || lower.includes('mixtral')) return 'mistral';
  if (lower.includes('deepseek')) return 'deepseek';
  if (lower.includes('phi')) return 'phi';
  return '';
}

// Show a compact "Suggested HF origin:" hint inline next to the repo line.
// Used only when origin is not confident; hidden once user picks an origin.
function _showInlineOriginHint(candidates, modelPath) {
  _removeInlineOriginHint();
  const repoEl = document.getElementById('hw-model-repo');
  if (!repoEl) return;

  const hint = document.createElement('span');
  hint.id = 'hw-inline-origin-hint';
  hint.style.cssText =
    'font-size:8px;color:var(--color-text-muted);margin-left:6px;white-space:nowrap;';

  const label = document.createElement('span');
  label.textContent = 'Suggested HF origin: ';
  hint.appendChild(label);

  candidates.slice(0, 2).forEach((c) => {
    const pill = document.createElement('button');
    pill.type = 'button';
    pill.className = 'btn-wizard-tertiary';
    pill.style.cssText =
      'font-size:8px;min-height:16px;padding:0 4px;cursor:pointer;border-radius:2px;' +
      'border:1px solid rgba(148,163,253,0.25);' +
      'background:rgba(80,120,200,0.15);color:var(--color-text-primary);white-space:nowrap;';
    pill.textContent = (c.repoId || '').split('/').slice(-1)[0];
    pill.title = `${c.repoId}\n${c.reason || ''}\n${c.family ? 'Family: ' + c.family : ''}${c.cardUrl ? '\n' + c.cardUrl : ''}`;

    pill.addEventListener('click', () => {
      // Open pencil editor with this candidate as currentRepo
      const current = wizardState.model.originRepo || c.repoId || '';
      _openHwRepoEditor(repoEl, current || c.repoId);
    });

    hint.appendChild(document.createTextNode(' '));
    hint.appendChild(pill);
  });

  repoEl.appendChild(hint);
}

function _removeInlineOriginHint() {
  document.getElementById('hw-inline-origin-hint')?.remove();
}

// Called by hfStartDownload onComplete when download finishes.
function onHfDownloadComplete(downloadId, localPath) {
  const effectivePath = localPath;
  if (!effectivePath) return;

  const downloadedFile = wizardState.model.hfFile || '';
  const downloadedRepo = wizardState.model.hfRepo || '';

  wizardState.model.source = 'local';
  wizardState.model.delivery = 'downloaded_hf';
  wizardState.model.path = effectivePath;
  wizardState.model.originRepo = downloadedRepo;
  wizardState.model.originFile = downloadedFile;
  // Persist origin so future wizard sessions skip the HF search entirely.
  _attachOriginTags(effectivePath, downloadedRepo);
  wizardState.model.localMeta = {
    path: effectivePath,
    filename: effectivePath.split(/[\\/]/).pop() || effectivePath,
    size_display: wizardState.model.modelBytes ? formatBytes(wizardState.model.modelBytes) : '',
    quant_type: guessQuantFromName(downloadedFile || effectivePath),
    param_b: wizardState.model.paramB || null,
  };

  // Companion mmproj
  if (_mmprojCompanionLocalPath) {
    const mmprojLocalPath = _mmprojCompanionLocalPath;
    const mmprojName = mmprojLocalPath.split(/[\\/]/).pop() || mmprojLocalPath;
    wizardState.model.mmprojPath = mmprojLocalPath;
    wizardState.model.mmprojFiles = [{
      path: mmprojLocalPath, name: mmprojName,
      size: wizardState.arch.mmprojBytes || 0, is_mmproj: true,
    }];
    _mmprojCompanionId = null;
    _mmprojCompanionLocalPath = null;
  }

  wizardState.model.hfRepo = '';
  wizardState.model.hfFile = '';
  if (dom.modelPathInput) dom.modelPathInput.value = effectivePath;
  dom.modelSourceCards?.forEach(c => {
    c.classList.toggle('selected', c.dataset.source === 'local');
  });
  updateModelInputVisibility();
  updateSelectedModelDisplay();
  renderLocalModelHint();
  refreshStepGuardrails();
}

async function _startCompanionMmprojDownload(repo, mmprojHfPath, modelHfPath) {
  const saveAs = _deriveMmprojSaveName(modelHfPath, mmprojHfPath);
  try {
    const headers = { ...getAuthHeaders(), 'Content-Type': 'application/json' };
    const res = await fetch('/api/hf/download', {
      method: 'POST',
      headers,
      body: JSON.stringify({ repo_id: repo, file_path: mmprojHfPath, save_as: saveAs, companion: true, resume: true }),
    });
    const data = await res.json().catch(() => ({}));
    if (res.ok && data.ok) {
      _mmprojCompanionId = data.download_id;
      _mmprojCompanionLocalPath = data.local_path;
    }
  } catch { /* companion download failure is non-fatal */ }
}

// Wrapper for hfSearch used by wizard (wires callbacks)
function hfSearchForWizard({ query, author, sort, limit }) {
  if (!wizardState.model.hfTokenSet) {
    showValidationError('HuggingFace token not set. Set it in the top-right panel to search for models.');
    return;
  }
  const minParamB = parseFloat(dom.hfMinSize?.value || '0') || 0;
  // When a size filter is active we need a large batch so enough results survive
  // the client-side filter. Without a filter, a smaller page is better UX.
  const effectiveLimit = limit ?? (minParamB > 0 ? 100 : 25);
  hfSearch({
    query,
    author,
    sort,
    limit: effectiveLimit,
    minParamB,
    container: dom.hfSearchResults,
    filelistContainer: dom.hfFileList,
    quickpicksContainer: dom.hfQuickpicks,
    discoverPillsContainerId: 'hf-discover-pills',
    onOpenCardPanel: (repoId) => openCardPanel(repoId),
    onSelectModel: (m) => {
      wizardState.model.hfRepo = m.id;
      if (dom.hfRepoInput) dom.hfRepoInput.value = m.id;
      if (m.param_b > 0) wizardState.model.paramB = m.param_b;
      if (dom.hfSearchResults) dom.hfSearchResults.style.display = 'none';
      dom.hfQuickpicks?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active'));
      fetchHfFiles(m.id);
      if (m.param_b > 0) triggerQuantAdvisor();
      clearValidationError();
      refreshStepGuardrails();
    },
  });
}

// ── Mode toggle ───────────────────────────────────────────────────────────────

// ── Step management ───────────────────────────────────────────────────────────

function showStep(index) {
  wizardState.currentStep = index;
  clearValidationError();

  dom.steps?.forEach(s => s.classList.remove('active'));
  document.getElementById(`wizard-step-${index}`)?.classList.add('active');

  // wizard-body is always overflow:hidden flex — reset both column scroll
  // positions when switching steps so nothing is trapped off-screen.
  const newStep = document.getElementById('wizard-step-' + index);
  if (newStep) {
    const m = newStep.querySelector('.wizard-main');
    if (m) m.scrollTop = 0;
    const s = newStep.querySelector('.wizard-sidebar, .hw-vram-sidebar');
    if (s) s.scrollTop = 0;
  }

  dom.stepBadges?.forEach(b => {
    const s = Number(b.dataset.step);
    b.classList.remove('active', 'completed');
    if (s === index) b.classList.add('active');
    else if (s < index) b.classList.add('completed');
  });

  if (dom.stepLabel) dom.stepLabel.textContent = STEP_LABELS[index] || '';
  if (dom.backBtn) dom.backBtn.style.display = index === 0 ? 'none' : '';
  if (dom.nextBtn) dom.nextBtn.style.display = index === STEP_LABELS.length - 1 ? 'none' : '';

  if (index === 1) {
    _loadModelDirSwitcher();
  }
  if (index === 2) {
    // Entering hardware step — refresh VRAM, then render model context + new sections
    updateCtxModelMaxHint();
    Promise.all([fetchGpuVram(), fetchMetalGpuLimit(), fetchSystemRam()]).then(() => {
      scheduleVramUpdate();
      renderHardwareModelHeader();
      _populateKvCacheOptions();
    });
    // Auto-hint thread count from system P-core count (Apple Silicon)
    _refreshThreadsHint();
    // When no active session the WS doesn't broadcast system metrics — fetch directly.
    if (!lastSystemMetrics) {
      _fetchSystemInfoAndRefreshHints();
    }
    renderMmprojSection();
    renderMtpSection();
    _updateSpecHint(dom.specTypeSelect?.value || '');
    // Trigger download panel now (moved from file-select to hardware step entry)
    const dlPanel = document.getElementById('hf-download-panel');
    if (wizardState.model.source === 'hf' && wizardState.model.hfFile) {
      hfShowDownloadPanel(dlPanel, wizardState.model.hfFile);
    } else {
      hfHideDownloadPanel(dlPanel);
    }
    // Fetch model-specific sampling defaults (temperature, presence_penalty, etc.)
    // so the review step pre-populates with the model's recommended settings.
    _fetchAndApplyModelSamplingDefaults();
  }
  if (index === 3) {
    refreshHfTokenState().finally(() => {
      Promise.all([fetchGpuVram(), fetchMetalGpuLimit()]).then(() => estimateVramFull().then(() => renderSummary()));
    });
  }
  if (index === 4) {
    _renderPresetParamsStep();
  }
  if (index === 5) {
    _renderSpawnConfigCard();
  }
  refreshStepGuardrails();
}

// ── KV cache options from llama-server capabilities ──────────────────────────

function _populateKvCacheOptions() {
  const kSelect = dom.cacheTypeKSelect;
  const vSelect = dom.cacheTypeVSelect;
  if (!kSelect || !vSelect) return;

  // Use capabilities if available; otherwise fall back to safe defaults.
  const caps = lastCapabilities || {};
  const kvTypes = (caps.kv_cache_types || []).map(String);

  const baseOptions = [
    { value: 'q8_0', label: 'q8_0 — recommended' },
    { value: 'f16', label: 'f16 — lossless' },
    { value: 'bf16', label: 'bf16' },
    { value: 'q4_0', label: 'q4_0 — saves VRAM' },
    { value: 'q4_1', label: 'q4_1' },
    { value: 'iq4_nl', label: 'iq4_nl' },
    { value: 'q5_0', label: 'q5_0' },
    { value: 'q5_1', label: 'q5_1' },
    { value: 'f32', label: 'f32 — full precision' },
  ];

  // Build options: always include base, then add any extra from capabilities.
  const used = new Set(baseOptions.map(o => o.value));
  const allOptions = baseOptions.slice();
  for (const t of kvTypes) {
    if (!used.has(t)) {
      allOptions.push({ value: t, label: t });
      used.add(t);
    }
  }

  const fillSelect = (sel) => {
    const current = sel.value || 'q8_0';
    sel.innerHTML = '';
    for (const o of allOptions) {
      const opt = document.createElement('option');
      opt.value = o.value;
      opt.textContent = o.label;
      if (o.value === current) opt.selected = true;
      sel.appendChild(opt);
    }
    // Ensure a valid default if current is not in list
    if (!sel.querySelector('option:checked')) {
      const q8 = sel.querySelector('option[value="q8_0"]');
      if (q8) q8.selected = true;
    }
  };

  fillSelect(kSelect);
  fillSelect(vSelect);
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
const PROFILE_HINTS = {
  quick:    'Fully auto-tuned — we pick safe defaults based on your hardware. No knobs to turn.',
  balanced: 'Guided tuning with sensible defaults. Recommended for most setups.',
  advanced: 'Full control over all parameters, including MoE tuning, KV cache quant, and multi-GPU.',
};

function updateProfileHint() {
  const el = document.getElementById('profile-seg-hint');
  if (el) el.textContent = PROFILE_HINTS[wizardState.profile] ?? '';
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
  refreshStepGuardrails();
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
  // Allow both .gguf files and Ollama blobs (sha256-* content-addressed files)
  const lower = path.toLowerCase();
  if (!lower.endsWith('.gguf') && !lower.includes('/blobs/sha256-') && !lower.includes('\\blobs\\sha256-')) return;
  if (introspectDebounce) clearTimeout(introspectDebounce);
  introspectDebounce = setTimeout(() => doIntrospect(path), 1200);
}

async function doIntrospect(path) {
  try {
    const headers = window.authHeaders ? { ...window.authHeaders(), 'Content-Type': 'application/json' } : { 'Content-Type': 'application/json' };
    const resp = await fetch('/api/model/introspect', { method: 'POST', headers, body: JSON.stringify({ model_path: path }) });
    if (!resp.ok) return false;
    const data = await resp.json();
    if (!data.ok || !data.metadata) return false;

    const m = data.metadata;

    // Use the actual file size from disk — exact, no estimation needed.
    if (data.file_size_bytes > 0) {
      wizardState.model.modelBytes = data.file_size_bytes;
    }

    // Merge arch state from GGUF metadata
    if (m.n_layers)       wizardState.arch.nLayers      = m.n_layers;
    if (m.n_kv_heads)     wizardState.arch.nKvHeads     = m.n_kv_heads;
    if (m.head_dim)       wizardState.arch.headDim       = m.head_dim;
    if (m.n_experts)      wizardState.arch.nExperts      = m.n_experts;
    if (m.n_experts_used) wizardState.arch.nExpertsUsed = m.n_experts_used;
    if (m.mtp_depth)      wizardState.arch.mtpDepth      = m.mtp_depth;

    // Restore HF origin from persisted model tag so quant-swap can skip search.
    if (!wizardState.model.originRepo &&
        (wizardState.model.source === 'local' || wizardState.model.source === 'import')) {
      try {
        const tagsResp = await fetch('/api/models/tags', { headers });
        if (tagsResp.ok) {
          const td = await tagsResp.json().catch(() => ({}));
          const modelTags = td.tags?.[path] || [];
          const originTag = modelTags.find(t => t.startsWith('hf_origin:'));
          if (originTag) {
            wizardState.model.originRepo = originTag.slice('hf_origin:'.length);
            // Also restore family and card URL
            const familyTag = modelTags.find(t => t.startsWith('family:'));
            if (familyTag) wizardState.model.family = familyTag.slice('family:'.length);
            if (wizardState.model.originRepo) {
              wizardState.model.cardUrl = `https://huggingface.co/${wizardState.model.originRepo}`;
            }
          }
        }
      } catch { /* non-fatal */ }
    }

    // Auto-resolve HF origin for local models that lack a persisted origin tag.
    // The resolve endpoint already includes family detection (backend fetches
    // HF card tags in the same pass), so no separate _detectFamilyForOrigin needed.
    // Returns a promise so autoInstallChatTemplate can await the resolution.
    if (!wizardState.model.originRepo &&
        (wizardState.model.source === 'local' || wizardState.model.source === 'import')) {
      // Defer to avoid blocking the introspect flow; 200ms delay is imperceptible.
      _originResolverPromise = (async () => {
        await new Promise(r => setTimeout(r, 200));
        return _autoResolveHfOrigin();
      })();
    }

    // Scan directory for companion mmproj file (local models only)
    if (wizardState.model.source === 'local' || wizardState.model.source === 'import') {
      const dir = path.replace(/[/\\][^/\\]+$/, '');
      try {
        const browseResp = await fetch(
          `/api/browse?path=${encodeURIComponent(dir)}&filter=gguf`,
          { headers }
        );
        if (browseResp.ok) {
          const bd = await browseResp.json();
          const found = (bd.entries || []).filter(
            e => !e.is_dir && e.name.toLowerCase().includes('mmproj')
          );
          if (found.length) {
            wizardState.model.mmprojFiles = found.map(e => ({
              path: e.path, name: e.name, size: e.size || 0, is_mmproj: true,
            }));
            // Pick best-matching mmproj by name proximity; avoids auto-selecting
            // a companion file from a different model in the same directory.
            const modelFilename = path.split(/[\\/]/).pop() || '';
            const bestMmproj = _bestMmprojForModel(modelFilename, wizardState.model.mmprojFiles);
            if (bestMmproj) {
              wizardState.model.mmprojPath = bestMmproj.path;
              wizardState.model.mmprojHfFile = bestMmproj.path;
              wizardState.arch.mmprojBytes = bestMmproj.size || 0;
            }
            // Re-render the mmproj section if hardware step is active
            if (wizardState.currentStep === 2) renderMmprojSection();
            scheduleVramUpdate();
          }
        }
      } catch { /* browse may be rate-limited; skip silently */ }
    }

    // Scan directory for MTP assistant/draft model files (local models)
    if (wizardState.model.source === 'local' || wizardState.model.source === 'import') {
      const dir = path.replace(/[/\\][^/\\]+$/, '');
      try {
        const browseResp = await fetch(
          `/api/browse?path=${encodeURIComponent(dir)}&filter=gguf`,
          { headers }
        );
        if (browseResp.ok) {
          const bd = await browseResp.json();
          const assistants = (bd.entries || []).filter(e => {
            if (e.is_dir) return false;
            const n = e.name.toLowerCase();
            return n.includes('assistant')
              || n.includes('mtp-draft')
              || n.includes('draft-model')
              || n.includes('mtp_small')
              || n.includes('mtp-heads')
              || n.startsWith('mtp-');
          });
          if (assistants.length) {
            wizardState.model.draftCandidates = assistants.map(e => ({
              path: e.path,
              name: e.name,
              size: e.size || 0,
              is_draft: true,
            }));
            const modelFilename = path.split(/[\\/]/).pop() || '';
            const bestDraft = _bestDraftForModel(modelFilename, wizardState.model.draftCandidates);
            if (bestDraft) {
              wizardState.model.selectedDraftPath = bestDraft.path;
            }
            // Re-render MTP section to include assistant selector
            if (wizardState.currentStep === 2) renderMtpSection();
            scheduleVramUpdate();
          }
        }
      } catch { /* non-fatal */ }
    }

    // Update MoE slider max — n_cpu_moe counts layers, so the max is the layer count
    if (wizardState.arch.nExperts > 0 && dom.moeOffloadSlider) {
      dom.moeOffloadSlider.max = wizardState.arch.nLayers || wizardState.arch.nExperts;
    }

      // Store the model's training context ceiling for UX warnings
    if (m.n_ctx_train) {
      wizardState.model.nCtxTrain = m.n_ctx_train;
      updateCtxTrainWarning();
      updateCtxModelMaxHint();
    }

    scheduleVramUpdate();
    if (wizardState.model.paramB > 0) triggerQuantAdvisor();
    return true;
  } catch {
    return false;
  }
}

// ── GPU VRAM query ────────────────────────────────────────────────────────────

let cachedVram = 0;
let cachedRamTotal = 0;
let cachedRamUsed  = 0;
let cachedMetalGpuLimitMb = 0; // 0 = system default; >0 = custom iogpu.wired_limit_mb

// ── Unified memory helpers (Apple Silicon) ────────────────────────────────────

// True when the platform backend is Metal (Apple Silicon unified memory).
// On unified memory, GPU and system RAM are the same pool; VRAM == RAM.
function isUnifiedMemory() {
  return _platformInfo?.auto_backend === 'metal';
}

// On unified memory, the Metal cap IS the budget. macOS compresses other processes'
// pages to give Metal up to cap bytes — that's the purpose of the cap.
// The suggested cap (total − 8 GB) already reserves OS headroom, so we use the
// cap directly rather than constraining by current free RAM.
// On discrete GPU, the cached VRAM figure is the dedicated VRAM pool.
// macOS Metal GPU wired memory cap (default, without sysctl tweak):
//   ≤ 36 GB RAM → ~66% (2/3)  e.g. 24 GB → 16 GB
//   > 36 GB RAM → ~75% (3/4)  e.g. 64 GB → 48 GB, 128 GB → 96 GB
// If the user has applied iogpu.wired_limit_mb, that value overrides the default.
function metalCap(ramTotal) {
  if (cachedMetalGpuLimitMb > 0) {
    return cachedMetalGpuLimitMb * 1024 * 1024; // MiB → bytes
  }
  const fraction = ramTotal <= 36 * 1024 ** 3 ? 2 / 3 : 3 / 4;
  return Math.floor(ramTotal * fraction);
}

// Suggested iogpu.wired_limit_mb value for the user's system.
// Leaves 8 GB for macOS (safe minimum per llama.cpp community docs).
// Returns 0 if the suggestion would not improve over the current/default cap.
function suggestedMetalLimitMb(ramTotal) {
  const currentCap = metalCap(ramTotal);
  const suggested = Math.floor(ramTotal / (1024 * 1024)) - 8192; // total_MB - 8 GB
  return suggested > Math.floor(currentCap / (1024 * 1024)) ? suggested : 0;
}

// Metal driver initialization reserve (~512 MB).
// sysinfo::used_memory() on macOS already includes wire_count (kernel wired pages),
// so freeRam = total - used_memory already excludes wired kernel memory.
// The Metal cap handles the macro OS headroom (25–33% of RAM).
// This small reserve covers Metal driver startup allocations not yet reflected in
// the pre-launch snapshot (argument tables, shader cache, command buffer pools).
// Inference-time burst compute buffers are handled by computeHeadroom() separately.
const APPLE_OS_RESERVE_BYTES = 512 * 1024 * 1024;

// Discrete GPU headroom: 5% but capped at 1.5 GB — driver overhead is flat, not percentage-based
const DISCRETE_MAX_HEADROOM_BYTES = 1.5 * 1024 ** 3;

function computeHeadroom(availVram) {
  if (isUnifiedMemory()) {
    if (!availVram) return 0.10;
    // 10% base capped at 2 GB absolute — Metal burst compute buffers are flat, not percentage-based
    return Math.min(0.10, (2 * 1024 ** 3) / availVram);
  }
  if (!availVram) return 0.05;
  return Math.min(0.05, DISCRETE_MAX_HEADROOM_BYTES / availVram);
}

function effectiveAvailBytes() {
  if (isUnifiedMemory() && cachedRamTotal > 0) {
    const cap = metalCap(cachedRamTotal);
    // Use the Metal cap as the budget. The cap was configured to leave OS headroom
    // (default: 75% of RAM; suggested sysctl: total − 8 GB). macOS compresses
    // other processes' pages as needed to give Metal up to cap bytes — the snapshot
    // free-RAM figure understates what's actually available on unified memory.
    return Math.max(0, Math.min(cap, cachedRamTotal) - APPLE_OS_RESERVE_BYTES);
  }
  return cachedVram || wizardState.vram.available;
}

async function fetchSystemRam() {
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch('/metrics/system', { headers });
    if (!resp.ok) return;
    const d = await resp.json();
    cachedRamTotal = (d.ram_total_gb || 0) * 1024 * 1024 * 1024;
    cachedRamUsed  = (d.ram_used_gb  || 0) * 1024 * 1024 * 1024;
  } catch {}
}

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
      // Capture Metal GPU wired limit from Apple backend (0 = system default)
      if (g.metal_gpu_limit_mb !== undefined && g.metal_gpu_limit_mb !== null) {
        cachedMetalGpuLimitMb = g.metal_gpu_limit_mb;
      }
    }
    if (totalVram > 0) {
      cachedVram = totalVram;
      wizardState.vram.available = totalVram;
    }
  } catch {}
}

async function fetchMetalGpuLimit() {
  if (!isUnifiedMemory()) return;
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch('/api/system/metal-gpu-limit', { headers });
    if (!resp.ok) return;
    const data = await resp.json().catch(() => ({}));
    if (!data.ok) return;
    cachedMetalGpuLimitMb = Number(data.limit_mb || 0);
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

  const availVram = effectiveAvailBytes();
  if (!availVram) return; // need VRAM to give useful numbers

  // Show loading hint so user knows we're working
  if (dom.quantAdvisorSubtitle) {
    dom.quantAdvisorSubtitle.textContent = 'Analyzing model…';
  }
  if (dom.quantAdvisor) {
    dom.quantAdvisor.style.display = '';
  }

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };

    // Pass arch info if we have it
    const body = {
      param_b: paramB,
      model_name: wizardState.model.path || wizardState.model.hfRepo || '',
      available_vram_bytes: availVram,
      is_unified_memory: isUnifiedMemory(),
      use_case: wizardState.useCase,
      parallel_slots: wizardState.hardware.parallelSlots,
      n_layers: wizardState.arch.nLayers || undefined,
      n_kv_heads: wizardState.arch.nKvHeads || undefined,
      head_dim: wizardState.arch.headDim || undefined,
      global_head_dim: buildHeuristicArch(
        wizardState.model.path || wizardState.model.hfRepo || '',
        paramB,
      ).globalHeadDim || undefined,
      n_experts: wizardState.arch.nExperts || undefined,
      mtp_depth: wizardState.arch.mtpDepth || undefined,
    };

    const resp = await fetch('/api/vram/quant-compare', { method: 'POST', headers, body: JSON.stringify(body) });
    if (!resp.ok) {
      if (dom.quantAdvisorSubtitle) dom.quantAdvisorSubtitle.textContent = 'Failed to analyze model.';
      return;
    }
    const data = await resp.json();
    if (!data.ok || !data.quants) {
      if (dom.quantAdvisorSubtitle) dom.quantAdvisorSubtitle.textContent = 'Failed to analyze model.';
      return;
    }

    renderQuantAdvisor(data.quants, availVram);
  } catch {
    if (dom.quantAdvisorSubtitle) dom.quantAdvisorSubtitle.textContent = 'Failed to analyze model.';
  }
}

function renderQuantAdvisor(quants, availVram) {
  if (!dom.quantAdvisor || !dom.quantAdvisorTable) return;
  if (!quants || quants.length === 0) { dom.quantAdvisor.style.display = 'none'; return; }

  const availGb = Math.round(availVram / (1024 ** 3));
  const budgetLabel = isUnifiedMemory() ? 'Unified memory available' : 'VRAM available';

  // Context-aware pass: find the best quant that fits the user's context target.
  // Only annotate when the user has set a non-trivial context (> 8k default).
  const desiredCtx = wizardState.hardware?.contextSize || 0;
  const annotateCtx = desiredCtx > 8192;
  // First quant (highest quality) whose q8_0 KV max context meets the target.
  const ctxFitQuant = annotateCtx
    ? quants.find(q => q.fits_vram && q.max_ctx_q8 >= desiredCtx)
    : null;
  const qualityRecQuant = quants.find(q => q.recommended && q.fits_vram);
  // Does the quality recommendation also satisfy the context target?
  const qualityRecFitsCtx = !annotateCtx || (qualityRecQuant && qualityRecQuant.max_ctx_q8 >= desiredCtx);

  let subtitle = `${budgetLabel}: ${availGb} GB`;
  if (annotateCtx) subtitle += ` · Context target: ${formatCtx(desiredCtx)}`;
  if (dom.quantAdvisorSubtitle) dom.quantAdvisorSubtitle.textContent = subtitle;

  const table = document.createElement('table');
  table.className = 'qa-table';

  const thead = table.createTHead();
  const hrow = thead.insertRow();
  ['', 'Quant', 'Size', 'Max ctx (q8_0 KV)', 'Max ctx (q4_0 KV)', 'Quality'].forEach(h => {
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

    // Quant name + badges
    const nameTd = tr.insertCell();
    const nameSpan = document.createElement('span');
    nameSpan.style.fontWeight = '600';
    nameSpan.textContent = q.label;
    nameTd.appendChild(nameSpan);

    if (q.recommended) {
      const badge = document.createElement('span');
      badge.className = 'qa-badge-rec';
      // If context target is active and this rec won't meet it, clarify it's quality-only
      badge.textContent = (annotateCtx && !qualityRecFitsCtx) ? '★ Quality' : '★ Rec';
      badge.style.marginLeft = '6px';
      nameTd.appendChild(badge);
    }
    // Context recommendation badge — shown when it differs from the quality pick
    if (annotateCtx && ctxFitQuant && q.label === ctxFitQuant.label && !qualityRecFitsCtx) {
      const ctxBadge = document.createElement('span');
      ctxBadge.className = 'qa-badge-ctx';
      ctxBadge.textContent = `✓ fits ${formatCtx(desiredCtx)}`;
      ctxBadge.style.marginLeft = '6px';
      nameTd.appendChild(ctxBadge);
    }
    if (q.is_imatrix) {
      const im = document.createElement('span');
      im.style.cssText = 'margin-left:4px; font-size:10px; color:var(--color-text-muted);';
      im.textContent = 'imatrix';
      nameTd.appendChild(im);
    }

    // Size
    const sizeTd = tr.insertCell();
    sizeTd.textContent = q.model_size_gb.toFixed(1) + ' GB';
    sizeTd.style.color = 'var(--color-text-muted)';

    // Max ctx q8_0 — warn if below context target
    const ctxQ8Td = tr.insertCell();
    ctxQ8Td.className = 'qa-ctx';
    if (q.max_ctx_q8 > 0) {
      ctxQ8Td.textContent = formatCtx(q.max_ctx_q8);
      const underTarget = annotateCtx && q.max_ctx_q8 < desiredCtx;
      ctxQ8Td.classList.add(underTarget ? 'qa-ctx-under' : 'qa-ctx-q8');
      if (underTarget) ctxQ8Td.title = `Max ${formatCtx(q.max_ctx_q8)} — below your ${formatCtx(desiredCtx)} target`;
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

// Cache of installed community templates keyed by template name.
// Avoids re-downloading the same template for each model of the same family.
const _installedTemplateCache = {};

// Community template registry keyed by model family
const COMMUNITY_TEMPLATES = {
  qwen: {
    name: 'qwen-froggeric-fixed',
    display: "froggeric's Fixed Template",
    repo: 'froggeric/Qwen-Fixed-Chat-Templates',
    file: 'chat_template.jinja',
    description: 'Fixes tool calling, KV cache invalidation & agentic loop bugs for Qwen 3.5 / 3.6',
    hfUrl: 'https://huggingface.co/froggeric/Qwen-Fixed-Chat-Templates',
  },
};

function _chatTemplateDisplayName(path) {
  if (!path) return 'Embedded (from model file)';
  return path.split(/[\\/]/).pop() || path;
}

function _applyCustomChatTemplate(path) {
  wizardState.model.chatTemplatePath = path || null;
  wizardState.model.chatTemplateMode = path ? 'custom' : 'embedded';
  const hiddenInput = document.getElementById('spawn-chat-template-path');
  if (hiddenInput) hiddenInput.value = path || '';
  const identityName = wizardState.model.source === 'hf' ? wizardState.model.hfRepo : wizardState.model.path;
  const family = detectModelFamily(identityName);
  const tpl = family ? COMMUNITY_TEMPLATES[family] : null;
  _renderChatTemplateStatus(path ? 'custom' : 'embedded', family, tpl, { path });
}

function detectModelFamily(name) {
  const lower = (name || '').toLowerCase();
  if (lower.includes('qwen') || lower.includes('qwopus')) return 'qwen';
  if (lower.includes('llama-3') || lower.includes('llama3') || lower.match(/llama.?3/)) return 'llama3';
  if (lower.includes('gemma')) return 'gemma';
  if (lower.includes('mistral') || lower.includes('mixtral')) return 'mistral';
  return null;
}

// Map GGUF general.architecture values to community template family keys
// (e.g. "qwen3_6" → "qwen", "llama" → "llama3" if LLaMA 3+)
function _ggufArchToFamily(arch) {
  const a = arch.toLowerCase();
  if (a.includes('qwen')) return 'qwen';
  if (a.includes('gemma')) return 'gemma';
  if (a.includes('mistral') || a.includes('mixtral')) return 'mistral';
  if (a.includes('llama')) return 'llama3';
  return null;
}

// Async family detection that tries multiple sources:
// 1) Persisted family tag from model-tags.json
// 2) GGUF metadata general.architecture (for local models — reads file header, instant)
// 3) HF model card base_model tag (via /api/hf/meta)
// 4) Filename heuristics (as fallback)
async function detectModelFamilyAsync(identityName, localPath, timeoutMs) {
  const timeout = timeoutMs || 5000;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeout);
  const headers = window.authHeaders ? window.authHeaders() : {};

  // 1) Check persisted family tag
  if (localPath) {
    try {
      const resp = await fetch('/api/models/tags', { headers, signal: controller.signal });
      if (resp.ok) {
        const td = await resp.json().catch(() => ({}));
        const tags = td.tags?.[localPath] || [];
        const familyTag = tags.find(t => t.startsWith('family:'));
        if (familyTag) return familyTag.slice('family:'.length);
      }
    } catch (e) { if (e.name !== 'AbortError') { /* non-fatal */ } }
  }

  // 2) Read GGUF metadata for local models — architecture field is authoritative
  if (localPath) {
    try {
      const metaResp = await fetch('/api/models/gguf-meta', {
        method: 'POST',
        headers: { ...headers, 'Content-Type': 'application/json' },
        signal: controller.signal,
        body: JSON.stringify({ model_path: localPath }),
      });
      if (metaResp.ok) {
        const meta = await metaResp.json().catch(() => ({}));
        if (meta.ok && meta.architecture) {
          const arch = meta.architecture.toLowerCase();
          const family = _ggufArchToFamily(arch);
          if (family) return family;
        }
      }
    } catch (e) { if (e.name !== 'AbortError') { /* non-fatal */ } }
  }

  // 2) Check HF model card base_model tag (for locally resolved origins or HF repos)
  const repoId = identityName || wizardState.model.originRepo;
  // Only query HF API if repoId looks like an HF repo (not a local file path).
  // HF repos are "owner/name" — no dots before the slash, no backslashes.
  const looksLikeHfRepo = repoId && repoId.includes('/') &&
    !repoId.includes('\\') &&
    !repoId.split('/')[0].includes('.');
  if (looksLikeHfRepo) {
    try {
      const metaResp = await fetch(`/api/hf/meta?repo=${encodeURIComponent(repoId)}`, { headers, signal: controller.signal });
      if (metaResp.ok) {
        const meta = await metaResp.json().catch(() => ({}));
        const tags = meta.tags || [];
        const baseModelTag = tags.find(t => {
          if (!t.startsWith('base_model:')) return false;
          const rest = t.slice('base_model:'.length);
          return rest.includes('/');
        });
        if (baseModelTag) {
          const baseRepo = baseModelTag.slice('base_model:'.length);
          const detected = detectModelFamily(baseRepo);
          if (detected) return detected;
        }
      }
    } catch (e) { if (e.name !== 'AbortError') { /* non-fatal */ } }
  }

  clearTimeout(timer);

  // 3) Filename heuristics
  return detectModelFamily(identityName || localPath || '');
}

async function autoInstallChatTemplate() {
  const { source, path, hfRepo } = wizardState.model;
  const identityName = source === 'hf' ? hfRepo : path;

  // Fast path: family already known (from wizard state or filename)
  let family = wizardState.model.family || detectModelFamily(identityName);
  const tpl = family ? COMMUNITY_TEMPLATES[family] : null;

  // If no family from fast path, we need to detect it.
  // For local/import models, await the origin resolver first (it fires from
  // the model path input handler and includes family detection in the same pass).
  if (!family && (source === 'local' || source === 'import')) {
    _renderChatTemplateStatus('detecting', null, null, null);
    // Await the origin resolver promise (created from model path input handler).
    // The resolver is idempotent — the one from loadLocalModel will be a no-op.
    // 1.5s timeout is generous: the HF search takes ~500ms.
    const resolveTimeout = new Promise(r => setTimeout(r, 1500));
    await Promise.race([
      (_originResolverPromise || Promise.resolve()),
      resolveTimeout,
    ]);
    // After the resolver, check again (family may now be set by the resolver).
    family = wizardState.model.family || detectModelFamily(identityName);
  }
  // If still no family, query HF directly (for models not covered by origin resolver)
  if (!family) {
    try {
      family = await detectModelFamilyAsync(identityName, path, 8000);
    } catch { /* non-fatal */ }
  }
  // Update wizard state for future use
  if (family) wizardState.model.family = family;

  const tplForFamily = family ? COMMUNITY_TEMPLATES[family] : null;

  if (wizardState.model.chatTemplateMode === 'custom' && wizardState.model.chatTemplatePath) {
    _renderChatTemplateStatus('custom', family, tplForFamily, { path: wizardState.model.chatTemplatePath });
    return;
  }

  if (wizardState.model.chatTemplateMode === 'embedded') {
    wizardState.model.chatTemplatePath = null;
    _renderChatTemplateStatus('embedded', family, tplForFamily, null);
    return;
  }

  if (!tplForFamily) {
    wizardState.model.chatTemplatePath = null;
    wizardState.model.chatTemplateMode = 'auto';
    _renderChatTemplateStatus('embedded', family, null, null);
    return;
  }

  // Cache hit: template already installed for this family
  const cached = _installedTemplateCache[tplForFamily.name];
  if (cached) {
    wizardState.model.chatTemplatePath = cached;
    wizardState.model.chatTemplateMode = 'auto';
    _renderChatTemplateStatus('installed', family, tplForFamily, { path: cached, already_existed: true });
    return;
  }

  _renderChatTemplateStatus('installing', family, tplForFamily, null);

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const resp = await fetch('/api/chat-template/install-hf', {
      method: 'POST', headers,
      body: JSON.stringify({ repo: tplForFamily.repo, file: tplForFamily.file, name: tplForFamily.name }),
    });
    const data = resp.ok ? await resp.json() : { ok: false, error: `HTTP ${resp.status}` };
    if (data.ok && data.path) {
      wizardState.model.chatTemplatePath = data.path;
      wizardState.model.chatTemplateMode = 'auto';
      // Cache the template path for this family (avoids re-downloading for
      // other models of the same family in the same session)
      _installedTemplateCache[tplForFamily.name] = data.path;
      _renderChatTemplateStatus('installed', family, tplForFamily, data);
    } else {
      _renderChatTemplateStatus('error', family, tplForFamily, data);
    }
  } catch (err) {
    _renderChatTemplateStatus('error', family, tplForFamily, { error: err.message || String(err) });
  }
}

function _renderChatTemplateStatus(state, family, tpl, data) {
  const section = document.getElementById('chat-template-section');
  const statusEl = document.getElementById('ct-status');
  const bodyEl = document.getElementById('ct-body');
  const actionsEl = document.getElementById('ct-actions');
  if (!section) return;

  section.style.display = '';

  if (actionsEl) {
    actionsEl.innerHTML = '';

    if (tpl) {
      const recommendedBtn = document.createElement('button');
      recommendedBtn.type = 'button';
      recommendedBtn.className = 'btn-wizard-tertiary ct-action-btn';
      const isUsing = wizardState.model.chatTemplateMode === 'auto';
      const familyLabel = family ? ` (${family} family)` : '';
      recommendedBtn.textContent = isUsing ? `Using Recommended${familyLabel}` : `Use ${tpl.display}${familyLabel}`;
      recommendedBtn.disabled = wizardState.model.chatTemplateMode === 'auto';
      recommendedBtn.addEventListener('click', async () => {
        wizardState.model.chatTemplateMode = 'auto';
        await autoInstallChatTemplate();
      });
      actionsEl.appendChild(recommendedBtn);
    }

    const embeddedBtn = document.createElement('button');
    embeddedBtn.type = 'button';
    embeddedBtn.className = 'btn-wizard-tertiary ct-action-btn';
    embeddedBtn.textContent = 'Use Embedded';
    embeddedBtn.disabled = wizardState.model.chatTemplateMode === 'embedded' && !wizardState.model.chatTemplatePath;
    embeddedBtn.addEventListener('click', () => {
      _applyCustomChatTemplate(null);
    });
    actionsEl.appendChild(embeddedBtn);

    const libraryBtn = document.createElement('button');
    libraryBtn.type = 'button';
    libraryBtn.className = 'btn-wizard-tertiary ct-action-btn';
    libraryBtn.textContent = 'Choose Existing';
    libraryBtn.addEventListener('click', async () => {
      try {
        await openChatTemplateLibraryBrowser('spawn-chat-template-path');
      } catch (err) {
        showToast('Template library unavailable: ' + (err.message || String(err)), 'error');
      }
    });
    actionsEl.appendChild(libraryBtn);

    const uploadBtn = document.createElement('button');
    uploadBtn.type = 'button';
    uploadBtn.className = 'btn-wizard-tertiary ct-action-btn';
    uploadBtn.textContent = 'Upload .jinja';
    uploadBtn.addEventListener('click', async () => {
      try {
        const uploaded = await uploadChatTemplateFromBrowser();
        if (!uploaded?.path) return;
        _applyCustomChatTemplate(uploaded.path);
        showToast('Template uploaded', 'success', uploaded.filename || 'Saved to template library');
      } catch {
        // uploadChatTemplateFromBrowser already surfaced the error
      }
                 });
    actionsEl.appendChild(uploadBtn);

    // Force family override — lets user manually pick a family when auto-detection fails
    const forceFamilyWrap = document.createElement('div');
    forceFamilyWrap.className = 'ct-force-family-wrap';
    forceFamilyWrap.style.display = 'flex';
    forceFamilyWrap.style.alignItems = 'center';
    forceFamilyWrap.style.gap = '6px';
    forceFamilyWrap.style.marginTop = '6px';

    const forceFamilyLabel = document.createElement('span');
    forceFamilyLabel.style.fontSize = '10px';
    forceFamilyLabel.style.fontWeight = '600';
    forceFamilyLabel.style.color = 'var(--color-text-muted)';
    forceFamilyLabel.style.textTransform = 'uppercase';
    forceFamilyLabel.style.letterSpacing = '0.06em';
    forceFamilyLabel.textContent = 'Force family';

    const forceFamilySelect = document.createElement('select');
    forceFamilySelect.className = 'ct-force-family-select';
    forceFamilySelect.style.fontSize = '11px';
    forceFamilySelect.style.padding = '3px 6px';
    forceFamilySelect.style.borderRadius = '4px';
    forceFamilySelect.style.border = '1px solid rgba(99,102,241,0.2)';
    forceFamilySelect.style.background = 'var(--color-surface-elevated)';
    forceFamilySelect.style.color = 'var(--color-text)';
    forceFamilySelect.title = 'Override the auto-detected model family to force a specific chat template';

    const currentFamily = wizardState.model.family || '';
    const families = Object.keys(COMMUNITY_TEMPLATES);

    const autoOpt = document.createElement('option');
    autoOpt.value = '';
    autoOpt.textContent = 'auto-detect';
    if (!currentFamily) autoOpt.selected = true;

    const autoLabel = document.createElement('optgroup');
    autoLabel.label = 'Detection';
    autoLabel.appendChild(autoOpt);

    families.forEach(fam => {
      const tpl = COMMUNITY_TEMPLATES[fam];
      const opt = document.createElement('option');
      opt.value = fam;
      opt.textContent = `${fam} — ${tpl.display}`;
      if (currentFamily === fam) opt.selected = true;
      autoLabel.appendChild(opt);
    });

    forceFamilySelect.appendChild(autoLabel);
    forceFamilySelect.addEventListener('change', () => {
      const chosen = forceFamilySelect.value;
      wizardState.model.family = chosen || null;
      if (chosen && COMMUNITY_TEMPLATES[chosen]) {
        wizardState.model.chatTemplateMode = 'auto';
        autoInstallChatTemplate();
      }
    });

    forceFamilyWrap.appendChild(forceFamilyLabel);
    forceFamilyWrap.appendChild(forceFamilySelect);
    actionsEl.appendChild(forceFamilyWrap);
  }

  if (state === 'detecting') {
    const modelName = (wizardState.model.path || '').split(/[\\/]/).pop() || '';
    if (statusEl) { statusEl.textContent = 'Detecting…'; statusEl.className = 'ct-status ct-installing'; }
    if (bodyEl) {
      bodyEl.textContent = modelName
        ? `Detecting family for ${modelName}…`
        : 'Checking HuggingFace for model family and recommended template…';
    }
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

  if (state === 'embedded') {
    if (statusEl) { statusEl.textContent = 'Embedded'; statusEl.className = 'ct-status ct-neutral'; }
    if (bodyEl) {
      bodyEl.textContent = family && tpl
        ? 'Using the template embedded in the model file instead of the recommended community override.'
        : 'Using template embedded in model file. You can choose an existing Jinja or upload a new one here.';
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

  if (state === 'custom') {
    if (statusEl) {
      statusEl.textContent = 'Custom';
      statusEl.className = 'ct-status ct-ok';
    }
    if (bodyEl) {
      bodyEl.textContent = '';
      const nameEl = document.createElement('strong');
      nameEl.textContent = _chatTemplateDisplayName(data?.path || wizardState.model.chatTemplatePath);
      const descEl = document.createElement('span');
      descEl.textContent = ' — using your selected template from the local template library.';
      bodyEl.appendChild(nameEl);
      bodyEl.appendChild(descEl);
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

// ── HF discover categories: imported from hf-browse.js ────────────────────────

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

// ── HF quick-picks: imported from hf-browse.js ────────────────────────────────

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
      _reloadHfQuickPicks();
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
    _reloadHfQuickPicks();
  } catch {}
}

function _deriveMmprojSaveName(modelHfPath, mmprojHfPath) {
  const modelBase = (modelHfPath.split('/').pop() || modelHfPath).replace(/\.gguf$/i, '');
  const stem = modelBase.replace(/-?(Q\d[\w.]*|IQ\d[\w.]*|BF16|F16)$/i, '');
  const mmprojBase = mmprojHfPath.split('/').pop() || mmprojHfPath;
  return `${stem}-${mmprojBase}`;
}

function _reloadHfQuickPicks() {
  hfLoadQuickPicks({
    container: dom.hfQuickpicks,
    discoverPillsContainerId: 'hf-discover-pills',
    onAuthorClick: (author) => {
      browseHfAuthor(author);
    },
  });
}

async function browseHfAuthor(author) {
  const sort = dom.hfSortSelect?.value || 'downloads';
  wizardState.hfBrowseAuthor = author;
  hfSearchForWizard({ author, sort });
}

// ── HF file listing ───────────────────────────────────────────────────────────

function triggerHfFileFetch() {
  const input = dom.hfRepoInput?.value.trim();
  if (!input) return;

  const isRepoId = input.includes('/') && !input.includes(' ');

  if (isRepoId) {
    wizardState.model.hfRepo = input;
    if (dom.hfSearchResults) dom.hfSearchResults.style.display = 'none';
    dom.hfQuickpicks?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active'));
    const inferredP = inferParamBFromName(input);
    if (inferredP > 0) wizardState.model.paramB = inferredP;
    fetchHfFiles(input);
  } else {
    const sort = dom.hfSortSelect?.value || 'downloads';
    hfSearchForWizard({ query: input, sort });
  }
}

// Sort or min-size change triggers a re-search from page 0
function bindHfSortSelect() {
  const refire = () => {
    const author = wizardState.hfBrowseAuthor;
    const query  = dom.hfRepoInput?.value.trim() || '';
    const sort   = dom.hfSortSelect?.value || 'downloads';
    if (author) {
      browseHfAuthor(author);
    } else if (query && !query.includes('/')) {
      hfSearchForWizard({ query, sort, limit: 20 });
    } else {
      const activePill = document.querySelector('#hf-discover-pills .hf-discover-pill.active');
      if (activePill) {
        const cat = HF_DISCOVER_CATEGORIES.find(c => c.id === activePill.dataset.catId);
        if (cat) hfSearchForWizard({ ...cat.params, sort });
      }
    }
  };
  dom.hfSortSelect?.addEventListener('change', refire);
  dom.hfMinSize?.addEventListener('change', refire);
}

async function fetchHfFiles(repo) {
  if (!dom.hfFileList) return;

  // Also fetch VRAM so quant advisor has numbers
  if (!cachedVram) await fetchGpuVram();

  const vramGb = effectiveAvailBytes() / (1024 ** 3);

  hfListFiles({
    repoId: repo,
    container: dom.hfFileList,
    vramGb,
    onOpenCardPanel: (repoId) => openCardPanel(repoId),
    onSelectFile: (file, repoId) => {
      const fname = file.path || file.name || '';
      if (!fname) return;

      if (file.is_mmproj) {
        wizardState.model.mmprojPath = fname;
        wizardState.model.mmprojHfFile = fname;
        wizardState.model.mmprojHfRepo = repoId;
        if (file.size) wizardState.arch.mmprojBytes = Number(file.size);
        showToast('mmproj selected', 'success', fname.split('/').pop());
        dom.hfFileList.querySelectorAll('.hf-file-item.selected[data-mmproj]').forEach(el => el.classList.remove('selected'));
        const itemEl = dom.hfFileList.querySelector(`.hf-file-item[data-filename="${fname}"]`);
        if (itemEl) { itemEl.classList.add('selected'); itemEl.dataset.mmproj = '1'; }
        scheduleVramUpdate();
        return;
      }

      if (file.is_draft_assistant) {
        showToast('Assistant file', 'info', 'Select a base model first — this file will be offered as the MTP draft assistant.');
        return;
      }

      dom.hfFileList.querySelectorAll('.hf-file-item.selected:not([data-mmproj])').forEach(el => el.classList.remove('selected'));
      const itemEl = dom.hfFileList.querySelector(`.hf-file-item[data-filename="${fname}"]`);
      if (itemEl) itemEl.classList.add('selected');

      wizardState.model.hfFile = fname;
      wizardState.model.delivery = 'stream_hf';
      wizardState.model.originRepo = repoId;
      wizardState.model.originFile = fname;
      wizardState.model.localMeta = null;
      wizardState.model.path = '';
      if (file.size) wizardState.model.modelBytes = Number(file.size);

      if (!wizardState.model.paramB) wizardState.model.paramB = inferParamBFromName(fname) || inferParamBFromName(repoId);

      if (detectMtpFromName(fname) && !wizardState.arch.mtpDepth) {
        wizardState.arch.mtpDepth = 1;
      }

      // Store file lists so hardware step can offer quant swap + mmproj
      wizardState.model.quantFiles = [];
      wizardState.model.mmprojFiles = [];
      dom.hfFileList.querySelectorAll('.hf-file-item').forEach(el => {
        const f = {
          path: el.dataset.filename,
          name: el.dataset.filename,
          size: el.dataset.size ? Number(el.dataset.size) : 0,
          label: el.dataset.label || '',
          is_mmproj: el.dataset.mmproj === '1',
          is_draft_assistant: el.dataset.draftAssistant === '1',
          repo_id: el.dataset.repoId || repoId,
          is_recommended_mmproj: el.dataset.recommendedMmproj === '1',
          mmproj_recommendation: el.dataset.mmprojRecommendation || '',
        };
        if (f.is_mmproj) wizardState.model.mmprojFiles.push(f);
        else if (f.is_draft_assistant) wizardState.model.draftCandidates.push(f);
        else wizardState.model.quantFiles.push(f);
      });

      updateSelectedModelDisplay();
      clearValidationError();
      if (wizardState.model.paramB > 0) triggerQuantAdvisor();
      scheduleVramUpdate();
      autoInstallChatTemplate();
      refreshStepGuardrails();
    },
  });
}

// ── Third-party model import ──────────────────────────────────────────────────

const TOOL_ICONS = {
  'Ollama': '🦙',
  'LM Studio': '🎨',
  'Jan': '🤖',
  'GPT4All': '🌍',
  'HuggingFace': '🤗',
};

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

      const headerEl = document.createElement('div');
      headerEl.className = 'import-tool-header';
      const iconEl = document.createElement('span');
      iconEl.className = 'import-tool-icon';
      iconEl.textContent = icon;
      const nameEl = document.createElement('span');
      nameEl.className = 'import-tool-name';
      nameEl.textContent = tool;
      headerEl.appendChild(iconEl);
      headerEl.appendChild(nameEl);
      groupEl.appendChild(headerEl);

      for (const m of toolModels) {
        const itemEl = document.createElement('div');
        itemEl.className = 'import-model-item';
        itemEl.setAttribute('role', 'button');
        itemEl.setAttribute('tabindex', '0');
        itemEl.dataset.path = m.path;

        const labelEl = document.createElement('span');
        labelEl.className = 'import-model-name';
        labelEl.textContent = m.name;
        itemEl.appendChild(labelEl);

        const sizeStr = formatBytes(m.size);
        if (sizeStr) {
          const sizeEl = document.createElement('span');
          sizeEl.className = 'import-model-size';
          sizeEl.textContent = sizeStr;
          itemEl.appendChild(sizeEl);
        }

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
  refreshStepGuardrails();
}

// ── Hardware change ───────────────────────────────────────────────────────────

let vramDebounce = null;

function onHardwareChange(e) {
  // Only save scroll position for toggle checkboxes that cause layout shifts
  // (fitTargetWrap appearing/disappearing). Regular number inputs don't shift
  // layout, so the deferred restore's nested rAF can conflict with the
  // browser's own Tab/focus scroll, making content appear to disappear.
  const isToggle = e && (
    e.target === dom.fitEnableCheck ||
    e.target === dom.kvUnifiedCheck ||
    e.target === dom.mlockCheck
  );

  // If any non-toggle field fires after a toggle, cancel the pending scroll
  // restore so we don't undo the browser's natural scroll-into-view when the
  // user tabs away from their input.
  if (!isToggle && pendingHardwareScrollRestore) {
    pendingHardwareScrollRestore = null;
  }

  if (isToggle && wizardState.currentStep === 2 && !pendingHardwareScrollReset) {
    const main = document.querySelector('#wizard-step-2 .wizard-main');
    const sidebar = document.querySelector('#wizard-step-2 .hw-vram-sidebar');
    pendingHardwareScrollRestore = {
      main: main?.scrollTop ?? 0,
      sidebar: sidebar?.scrollTop ?? 0,
    };
  }
  readHardwareState();
  scheduleVramUpdate();
  refreshStepGuardrails();
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
  if (dom.ubatchSizeInput)  { const v = Number(dom.ubatchSizeInput.value);  h.ubatchSize  = v > 0 ? v : 2048; }
  if (dom.parallelSlotsInput) { const v = Number(dom.parallelSlotsInput.value); h.parallelSlots = v > 0 ? v : 1; }
  if (dom.cacheTypeKSelect) h.cacheTypeK = dom.cacheTypeKSelect.value || 'q8_0';
  if (dom.cacheTypeVSelect) h.cacheTypeV = dom.cacheTypeVSelect.value || 'q8_0';
  if (dom.nCpuMoeInput) { const v = dom.nCpuMoeInput.value; h.nCpuMoe = v !== '' ? Number(v) : 0; }
  if (dom.tensorSplitInput) h.tensorSplit = dom.tensorSplitInput.value.trim() || '';
  if (dom.fitEnableCheck) {
    const enabled = dom.fitEnableCheck.checked;
    if (dom.fitTargetWrap) dom.fitTargetWrap.style.display = enabled ? '' : 'none';
    if (enabled && dom.fitTargetInput && !dom.fitTargetInput.value.trim()) {
      dom.fitTargetInput.value = '2048';
    }
    h.fitTarget = enabled && dom.fitTargetInput ? (dom.fitTargetInput.value.trim() || '') : '';
  } else if (dom.fitTargetInput) {
    h.fitTarget = dom.fitTargetInput.value.trim() || '';
  }
  if (dom.kvUnifiedCheck) h.kvUnified = dom.kvUnifiedCheck.checked;
  if (dom.flashAttnSelect) h.flashAttn = dom.flashAttnSelect.value || '';
  if (dom.mlockCheck) h.mlock = dom.mlockCheck.checked;
  if (dom.prioSelect) { const v = dom.prioSelect.value; h.prio = v !== '' ? Number(v) : null; }
  if (dom.threadsInput) { const v = dom.threadsInput.value; h.threads = v !== '' ? Number(v) : null; }
  if (dom.threadsBatchInput) { const v = dom.threadsBatchInput.value; h.threadsBatch = v !== '' ? Number(v) : null; }
  if (dom.specDraftNMinInput) { const v = dom.specDraftNMinInput.value; h.mtpDraftNMin = v !== '' ? Number(v) : null; }
  if (dom.specDraftPMinInput) { const v = dom.specDraftPMinInput.value; h.mtpDraftPMin = v !== '' ? parseFloat(v) : null; }
  if (dom.cacheRamInput) {
    const v = dom.cacheRamInput.value.trim();
    h.cacheRam = v !== '' ? parseInt(v, 10) : null;
  }
}

export function scheduleVramUpdate() {
  if (vramDebounce) clearTimeout(vramDebounce);
  vramDebounce = setTimeout(updateVramDisplay, 250);
}

function maybeResetHardwareStepScroll() {
  if (!pendingHardwareScrollReset || wizardState.currentStep !== 2) return;
  pendingHardwareScrollReset = false;

  const main = document.querySelector('#wizard-step-2 .wizard-main');
  const sidebar = document.querySelector('#wizard-step-2 .hw-vram-sidebar');
  // When a toggle hides part of the hardware form, the column can shrink from
  // scrollable to non-scrollable in the same frame. Reset unconditionally so we
  // never leave the viewport stranded at a stale offset showing blank space.
  if (main) main.scrollTop = 0;
  if (sidebar) sidebar.scrollTop = 0;
}

function maybeRestoreHardwareStepScroll() {
  if (!pendingHardwareScrollRestore || wizardState.currentStep !== 2) return;

  const snapshot = pendingHardwareScrollRestore;
  pendingHardwareScrollRestore = null;

  const restore = () => {
    if (wizardState.currentStep !== 2) return;

    const focused = document.activeElement;
    if (focused === dom.kvUnifiedCheck || focused === dom.fitEnableCheck) {
      focused.blur?.();
    }

    const main = document.querySelector('#wizard-step-2 .wizard-main');
    const sidebar = document.querySelector('#wizard-step-2 .hw-vram-sidebar');
    if (main) {
      const maxScroll = Math.max(0, main.scrollHeight - main.clientHeight);
      main.scrollTop = Math.min(snapshot.main, maxScroll);
    }
    if (sidebar) {
      const maxScroll = Math.max(0, sidebar.scrollHeight - sidebar.clientHeight);
      sidebar.scrollTop = Math.min(snapshot.sidebar, maxScroll);
    }
  };

  requestAnimationFrame(() => {
    restore();
    requestAnimationFrame(restore);
  });
}

function bindHardwareToggleSwitch(labelEl, inputEl) {
  if (!labelEl || !inputEl) return;

  labelEl.addEventListener('pointerdown', e => {
    e.preventDefault();
  });

  labelEl.addEventListener('click', e => {
    if (e.target === inputEl) return;
    e.preventDefault();
    inputEl.checked = !inputEl.checked;
    inputEl.dispatchEvent(new Event('input', { bubbles: true }));
    inputEl.dispatchEvent(new Event('change', { bubbles: true }));
  });
}

// ── Animated VRAM display ─────────────────────────────────────────────────────

function getEffectiveArch() {
  const a = wizardState.arch;
  // Always apply heuristics to ensure sliding window fields are populated
  // for models that support them (Gemma4, Gemma3, etc.)
  if (wizardState.model.paramB > 0) {
    const heuristicArch = buildHeuristicArch(wizardState.model.path || wizardState.model.hfRepo, wizardState.model.paramB);
    // Merge heuristic values with existing introspection data
    // Heuristic provides defaults; introspection provides actual values when available
    return {
      ...heuristicArch,
      ...a,
      // Preserve existing non-default fields that might be set by introspection
      nLayers: a.nLayers || heuristicArch.nLayers,
      nKvHeads: a.nKvHeads || heuristicArch.nKvHeads,
      headDim: a.headDim || heuristicArch.headDim,
    };
  }
  return a;
}

function getSizingArch() {
  const base = getEffectiveArch();
  const arch = { ...base };
  const hasSelectedMmproj = !!(wizardState.model.mmprojPath || wizardState.model.mmprojHfFile);

  // Size against the current wizard choices, not just what the model could support.
  arch.mtpDepth = wizardState.hardware.mtpEnabled ? (base.mtpDepth || 0) : 0;
  arch.mmprojBytes = hasSelectedMmproj ? (base.mmprojBytes || 0) : 0;

  return arch;
}

function clampAutoSizeResultToSizingMath(result, arch, modelBytes, availVram) {
  if (!result || !modelBytes || !availVram) return { result, adjusted: false };

  const fitGran = 1024;
  const slots = wizardState.hardware.parallelSlots || 1;
  const headroom = computeHeadroom(availVram);
  const nCpuMoe = result.n_cpu_moe ?? wizardState.hardware.nCpuMoe ?? 0;
  const maxCtx = maxContext(
    modelBytes,
    arch,
    result.kv_quant_k || 'q8_0',
    result.kv_quant_v || 'q8_0',
    slots,
    result.ubatch_size || wizardState.hardware.ubatchSize || 2048,
    nCpuMoe,
    availVram,
    fitGran,
    headroom,
  );
  const modelCap = wizardState.model.nCtxTrain || 0;
  const safeCap = modelCap > 0 ? Math.min(maxCtx, modelCap) : maxCtx;
  if (!safeCap || result.context_size <= safeCap) return { result, adjusted: false };

  return {
    result: {
      ...result,
      context_size: safeCap,
      warnings: [
        ...(result.warnings || []),
        `Adjusted to ${formatCtx(safeCap)} so auto-size matches the hardware-step fit math.`,
      ],
    },
    adjusted: true,
  };
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
   // Qwopus3.6 derivatives, and all finetunes/distillations that mention Qwen3.6.
   if (lower.includes('qwen3.6') || lower.includes('qwen3-6') || lower.includes('qwopus3.6') || lower.includes('qwopus3-6') || lower.includes('qwopus36')) {
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

  const isGemma4 = lower.includes('gemma-4') || lower.includes('gemma4');
  if (isGemma4) {
    const namedE2B = lower.includes('e2b');
    const namedE4B = lower.includes('e4b');
    const named12B = lower.includes('12b');
    const named26BA4B = lower.includes('26b-a4b') || lower.includes('26b_a4b') || lower.includes('a4b');
    const named31B = lower.includes('31b');
    const hasNamedSize = namedE2B || namedE4B || named12B || named26BA4B || named31B;
    const isE2B = namedE2B || (!hasNamedSize && paramB < 6);
    const isE4B = namedE4B || (!hasNamedSize && !isE2B && paramB < 10);
    const is12B = named12B || (!hasNamedSize && !isE2B && !isE4B && paramB < 20);
    let cfg;
    if (isE2B) cfg = [35, 7, 1, 1, 512, 0, 0];
    else if (isE4B) cfg = [42, 7, 2, 2, 512, 0, 0];
    else if (is12B) cfg = [48, 8, 1, 8, 1024, 0, 0];
    else if (named26BA4B || (!hasNamedSize && paramB < 30)) cfg = [30, 5, 2, 8, 1024, 128, 9];
    else cfg = [60, 10, 4, 16, 1024, 0, 0];
    return {
      nLayers: wizardState.arch.nLayers || cfg[0],
      nKvHeads: wizardState.arch.nKvHeads || cfg[2],
      headDim: wizardState.arch.headDim || 256,
      globalHeadDim: 512,
      nGlobalAttnLayers: cfg[1],
      localAttnWindow: cfg[4],
      localKvHeads: cfg[3],
      nExperts: wizardState.arch.nExperts || cfg[5],
      nExpertsUsed: wizardState.arch.nExpertsUsed || cfg[6],
      expertFraction: 0.65,
      mtpDepth: wizardState.arch.mtpDepth || 0,
      mmprojBytes: wizardState.arch.mmprojBytes || 0,
      paramB,
    };
  }

  const isGemma3 = lower.includes('gemma-3') || lower.includes('gemma3');
  if (isGemma3) {
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
  const paramB = wizardState.model.paramB;
  if (!paramB) return 0;
  // Estimate from param count + quant for both local and HF models.
  // Use the selected HF file name first, then fall back to the local path.
  const fname = (wizardState.model.hfFile || wizardState.model.path || '').toLowerCase();
  const quant = guessQuantFromName(fname);
  const BPW = { f16:16, q8_0:8.5, q6_k:6.5625, q5_k_m:5.69, q5_k_s:5.52, q4_k_m:4.85, q4_k_s:4.58, q4_0:4.55, iq4_xs:4.25, q3_k_m:3.875, q2_k:2.625, iq2_xxs:2.0625, iq1_m:1.75 };
  const bpw = BPW[quant] ?? 4.85;
  return Math.round(paramB * 1e9 * bpw / 8);
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

// ── Performance advisor (config-time hints) ───────────────────────────────────
let _advisorTimer = null;
let _advisorSeq = 0;

// Map an advisor suggestion's patch onto the wizard controls. We drive the real
// DOM inputs and dispatch their events so existing handlers keep wizardState in
// sync, then refresh the advice.
function applyWizardSuggestion(suggestion) {
  const patch = suggestionPatch(suggestion);
  const map = {
    ctk: { id: 'spawn-cache-type-k', evt: 'change' },
    ctv: { id: 'spawn-cache-type-v', evt: 'change' },
    context_size: { id: 'spawn-context-size', evt: 'input' },
    spec_draft_n_max: { id: 'hw-mtp-depth', evt: 'input' },
  };
  Object.entries(patch).forEach(([k, v]) => {
    if (k === 'spec_type') {
      const useMtp = String(v).includes('draft-mtp');
      const cb = document.getElementById('hw-use-mtp');
      if (cb && cb.checked !== useMtp) {
        cb.checked = useMtp;
        cb.dispatchEvent(new Event('change', { bubbles: true }));
      }
      return;
    }
    const m = map[k];
    const el = m && document.getElementById(m.id);
    if (!el) return;
    el.value = String(v);
    el.dispatchEvent(new Event(m.evt, { bubbles: true }));
  });
  updateAdvisor();
  showToast('Applied', 'success', suggestion.label);
}

// Auto-tune n_cpu_moe: instant estimate, or empirical llama-bench sweep (verify).
async function autoTuneWizard(verify) {
  const statusEl = document.getElementById('spawn-moe-autotune-status');
  const arch = getSizingArch();
  const hw = wizardState.hardware;
  const m = wizardState.model;
  const body = {
    name: (m.path || m.hfRepo || '').split('/').pop() || '',
    param_b: arch.paramB || m.paramB || 0,
    model_size_bytes: getModelBytes(),
    available_vram_bytes: effectiveAvailBytes(),
    ubatch_size: hw.ubatchSize || 2048,
    verify: !!verify,
  };
  if (verify) {
    if (!m.path) { showToast('Verify needs a local model file', 'warn'); return; }
    body.model_path = m.path;
    body.ngl = 99;
    body.ctk = hw.cacheTypeK;
    body.ctv = hw.cacheTypeV;
    body.flash_attn = hw.flashAttn === 'on';
  }
  if (statusEl) {
    statusEl.textContent = verify
      ? 'Running sweep… this can take a few minutes'
      : 'Estimating…';
  }
  try {
    const data = await requestNcpuMoeTune(body);
    if (data.error) { if (statusEl) statusEl.textContent = data.error; return; }
    const rec = data.recommended_n_cpu_moe;
    const input = document.getElementById('spawn-n-cpu-moe');
    if (input) {
      input.value = String(rec);
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }
    if (statusEl) {
      statusEl.textContent = data.verified ? `Verified best: ${rec} (measured)` : `Estimated: ${rec}`;
    }
  } catch {
    if (statusEl) statusEl.textContent = 'Auto-tune failed';
  }
}

// Depth sweep: measure decode/prefill at several context depths via llama-bench.
async function runDepthSweep() {
  const statusEl = document.getElementById('wizard-depth-sweep-status');
  const resultsEl = document.getElementById('wizard-depth-sweep-results');
  const hw = wizardState.hardware;
  const m = wizardState.model;
  if (!m.path || !m.path.toLowerCase().endsWith('.gguf')) {
    showToast('Depth sweep needs a local .gguf file', 'warn');
    return;
  }
  const ctx = hw.contextSize || 32768;
  const depths = [0, 16384, 32768, 65536, 131072].filter((d) => d === 0 || d < ctx);
  const body = {
    model_path: m.path,
    ngl: 99,
    ctk: hw.cacheTypeK,
    ctv: hw.cacheTypeV,
    flash_attn: hw.flashAttn === 'on',
    n_cpu_moe: hw.nCpuMoe || null,
    depths,
  };
  if (statusEl) statusEl.textContent = 'Running… llama-bench reloads per depth, so this can take several minutes.';
  if (resultsEl) resultsEl.replaceChildren();
  try {
    const data = await requestDepthSweep(body);
    if (data.error) { if (statusEl) statusEl.textContent = data.error; return; }
    if (statusEl) statusEl.textContent = '';
    renderDepthSweep(resultsEl, data.points || []);
  } catch {
    if (statusEl) statusEl.textContent = 'Depth sweep failed';
  }
}

function updateAdvisor() {
  const box = document.getElementById('wizard-advisor');
  const cards = document.getElementById('wizard-advisor-cards');
  if (!box || !cards) return;

  clearTimeout(_advisorTimer);
  _advisorTimer = setTimeout(async () => {
    const arch = getSizingArch();
    const baseArch = getEffectiveArch(); // capability arch — mtpDepth not zeroed when MTP is off
    const hw = wizardState.hardware;
    const m = wizardState.model;

    // Show the n_cpu_moe auto-tuner only for MoE models.
    const moeBox = document.getElementById('spawn-moe-autotune');
    if (moeBox) moeBox.style.display = (arch.nExperts || 0) > 0 ? '' : 'none';

    // Depth sweep is available once a local .gguf is selected.
    const sweepBox = document.getElementById('wizard-depth-sweep');
    if (sweepBox) {
      const localGguf = !!(m.path && m.path.toLowerCase().endsWith('.gguf'));
      sweepBox.style.display = localGguf ? '' : 'none';
    }

    const name = (m.path || m.hfFile || m.hfRepo || m.originFile || '').split('/').pop() || '';
    const paramB = arch.paramB || m.paramB || 0;
    if (!name && !paramB) { box.style.display = 'none'; return; }

    const hasMtp = (baseArch.mtpDepth || 0) > 0 || /mtp/i.test(name);
    const body = {
      name,
      param_b: paramB,
      context_size: hw.contextSize,
      ctk: hw.cacheTypeK,
      ctv: hw.cacheTypeV,
      is_unified: isUnifiedMemory(),
      spec_type: hw.mtpEnabled ? 'draft-mtp' : null,
      has_mtp: hasMtp,
    };

    const seq = ++_advisorSeq;
    try {
      const headers = window.authHeaders
        ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
        : { 'Content-Type': 'application/json' };
      const r = await fetch('/api/advise', { method: 'POST', headers, body: JSON.stringify(body) });
      if (seq !== _advisorSeq) return; // a newer recompute superseded this one
      const data = await r.json();
      const suggestions = (data && data.suggestions) || [];
      const cfgView = {
        ctk: hw.cacheTypeK,
        ctv: hw.cacheTypeV,
        context_size: hw.contextSize,
        spec_type: hw.mtpEnabled ? 'draft-mtp' : '',
        spec_draft_n_max: hw.mtpDraftNMax,
      };
      renderSuggestionCards(cards, suggestions, { onApply: applyWizardSuggestion, config: cfgView });
      box.style.display = cards.childElementCount ? '' : 'none';
    } catch {
      box.style.display = 'none';
    }
  }, 250);
}

function updateVramDisplay() {
  const availVram = effectiveAvailBytes();
  if (!dom.vramPanel) return;

  const hw = wizardState.hardware;
  const arch = getSizingArch();
  const modelBytes = getModelBytes();

  // Compute breakdown (layer-based MoE offload — see moeWeightSplit)
  const nCpuMoe = hw.nCpuMoe || 0;
  const { vram: weightVram, ram: ramBytes } = moeWeightSplit(modelBytes, arch, nCpuMoe);
  const kv          = kvBytes(arch, hw.contextSize, hw.parallelSlots, hw.cacheTypeK, hw.cacheTypeV);
  const mmproj      = arch.mmprojBytes || 0;
  const mtp         = mtpBytes(modelBytes, arch.mtpDepth || 0);
  const linearState = arch.linearAttnStateBytes || 0;
  const oh          = gpuOverheadBytes(hw.ubatchSize);
  const total       = weightVram + kv + linearState + mmproj + mtp + oh;
  const free = availVram - total;

  // Update total label
  if (dom.vramPanelTotal) {
    if (availVram > 0) {
      if (isUnifiedMemory() && cachedRamTotal > 0) {
        // Show "X GB Metal cap (of Y GB total)" — budget is the configured Metal cap, not free RAM
        dom.vramPanelTotal.textContent =
          formatVramTotal(availVram) + ' Metal cap (of ' + formatVramTotal(cachedRamTotal) + ' total)';
      } else {
        dom.vramPanelTotal.textContent = formatVramTotal(availVram) + ' total';
      }
    } else {
      dom.vramPanelTotal.textContent = isUnifiedMemory() ? 'Unified memory unknown' : 'GPU VRAM unknown';
    }
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
    dom.vramBar.classList.toggle('has-data', total > 0);
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
    if (dom.vLegFreeDot) dom.vLegFreeDot.style.background = free >= 0 ? '' : 'var(--color-error)';
  }

  // Show/hide MoE panel
  if (arch.nExperts > 1) {
    if (dom.moeOffloadPanel) dom.moeOffloadPanel.style.display = '';
    if (dom.moeOffloadSlider) {
      dom.moeOffloadSlider.max = arch.nLayers || arch.nExperts;
      dom.moeOffloadSlider.value = nCpuMoe;
    }
    updateMoeSliderVisuals();
  } else {
    if (dom.moeOffloadPanel) dom.moeOffloadPanel.style.display = 'none';
  }

  // Render scenario cards
  renderScenarioCards(modelBytes, arch, availVram);

  // Config-time performance advisor (dense-vs-MoE, KV type, MTP)
  updateAdvisor();

  // Legacy VRAM pill (backward compat)
  if (dom.vramPill || dom.vramEstimateText) {
    updateLegacyVramPill(total, availVram);
  }

  // Unified memory label (Apple Silicon / DGX Spark — VRAM and RAM are the same pool)
  const isUnified = _platformInfo?.auto_backend === 'metal';
  if (dom.vramPanelLabel) {
    dom.vramPanelLabel.textContent = isUnified ? 'Unified Memory' : 'VRAM budget';
  }

  // Metal GPU limit row — Apple Silicon only
  if (dom.metalLimitRow) {
    if (isUnified && cachedRamTotal > 0) {
      dom.metalLimitRow.style.display = '';
      const currentCapMb = Math.round(metalCap(cachedRamTotal) / (1024 * 1024));
      const isCustom = cachedMetalGpuLimitMb > 0;
      const capGb = (currentCapMb / 1024).toFixed(0);
      const totalGb = (cachedRamTotal / (1024 ** 3)).toFixed(0);
      const label = isCustom
        ? `Metal GPU cap: ${capGb} GB (custom) — of ${totalGb} GB total`
        : `Metal GPU cap: ${capGb} GB (default) — of ${totalGb} GB total`;
      if (dom.metalLimitText) dom.metalLimitText.textContent = label;

      // Show "Increase" button if a meaningfully larger cap is achievable
      const suggested = suggestedMetalLimitMb(cachedRamTotal);
      if (dom.metalLimitBtn) {
        if (suggested > 0) {
          const suggestedGb = Math.round(suggested / 1024);
          dom.metalLimitBtn.disabled = false; // clear disabled from any previous attempt
          dom.metalLimitBtn.style.display = '';
          dom.metalLimitBtn.textContent = `Increase to ${suggestedGb} GB`;
          dom.metalLimitBtn.onclick = () => applyMetalGpuLimit(suggested);
          // Remove any stale fallback panel from a previous failed attempt
          dom.metalLimitRow?.querySelector('.metal-limit-fallback')?.remove();
        } else {
          dom.metalLimitBtn.style.display = 'none';
        }
      }
    } else {
      dom.metalLimitRow.style.display = 'none';
    }
  }

  // RAM bar — only shown on discrete GPU systems; on unified the VRAM bar already covers it
  if (dom.ramPanel) {
    if (isUnified || cachedRamTotal === 0) {
      dom.ramPanel.style.display = 'none';
    } else {
      dom.ramPanel.style.display = '';
      const cramMib = (hw.cacheRam !== null && hw.cacheRam !== undefined) ? hw.cacheRam : 8192;
      const cramBytes = cramMib < 0 ? 0 : cramMib * 1024 * 1024;
      const ramDenom = cachedRamTotal;
      const inUsePct   = cachedRamUsed / ramDenom;
      const moePct     = ramBytes / ramDenom;
      const cramPct    = cramBytes / ramDenom;
      const freePct    = Math.max(0, (cachedRamTotal - cachedRamUsed - ramBytes - cramBytes) / ramDenom);
      setSegWidth(dom.rSegUsed, inUsePct);
      setSegWidth(dom.rSegMoe,  moePct);
      setSegWidth(dom.rSegCram, cramPct);
      setSegWidth(dom.rSegFree, freePct);
      const totalNeeded = cachedRamUsed + ramBytes + cramBytes;
      const isOver = totalNeeded > cachedRamTotal;
      if (dom.ramPanelTotal) {
        dom.ramPanelTotal.textContent = formatVramTotal(cachedRamTotal) + ' total';
      }
      if (dom.rLegUsed)  dom.rLegUsed.textContent  = `In use ${formatGB(cachedRamUsed)}`;
      if (dom.rLegCram) {
        const cramLabel = cramMib < 0 ? 'no limit' : `${formatGB(cramBytes)}`;
        dom.rLegCram.textContent = `Cache ${cramLabel}`;
      }
      if (ramBytes > 0) {
        if (dom.rLegMoeItem) dom.rLegMoeItem.style.display = '';
        if (dom.rLegMoe) dom.rLegMoe.textContent = `MoE ${formatGB(ramBytes)}`;
      } else {
        if (dom.rLegMoeItem) dom.rLegMoeItem.style.display = 'none';
      }
      const freeBytes = cachedRamTotal - totalNeeded;
      if (dom.rLegFree) {
        dom.rLegFree.textContent = isOver
          ? `Over ${formatGB(Math.abs(freeBytes))}`
          : `Free ${formatGB(freeBytes)}`;
      }
      if (dom.ramPanel) dom.ramPanel.classList.toggle('over-budget', isOver);
    }
  }

  maybeResetHardwareStepScroll();
  maybeRestoreHardwareStepScroll();
}

function setSegWidth(el, frac) {
  if (!el) return;
  const pct = Math.max(0, Math.min(1, frac)) * 100;
  el.style.width = pct.toFixed(2) + '%';
  el.style.display = pct < 0.3 ? 'none' : '';
}

function updateMoeSliderVisuals() {
  const arch = getSizingArch();
  if (!(arch.nExperts > 0)) return; // not a MoE model
  const n = arch.nLayers || 0;
  if (!n) return;
  const cpu = wizardState.hardware.nCpuMoe || 0;
  const gpu = n - cpu;
  const pct = cpu / n * 100;

  if (dom.moeOffloadSlider) {
    dom.moeOffloadSlider.style.background =
         `linear-gradient(90deg, var(--color-purple) ${pct.toFixed(1)}%, var(--neutral-soft-bg-strong) ${pct.toFixed(1)}%)`;
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

// ── Context fit modes ────────────────────────────────────────────────────────

function updateContextRailSummary() {
  if (!dom.ctxRailSummaryValue || !dom.ctxRailSummaryStatus || !dom.ctxRailSummaryNote) return;

  const currentCtx = wizardState.hardware.contextSize || 8192;
  const nCtxTrain = wizardState.model.nCtxTrain || 0;
  const target = CTX_TARGETS[wizardState.useCase] || 0;

  dom.ctxRailSummaryValue.textContent = formatCtx(currentCtx);
  dom.ctxRailSummaryStatus.classList.remove('warning');

  if (nCtxTrain && currentCtx > nCtxTrain) {
    dom.ctxRailSummaryStatus.textContent = 'Extended beyond trained context';
    dom.ctxRailSummaryStatus.classList.add('warning');
    dom.ctxRailSummaryNote.textContent = `Model max is ${formatCtx(nCtxTrain)}. This can still fit in memory, but quality may degrade unless you intentionally extend context with RoPE/YaRN.`;
    return;
  }

  if (nCtxTrain && currentCtx === nCtxTrain) {
    dom.ctxRailSummaryStatus.textContent = 'At model max';
    dom.ctxRailSummaryNote.textContent = 'You are using the model’s full trained context. Going higher is possible only as an advanced extension with RoPE/YaRN.';
    return;
  }

  if (nCtxTrain) {
    dom.ctxRailSummaryStatus.textContent = 'Within trained context';
    dom.ctxRailSummaryNote.textContent = currentCtx < target
      ? `Use-case target is ${formatCtx(target)}. Lower values save memory, but leave less room for long chats, retrieval, or tool loops.`
      : `Model max is ${formatCtx(nCtxTrain)}. Use a custom value above that only if you intentionally want extended context with RoPE/YaRN.`;
    return;
  }

  dom.ctxRailSummaryStatus.textContent = 'Training max unavailable';
  dom.ctxRailSummaryNote.textContent = currentCtx < target
    ? `Use-case target is ${formatCtx(target)}. Larger contexts use more KV memory.`
    : 'Larger contexts use more KV memory. Stay conservative unless you know the model’s training limit.';
}

function renderScenarioCards(modelBytes, arch, availVram) {
  if (!dom.vramScenarios || !availVram || !modelBytes) return;

  const hw = wizardState.hardware;
  const fitGran = 1024;
  const slots = hw.parallelSlots || 1;
  const ubatch = hw.ubatchSize || 512;
  const nCpuMoe = hw.nCpuMoe || 0;
  const uc = wizardState.useCase;
  const headroom = computeHeadroom(availVram);
  const nCtxTrain = wizardState.model.nCtxTrain || 0;
  const currentCtx = hw.contextSize || 8192;

  updateContextRailSummary();

  const q8Max = maxContext(modelBytes, arch, 'q8_0', 'q8_0', slots, ubatch, nCpuMoe, availVram, fitGran, headroom);
  const q4Max = maxContext(modelBytes, arch, 'q4_0', 'q4_0', slots, ubatch, nCpuMoe, availVram, fitGran, headroom);

  const scenarios = [
    {
      key: 'q8_0',
      mode: 'Reliable agents',
      detail: 'High-precision KV cache',
      kk: 'q8_0',
      kv: 'q8_0',
      desc: uc === 'roleplay' ? 'Best long-context quality with less headroom for very large transcripts.' : 'Best long-context quality for tools, retrieval, and multi-step work.',
      rec: uc !== 'roleplay',
    },
    {
      key: 'q4_0',
      mode: 'More context',
      detail: 'Lower-precision KV cache',
      kk: 'q4_0',
      kv: 'q4_0',
      desc: uc === 'agentic' ? 'Fits more tokens, but lower cache precision can hurt tool-call coherence.' : 'Fits the most context if you care more about length than cache quality.',
      rec: uc === 'roleplay',
      warnAgentic: uc === 'agentic',
    },
    {
      key: 'f16',
      mode: 'Full precision',
      detail: 'Lossless KV cache',
      kk: 'f16',
      kv: 'f16',
      desc: 'Uses the most KV memory. Best reserved for comparison or when you want the most exact cache.',
      rec: false,
    },
  ];

  dom.vramScenarios.innerHTML = '';
  const activeQuant = hw.cacheTypeK === '' ? 'f16' : (hw.cacheTypeK || 'q8_0');

  for (const s of scenarios) {
    const vramCtx = s.kk === 'q8_0' ? q8Max : s.kk === 'q4_0' ? q4Max
      : maxContext(modelBytes, arch, s.kk, s.kv, slots, ubatch, nCpuMoe, availVram, fitGran, headroom);
    const cappedByModel = nCtxTrain > 0 && vramCtx > nCtxTrain;
    const ctx = cappedByModel ? nCtxTrain : vramCtx;
    const selectable = ctx > 0;

    const card = document.createElement('div');
    const isActive = s.key === activeQuant;
    card.className = 'vram-scenario-card' + (s.rec ? ' scenario-rec' : '') + (isActive ? ' selected' : '');
    card.setAttribute('tabindex', '0');
    card.setAttribute('role', 'button');
    card.setAttribute('aria-label', `${s.mode}: ${formatCtx(ctx)} tokens — ${s.desc}`);

    let desc = s.desc;
    if (cappedByModel) {
      if (s.key === 'q8_0') desc = 'Best long-context quality. VRAM is no longer the limit.';
      else if (s.key === 'q4_0') desc = 'More headroom, but the model max is already the real ceiling.';
      else if (s.key === 'f16') desc = 'Full precision cache. Context is capped by the model, not VRAM.';
    }

    const limitNote = cappedByModel ? '<span class="vsc-limit-note">model max</span>' : '';
    const ctxWontFit = selectable && !cappedByModel && ctx < currentCtx;
    const ctxWarnNote = ctxWontFit ? `<span class="vsc-warn">⚠ won't fit your ${formatCtx(currentCtx)} ctx</span>` : '';

    // All values are internal constants — no user input reaches this template.
    // eslint-disable-next-line no-unsanitized/property
    card.innerHTML = `
      <div class="vsc-mode-name">${s.mode}</div>
      <div class="vsc-mode-detail">${s.detail}</div>
      <div class="vsc-ctx-row">
        <span class="vsc-ctx">${selectable ? formatCtx(ctx) : '—'}</span>
        ${selectable ? '<span class="vsc-ctx-unit">tokens</span>' : ''}
        ${limitNote}
      </div>
      <div class="vsc-desc">${desc}</div>
      ${s.rec ? '<span class="vsc-rec-badge">★ Recommended</span>' : ''}
      ${isActive ? '<span class="vsc-active-badge">✓ Active</span>' : ''}
      ${s.warnAgentic ? '<span class="vsc-warn">⚠ Not ideal for tool-heavy agents</span>' : ''}
      ${ctxWarnNote}
      <span class="vsc-footnote">KV cache: ${s.kk}/${s.kv}</span>
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
      card.addEventListener('click', e => { e.stopPropagation(); applyScenario(); });
      card.addEventListener('keydown', e => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          applyScenario();
        }
      });
    }

    dom.vramScenarios.appendChild(card);
  }
}

// ── Hardware step: model header + quant swap ─────────────────────────────────

function _refreshThreadsHint() {
  const hintEl = document.getElementById('spawn-threads-hint');
  const batchHintEl = document.getElementById('spawn-threads-batch-hint');
  if (!hintEl && !batchHintEl && !dom.threadsInput && !dom.threadsBatchInput) return;

  const pCores = lastSystemMetrics?.p_cores || 0;
  const metricsReady = lastSystemMetrics != null;

  if (pCores > 0 && metricsReady) {
    if (hintEl) {
      hintEl.textContent =
        `Apple Silicon detected: ${pCores} P-cores. Apple recommends 1 for -t on Metal, or at most ${pCores}. Blank = server default.`;
    }
    if (batchHintEl) {
      batchHintEl.textContent =
        `Prompt processing can use more CPU. Recommended: ${pCores} for -tb, or leave blank to inherit -t.`;
    }
    if (dom.threadsInput && !dom.threadsInput.value) {
      dom.threadsInput.placeholder = '1 recommended';
    }
    if (dom.threadsBatchInput && !dom.threadsBatchInput.value) {
      dom.threadsBatchInput.placeholder = `${pCores} recommended`;
    }
    return;
  }

  if (!metricsReady) {
    if (hintEl) {
      hintEl.textContent = 'Blank = server default (-t). Apple Silicon guidance loads automatically.';
    }
    if (batchHintEl) {
      batchHintEl.textContent = 'Prompt processing threads. Blank = inherit from -t.';
    }
    if (dom.threadsInput && !dom.threadsInput.value) {
      dom.threadsInput.placeholder = 'default';
    }
    if (dom.threadsBatchInput && !dom.threadsBatchInput.value) {
      dom.threadsBatchInput.placeholder = 'default';
    }
    return;
  }

  // Non-Apple Silicon / no P-cores: generic hint.
  if (hintEl) {
    hintEl.textContent = 'Blank = server default (-t). Sets CPU threads for inference. Do not exceed physical P-core count.';
  }
  if (batchHintEl) {
    batchHintEl.textContent = 'Prompt processing threads. Blank = inherit from -t.';
  }
  if (dom.threadsInput && !dom.threadsInput.value) {
    dom.threadsInput.placeholder = 'default';
  }
  if (dom.threadsBatchInput && !dom.threadsBatchInput.value) {
    dom.threadsBatchInput.placeholder = 'default';
  }
}
window.__refreshSpawnWizardHints = _refreshThreadsHint;

async function _fetchSystemInfoAndRefreshHints() {
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const res = await fetch('/api/system/info', { headers });
    if (!res.ok) return;
    const data = await res.json();
    if (data.ok && data.p_cores > 0) {
      // Populate lastSystemMetrics with at minimum the core counts so hints work.
      // Use setLastSystemMetrics from app-state so the live binding updates.
      const { setLastSystemMetrics } = await import('../core/app-state.js');
      setLastSystemMetrics({ p_cores: data.p_cores, e_cores: data.e_cores, cpu_name: data.cpu_name });
      _refreshThreadsHint();
    }
  } catch { /* non-fatal */ }
}

function renderHardwareModelHeader() {
  const header = document.getElementById('hw-model-header');
  if (!header) return;
  const { source, path, hfRepo, hfFile, quantFiles } = wizardState.model;
  if (!hfRepo && !path) { header.style.display = 'none'; return; }
  header.style.display = '';

  const repoEl = document.getElementById('hw-model-repo');
  if (repoEl) {
    // Remove any previous inline editor
    repoEl.classList.remove('hw-model-repo-editing');
    repoEl.innerHTML = '';
    repoEl.style.cursor = 'default';

    const fullRepo = hfRepo || (wizardState.model.originRepo || '');
    const displayName = fullRepo || path.split(/[/\\]/).pop() || path;
    const slashIdx = fullRepo ? fullRepo.indexOf('/') : -1;

    if (fullRepo && slashIdx > 0) {
      const author = fullRepo.slice(0, slashIdx + 1);
      const modelName = fullRepo.slice(slashIdx + 1);
      const authorSpan = document.createElement('span');
      authorSpan.className = 'hw-model-author';
      authorSpan.textContent = author;
      const nameSpan = document.createElement('span');
      nameSpan.className = 'hw-model-name';
      nameSpan.textContent = modelName;
      repoEl.appendChild(authorSpan);
      repoEl.appendChild(nameSpan);
    } else {
      repoEl.textContent = displayName;
    }

    // Add a subtle change button so the user can alter the HF repo.
    const changeBtn = document.createElement('span');
    changeBtn.className = 'hw-model-repo-change';
    changeBtn.textContent = '✎';
    changeBtn.title = 'Change HuggingFace repo';
    changeBtn.style.cssText =
      'margin-left:6px;font-size:10px;cursor:pointer;opacity:0.35;';
    changeBtn.addEventListener('mouseenter', () => { changeBtn.style.opacity = '1'; });
    changeBtn.addEventListener('mouseleave', () => { changeBtn.style.opacity = '0.35'; });
    changeBtn.addEventListener('click', () => {
      _openHwRepoEditor(repoEl, fullRepo || '');
    });
    repoEl.appendChild(changeBtn);
  }

  const quantRow = document.getElementById('hw-quant-row');
  const quantSelect = document.getElementById('hw-quant-select');
  const vramGb = effectiveAvailBytes() / (1024 ** 3);

  if (quantSelect && quantFiles && quantFiles.length > 1) {
    quantSelect.innerHTML = '';
    const loadedBasename = (wizardState.model.path || '').split(/[\\/]/).pop().toLowerCase();
    let matched = false;
    let recOpt = null;

    quantFiles.forEach(qf => {
      const fpath = qf.path || qf.name || '';
      const fname = fpath.split('/').pop();
      if (!fname) return;
      const opt = document.createElement('option');
      opt.value = fpath;
      const dispLabel = qf.label || fname;
      const sizeStr = qf.size ? ` · ${formatBytes(qf.size)}` : '';
      const isRec = qf.label && vramGb > 0 && qf.label === getRecommendedQuant(vramGb);
      opt.textContent = dispLabel + sizeStr + (isRec ? ' ★' : '');

      // Primary: match exact HF path
      if (fpath === hfFile) { opt.selected = true; matched = true; }
      // Secondary: local file basename matches
      else if (!matched && loadedBasename && fname.toLowerCase() === loadedBasename) {
        opt.selected = true; matched = true;
      }

      if (!recOpt && isRec) recOpt = opt;
      quantSelect.appendChild(opt);
    });

    // Tertiary: if still nothing selected, pick the VRAM-appropriate recommended quant
    if (!matched && recOpt) { recOpt.selected = true; }
    if (quantRow) quantRow.style.display = '';
  } else {
    if (quantRow) quantRow.style.display = 'none';
    const fileEl = document.getElementById('hw-model-file');
    if (fileEl) fileEl.textContent = hfFile ? hfFile.split('/').pop() : (path.split(/[/\\]/).pop() || '');
  }

  // Library tags row — refresh whenever origin is known (non-blocking async).
  _refreshHwTagsRow();

  // Show "Find on HuggingFace" row for local models that haven't loaded quant files yet.
  // Reset the swap-actions bar whenever we re-render the header.
  const localRow = document.getElementById('hw-quant-local-row');
  if (localRow) {
    const isLocal = source === 'local' || source === 'import';
    const hasQuants = quantFiles && quantFiles.length > 1;
    localRow.style.display = (isLocal && !hasQuants) ? '' : 'none';
  }
  const actionsRow = document.getElementById('hw-quant-swap-actions');
  if (actionsRow && actionsRow.style.display !== 'none') {
    // Keep visible only if user already selected a swap target
  }
}

// ── Hardware step: mmproj name-matching helper ────────────────────────────────

function _mmprojQuantLabel(file) {
  if (file.label) return String(file.label).toUpperCase();
  const name = (file.path || file.name || '').toUpperCase();
  if (name.includes('BF16')) return 'BF16';
  if (/(?:^|[-_.])F16(?:[-_.]|$)/.test(name)) return 'F16';
  if (name.includes('Q8_0')) return 'Q8_0';
  if (name.includes('F32')) return 'F32';
  return '';
}

function _preferredMmprojQuant(modelFilename = '') {
  const family = wizardState.model.family
    || _inferFamilyFromName(wizardState.model.hfRepo || '')
    || _inferFamilyFromName(modelFilename);
  if (family === 'qwen3.5' || family === 'qwen3.6') return 'F16';
  if (family === 'gemma4') return 'F16';
  return '';
}

function _isRecommendedMmproj(file, modelFilename = '') {
  if (file.is_recommended_mmproj) return true;
  const preferred = _preferredMmprojQuant(modelFilename);
  return !!preferred && _mmprojQuantLabel(file) === preferred;
}

function _mmprojPracticalRank(file) {
  return { F16: 0, BF16: 1, Q8_0: 2, F32: 3 }[_mmprojQuantLabel(file)] ?? 4;
}

// Return the mmproj file whose name shares the longest common prefix (after
// stripping quant suffix and normalising to alphanumeric) with the model stem.
// Returns null if the best match is shorter than 5 normalised characters —
// that threshold prevents grabbing a completely unrelated model's mmproj.
function _bestMmprojForModel(modelFilename, files) {
  if (!files.length) return null;
  const stem = _modelStemForSearch(modelFilename)
    .toLowerCase().replace(/[^a-z0-9]/g, '');
  if (!stem) return files.length === 1 ? files[0] : null;
  let best = null, bestScore = -1, bestRecommended = false, bestQuantRank = Infinity;
  for (const f of files) {
    const base = (f.path || f.name || '').split(/[\\/]/).pop() || '';
    const fstem = _modelStemForSearch(base)
      .toLowerCase().replace(/[^a-z0-9]/g, '');
    let score = 0;
    for (let i = 0; i < Math.min(stem.length, fstem.length); i++) {
      if (stem[i] === fstem[i]) score++;
      else break;
    }
    const recommended = _isRecommendedMmproj(f, modelFilename);
    const quantRank = _mmprojPracticalRank(f);
    if (score > bestScore
      || (score === bestScore && recommended && !bestRecommended)
      || (score === bestScore && recommended === bestRecommended && quantRank < bestQuantRank)) {
      bestScore = score;
      bestRecommended = recommended;
      bestQuantRank = quantRank;
      best = f;
    }
  }
  if (bestScore >= 5) return best;
  const recommended = files.filter(f => _isRecommendedMmproj(f, modelFilename));
  return recommended.length === 1 ? recommended[0] : null;
}

// Generic draft-assistant matching: score candidates by shared token overlap.
function _bestDraftForModel(modelFilename, candidates) {
  if (!candidates.length) return null;
  const stem = _modelStemForSearch(modelFilename)
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, ' ')
    .trim()
    .split(/\s+/)
    .filter(Boolean);

  const scoreCandidate = (f) => {
    const base = (f.path || f.name || '').split(/[\\/]/).pop() || '';
    const fstem = _modelStemForSearch(base)
      .toLowerCase()
      .replace(/[^a-z0-9-]/g, ' ')
      .trim()
      .split(/\s+/)
      .filter(Boolean);

    // Count shared tokens between model stem and candidate stem.
    const cset = new Set(fstem);
    let shared = 0;
    for (const t of stem) {
      if (cset.has(t)) shared++;
    }

    // Prefer smaller candidates (more likely real draft/assistant).
    const size = f.size || 0;
    const sizeBonus = size > 0 && size < 1_500_000_000 ? 1 : 0;

    return { shared, sizeBonus };
  };

  let best = null, bestShared = -1, bestSizeBonus = 0;
  for (const f of candidates) {
     const { shared, sizeBonus } = scoreCandidate(f);
     if (shared >= 3 &&
         (shared > bestShared || (shared === bestShared && sizeBonus > bestSizeBonus))) {
       bestShared = shared;
       bestSizeBonus = sizeBonus;
       best = f;
     }
   }

   // Allow a single candidate even with a weak score.
   if (best) return best;
   if (candidates.length === 1) return candidates[0];
   return null;
}

// ── Draft candidate pill buttons ─────────────────────────────────────────────

function _renderDraftCandidatePills() {
  const container = document.getElementById('spawn-draft-candidates');
  if (!container) return;

  const candidates = wizardState.model.draftCandidates || [];
  container.innerHTML = '';

  if (candidates.length === 0) {
    const emptySpan = document.createElement('span');
    emptySpan.style.cssText = 'color:var(--color-text-secondary);font-size:12px;';
    emptySpan.textContent = '(no draft model candidates detected)';
    container.appendChild(emptySpan);
    return;
  }

  candidates.forEach((candidate, index) => {
    const pill = document.createElement('button');
    pill.type = 'button';
    pill.className = 'btn-wizard-tertiary';
    pill.style.cssText =
      'font-size:11px;min-height:24px;padding:0 8px;cursor:pointer;border-radius:12px;' +
      'border:1px solid rgba(148,163,253,0.25);' +
      'background:rgba(80,120,200,0.15);color:var(--color-text-primary);white-space:nowrap;';
    
    const fname = (candidate.path || candidate.name || '').split(/[\\/]/).pop();
    const sizeStr = candidate.size ? ` · ${formatBytes(candidate.size)}` : '';
    pill.textContent = fname + sizeStr;
    pill.title = candidate.path || '';
    pill.dataset.index = index;

    pill.addEventListener('click', () => {
      wizardState.model.selectedDraftPath = candidate.path;
      if (dom.draftModelInput) dom.draftModelInput.value = candidate.path;
      scheduleVramUpdate();
    });

    container.appendChild(pill);
  });
}

// ── Hardware step: mmproj section ────────────────────────────────────────────

function renderMmprojSection() {
  const row = document.getElementById('hw-mmproj-row');
  if (!row) return;
  const files = wizardState.model.mmprojFiles || [];

  // When no local mmproj files exist, show a "download from HuggingFace" option
  // so users can fetch a companion mmproj for any model (especially ones that
  // were already downloaded without the mmproj).
  if (!files.length) {
    _renderMmprojDownloadFromHf(row);
    return;
  }
  row.style.display = '';

  // Clear any "download from HF" panel that was previously shown
  const hfPanel = row.querySelector('.hw-mmproj-hf-panel');
  if (hfPanel) hfPanel.remove();

  const select = document.getElementById('hw-mmproj-select');
  if (!select) return;
  const modelFilename = (wizardState.model.path || wizardState.model.hfFile || '')
    .split(/[\\/]/).pop() || '';
  const populationKey = `${files.length}:${_preferredMmprojQuant(modelFilename)}`;

  // Re-populate if the file list changed (e.g. after a companion download)
  if (select.dataset.populated !== populationKey) {
    select.dataset.populated = populationKey;
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
      const recommended = _isRecommendedMmproj(f, modelFilename);
      opt.textContent = fname + sizeStr + (recommended ? ' · Recommended' : '');
      if (recommended) {
        opt.title = f.mmproj_recommendation || `${_mmprojQuantLabel(f)} is preferred for this model family`;
      }
      if (fpath === (wizardState.model.mmprojPath || wizardState.model.mmprojHfFile)) opt.selected = true;
      select.appendChild(opt);
    });
    select.addEventListener('change', () => {
      const fpath = select.value;
      wizardState.model.mmprojHfFile = fpath;
      wizardState.model.mmprojPath = fpath;
      const f = files.find(x => (x.path || x.name) === fpath);
      wizardState.model.mmprojHfRepo =
        f?.repo_id || wizardState.model.originRepo || wizardState.model.hfRepo || '';
      wizardState.arch.mmprojBytes = f?.size ? Number(f.size) : 0;
      scheduleVramUpdate();
    });
  }

  // Sync selection state
  const active = wizardState.model.mmprojPath || wizardState.model.mmprojHfFile;
  if (active) select.value = active;

  // Auto-select using name proximity to the model file rather than
  // alphabetical order — avoids grabbing a different model's mmproj.
  if (!select.value && files.length) {
    const best = _bestMmprojForModel(modelFilename, files);
    if (best) {
      const bestPath = best.path || best.name || '';
      select.value = bestPath;
      wizardState.model.mmprojPath = bestPath;
      wizardState.model.mmprojHfFile = bestPath;
      wizardState.model.mmprojHfRepo =
        best.repo_id || wizardState.model.originRepo || wizardState.model.hfRepo || '';
      wizardState.arch.mmprojBytes = best.size ? Number(best.size) : 0;
      scheduleVramUpdate();
    }
  }
}

// Strip quant suffix from a model filename to produce a clean search query.
// e.g. "Qwopus3.6-27B-v2-MTP-Q8_0.gguf" → "Qwopus3.6-27B-v2-MTP"
function _modelStemForSearch(filename) {
  return filename
    .replace(/\.gguf$/i, '')
    .replace(/-(?:(?:UD-)?(?:IQ|Q)[0-9][A-Z0-9_]*|BF16|F16|FP16|FP32)$/i, '');
}

// POST to /api/hf/files and return parsed JSON, or null on error.
async function _hfFilesPost(repoId) {
  const headers = window.authHeaders ? window.authHeaders() : {};
  try {
    const res = await fetch('/api/hf/files', {
      method: 'POST',
      headers: { ...headers, 'Content-Type': 'application/json' },
      body: JSON.stringify({ repo_id: repoId }),
    });
    if (!res.ok) return null;
    return await res.json();
  } catch { return null; }
}

// ── HF tag → local library tag mapping ───────────────────────────────────────

// Maps raw HF tag strings to a normalised, user-facing category key.
// Only tags that have meaningful categorisation value are included —
// infrastructure tags (gguf, transformers, llama.cpp, region:us, etc.) are ignored.
// ── HF tag normalisation: curated map + block-list + pass-through ─────────────
//
// Architecture: three-tier system so we never need to pre-map every domain tag.
//
//  Tier 1 — Curated map: well-known HF tags → our 9 normalised category keys.
//            Covers the most common cross-model categories (vision, agentic…).
//
//  Tier 2 — Block-list: infrastructure / noise tags that carry no useful signal
//            for a local model library.  Every tag that survives this AND is not
//            in Tier 1 passes through as-is to Tier 3.
//
//  Tier 3 — Pass-through: domain tags not in either list (medical, legal, biomed,
//            translation, cybersecurity, …) are shown verbatim after normalisation.
//            No code change needed when new HF domains appear.

const HF_TAG_MAP = {
  // Vision / multimodal
  vision: 'vision', multimodal: 'vision', 'image-text-to-text': 'vision',
  image: 'vision', vqa: 'vision', visual: 'vision', 'text-to-image': 'vision',
  'image-to-text': 'vision', 'video-text-to-text': 'vision',
  // Agentic / tool use
  agent: 'agentic', agentic: 'agentic', 'tool-use': 'agentic',
  'function-calling': 'agentic', tool_use: 'agentic', tool_calling: 'agentic',
  // Coding
  coder: 'coding', code: 'coding', coding: 'coding',
  'code-generation': 'coding', devops: 'coding', programming: 'coding',
  'code-llm': 'coding',
  // Reasoning / thinking
  reasoning: 'reasoning', 'chain-of-thought': 'reasoning',
  thinking: 'reasoning', cot: 'reasoning', 'step-by-step': 'reasoning',
  // Roleplay / creative writing
  roleplay: 'roleplay', creative: 'roleplay', storytelling: 'roleplay',
  'creative-writing': 'roleplay', fiction: 'roleplay', 'role-playing': 'roleplay',
  // NSFW / adult content — 'not-for-all-audiences' is HF boilerplate for the same thing
  nsfw: 'nsfw', 'not-for-all-audiences': 'nsfw', adult: 'nsfw', explicit: 'nsfw',
  // Uncensored / guardrails removed — technique varies but concept is the same
  uncensored: 'uncensored', decensored: 'uncensored', abliterated: 'uncensored',
  heretic: 'uncensored', jailbreak: 'uncensored', 'no-refusals': 'uncensored',
  unfiltered: 'uncensored',
  // Math / STEM
  math: 'math', mathematics: 'math', science: 'math', stem: 'math',
  arithmetic: 'math',
  // General chat / instruction following
  conversational: 'chat', chat: 'chat', instruct: 'chat',
  'instruction-following': 'chat',
};

// Two-letter ISO language codes — blocked outright (not mapped to any category).
const ISO2_LANGS = new Set([
  'zh','ja','ko','ru','es','fr','de','ar','pt','it','nl','pl','sv','tr',
  'hi','vi','uk','cs','ro','hu','da','fi','no','id','th','he','el','bg',
  'en',
]);

// Tags that carry no useful signal for a local model library.
// Anything matching these patterns is silently dropped before pass-through.
const HF_TAG_BLOCKLIST = new Set([
  // File / library format
  'gguf','safetensors','pytorch','tflite','onnx','mlx','coreml','openvino',
  'ggml','llamafile',
  // Serving infrastructure
  'transformers','llama.cpp','text-generation-inference','vllm',
  'unsloth','ctransformers','ggerganov','endpoints_compatible',
  'text-generation','text2text-generation','fill-mask','token-classification',
  // Frontend clients (not model capabilities)
  'sillytavern','openwebui','open-webui','koboldcpp','ollama-library',
  // Training / alignment methodology (not a use-case)
  'lora','qlora','sft','rlhf','dpo','ppo','orpo','grpo','kto',
  'generated_from_trainer','adapter','merge','mergekit','finetuned',
  // Quantisation method tags (already captured by wizard hardware step)
  'imatrix','awq','gptq','eetq','exl2','nvfp4','fp8','int4','int8',
  // Model-card boilerplate
  'autotrain_compatible','has_space',
]);

// Regex patterns for tags that are always noise regardless of exact value.
const HF_TAG_BLOCK_PATTERNS = [
  /^base_model:/,      // base_model:owner/repo — parsed separately for inheritance
  /^dataset:/,         // dataset:owner/dataset
  /^license:/,         // license:apache-2.0 etc.
  /^region:/,          // region:us
  /^doi:/,
  /^arxiv:/,
  /^\d/,               // tags starting with a digit (version numbers etc.)
  /^[a-z]{2,3}_[A-Z]{2}$/,   // locale codes: zh_CN, pt_BR
  /^[a-z]{2}_[a-z]{2}$/,     // locale codes: zh_cn
  // Model family identifiers: llama3, llama-3, mistral7b, qwen3_6, gemma2, phi3…
  // Match known family names immediately followed by a digit or version separator.
  /^(llama|mistral|qwen|gemma|phi|falcon|gpt|bloom|mpt|opt|yi|deepseek|starcoder|codellama|vicuna|alpaca|wizardlm|orca|openchat|solar|nous|hermes|dolphin|beluga|airoboros|guanaco|koala|zephyr|stablelm|openhermes|chatml|neural|magnum|euryale|midnight|psyfighter|noromaid|lumimaid)[-_]?\d/i,
];

const HF_CATEGORY_LABEL = {
  vision: 'Vision',
  agentic: 'Agentic',
  coding: 'Coding',
  reasoning: 'Reasoning',
  roleplay: 'Roleplay',
  nsfw: 'NSFW',
  uncensored: 'Uncensored',
  math: 'Math/STEM',
  chat: 'Chat',
};

// Standard vocabulary always offered in the picker, independent of HF.
const ALL_KNOWN_TAGS = [
  'coding', 'roleplay', 'nsfw', 'uncensored', 'general', 'art', 'fast', 'default',
  'vision', 'agentic', 'reasoning', 'math', 'chat',
];

// Normalise a raw HF tag string for use as a local library tag:
// lowercase, spaces/& stripped to hyphens, trailing hyphens removed.
function _normaliseTag(raw) {
  return raw
    .toLowerCase()
    .replace(/[&/\\]/g, '-')
    .replace(/\s+/g, '-')
    .replace(/-{2,}/g, '-')
    .replace(/^-|-$/g, '');
}

// Returns true if the tag should be silently dropped.
function _isBlockedHfTag(raw) {
  const lower = raw.toLowerCase();
  if (HF_TAG_BLOCKLIST.has(lower)) return true;
  if (HF_TAG_BLOCK_PATTERNS.some(re => re.test(raw))) return true;
  return false;
}

// Analyse raw HF tags and return:
//   { categories: Set<string>, passthrough: string[] }
// categories — matched curated keys (vision, agentic …)
// passthrough — normalised tags that are not blocked and not mapped to a category;
//               these are shown verbatim in the "From this model" picker section.
function _hfTagsToCategories(rawTags) {
  const categories = new Set();
  const passthroughSet = new Set();

  for (const raw of rawTags) {
    const lower = raw.toLowerCase();
    if (_isBlockedHfTag(raw)) continue;
    if (ISO2_LANGS.has(lower)) continue;

    const cat = HF_TAG_MAP[lower];
    if (cat) {
      categories.add(cat);
    } else {
      // Pass-through: normalise and keep if it's a non-trivial string
      const norm = _normaliseTag(raw);
      if (norm.length >= 2 && !ALL_KNOWN_TAGS.includes(norm)) {
        passthroughSet.add(norm);
      }
    }
  }

  // Remove passthrough entries that are already covered by a category label
  for (const cat of categories) {
    const label = _normaliseTag(HF_CATEGORY_LABEL[cat] || cat);
    passthroughSet.delete(label);
  }

  return { categories, passthrough: [...passthroughSet] };
}

// Fetch tags for a HF repo and, if it has a base_model: tag pointing to a
// non-quantized source, merge that source's tags too (one level only).
// Quantizers like bartowski often strip use-case tags, but the base model keeps them.
// Returns { categories: Set, passthrough: string[] }.
async function _fetchHfTagsWithBaseModel(repoId) {
  const headers = window.authHeaders ? window.authHeaders() : {};
  let rawTags = [];
  try {
    const r = await fetch(`/api/hf/meta?repo=${encodeURIComponent(repoId)}`, { headers });
    if (r.ok) {
      const d = await r.json().catch(() => ({}));
      if (d.ok && d.tags) rawTags = d.tags;
    }
  } catch { /* non-fatal */ }

  // Look for base_model:owner/repo (not base_model:quantized: or base_model:adapter:)
  const baseTag = rawTags.find(t => {
    if (!t.startsWith('base_model:')) return false;
    const rest = t.slice('base_model:'.length);
    return !rest.startsWith('quantized:') && !rest.startsWith('adapter:') && rest.includes('/');
  });
  if (baseTag) {
    const baseRepo = baseTag.slice('base_model:'.length);
    try {
      const r = await fetch(`/api/hf/meta?repo=${encodeURIComponent(baseRepo)}`, { headers });
      if (r.ok) {
        const d = await r.json().catch(() => ({}));
        if (d.ok && d.tags) rawTags = [...rawTags, ...d.tags];
      }
    } catch { /* non-fatal */ }
  }

  return _hfTagsToCategories(rawTags);
}

// Fetch HF model metadata and render the tags row for the hardware step.
// No-ops if originRepo is unknown or the row element is missing.
let _tagsRowOrigin = ''; // track which repo is currently loaded in the row

// Inline repo editor / picker: always shows candidates + custom input.
  //  - Searches by filename, builds a short candidate list.
  //  - If currentRepo is known, it is first and marked recommended.
   //  - User can pick from a dropdown or type a custom repo ID.
   function _openHwRepoEditor(repoEl, currentRepo) {
     if (!repoEl) return;
     if (repoEl.classList.contains('hw-model-repo-editing')) return;

     repoEl.classList.add('hw-model-repo-editing');
     repoEl.innerHTML = '';

     const statusEl = document.createElement('span');
     statusEl.style.cssText =
       'font-size:10px;color:var(--color-text-muted);margin-left:6px;white-space:nowrap;';
     statusEl.textContent = 'Searching HuggingFace…';
     repoEl.appendChild(statusEl);

     const restore = () => {
       repoEl.classList.remove('hw-model-repo-editing');
       renderHardwareModelHeader();
     };

     const filename = (wizardState.model.path || '').split(/[\\/]/).pop() || '';
     const modelBytes = wizardState.model.modelBytes || 0;

     // Use resolve-origin to get ranked, verified candidates.
     const fetchCandidates = async () => {
       const headers = window.authHeaders ? { ...window.authHeaders(), 'Content-Type': 'application/json' } : { 'Content-Type': 'application/json' };
       try {
         const res = await fetch('/api/hf/resolve-origin', {
           method: 'POST',
           headers,
           body: JSON.stringify({ filename, size_bytes: modelBytes }),
         });
         if (!res.ok) return { confident: false, candidates: [] };
         return await res.json();
       } catch {
         return { confident: false, candidates: [] };
       }
     };

     // Apply a selected repo: fetch files, validate, and set wizardState.
     const applyRepo = async (repoId) => {
       if (!repoId) return;
       try {
         const data = await _hfFilesPost(repoId);

         if (!data?.ok) {
           showToast('Repo not found', 'error', 'Check the repo ID and try again.');
           restore();
           return;
         }

         const rawFiles = (data.files || []).filter(f =>
           !f.is_mmproj && (f.rfilename || f.path || '').toLowerCase().endsWith('.gguf'));
         if (!rawFiles.length) {
           showToast('No GGUFs', 'error', 'No GGUF files found in this repo.');
           restore();
           return;
         }

         wizardState.model.originRepo = repoId;
         wizardState.model.hfRepo = repoId;
         wizardState.model.quantFiles = rawFiles.map(f => ({
           path: f.rfilename || f.path || '',
           name: f.rfilename || f.path || '',
           size: f.size || 0,
           label: _extractQuantLabel(f.rfilename || f.path || ''),
         }));
         wizardState.model._quantSwapRepo = repoId;

         wizardState.model.mmprojFiles = (data.files || []).filter(f => f.is_mmproj).map(f => ({
           repo_id: f.repo_id || repoId,
           path: f.rfilename || f.path || '',
           name: f.rfilename || f.path || '',
           size: f.size || 0,
           is_mmproj: true,
           is_recommended_mmproj: f.is_recommended_mmproj || false,
           mmproj_recommendation: f.mmproj_recommendation || '',
         }));

          _tagsRowOrigin = '';
         _removeInlineOriginHint();
         showToast('Repo updated', 'success', `${rawFiles.length} quants loaded`);
         renderHardwareModelHeader();
       } catch {
         showToast('Error', 'error', 'Failed to load repo.');
         restore();
       }
     };

     // Render picker as dropdown with recommended + others + custom input.
     const renderPicker = (resolveData) => {
       repoEl.innerHTML = '';

       const wrap = document.createElement('span');
       wrap.style.cssText =
         'display:inline-flex;align-items:center;gap:4px;margin-left:4px;flex-wrap:wrap;';

       const candidates = (resolveData.candidates || []).slice(0, 5);
       const confident = !!resolveData.confident;
       const recommendedRepo = confident && candidates.length > 0 ? candidates[0].repoId : (currentRepo || (candidates.length > 0 ? candidates[0].repoId : ''));

       if (candidates.length > 0) {
         // Dropdown with full author/repo.
         const select = document.createElement('select');
         select.style.cssText =
           'font-size:9px;padding:1px 4px;border-radius:3px;border:1px solid rgba(148,163,253,0.4);' +
           'background:rgba(15,23,42,0.98);color:var(--color-text-primary);min-height:18px;';

         const defaultOption = document.createElement('option');
         defaultOption.value = '';
         defaultOption.textContent = 'Select origin…';
         select.appendChild(defaultOption);

         candidates.forEach((c, i) => {
           const repoId = c.repoId || '';
           const opt = document.createElement('option');
           opt.value = repoId;
           const isRecommended = !!(confident && i === 0);
           opt.textContent = (isRecommended ? '★ Recommended: ' : '') + repoId;
           if (repoId === recommendedRepo) {
             opt.selected = true;
           }
           select.appendChild(opt);
         });

         select.addEventListener('change', () => {
           const repoId = select.value || recommendedRepo;
           if (repoId && repoId !== wizardState.model.originRepo) {
             applyRepo(repoId);
           }
         });

         wrap.appendChild(select);
       }

       // Separator
       const sep = document.createElement('span');
       sep.style.cssText = 'font-size:8px;color:var(--color-text-muted);margin:0 1px;';
       sep.textContent = '|';
       wrap.appendChild(sep);

       // Custom input for typing any HF repo ID.
       const input = document.createElement('input');
       input.type = 'text';
       input.value = currentRepo || '';
       input.placeholder = 'owner/repo-GGUF';
       input.style.cssText =
         'width:160px;padding:2px 5px;border-radius:3px;border:1px solid rgba(148,163,253,0.4);' +
         'background:rgba(15,23,42,0.95);color:var(--color-text-primary);font-size:9px;';

       const loadBtn = document.createElement('button');
       loadBtn.type = 'button';
       loadBtn.className = 'btn-wizard-tertiary';
       loadBtn.style.cssText =
         'font-size:9px;min-height:18px;padding:1px 5px;flex-shrink:0;';
       loadBtn.textContent = 'Load';

       const cancelBtn = document.createElement('button');
       cancelBtn.type = 'button';
       cancelBtn.className = 'btn-wizard-tertiary';
       cancelBtn.style.cssText =
         'font-size:9px;min-height:18px;padding:1px 5px;flex-shrink:0;opacity:0.7;';
       cancelBtn.textContent = '✕';

       wrap.appendChild(input);
       wrap.appendChild(loadBtn);
       wrap.appendChild(cancelBtn);
       repoEl.appendChild(wrap);

       input.focus();
       input.select();

       const doLoad = async () => {
         const repoId = input.value.trim();
         if (!repoId) return;
         loadBtn.disabled = true;
         loadBtn.textContent = '⠋';

         try {
           await applyRepo(repoId);
         } catch {
           loadBtn.disabled = false;
           loadBtn.textContent = 'Load';
           showToast('Error', 'error', 'Failed to load repo.');
           restore();
         }
       };

       loadBtn.addEventListener('click', doLoad);
       cancelBtn.addEventListener('click', restore);
       input.addEventListener('keydown', e => {
         if (e.key === 'Enter') { e.preventDefault(); doLoad(); }
         if (e.key === 'Escape') { restore(); }
       });
     };

     (async () => {
       const resolveData = await fetchCandidates();
       renderPicker(resolveData);
     })();
   }

async function _refreshHwTagsRow() {
  const row = document.getElementById('hw-tags-row');
  if (!row) return;
  const { originRepo, path, cardUrl, family } = wizardState.model;
  if (!originRepo) { row.style.display = 'none'; return; }
  row.style.display = '';
  if (_tagsRowOrigin === originRepo) return; // already populated for this repo
  _tagsRowOrigin = originRepo;

  // Load current library tags for this model path — exclude system tags.
  let currentTags = [];
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const r = await fetch('/api/models/tags', { headers });
    if (r.ok) {
      const d = await r.json().catch(() => ({}));
      currentTags = (d.tags?.[path] || []).filter(
        t => !t.startsWith('hf_origin:') && !t.startsWith('family:')
      );
    }
  } catch { /* non-fatal */ }

  // Fetch HF tags (including base model tags if the GGUF repo stripped them).
  let suggestedCats = new Set();
  let passthroughTags = [];
  try {
    ({ categories: suggestedCats, passthrough: passthroughTags } =
      await _fetchHfTagsWithBaseModel(originRepo));
  } catch { /* non-fatal */ }

  _renderHwTagPills(currentTags, suggestedCats, passthroughTags, path, originRepo);

  // Append family + card link pills
  const pillsWrap = document.getElementById('hw-tags-pills');
  if (pillsWrap) {
    if (family) {
      const famPill = document.createElement('span');
      famPill.className = 'mm-tag-pill';
      famPill.style.cssText = 'font-size:9px;padding:2px 7px;opacity:0.6;white-space:nowrap;';
      famPill.textContent = `family: ${family}`;
      famPill.title = `Detected model family (auto)`;
      pillsWrap.appendChild(famPill);
    }
    // Card pill: opens inline model card panel (uses existing wizard-card-panel)
    if (originRepo) {
      const cardPill = document.createElement('button');
      cardPill.type = 'button';
      cardPill.className = 'mm-tag-pill';
      cardPill.style.cssText = 'font-size:9px;padding:2px 7px;cursor:pointer;text-decoration:none;opacity:0.7;white-space:nowrap;border:none;background:none;font:inherit;';
      cardPill.textContent = '📄 Card';
      cardPill.title = 'View model card inline';
      cardPill.addEventListener('click', () => openCardPanel(originRepo));
      pillsWrap.appendChild(cardPill);
    }
  }
}

function _renderHwTagPills(currentTags, suggestedCats, passthroughTags, modelPath, originRepo) {
  const pillsWrap = document.getElementById('hw-tags-pills');
  if (!pillsWrap) return;
  pillsWrap.innerHTML = '';

  if (!currentTags.length) {
    // "No tags yet" hint
    const hint = document.createElement('span');
    hint.style.cssText = 'font-size:10px;color:var(--color-text-muted);margin-right:4px;';
    hint.textContent = 'No tags yet';
    pillsWrap.appendChild(hint);
  } else {
    // Render existing tag pills
    currentTags.forEach(tag => {
      const pill = document.createElement('span');
      pill.className = 'mm-tag-pill mm-tag-pill--active';
      pill.style.cssText = 'font-size:9px;padding:2px 7px;cursor:pointer;';
      pill.title = `Remove tag "${tag}"`;
      pill.textContent = tag + ' ×';
      pill.addEventListener('click', async () => {
        const newTags = currentTags.filter(t => t !== tag);
        await _saveHwModelTags(modelPath, newTags);
        _tagsRowOrigin = '';
        await _refreshHwTagsRow();
      });
      pillsWrap.appendChild(pill);
    });
  }

  // "Sync with HF" pill: shown whenever HF origin is known and HF offers tags.
  // This lets the user pull new tags OR refresh/remove tags the author changed.
  if (originRepo && (suggestedCats.size > 0 || (passthroughTags && passthroughTags.length > 0))) {
    const syncBtn = document.createElement('button');
    syncBtn.type = 'button';
    syncBtn.className = 'mm-tag-pill';
    syncBtn.style.cssText =
      'font-size:9px;padding:2px 7px;cursor:pointer;text-decoration:none;opacity:0.75;white-space:nowrap;border:none;background:none;font:inherit;display:inline-flex;align-items:center;gap:4px;';
    syncBtn.textContent = '⎇ Sync with HF';
    syncBtn.title = 'Sync library tags with this model\'s HuggingFace card';

    syncBtn.addEventListener('click', async () => {
      try {
        const hfResult = await _fetchHfTagsWithBaseModel(originRepo);
        const cats = Array.from(hfResult.categories || []);
        const pass = Array.from(hfResult.passthrough || []);
        const hfSet = new Set([...cats, ...pass]);

        // Re-read current tags to ensure we’re in sync.
        const headers = window.authHeaders ? window.authHeaders() : {};
        const r = await fetch('/api/models/tags', { headers });
        const td = r.ok ? await r.json().catch(() => ({})) : {};
        const allCurrent = (td.tags?.[modelPath] || []).filter(
          t => !t.startsWith('hf_origin:')
        );

        // Sync logic:
        // - Keep all core / user-friendly tags (ALL_KNOWN_TAGS).
        // - Keep any HF tags still present on the card.
        // - Drop any tag not in HF and not in ALL_KNOWN_TAGS
        //   so that removed / outdated tags are cleaned up.
        const newTags = allCurrent.filter(tag => {
          const inHf = hfSet.has(tag);
          const inCore = ALL_KNOWN_TAGS.includes(tag);
          return inHf || inCore;
        });

        // Add any HF tags not yet present.
        for (const t of hfSet) {
          if (!newTags.includes(t)) newTags.push(t);
        }

        if (newTags.length === 0) return;

        await _saveHwModelTags(modelPath, newTags);
        _tagsRowOrigin = '';
        await _refreshHwTagsRow();
      } catch {
        // Non-fatal: user can still use the manual tag picker.
      }
    });

    pillsWrap.appendChild(syncBtn);
  }
}

async function _saveHwModelTags(modelPath, tags) {
  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const r = await fetch('/api/models/tags', {
      headers,
      method: 'GET',
    });
    const existing = r.ok ? ((await r.json().catch(() => ({}))).tags?.[modelPath] || []) : [];
    const originTags = existing.filter(t => t.startsWith('hf_origin:'));
    const merged = [...originTags, ...tags.filter(t => !t.startsWith('hf_origin:'))];
    await fetch('/api/models/tags', {
      method: 'PUT',
      headers,
      body: JSON.stringify({ model_path: modelPath, tags: merged }),
    });
  } catch { /* non-fatal */ }
}

function _openHwTagPicker(btn, modelPath, originRepo) {
  // Remove any existing picker.
  document.getElementById('hw-tag-picker-popup')?.remove();

  const popup = document.createElement('div');
  popup.id = 'hw-tag-picker-popup';
  popup.className = 'hw-tag-picker-popup';

  // Fetch current state fresh so the picker reflects reality.
  const headers = window.authHeaders ? window.authHeaders() : {};
  Promise.all([
    fetch('/api/models/tags', { headers }).then(r => r.ok ? r.json().catch(() => ({})) : {}),
    originRepo ? _fetchHfTagsWithBaseModel(originRepo).catch(() => ({ categories: new Set(), passthrough: [] })) : Promise.resolve({ categories: new Set(), passthrough: [] }),
  ]).then(([tagsData, hfResult]) => {
    const rawCurrent = (tagsData.tags?.[modelPath] || [])
      .filter(t => !t.startsWith('hf_origin:'));
    const { categories: suggestedCats, passthrough: passthroughTags } = hfResult;

    popup.innerHTML = '';

    // ── Section: Curated HF suggestions ──────────────────────────────────────
    if (suggestedCats.size > 0) {
      const hdr = document.createElement('div');
      hdr.className = 'hw-tag-picker-section';
      hdr.textContent = `Suggested from ${originRepo.split('/')[0]}`;
      popup.appendChild(hdr);

      const sugRow = document.createElement('div');
      sugRow.className = 'hw-tag-picker-pills';
      [...suggestedCats].forEach(cat => {
        _appendTagPill(sugRow, HF_CATEGORY_LABEL[cat] || cat, cat, rawCurrent, modelPath, originRepo, popup);
      });
      popup.appendChild(sugRow);
    }

    // ── Section: Pass-through domain tags (medical, legal, biomed, …) ────────
    // These are HF tags that didn't map to a curated category but aren't noise.
    // Shown here so they can be applied without needing the custom input.
    if (passthroughTags.length > 0) {
      const hdr = document.createElement('div');
      hdr.className = 'hw-tag-picker-section';
      hdr.textContent = 'From this model';
      popup.appendChild(hdr);

      const ptRow = document.createElement('div');
      ptRow.className = 'hw-tag-picker-pills';
      passthroughTags.forEach(tag => {
        _appendTagPill(ptRow, tag, tag, rawCurrent, modelPath, originRepo, popup);
      });
      popup.appendChild(ptRow);
    }

    // ── Section: Standard vocabulary ─────────────────────────────────────────
    const hdr2 = document.createElement('div');
    hdr2.className = 'hw-tag-picker-section';
    hdr2.textContent = 'All tags';
    popup.appendChild(hdr2);

    const allRow = document.createElement('div');
    allRow.className = 'hw-tag-picker-pills';
    // Exclude tags already shown in the HF or pass-through sections to avoid duplication.
    const shownKeys = new Set([...suggestedCats, ...passthroughTags]);
    ALL_KNOWN_TAGS.filter(t => !shownKeys.has(t)).forEach(tag => {
      _appendTagPill(allRow, tag, tag, rawCurrent, modelPath, originRepo, popup);
    });
    popup.appendChild(allRow);

    // ── Custom tag input ──────────────────────────────────────────────────────
    const customRow = document.createElement('div');
    customRow.style.cssText = 'display:flex;gap:5px;margin-top:7px;';
    const customInput = document.createElement('input');
    customInput.type = 'text';
    customInput.placeholder = 'Custom tag…';
    customInput.style.cssText = 'flex:1;padding:4px 7px;border-radius:5px;border:1px solid rgba(255,255,255,0.1);background:rgba(28,34,42,0.9);color:var(--color-text-primary);font-size:10px;';
    const addBtn = document.createElement('button');
    addBtn.type = 'button';
    addBtn.className = 'btn-wizard-secondary';
    addBtn.style.cssText = 'font-size:10px;min-height:24px;padding:2px 8px;flex-shrink:0;';
    addBtn.textContent = 'Add';

    const doAdd = async () => {
      const val = customInput.value.trim().toLowerCase().replace(/\s+/g, '-');
      if (!val || rawCurrent.includes(val)) return;
      rawCurrent.push(val);
      await _saveHwModelTags(modelPath, rawCurrent);
      _tagsRowOrigin = ''; // force re-render of pills row
      await _refreshHwTagsRow();
      popup.remove();
      showToast(`Tagged "${val}"`, 'success');
    };
    addBtn.addEventListener('click', doAdd);
    customInput.addEventListener('keydown', e => { if (e.key === 'Enter') doAdd(); });
    customRow.appendChild(customInput);
    customRow.appendChild(addBtn);
    popup.appendChild(customRow);
  });

  // Position below the + button.
  document.body.appendChild(popup);
  const rect = btn.getBoundingClientRect();
  popup.style.left = `${rect.left}px`;
  popup.style.top = `${rect.bottom + 4}px`;

  // Close on outside click.
  setTimeout(() => {
    const close = e => {
      if (!popup.contains(e.target) && e.target !== btn) {
        popup.remove();
        document.removeEventListener('mousedown', close);
      }
    };
    document.addEventListener('mousedown', close);
  }, 0);
}

function _appendTagPill(container, label, tagKey, currentTags, modelPath, originRepo, popup) {
  const has = currentTags.includes(tagKey);
  const pill = document.createElement('span');
  pill.className = 'mm-tag-pill' + (has ? ' mm-tag-pill--active' : '');
  pill.style.cssText = 'font-size:10px;padding:3px 9px;cursor:pointer;user-select:none;';
  pill.textContent = label;
  pill.title = has ? `Remove tag "${tagKey}"` : `Add tag "${tagKey}"`;
  pill.addEventListener('click', async () => {
    const newTags = has
      ? currentTags.filter(t => t !== tagKey)
      : [...currentTags, tagKey];
    // Mutate in place so other pills in the same picker stay in sync.
    currentTags.length = 0;
    newTags.forEach(t => currentTags.push(t));
    await _saveHwModelTags(modelPath, newTags);
    pill.className = 'mm-tag-pill' + (newTags.includes(tagKey) ? ' mm-tag-pill--active' : '');
    pill.title = newTags.includes(tagKey) ? `Remove tag "${tagKey}"` : `Add tag "${tagKey}"`;
    // Refresh the pills row in the header.
    _tagsRowOrigin = '';
    _refreshHwTagsRow();
  });
  container.appendChild(pill);
}

// ── Local-model quant-swap discovery ─────────────────────────────────────────

// Extract a short quant label from a GGUF filename, e.g. "Q4_K_M" or "IQ3_M".
function _extractQuantLabel(filename) {
  const fname = (filename.split('/').pop() || filename).replace(/\.gguf$/i, '');
  const m = fname.match(/[-_]((?:UD-)?(?:IQ|Q)\d[\w.]*|BF16|F16|FP16|FP32)(?:[-_.]|$)/i);
  return m ? m[1].toUpperCase() : fname;
}

let _lastQuantSearchFile = ''; // prevent redundant searches
let _quantSwapSearching = false;

async function _autoDiscoverLocalModelQuants() {
  if (_quantSwapSearching) return;
  const { source, path, originRepo } = wizardState.model;
  if (source !== 'local' && source !== 'import') return;

  const filename = (path || '').split(/[\\/]/).pop() || '';
  if (!filename || filename === _lastQuantSearchFile) return;
  _lastQuantSearchFile = filename;
  _quantSwapSearching = true;

  const statusEl = document.getElementById('hw-quant-local-status');
  const btn = document.getElementById('hw-quant-local-btn');
  if (statusEl) statusEl.textContent = 'Searching HuggingFace…';
  if (btn) btn.disabled = true;

  try {
        let repoId = originRepo || '';
        let rawFiles = [];
        let showCandidateList = false;

        // 1) If originRepo is already known (e.g., from pencil editor), use it.
        //    Even a single GGUF is acceptable since this is the confirmed source.
        if (repoId) {
          const data = await _hfFilesPost(repoId);
          if (data?.ok) {
            rawFiles = (data.files || []).filter(f =>
              !f.is_mmproj && (f.rfilename || f.path || '').toLowerCase().endsWith('.gguf'));
          }
        }

        // 2) If no originRepo or it has no GGUFs, use resolve-origin to search.
        if (!rawFiles.length && !repoId) {
          const headers = window.authHeaders ? { ...window.authHeaders(), 'Content-Type': 'application/json' } : { 'Content-Type': 'application/json' };
          const modelBytes = wizardState.model.modelBytes || 0;
          const res = await fetch('/api/hf/resolve-origin', {
            method: 'POST',
            headers,
            body: JSON.stringify({ filename, size_bytes: modelBytes }),
          });

          if (res.ok) {
            const data = await res.json();
            const candidates = (data.candidates || [])
              .slice(0, 5)
              .map(c => ({ repoId: c.repoId, confidence: c.confidence }));

            if (data.confident && candidates.length > 0) {
              // Confident match: use top candidate
              repoId = candidates[0].repoId;
              const fd = await _hfFilesPost(repoId);
              if (fd?.ok) {
                rawFiles = (fd.files || []).filter(f =>
                  !f.is_mmproj && (f.rfilename || f.path || '').toLowerCase().endsWith('.gguf'));
              }
            } else if (candidates.length > 1) {
              // Multiple plausible repos: show list of those with ≥1 GGUF
              const withFiles = [];
              for (const c of candidates) {
                const fd = await _hfFilesPost(c.repoId);
                if (!fd?.ok) continue;
                const gguf = (fd.files || []).filter(f =>
                  !f.is_mmproj && (f.rfilename || f.path || '').toLowerCase().endsWith('.gguf'));
                if (gguf.length >= 1) {
                  withFiles.push({ repoId: c.repoId, ggufFiles: gguf });
                }
              }
              if (withFiles.length > 0) {
                if (statusEl) statusEl.textContent = 'Multiple possible sources found';
                _showQuantSwapCandidateList(withFiles);
                showCandidateList = true;
              }
            } else if (candidates.length === 1) {
              // Single candidate with ≥1 GGUF
              const c = candidates[0];
              const fd = await _hfFilesPost(c.repoId);
              if (fd?.ok) {
                const gguf = (fd.files || []).filter(f =>
                  !f.is_mmproj && (f.rfilename || f.path || '').toLowerCase().endsWith('.gguf'));
                if (gguf.length >= 1) {
                  repoId = c.repoId;
                  rawFiles = gguf;
                }
              }
            }
          }
        }

      if (showCandidateList) {
        return;
      }

      // 3) If we have any GGUFs from the chosen repo, use it.
      if (rawFiles.length > 0 && repoId) {
        wizardState.model.quantFiles = rawFiles.map(f => ({
          path: f.rfilename || f.path || '',
          name: f.rfilename || f.path || '',
          size: f.size || 0,
          label: _extractQuantLabel(f.rfilename || f.path || ''),
        }));
        wizardState.model._quantSwapRepo = repoId;

        // Pre-select the entry matching the currently loaded local file.
        const currentLower = filename.toLowerCase();
        const match = rawFiles.find(f =>
          (f.rfilename || f.path || '').split('/').pop().toLowerCase() === currentLower);
        if (match) wizardState.model.hfFile = match.rfilename || match.path || '';

        if (statusEl) statusEl.textContent = rawFiles.length === 1
            ? 'Only your current quant is available'
            : `${rawFiles.length} quants found`;
        setTimeout(() => { if (statusEl) statusEl.textContent = ''; }, 3000);
        renderHardwareModelHeader();
      } else {
        // 4) Fallback: let user type a repo manually.
        if (statusEl) statusEl.textContent = 'Not found — type or paste the repo:';
        _showQuantSwapManualInput();
      }
   } catch {
     if (statusEl) statusEl.textContent = 'Search failed';
   } finally {
     _quantSwapSearching = false;
     if (btn) btn.disabled = false;
   }
}

// Show a compact list of candidate repos when multiple are found.
// Includes a "Not this one?" option to let user type manually.
// No auto-select: user must choose explicitly.
function _showQuantSwapCandidateList(candidates) {
  const row = document.getElementById('hw-quant-local-row');
  if (!row) return;
  const btn = row.querySelector('#hw-quant-local-btn');
  if (!btn) return;

  // Clear existing controls in this row and replace with list.
  row.innerHTML = '';
  row.style.display = '';

  const listWrap = document.createElement('div');
  listWrap.style.cssText =
    'display:flex;flex-direction:column;gap:4px;margin-top:4px;min-width:0;';

  const selectCandidate = (candidate) => {
    wizardState.model.quantFiles = candidate.ggufFiles.map(f => ({
      path: f.rfilename || f.path || '',
      name: f.rfilename || f.path || '',
      size: f.size || 0,
      label: _extractQuantLabel(f.rfilename || f.path || ''),
    }));
    wizardState.model._quantSwapRepo = candidate.repoId;

    // Pre-select the entry matching the currently loaded local file.
    const filename = (wizardState.model.path || '').split(/[\\/]/).pop() || '';
    const currentLower = filename.toLowerCase();
    const match = candidate.ggufFiles.find(f =>
      (f.rfilename || f.path || '').split('/').pop().toLowerCase() === currentLower);
    if (match) wizardState.model.hfFile = match.rfilename || match.path || '';

    const statusEl = document.getElementById('hw-quant-local-status');
    if (statusEl) statusEl.textContent = `${candidate.ggufFiles.length} quants selected`;
    setTimeout(() => { if (statusEl) statusEl.textContent = ''; }, 3000);
    renderHardwareModelHeader();
  };

  // Render candidate options (no auto-select).
  candidates.forEach((candidate, index) => {
    const item = document.createElement('div');
    item.style.cssText =
      'display:flex;justify-content:space-between;align-items:center;padding:4px 6px;' +
      'border-radius:4px;border:1px solid rgba(255,255,255,0.1);cursor:pointer;' +
      'background:rgba(15,23,42,0.6);font-size:10px;';

    const repoText = document.createElement('span');
    repoText.style.cssText = 'flex:1;min-width:0;overflow:hidden;white-space:nowrap;text-overflow:ellipsis;';
    repoText.textContent = candidate.repoId;

    const meta = document.createElement('span');
    meta.style.cssText =
      'margin-left:6px;font-size:9px;color:var(--color-text-muted);flex-shrink:0;';
    meta.textContent = `${candidate.ggufFiles.length} GGUFs`;

    item.appendChild(repoText);
    item.appendChild(meta);

    item.addEventListener('click', () => {
      selectCandidate(candidate);
    });

    listWrap.appendChild(item);
  });

  // "Not this one?" option → manual input.
  const notThisOne = document.createElement('div');
  notThisOne.style.cssText =
    'margin-top:2px;font-size:9px;color:var(--color-text-muted);cursor:pointer;' +
    'text-decoration:underline;text-underline-offset:2px;';
  notThisOne.textContent = 'Not the right repository? Enter it manually…';
  notThisOne.addEventListener('click', () => {
    listWrap.innerHTML = '';
    _showQuantSwapManualInput();
  });
  listWrap.appendChild(notThisOne);

  row.appendChild(listWrap);
}

function _showQuantSwapManualInput() {
  const row = document.getElementById('hw-quant-local-row');
  if (!row) return;
  const btn = row.querySelector('#hw-quant-local-btn');
  if (!btn) return;

  const wrap = document.createElement('span');
  wrap.style.cssText = 'display:flex;gap:5px;align-items:center;flex:1;';
  const input = document.createElement('input');
  input.type = 'text';
  input.placeholder = 'owner/repo-GGUF';
  input.style.cssText = 'flex:1;padding:4px 8px;border-radius:5px;border:1px solid rgba(255,255,255,0.1);background:rgba(28,34,42,0.9);color:var(--color-text-primary);font-size:10px;min-width:0;';
  const goBtn = document.createElement('button');
  goBtn.type = 'button';
  goBtn.className = 'btn-wizard-tertiary';
  goBtn.style.cssText = 'font-size:10px;min-height:22px;padding:2px 8px;flex-shrink:0;';
  goBtn.textContent = 'Load';
  wrap.appendChild(input);
  wrap.appendChild(goBtn);
  btn.replaceWith(wrap);

  const statusEl = document.getElementById('hw-quant-local-status');

  const doFetch = async () => {
    const repoId = input.value.trim();
    if (!repoId) return;
    goBtn.disabled = true;
    if (statusEl) statusEl.textContent = 'Loading…';
    const data = await _hfFilesPost(repoId);
    goBtn.disabled = false;
    if (!data?.ok) { if (statusEl) statusEl.textContent = 'Repo not found'; return; }
    const rawFiles = (data.files || []).filter(f =>
      !f.is_mmproj && (f.rfilename || f.path || '').toLowerCase().endsWith('.gguf'));
    if (!rawFiles.length) { if (statusEl) statusEl.textContent = 'No GGUFs found'; return; }
    wizardState.model.quantFiles = rawFiles.map(f => ({
      path: f.rfilename || f.path || '',
      name: f.rfilename || f.path || '',
      size: f.size || 0,
      label: _extractQuantLabel(f.rfilename || f.path || ''),
    }));
    wizardState.model._quantSwapRepo = repoId;
    if (statusEl) statusEl.textContent = '';
    renderHardwareModelHeader();
  };
  goBtn.addEventListener('click', doFetch);
  input.addEventListener('keydown', e => { if (e.key === 'Enter') doFetch(); });
}

function _renderQuantSwapActions(quantPath, repoId) {
  const actionsRow = document.getElementById('hw-quant-swap-actions');
  if (!actionsRow) return;
  const quantName = quantPath.split('/').pop() || quantPath;

  actionsRow.innerHTML = '';
  actionsRow.style.display = '';

  const dlBtn = document.createElement('button');
  dlBtn.type = 'button';
  dlBtn.className = 'btn-wizard-secondary';
  dlBtn.style.cssText = 'font-size:10px;min-height:24px;padding:3px 10px;';
  dlBtn.textContent = `⬇ Download ${quantName}`;

  const streamBtn = document.createElement('button');
  streamBtn.type = 'button';
  streamBtn.className = 'btn-wizard-tertiary';
  streamBtn.style.cssText = 'font-size:10px;min-height:24px;padding:3px 10px;';
  streamBtn.textContent = '▶ Stream from HF';

  const statusEl = document.createElement('span');
  statusEl.style.cssText = 'font-size:10px;color:var(--color-text-muted);margin-left:4px;';

  dlBtn.addEventListener('click', () => {
    dlBtn.disabled = true; streamBtn.disabled = true;
    statusEl.textContent = 'Starting download…';
    const dlPanel = document.getElementById('hf-download-panel');
    if (dlPanel) {
      // Temporarily set hfRepo so hfStartDownload resolves the URL correctly.
      const prevRepo = wizardState.model.hfRepo;
      wizardState.model.hfRepo = repoId;
      hfShowDownloadPanel(dlPanel, quantName);
      hfStartDownload({
        repoId,
        filePath: quantPath,
        panelEl: dlPanel,
        onComplete: (_id, localPath) => {
          wizardState.model.source = 'local';
          wizardState.model.delivery = 'downloaded_hf';
          wizardState.model.path = localPath;
          wizardState.model.hfRepo = '';
          wizardState.model.hfFile = '';
          wizardState.model.originRepo = repoId;
          wizardState.model.originFile = quantPath;
          _attachOriginTags(localPath, repoId);
          actionsRow.style.display = 'none';
          statusEl.textContent = '✓ Downloaded and selected';
          showToast('Quant downloaded', 'success', quantName);
        },
        onValidationError: msg => { statusEl.textContent = msg; dlBtn.disabled = false; streamBtn.disabled = false; },
        onClearValidationError: () => {},
      });
      if (!prevRepo) wizardState.model.hfRepo = '';
    }
  });

  streamBtn.addEventListener('click', () => {
    wizardState.model.source = 'hf';
    wizardState.model.hfRepo = repoId;
    wizardState.model.hfFile = quantPath;
    wizardState.model.delivery = 'stream_hf';
    wizardState.model.path = '';
    actionsRow.style.display = 'none';
    showToast('Switched to HF stream', 'success', quantName);
    scheduleVramUpdate();
  });

  actionsRow.appendChild(dlBtn);
  actionsRow.appendChild(streamBtn);
  actionsRow.appendChild(statusEl);
}

// Search HF for the first GGUF repo that contains an mmproj file matching this model.
// Returns {repoId, mmprojFiles} or null.
async function _autoFindMmprojRepo(modelFilename) {
  const stem = _modelStemForSearch(modelFilename);
  if (!stem) return null;
  const headers = window.authHeaders ? window.authHeaders() : {};

  // Try progressively broader queries: exact stem, stem + GGUF keyword,
  // then a shorter version without minor version/variant suffixes.
  const shorter = stem
    .replace(/-v\d+(?:\.\d+)?(?:-[A-Za-z]+)*$/i, '') // strip -v2-MTP etc.
    .replace(/-MTP$/i, '');
  const queries = [...new Set([stem, stem + ' GGUF', shorter])].filter(Boolean);

  for (const query of queries) {
    try {
      const searchRes = await fetch('/api/hf/search', {
        method: 'POST',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify({ query, sort: 'downloads', limit: 10 }),
      });
      if (!searchRes.ok) continue;
      const searchData = await searchRes.json();
      if (!searchData.ok || !searchData.models?.length) continue;

      for (const model of searchData.models) {
        const filesData = await _hfFilesPost(model.id);
        if (!filesData?.ok) continue;
        const mmprojFiles = (filesData.files || []).filter(f => f.is_mmproj);
        if (mmprojFiles.length > 0) {
          return { repoId: mmprojFiles[0].repo_id || model.id, mmprojFiles };
        }
      }
    } catch { continue; }
  }
  return null;
}

function _renderMmprojDownloadFromHf(row) {
  // Show the row with a "download mmproj from HuggingFace" mini-panel
  row.style.display = '';
  const select = document.getElementById('hw-mmproj-select');
  if (select) select.style.display = 'none';

  // Remove stale panel so auto-search re-runs if user navigates back and forward
  row.querySelector('.hw-mmproj-hf-panel')?.remove();

  const panel = document.createElement('div');
  panel.className = 'hw-mmproj-hf-panel';
  row.appendChild(panel);

  const originRepo = wizardState.model.originRepo || '';
  const modelFilename = (wizardState.model.path || wizardState.model.hfFile || '')
    .split(/[\\/]/).pop() || '';

  if (originRepo) {
    // Already know the repo — go straight to the fetch form, which auto-fetches
    _showMmprojHfFetchForm(row, panel);
    return;
  }

  // No originRepo — try to auto-find from model filename
  panel.innerHTML = `
    <span class="hw-quant-label" style="color:var(--color-text-muted);font-size:10px;">
      No mmproj found. Searching HuggingFace…
    </span>
  `;

  _autoFindMmprojRepo(modelFilename).then(result => {
    panel.innerHTML = '';

    if (result) {
      // Auto-found a repo with mmproj — show it for one-click download
      const statusEl = document.createElement('div');
      statusEl.style.cssText = 'font-size:10px;color:var(--color-text-muted);margin-top:4px;';

      const repoLabel = document.createElement('span');
      repoLabel.className = 'hw-quant-label';
      repoLabel.style.cssText = 'font-size:10px;color:var(--color-text-muted);';
      repoLabel.textContent = `Found: ${result.repoId}`;
      panel.appendChild(repoLabel);

      const listEl = document.createElement('div');
      listEl.style.cssText = 'display:flex;flex-direction:column;gap:4px;margin-top:6px;';
      result.mmprojFiles.forEach(f => {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'btn-wizard-secondary';
        btn.style.cssText = 'min-height:26px;padding:4px 10px;font-size:10px;text-align:left;';
        const fname = (f.rfilename || f.path || '').split('/').pop();
        const sizeStr = f.size ? ` · ${formatBytes(f.size)}` : '';
        const recommended = _isRecommendedMmproj(f, modelFilename);
        btn.textContent = `⬇ ${fname}${sizeStr}${recommended ? ' · Recommended' : ''}`;
        if (recommended) btn.title = f.mmproj_recommendation || 'Preferred projector format for this model family';
        btn.addEventListener('click', () => _downloadMmprojFromHf(result.repoId, f, wizardState.model.path || wizardState.model.hfFile, statusEl));
        listEl.appendChild(btn);
      });
      panel.appendChild(listEl);
      panel.appendChild(statusEl);

      const manualLink = document.createElement('a');
      manualLink.href = '#';
      manualLink.style.cssText = 'font-size:10px;color:var(--color-text-muted);margin-top:6px;display:inline-block;';
      manualLink.textContent = 'Wrong repo? Search manually…';
      manualLink.addEventListener('click', e => {
        e.preventDefault();
        panel.innerHTML = '';
        _showMmprojHfFetchForm(row, panel);
      });
      panel.appendChild(manualLink);

      const browseLocalLink = document.createElement('a');
      browseLocalLink.href = '#';
      browseLocalLink.style.cssText = 'font-size:10px;color:var(--color-text-muted);margin-top:2px;display:inline-block;';
      browseLocalLink.textContent = 'Or browse local files…';
      browseLocalLink.addEventListener('click', e => {
        e.preventDefault();
        document.getElementById('hw-mmproj-browse-btn')?.click();
      });
      panel.appendChild(browseLocalLink);
    } else {
      // Auto-find failed — show manual form with the stem pre-filled and a note
      const stem = _modelStemForSearch(modelFilename);
      _showMmprojHfFetchForm(row, panel, stem);
    }
  });
}

function _showMmprojHfFetchForm(row, panel, prefill = '') {
  const originRepo = wizardState.model.originRepo || '';
  const initialValue = originRepo || prefill;
  const showNotFound = !originRepo && prefill;

  // eslint-disable-next-line no-unsanitized/property -- static HTML, no user data
  panel.innerHTML = `
    ${showNotFound ? `<div style="font-size:10px;color:var(--color-text-muted);margin-bottom:6px;">Couldn't auto-find it — enter the HuggingFace repo that contains the mmproj:</div>` : ''}
    <div style="display:flex;gap:6px;align-items:center;width:100%;flex-wrap:wrap;">
      <input type="text" class="hw-mmproj-repo-input" placeholder="owner/repo (e.g. unsloth/Qwen3-VL-7B-GGUF)"
        style="flex:1;min-width:120px;padding:6px 10px;border-radius:6px;border:1px solid rgba(255,255,255,0.1);
          background:rgba(28,34,42,0.9);color:var(--color-text-primary);font-size:11px;">
      <button type="button" class="btn-wizard-secondary hw-mmproj-repo-go" style="min-height:28px;padding:5px 10px;font-size:11px;">Fetch</button>
      <button type="button" class="btn-wizard-tertiary hw-mmproj-cancel" style="font-size:10px;">Cancel</button>
    </div>
    <div class="hw-mmproj-repo-status" style="font-size:10px;color:var(--color-text-muted);margin-top:4px;"></div>
    <div class="hw-mmproj-repo-list" style="display:none;flex-direction:column;gap:4px;margin-top:6px;max-height:120px;overflow-y:auto;"></div>
  `;

  const input = panel.querySelector('.hw-mmproj-repo-input');
  if (initialValue) input.value = initialValue;

  panel.querySelector('.hw-mmproj-cancel').addEventListener('click', () => {
    panel.remove();
    if (row.querySelector('#hw-mmproj-select')) {
      row.querySelector('#hw-mmproj-select').style.display = '';
    }
    row.style.display = 'none';
  });

  const goBtn = panel.querySelector('.hw-mmproj-repo-go');
  const statusEl = panel.querySelector('.hw-mmproj-repo-status');
  const listEl = panel.querySelector('.hw-mmproj-repo-list');

  async function doFetch() {
    const repoId = input.value.trim();
    if (!repoId) return;
    goBtn.disabled = true; statusEl.textContent = 'Fetching files…';
    const data = await _hfFilesPost(repoId);
    goBtn.disabled = false;
    if (!data?.ok || !data.files?.length) {
      statusEl.textContent = 'No GGUF files found. Check the repo ID.'; return;
    }
    const mmprojFiles = data.files.filter(f => f.is_mmproj);
    if (!mmprojFiles.length) {
      statusEl.textContent = 'No mmproj file found in this repo.'; return;
    }
    statusEl.textContent = '';
    listEl.style.display = 'flex';
    listEl.innerHTML = '';
    mmprojFiles.forEach(f => {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'btn-wizard-secondary';
      btn.style.cssText = 'min-height:26px;padding:4px 10px;font-size:10px;text-align:left;';
      const fname = (f.rfilename || f.path || '').split('/').pop();
      const sizeStr = f.size ? ` · ${formatBytes(f.size)}` : '';
      const modelFilename = (wizardState.model.path || wizardState.model.hfFile || '')
        .split(/[\\/]/).pop() || '';
      const recommended = _isRecommendedMmproj(f, modelFilename);
      btn.textContent = `⬇ ${fname}${sizeStr}${recommended ? ' · Recommended' : ''}`;
      if (recommended) btn.title = f.mmproj_recommendation || 'Preferred projector format for this model family';
      btn.addEventListener('click', () => _downloadMmprojFromHf(
        f.repo_id || repoId,
        f,
        wizardState.model.path || wizardState.model.hfFile,
        statusEl
      ));
      listEl.appendChild(btn);
    });
  }

  goBtn.addEventListener('click', doFetch);
  input.addEventListener('keydown', e => { if (e.key === 'Enter') doFetch(); });
  // Auto-fetch when we already know the exact repo (originRepo), not for search hints
  if (originRepo) doFetch();
}

async function _downloadMmprojFromHf(repoId, file, modelPath, statusEl) {
  const mmprojHfPath = file.rfilename || file.path || '';
  const modelFilename = (modelPath || '').split(/[\\/]/).pop() || '';
  const saveAs = modelFilename ? _deriveMmprojSaveName(modelFilename, mmprojHfPath) : mmprojHfPath.split('/').pop();
  if (statusEl) statusEl.textContent = `Downloading ${saveAs}…`;
  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const res = await fetch('/api/hf/download', {
      method: 'POST',
      headers,
      body: JSON.stringify({ repo_id: repoId, file_path: mmprojHfPath, save_as: saveAs, companion: true, resume: true }),
    });
    const data = await res.json().catch(() => ({}));
    if (!res.ok || !data.ok) {
      if (statusEl) statusEl.textContent = data.error || 'Download failed to start.';
      return;
    }
    // Poll until complete, then update mmproj state
    _pollMmprojDownload(data.download_id, data.local_path, file.size || 0, statusEl);
  } catch (e) {
    if (statusEl) statusEl.textContent = `Error: ${e.message}`;
  }
}

function _pollMmprojDownload(downloadId, localPath, expectedSize, statusEl) {
  const headers = window.authHeaders ? window.authHeaders() : {};
  async function poll() {
    try {
      const res = await fetch(`/api/models/download/${downloadId}/status`, { headers });
      if (!res.ok) { setTimeout(poll, 1000); return; }
      const data = await res.json();
      const s = data.status;
      if (!s) { setTimeout(poll, 1000); return; }
      const { status, bytes_downloaded = 0, total_bytes = 0 } = s;
      const pct = total_bytes > 0 ? Math.round(bytes_downloaded / total_bytes * 100) : 0;
      if (status === 'running') {
        if (statusEl) statusEl.textContent = `Downloading mmproj… ${pct}%`;
        setTimeout(poll, 1000);
        return;
      }
      if (status === 'completed') {
        const mmprojName = localPath.split(/[\\/]/).pop() || localPath;
        wizardState.model.mmprojPath = localPath;
        wizardState.model.mmprojHfFile = localPath;
        wizardState.model.mmprojFiles = [{
          path: localPath, name: mmprojName,
          size: expectedSize || 0, is_mmproj: true,
        }];
        wizardState.arch.mmprojBytes = expectedSize || 0;
        if (statusEl) statusEl.textContent = '';
        // Re-render mmproj section to show the new file in the dropdown
        renderMmprojSection();
        scheduleVramUpdate();
        showToast('mmproj downloaded', 'success', mmprojName);
      } else if (status === 'failed') {
        if (statusEl) statusEl.textContent = s.message || 'Download failed.';
      }
    } catch { setTimeout(poll, 1000); }
  }
  setTimeout(poll, 800);
}

// ── Hardware step: MTP section ───────────────────────────────────────────────

function renderMtpSection() {
  const section = document.getElementById('hw-mtp-section');
  if (!section) return;
  const hasMtp = wizardState.arch.mtpDepth > 0 ||
    detectMtpFromName(wizardState.model.hfFile || wizardState.model.path || '');
  if (!hasMtp) {
    section.style.display = 'none';
    if (dom.mtpAssistantSection) dom.mtpAssistantSection.style.display = 'none';
    return;
  }
  section.style.display = '';

  const infoNote = document.getElementById('hw-mtp-info-note');
  if (infoNote && hasMtp) { infoNote.style.display = ''; }

  // Render companion assistant selector: always show for MTP models even
  // if no candidates were auto-detected, so user can still browse.
  if (dom.mtpAssistantSection && dom.mtpAssistantSelect) {
    const candidates = wizardState.model.draftCandidates || [];

    // Bind the change listener once; always repopulate options so new
    // candidates discovered after first render (or from a different model)
    // are not silently lost.
    if (!dom.mtpAssistantSelect.dataset.bound) {
      dom.mtpAssistantSelect.dataset.bound = '1';
      dom.mtpAssistantSelect.addEventListener('change', () => {
        const selected = dom.mtpAssistantSelect.value || '';
        // Browse sentinel: open file browser for companion assistant
        if (selected === '__browse__') {
          openDeferredFileBrowser(
            'hw-mtp-assistant-select',
            'gguf',
            '',
            'draft-model',
          );
          return;
        }
        wizardState.model.selectedDraftPath = selected;
        // When assistant is selected, recommend higher n-max.
        if (selected) {
          wizardState.hardware.mtpDraftNMax = wizardState.hardware.mtpDraftNMax || 4;
        }
        scheduleVramUpdate();
      });
    }

    dom.mtpAssistantSection.style.display = '';

    dom.mtpAssistantSelect.innerHTML = '';
    const noneOpt = document.createElement('option');
    noneOpt.value = '';
    noneOpt.textContent = '(none — use built-in MTP only)';
    dom.mtpAssistantSelect.appendChild(noneOpt);

    candidates.forEach(f => {
      const fpath = f.path || f.name || '';
      const fname = fpath.split(/[\\/]/).pop();
      const sizeStr = f.size ? ` · ${formatBytes(f.size)}` : '';
      const opt = document.createElement('option');
      opt.value = fpath;
      opt.textContent = fname + sizeStr;
      dom.mtpAssistantSelect.appendChild(opt);
    });

    // If no candidates were auto-detected, add a sentinel option that
    // triggers a file browser when chosen by the user.
    if (candidates.length === 0) {
      const browseOpt = document.createElement('option');
      browseOpt.value = '__browse__';
      browseOpt.textContent = '(browse for a companion assistant GGUF…)';
      dom.mtpAssistantSelect.appendChild(browseOpt);
    }

    // Sync selection
    const current = wizardState.model.selectedDraftPath || '';
    if (current) dom.mtpAssistantSelect.value = current;
  }

  // Render draft candidate pills (quick selection buttons)
  _renderDraftCandidatePills();

  const checkbox = document.getElementById('hw-use-mtp');
  // The user-facing control is spec-draft-n-max (draft tokens per step), not "depth"
  // arch.mtpDepth = number of MTP heads built into the model (VRAM estimation only)
  const draftNMaxInput = document.getElementById('hw-mtp-depth');

  if (draftNMaxInput) {
    if (!draftNMaxInput.dataset.bound) {
      draftNMaxInput.dataset.bound = '1';
      draftNMaxInput.addEventListener('input', () => {
        const v = parseInt(draftNMaxInput.value, 10);
        if (v >= 0 && v <= 8) {
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
        _mtpUserConfigured = true;
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

// ── Apple Silicon: apply Metal GPU wired limit via osascript ─────────────────

async function applyMetalGpuLimit(limitMb) {
  if (!dom.metalLimitBtn) return;
  const btn = dom.metalLimitBtn;
  const orig = btn.textContent;
  btn.disabled = true;
  btn.textContent = 'Applying…';

  try {
    const tokenResp = await fetch('/api/db/admin-token', {
      headers: window.authHeaders ? window.authHeaders() : {},
    });
    const tokenData = tokenResp.ok ? await tokenResp.json().catch(() => ({})) : {};
    const adminToken = tokenData.token;
    const headers = {
      'Content-Type': 'application/json',
      ...(adminToken ? { 'Authorization': `Bearer ${adminToken}` } : {}),
    };

    const resp = await fetch('/api/system/set-metal-gpu-limit', {
      method: 'POST',
      headers,
      body: JSON.stringify({ limit_mb: limitMb }),
    });
    const data = await resp.json();

    if (data.ok) {
      const gb = Math.round((data.limit_mb || limitMb) / 1024);
      showToast(`Metal GPU limit set to ${gb} GB — saved to /etc/sysctl.conf, survives reboots.`, 'success');
      await fetchGpuVram();
      await fetchMetalGpuLimit();
      scheduleVramUpdate();
    } else {
      const msg = data.error || 'Failed to apply Metal GPU limit.';
      if (msg.toLowerCase().includes('cancel')) {
        showToast('Cancelled — no changes made.', 'info');
      } else {
        // osascript failed — show the error and a Terminal fallback the user can copy
        const manualCmd = data.manual_cmd || `sudo /usr/sbin/sysctl -w iogpu.wired_limit_mb=${limitMb}`;
        _showMetalLimitFallback(btn, msg, manualCmd);
      }
      btn.disabled = false;
      btn.textContent = orig;
    }
  } catch (e) {
    showToast('Failed to contact server: ' + e.message, 'error');
    btn.disabled = false;
    btn.textContent = orig;
  }
}

function _showMetalLimitFallback(btn, errorMsg, manualCmd) {
  // Replace the button row with an inline error + copyable Terminal command
  const row = btn.closest('.metal-limit-row');
  if (!row) { showToast(errorMsg, 'error'); return; }

  const fallback = document.createElement('div');
  fallback.className = 'metal-limit-fallback';

  const errorDiv = document.createElement('div');
  errorDiv.className = 'metal-limit-fallback-error';
  errorDiv.textContent = errorMsg || '';

  const hintDiv = document.createElement('div');
  hintDiv.className = 'metal-limit-fallback-hint';
  hintDiv.textContent = 'Run this in Terminal instead:';

  const cmdRow = document.createElement('div');
  cmdRow.className = 'metal-limit-fallback-cmd-row';

  const codeEl = document.createElement('code');
  codeEl.className = 'metal-limit-fallback-cmd';
  codeEl.textContent = manualCmd || '';

  const copyBtn = document.createElement('button');
  copyBtn.type = 'button';
  copyBtn.className = 'metal-limit-fallback-copy';
  copyBtn.textContent = 'Copy';

  cmdRow.appendChild(codeEl);
  cmdRow.appendChild(copyBtn);

  fallback.appendChild(errorDiv);
  fallback.appendChild(hintDiv);
  fallback.appendChild(cmdRow);
  fallback.querySelector('.metal-limit-fallback-copy').addEventListener('click', () => {
    navigator.clipboard.writeText(manualCmd).then(() => {
      showToast('Copied to clipboard', 'success');
    });
  });

  // Append below the existing row content (don't remove the row itself)
  row.appendChild(fallback);
}

async function triggerAutoSize() {
  if (!dom.vramAutosizeBtn) return;
  const btn = dom.vramAutosizeBtn;
  const origText = btn.textContent;
  btn.disabled = true; btn.textContent = 'Sizing…';
  if (dom.vramAutosizeNote) dom.vramAutosizeNote.textContent = '';

  try {
    const modelPath = wizardState.model.path || '';
    if (modelPath) {
      await doIntrospect(modelPath);
    }

    const availVram = effectiveAvailBytes();
    const modelBytes = getModelBytes();
    const arch = getSizingArch();

    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };

    const body = {
      model_path: wizardState.model.path || undefined,
      model_size_bytes: modelBytes || undefined,
      param_b: wizardState.model.paramB || undefined,
      model_name: wizardState.model.path || wizardState.model.hfRepo || '',
      available_vram_bytes: availVram,
      is_unified_memory: isUnifiedMemory(),
      use_case: wizardState.useCase,
      parallel_slots: wizardState.hardware.parallelSlots,
      fit_granularity: 1024,
      quant: guessQuantFromName(wizardState.model.hfFile || wizardState.model.path || ''),
      n_layers:              arch.nLayers    || undefined,
      n_kv_heads:            arch.nKvHeads   || undefined,
      head_dim:              arch.headDim    || undefined,
      global_head_dim:       arch.globalHeadDim || undefined,
      n_attn_layers:         arch.nAttnLayers || undefined,
      linear_attn_state_bytes: arch.linearAttnStateBytes || undefined,
      n_global_attn_layers:  arch.nGlobalAttnLayers || undefined,
      local_attn_window:     arch.localAttnWindow || undefined,
      local_kv_heads:        arch.localKvHeads || undefined,
      n_experts:             arch.nExperts   || undefined,
      n_experts_used:        arch.nExpertsUsed || undefined,
      expert_fraction:       arch.expertFraction || undefined,
      mtp_depth:             arch.mtpDepth   || undefined,
      mmproj_bytes:          arch.mmprojBytes || undefined,
    };

    const resp = await fetch('/api/vram/auto-size', { method: 'POST', headers, body: JSON.stringify(body) });
    if (!resp.ok) { showToast('Auto-size failed', 'error'); return; }
    const data = await resp.json();
    if (!data.ok || !data.result) { showToast('Auto-size: no result', 'warning'); return; }

    const { result: r, adjusted } = clampAutoSizeResultToSizingMath(data.result, arch, modelBytes, availVram);

    // Apply recommended settings
    wizardState.hardware.contextSize = r.context_size;
    wizardState.hardware.cacheTypeK  = r.kv_quant_k;
    wizardState.hardware.cacheTypeV  = r.kv_quant_v;
    wizardState.hardware.ubatchSize  = r.ubatch_size;

    if (r.n_cpu_moe != null) wizardState.hardware.nCpuMoe = r.n_cpu_moe;

    // On unified memory (Apple Silicon), disable the 8 GB KV prefix-cache RAM reservation
    // (--cache-ram). It's a separate pool for caching previous request prompts and defaults
    // to 8 GB, which wastes unified memory that could be used for model weights or context.
    // Only apply if the user hasn't explicitly set a value.
    if (isUnifiedMemory() && (wizardState.hardware.cacheRam == null || wizardState.hardware.cacheRam === 8192)) {
      wizardState.hardware.cacheRam = 0;
      if (dom.cacheRamInput) dom.cacheRamInput.value = '0';
    }

    // Sync form fields
    if (dom.contextSizeInput) dom.contextSizeInput.value = r.context_size;
    if (dom.cacheTypeKSelect) dom.cacheTypeKSelect.value  = r.kv_quant_k;
    if (dom.cacheTypeVSelect) dom.cacheTypeVSelect.value  = r.kv_quant_v;
    if (dom.ubatchSizeInput)  dom.ubatchSizeInput.value   = r.ubatch_size;
    if (dom.fitTargetInput)   dom.fitTargetInput.value    = wizardState.hardware.fitTarget || '';
    if (r.n_cpu_moe != null && dom.nCpuMoeInput) dom.nCpuMoeInput.value = r.n_cpu_moe;
    if (r.n_cpu_moe != null && dom.moeOffloadSlider) dom.moeOffloadSlider.value = r.n_cpu_moe;

    const note = `${adjusted ? 'Adjusted:' : 'Set:'} ${formatCtx(r.context_size)} ctx · ${r.kv_quant_k.toUpperCase()} KV · ubatch ${r.ubatch_size}`;
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

// ── Collapsible hardware sections ─────────────────────────────────────────────

function bindSectionToggles() {
   // Response shaping is visible by default for review-step discoverability.
   const divider = document.getElementById('hw-section-response-shaping');
   const toggle  = document.getElementById('hw-toggle-response-shaping');
   const body    = document.getElementById('hw-body-response-shaping');
   if (!divider || !body) return;

   divider.style.cursor = 'pointer';
   divider.addEventListener('click', () => {
       const collapsed = body.style.display === 'none';
       body.style.display = collapsed ? '' : 'none';
       if (toggle) toggle.textContent = collapsed ? '▾' : '▸';
   });
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
      ` — auto-sized to ${got} with the high-precision cache mode. Try a smaller model quant like Q4_K_M or IQ3_XXS to shrink weights, or override by typing a custom value.`
    ));
  } else if (useCase === 'roleplay') {
    strong.textContent = `Below ${need} RP target`;
    el.appendChild(strong);
    el.appendChild(document.createTextNode(
      ` — auto-sized to ${got}. Try the More context mode or use a smaller model quant if long transcripts matter more than cache precision.`
    ));
  } else {
    el.appendChild(document.createTextNode(
      `Auto-size returned ${got} (target ${need}). Consider a smaller quantization.`
    ));
  }
  el.className = ctx < target * 0.5 ? 'ctx-fit-warning ctx-fit-error' : 'ctx-fit-warning';
  el.style.display = '';
}

// ── Model directory switcher ──────────────────────────────────────────────────

// ── Browse split-button dropdown ──────────────────────────────────────────────

function _closeBrowseDropdowns() {
  ['spawn-browse-dropdown', 'spawn-import-browse-dropdown'].forEach(id => {
    const dd = document.getElementById(id);
    if (dd) dd.style.display = 'none';
  });
  document.getElementById('spawn-browse-arrow-btn')?.setAttribute('aria-expanded', 'false');
  document.getElementById('spawn-import-browse-arrow-btn')?.setAttribute('aria-expanded', 'false');
}

function _buildBrowseDropdown(dropdownEl, targetInputId, allDirs) {
  dropdownEl.innerHTML = '';

  allDirs.forEach((dir, i) => {
    const parts = dir.replace(/\\/g, '/').split('/').filter(Boolean);
    const label = parts[parts.length - 1] || dir;
    const pathHint = parts.slice(0, -1).join('/');

    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'browse-dd-item';
    btn.title = dir;

    const labelEl = document.createElement('span');
    labelEl.className = 'dd-item-label';
    labelEl.textContent = label;
    btn.appendChild(labelEl);

    if (pathHint) {
      const pathEl = document.createElement('span');
      pathEl.className = 'dd-item-path';
      pathEl.textContent = '/' + pathHint;
      btn.appendChild(pathEl);
    }

    btn.addEventListener('click', () => {
      _closeBrowseDropdowns();
      const ctx = targetInputId === 'spawn-model-path' ? 'model' : '';
      openDeferredFileBrowser(targetInputId, 'gguf', dir, ctx);
    });
    dropdownEl.appendChild(btn);
  });

  const divider = document.createElement('div');
  divider.className = 'browse-dd-divider';
  dropdownEl.appendChild(divider);

  const manageBtn = document.createElement('button');
  manageBtn.type = 'button';
  manageBtn.className = 'browse-dd-item dd-manage';
  manageBtn.textContent = '⚙ Manage model locations…';
  manageBtn.addEventListener('click', () => {
    _closeBrowseDropdowns();
    window.openSettingsModal?.();
    setTimeout(() => document.querySelector('.settings-tab[data-tab="models"]')?.click(), 80);
  });
  dropdownEl.appendChild(manageBtn);
}

function _toggleBrowseDropdown(arrowBtnId, dropdownId, targetInputId) {
  const arrow = document.getElementById(arrowBtnId);
  const dd    = document.getElementById(dropdownId);
  if (!arrow || !dd) return;

  const isOpen = dd.style.display !== 'none';

  // Close all first, then open this one if it was closed
  _closeBrowseDropdowns();

  if (!isOpen) {
    dd.style.display = 'block';
    arrow.setAttribute('aria-expanded', 'true');
  }
}

async function _loadModelDirSwitcher() {
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const r = await fetch('/api/settings', { headers });
    if (!r.ok) return;
    const s = await r.json();

    const primary = s.models_dir || '';
    const extras = Array.isArray(s.extra_models_dirs) ? s.extra_models_dirs.filter(Boolean) : [];
    const allDirs = [primary, ...extras].filter(Boolean);

    const localDd  = document.getElementById('spawn-browse-dropdown');
    const importDd = document.getElementById('spawn-import-browse-dropdown');
    if (localDd)  _buildBrowseDropdown(localDd,  'spawn-model-path',  allDirs);
    if (importDd) _buildBrowseDropdown(importDd, 'spawn-import-path', allDirs);

    // Wire arrow buttons (idempotent — clone to remove old listeners)
    const wireArrow = (arrowId, dropdownId, targetInputId) => {
      const old = document.getElementById(arrowId);
      if (!old) return;
      const fresh = old.cloneNode(true);
      old.replaceWith(fresh);
      fresh.addEventListener('click', e => {
        e.stopPropagation();
        _toggleBrowseDropdown(arrowId, dropdownId, targetInputId);
      });
    };
    wireArrow('spawn-browse-arrow-btn',        'spawn-browse-dropdown',        'spawn-model-path');
    wireArrow('spawn-import-browse-arrow-btn', 'spawn-import-browse-dropdown', 'spawn-import-path');
  } catch { /* ignore */ }
}

// ── Model-specific sampling defaults (from /api/model-defaults) ──────────────

function _applyPresetToHardware(preset) {
  const h = wizardState.hardware;
  if (preset.temperature != null) h.temperature = preset.temperature;
  if (preset.top_p != null) h.topP = preset.top_p;
  if (preset.top_k != null) h.topK = preset.top_k > 0 ? preset.top_k : null;
  if (preset.min_p != null) h.minP = preset.min_p;
  if (preset.repeat_penalty != null) h.repeatPenalty = preset.repeat_penalty;
  h.presencePenalty = (preset.presence_penalty != null && preset.presence_penalty > 0)
    ? preset.presence_penalty : null;
  h.maxTokens = preset.max_tokens != null ? preset.max_tokens : null;
  h.enableThinking   = preset.enable_thinking   ?? null;
  h.preserveThinking = preset.preserve_thinking ?? null;
  h.reasoningBudget  = preset.reasoning_budget  ?? null;
  h.reasoningMode = typeof preset.reasoning === 'boolean'
    ? (preset.reasoning ? 'on' : 'off')
    : (preset.reasoning || null);
  h.reasoningBudgetMessage = preset.reasoning_budget_message ?? null;
  _syncThinkingFields();
}

function _renderSamplingPresetPills(presets) {
  const container = document.getElementById('spawn-sampling-presets');
  if (!container) return;

  if (!presets || presets.length <= 1) {
    container.style.display = 'none';
    container.innerHTML = '';
    return;
  }

  container.style.display = 'flex';
  container.style.cssText = 'display:flex;align-items:center;gap:6px;flex-wrap:wrap;margin-bottom:10px;';
  container.innerHTML = '';

  const label = document.createElement('span');
  label.style.cssText = 'font-size:11px;color:var(--color-text-muted);flex-shrink:0;';
  label.textContent = 'Mode:';
  container.appendChild(label);

  presets.forEach((preset, i) => {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'sampling-preset-pill' + (i === 0 ? ' active' : '');
    btn.textContent = preset.name;
    if (preset.description) btn.title = preset.description;
    btn.dataset.presetIndex = String(i);
    btn.addEventListener('click', () => {
      container.querySelectorAll('.sampling-preset-pill').forEach(p => p.classList.remove('active'));
      btn.classList.add('active');
      _applyPresetToHardware(preset);
      _syncSamplingFields();
    });
    container.appendChild(btn);
  });
}

async function _fetchAndApplyModelSamplingDefaults() {
  const m = wizardState.model;
  const name = m.hfFile
    ? (m.hfFile.split('/').pop() || m.hfFile)
    : (m.path ? m.path.split(/[\\/]/).pop() : '') || m.hfRepo || '';
  if (!name) return;

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const res = await fetch('/api/model-defaults', {
      method: 'POST',
      headers,
      body: JSON.stringify({ model_name_or_repo: name, size_bytes: m.modelBytes || 0, tags: [] }),
    });
    if (!res.ok) return;
    const data = await res.json();
    const defaults = data.defaults || data;
    const h = wizardState.hardware;

    // Apply first preset values (only overwrite fields not yet explicitly set)
    if (h.temperature == null && defaults.temperature != null) h.temperature = defaults.temperature;
    if (h.topP == null && defaults.top_p != null) h.topP = defaults.top_p;
    if (h.topK == null && defaults.top_k != null && defaults.top_k > 0) h.topK = defaults.top_k;
    if (h.minP == null && defaults.min_p != null) h.minP = defaults.min_p;
    if (h.repeatPenalty == null && defaults.repeat_penalty != null) h.repeatPenalty = defaults.repeat_penalty;
    if (h.presencePenalty == null && defaults.presence_penalty != null && defaults.presence_penalty > 0) {
      h.presencePenalty = defaults.presence_penalty;
    }
    if (h.maxTokens == null && defaults.max_tokens != null) h.maxTokens = defaults.max_tokens;
    if (h.enableThinking == null && defaults.enable_thinking != null) h.enableThinking = defaults.enable_thinking;
    if (h.preserveThinking == null && defaults.preserve_thinking != null) h.preserveThinking = defaults.preserve_thinking;
    if (h.reasoningMode == null && defaults.reasoning != null) {
      h.reasoningMode = defaults.reasoning ? 'on' : 'off';
    }
    if (h.reasoningBudget == null && defaults.reasoning_budget != null) h.reasoningBudget = defaults.reasoning_budget;
    if (h.reasoningBudgetMessage == null && defaults.reasoning_budget_message != null) {
      h.reasoningBudgetMessage = defaults.reasoning_budget_message;
    }
    _syncThinkingFields();

    // Render mode pills if multiple presets are available
    _renderSamplingPresetPills(data.presets || []);
  } catch { /* non-fatal */ }
}

// Use-case sampling defaults (temperature, top-p, min-p, repeat-penalty)
const SAMPLING_DEFAULTS = {
  agentic:  { temperature: 0.3,  topP: 0.95, minP: 0.02, topK: null, repeatPenalty: 1.05, presencePenalty: null, seed: null },
  general:  { temperature: 0.7,  topP: 0.9,  minP: 0.05, topK: null, repeatPenalty: 1.05, presencePenalty: null, seed: null },
  roleplay: { temperature: 1.0,  topP: 0.95, minP: 0.05, topK: null, repeatPenalty: 1.05, presencePenalty: null, seed: null },
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
  if (h.presencePenalty == null && def.presencePenalty != null) h.presencePenalty = def.presencePenalty;
}

// ── Summary (Step 4) ──────────────────────────────────────────────────────────

function renderSummary() {
  if (!dom.summaryList) return;
  dom.summaryList.innerHTML = '';

  // Apply use-case sampling defaults before rendering
  applyUseCaseSamplingDefaults();
  // Sync sampling fields in the review step form
  _syncSamplingFields();

  // Pre-fill preset name input if empty
   if (dom.presetNameInput && !dom.presetNameInput.value.trim()) {
     const m = wizardState.model;
     const ctx = wizardState.hardware.contextSize || 0;
     const modelFile = (m.path || m.hfRepo || '').split(/[/\\]/).pop() || '';
     const base = (modelFile || '').replace(/\.gguf$/i, '').trim();
     const name = base && ctx
       ? base + '-' + formatCtx(ctx).toLowerCase()
       : base || 'My Preset';
     dom.presetNameInput.value = name;
   }

  const m = wizardState.model, hw = wizardState.hardware;
  const arch = getSizingArch();
  const availVram = effectiveAvailBytes();
  const modelBytes = getModelBytes();

  const modelDisplay = m.source === 'hf'
    ? (m.hfFile ? `${m.hfRepo} / ${m.hfFile.split('/').pop()}` : m.hfRepo || '(none)')
    : (m.path ? m.path.split(/[\\/]/).pop() || m.path : '(none)');

  let acquisition = 'Local file';
  if (m.delivery === 'stream_hf') {
    // originRepo is set for normal HF models; hfRepo is set when streaming via quant swap
    const repo = m.originRepo || m.hfRepo || '';
    const file = m.originFile || m.hfFile || '';
    if (repo) acquisition = `Stream from HuggingFace · ${repo}${file ? ` / ${file.split('/').pop()}` : ''}`;
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
    ...(hw.fitTarget ? [{ label: '--fit-target', value: String(hw.fitTarget) }] : []),
  ];
  if (hw.flashAttn && hw.flashAttn !== 'auto') rows.push({ label: 'Flash Attn', value: hw.flashAttn });
  if (hw.kvUnified) rows.push({ label: 'KV unified', value: 'Yes' });
  if (hw.mlock) rows.push({ label: 'mlock', value: 'Yes' });
  if (hw.prio != null) rows.push({ label: 'Priority', value: ['Normal', 'Medium', 'High', 'Realtime'][hw.prio] ?? String(hw.prio) });
  if (hw.nCpuMoe > 0 && arch.nExperts > 0) rows.push({ label: 'MoE CPU offload', value: `${hw.nCpuMoe} of ${arch.nLayers} layers` });
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
      // Show draft model filename for draft-model or MTP modes with external assistant
      if (dom.draftModelInput?.value) {
        const fileName = dom.draftModelInput.value.split(/[\\/]/).pop();
        if (specType === 'draft-model') sv += ` (${fileName})`;
        else if (specType.includes('draft-mtp')) sv += ` + ${fileName}`;
      }
      rows.push({ label: 'Speculative', value: sv });
    }
  if (hw.fitTarget) rows.push({ label: '--fit-target', value: `${hw.fitTarget} MB` });
  if (hw.cacheRam !== null && hw.cacheRam !== undefined) {
    const cramDisplay = hw.cacheRam < 0 ? 'no limit' : hw.cacheRam === 0 ? 'disabled' : `${hw.cacheRam} MiB`;
    rows.push({ label: '-cram', value: cramDisplay });
  }
  if (wizardState.access.apiKey) rows.push({ label: 'Server API key', value: `${wizardState.access.apiKey.slice(0, 4)}…${wizardState.access.apiKey.slice(-4)}` });

  // Summary list header
  const listHeader = document.createElement('div');
  listHeader.className = 'summary-list-header';
  listHeader.textContent = 'Configuration summary';
  dom.summaryList.appendChild(listHeader);

  rows.forEach(r => {
    const row = document.createElement('div');
    row.className = 'summary-row';
    const lbl = document.createElement('span'); lbl.className = 'summary-label'; lbl.textContent = r.label;
    const val = document.createElement('span'); val.className = 'summary-value'; val.textContent = r.value;
    row.appendChild(lbl); row.appendChild(val);
    dom.summaryList.appendChild(row);
  });

  // Warnings — stronger and more explicit for risky configs
  if (dom.summaryWarnings) {
    const warns = [];
    if (!modelDisplay || modelDisplay === '(none)') warns.push('No model selected.');
    const ratio = availVram > 0 && modelBytes > 0 ? (modelBytes + kvSize) / availVram : 0;

    if (ratio > 1.5) {
      warns.push("CRITICAL: Your configuration heavily exceeds available VRAM. The server will likely crash or run extremely slowly. Reduce context size, increase KV quant, or choose a smaller model.");
    } else if (ratio > 1.2) {
      warns.push("HIGH RISK: Configuration likely exceeds VRAM. The server may crash. Reduce context size or use a stronger KV quant (e.g., q4_K_M).");
    } else if (ratio > 1.0) {
      warns.push("RISKY: VRAM is exceeded or barely covered. Expect instability. Consider reducing context or using KV quantization.");
    } else if (ratio > 0.88) {
      warns.push("VRAM is tight. Minor increases in context or requests can trigger OOM errors. Watch GPU memory.");
    }

    // High context size: warn if user sets a very large context that strains VRAM
    if (hw.contextSize >= 32768) {
      const ctxRisk = ratio > 0.9;
      warns.push(
        ctxRisk
          ? "Very large context size selected. This puts significant pressure on VRAM and may slow generation."
          : "Large context size selected. This improves long tasks but uses more VRAM and may slow generation."
      );
    }

    // Agentic use case KV recommendation
    if (wizardState.useCase === 'agentic' && kvBpe(ctxK) < 1.0) {
      warns.push("q4_0 KV not recommended for agentic workflows — reduces tool-call coherence. Prefer q8_0 or q6_K when VRAM allows.");
    }

    // Binding/host visibility warnings
    if (wizardState.access.bindHost === '0.0.0.0' && !wizardState.access.apiKey) {
      warns.push('LAN-visible endpoint without a server API key. Set one unless you intentionally want an open local-network server.');
    } else if (wizardState.access.bindHost === '0.0.0.0') {
      warns.push('LAN-visible endpoint enabled. Make sure clients know the API key you set.');
    }

    if (warns.length) {
      dom.summaryWarnings.style.display = '';
      dom.summaryWarnings.innerHTML = '';
      const textWrap = document.createElement('div');
      textWrap.className = 'summary-warnings-text';
      warns.forEach(w => {
        const p = document.createElement('div');
        p.textContent = w;
        textWrap.appendChild(p);
      });
      dom.summaryWarnings.appendChild(textWrap);
    } else {
      dom.summaryWarnings.style.display = 'none';
    }
  }
  // (health check button removed — it checked the currently-running server, not the new config)

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
  // Round f32 API values to avoid 0.699999988079071 style display artifacts
  const fmt = v => v == null ? '' : String(parseFloat(Number(v).toFixed(4)));
  const setVal = (id, val) => {
    const el = document.getElementById(id);
    if (el && val != null) el.value = fmt(val);
    else if (el) el.value = '';
  };
  setVal('spawn-temperature', h.temperature);
  setVal('spawn-seed', h.seed);
  setVal('spawn-top-p', h.topP);
  setVal('spawn-top-k', h.topK);
  setVal('spawn-min-p', h.minP);
  setVal('spawn-repeat-penalty', h.repeatPenalty);
  setVal('spawn-presence-penalty', h.presencePenalty);
  setVal('spawn-max-tokens', h.maxTokens);
  if (dom.bindHostSelect) dom.bindHostSelect.value = wizardState.access.bindHost || '127.0.0.1';
  if (dom.portInput) dom.portInput.value = String(wizardState.access.port || 8001);
  if (dom.apiKeyInput) dom.apiKeyInput.value = wizardState.access.apiKey || '';
  const aliasEl = document.getElementById('spawn-alias');
  if (aliasEl) aliasEl.value = h.alias || '';
  const extraArgsEl = document.getElementById('spawn-extra-args');
  if (extraArgsEl) extraArgsEl.value = h.extraArgs || '';
  _syncStructuredOutputFields();
}

function _structuredOutputMode() {
  const h = wizardState.hardware;
  if (h.outputMode) return h.outputMode;
  if (h.jsonSchema) return 'json_schema';
  if (h.grammar) return 'grammar';
  return '';
}

function _syncStructuredOutputFields() {
  const mode = _structuredOutputMode();
  if (dom.outputModeSelect) dom.outputModeSelect.value = mode;
  if (dom.grammarWrap) dom.grammarWrap.style.display = mode === 'grammar' ? '' : 'none';
  if (dom.jsonSchemaWrap) dom.jsonSchemaWrap.style.display = mode === 'json_schema' ? '' : 'none';
  if (dom.grammarInput) dom.grammarInput.value = wizardState.hardware.grammar || '';
  if (dom.jsonSchemaInput) dom.jsonSchemaInput.value = wizardState.hardware.jsonSchema || '';
}

function _syncThinkingFields() {
  const h = wizardState.hardware;
  const section = document.getElementById('spawn-thinking-section');
  const hasThinking =
    h.enableThinking != null ||
    h.preserveThinking != null ||
    h.reasoningMode != null ||
    h.reasoningBudget != null ||
    h.reasoningBudgetMessage != null;

  if (section) {
    section.style.display = hasThinking ? '' : 'none';
    const preserveRow = document.getElementById('spawn-preserve-thinking-row');
    if (preserveRow) preserveRow.style.display = h.preserveThinking != null ? '' : 'none';
  }

  const chk = id => {
    const el = document.getElementById(id);
    if (el) el.checked = !!wizardState.hardware[id === 'spawn-enable-thinking' ? 'enableThinking' : 'preserveThinking'];
  };
  chk('spawn-enable-thinking');
  chk('spawn-preserve-thinking');

  const sel = document.getElementById('spawn-reasoning-mode');
  if (sel) sel.value = h.reasoningMode || '';
  const budgetEl = document.getElementById('spawn-reasoning-budget');
  if (budgetEl) budgetEl.value = h.reasoningBudget != null ? String(h.reasoningBudget) : '';
  const msgEl = document.getElementById('spawn-reasoning-budget-message');
  if (msgEl) msgEl.value = (h.reasoningBudgetMessage || '').replace(/\n/g, '\\n');
}

function _bindThinkingFields() {
  const bindChk = (id, key) => {
    const el = document.getElementById(id);
    if (!el || el.dataset.bound) return;
    el.dataset.bound = '1';
    el.addEventListener('change', () => { wizardState.hardware[key] = el.checked; });
  };
  const bindInput = (id, key, isInt = false) => {
    const el = document.getElementById(id);
    if (!el || el.dataset.bound) return;
    el.dataset.bound = '1';
    el.addEventListener('input', () => {
      const raw = el.value.trim();
      if (raw === '') { wizardState.hardware[key] = null; return; }
      wizardState.hardware[key] = isInt ? parseInt(raw, 10) : raw;
    });
  };
  const bindSel = (id, key) => {
    const el = document.getElementById(id);
    if (!el || el.dataset.bound) return;
    el.dataset.bound = '1';
    el.addEventListener('change', () => { wizardState.hardware[key] = el.value || null; });
  };
  bindChk('spawn-enable-thinking', 'enableThinking');
  bindChk('spawn-preserve-thinking', 'preserveThinking');
  // Reasoning mode: auto-fill budget + message defaults when user selects "on"
  const reasoningModeEl = document.getElementById('spawn-reasoning-mode');
  if (reasoningModeEl && !reasoningModeEl.dataset.bound) {
    reasoningModeEl.dataset.bound = '1';
    reasoningModeEl.addEventListener('change', () => {
      wizardState.hardware.reasoningMode = reasoningModeEl.value || null;
      if (reasoningModeEl.value === 'on') {
        const budgetEl  = document.getElementById('spawn-reasoning-budget');
        const msgEl     = document.getElementById('spawn-reasoning-budget-message');
        if (budgetEl && !budgetEl.value) {
          budgetEl.value = '16384';
          wizardState.hardware.reasoningBudget = 16384;
        }
        if (msgEl && !msgEl.value) {
          msgEl.value = '\\nFinal Answer:';
          wizardState.hardware.reasoningBudgetMessage = '\nFinal Answer:';
        }
      }
    });
  }
  bindInput('spawn-reasoning-budget', 'reasoningBudget', true);
  const bmEl = document.getElementById('spawn-reasoning-budget-message');
  if (bmEl && !bmEl.dataset.bound) {
    bmEl.dataset.bound = '1';
    bmEl.addEventListener('input', () => {
      const raw = bmEl.value.trim();
      wizardState.hardware.reasoningBudgetMessage = raw === '' ? null : raw.replace(/\\n/g, '\n');
    });
  }
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
  bind('spawn-top-k', 'topK', true);
  bind('spawn-min-p', 'minP');
  bind('spawn-repeat-penalty', 'repeatPenalty');
  bind('spawn-presence-penalty', 'presencePenalty');
  bind('spawn-max-tokens', 'maxTokens', true);
  _bindThinkingFields();

  if (dom.outputModeSelect && !dom.outputModeSelect.dataset.bound) {
    dom.outputModeSelect.dataset.bound = '1';
    dom.outputModeSelect.addEventListener('change', () => {
      const mode = dom.outputModeSelect.value || '';
      wizardState.hardware.outputMode = mode;
      if (!mode) {
        wizardState.hardware.grammar = '';
        wizardState.hardware.jsonSchema = '';
      }
      if (mode === 'grammar') wizardState.hardware.jsonSchema = '';
      if (mode === 'json_schema') wizardState.hardware.grammar = '';
      _syncStructuredOutputFields();
    });
  }
  if (dom.grammarInput && !dom.grammarInput.dataset.bound) {
    dom.grammarInput.dataset.bound = '1';
    dom.grammarInput.addEventListener('input', () => {
      wizardState.hardware.outputMode = 'grammar';
      wizardState.hardware.grammar = dom.grammarInput.value || '';
    });
  }
  if (dom.jsonSchemaInput && !dom.jsonSchemaInput.dataset.bound) {
    dom.jsonSchemaInput.dataset.bound = '1';
    dom.jsonSchemaInput.addEventListener('input', () => {
      wizardState.hardware.outputMode = 'json_schema';
      wizardState.hardware.jsonSchema = dom.jsonSchemaInput.value || '';
    });
  }

  // Alias and extra args — string fields, no parsing
  const bindStr = (id, key) => {
    const el = document.getElementById(id);
    if (!el || el.dataset.bound) return;
    el.dataset.bound = '1';
    el.addEventListener('input', () => { wizardState.hardware[key] = el.value; });
  };
  bindStr('spawn-alias', 'alias');
  bindStr('spawn-extra-args', 'extraArgs');
}

// ── Save as preset ────────────────────────────────────────────────────────────

async function saveAsPreset() {
  const nameInput = dom.presetNameInput;
  const name = nameInput ? nameInput.value.trim() : '';
  if (!name) {
    if (nameInput) {
      nameInput.focus();
      nameInput.classList.add('field-error');
      setTimeout(() => nameInput.classList.remove('field-error'), 1500);
    }
    showToast('Enter a preset name first', 'warn');
    return;
  }

  const payload = buildPresetPayload();
  payload.name = name;

  const btn = dom.savePresetBtn;
  if (btn) { btn.disabled = true; btn.textContent = 'Saving…'; }

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };

    let isUpdate = Boolean(wizardState.savedPresetId);

    // If we have a saved preset ID, decide whether to update it or create new.
    // If config has significantly diverged (different model or context size),
    // we still update in-place but let the user rename if desired.
    if (isUpdate) {
      try {
        const r = await fetch(`/api/presets/${wizardState.savedPresetId}`, { headers });
        const existing = r.ok ? await r.json().catch(() => null) : null;
        if (existing && (
              existing.model_path !== payload.model_path ||
              (existing.context_size || 0) !== (payload.context_size || 0)
            )) {
          // Config diverged — we’ll still update the existing preset (user can rename),
          // but we hint that it’s been changed.
          isUpdate = true;
        }
      } catch {
        // If we can’t read it, proceed to update anyway.
      }
    }

    let resp;
    if (isUpdate) {
      // Update existing preset
      resp = await fetch(`/api/presets/${wizardState.savedPresetId}`, {
        method: 'PUT',
        headers,
        body: JSON.stringify(payload),
      });
    } else {
      // Create new preset
      resp = await fetch('/api/presets', {
        method: 'POST',
        headers,
        body: JSON.stringify(payload),
      });
    }

    if (!resp.ok) {
      showToast('Save preset failed: ' + await resp.text().catch(() => ''), 'error');
      return;
    }

    // If this was the first save, store the preset id so next saves update the same preset.
    if (!wizardState.savedPresetId) {
      try {
        const data = await resp.json().catch(() => ({}));
        if (data.id) wizardState.savedPresetId = data.id;
      } catch {
        // non-fatal
      }
    }

    // Refresh the setup view preset dropdown
    import('./presets.js').then(({ loadPresets }) => loadPresets().then(() => {
      import('./setup-view.js').then(({ syncSetupPresetSelect }) => syncSetupPresetSelect());
    }));

    const verb = isUpdate ? 'updated' : 'saved';
    showToast(`Preset "${name}" ${verb}`, 'success');
    if (dom.savedPresetName) {
      dom.savedPresetName.textContent = `✓ ${isUpdate ? 'Updated' : 'Saved'} as "${name}"`;
      dom.savedPresetName.style.display = '';
    }
  } catch (err) {
    showToast('Save preset failed: ' + (err.message || String(err)), 'error');
  } finally {
    if (btn) { btn.disabled = false; btn.textContent = 'Save as Preset'; }
  }
}

function buildPresetPayload() {
  const spawnPayload = buildSpawnPayload();
  return {
    name: 'Spawn Wizard Preset',
    ...spawnPayload,
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

// ── Preset parameters review (step 5) ─────────────────────────────────────────

function _renderPresetParamsStep() {
  const container = dom.presetParamsTable;
  if (!container) return;

 // Pre-fill preset name from model filename if empty
   if (dom.presetNameInput && !dom.presetNameInput.value.trim()) {
     const m = wizardState.model;
     const ctx = wizardState.hardware.contextSize || 0;
     const modelFile = (m.path || m.hfRepo || '').split(/[/\\]/).pop() || '';
     const base = (modelFile || '').replace(/\.gguf$/i, '').trim();
     const name = base && ctx
       ? base + '-' + formatCtx(ctx).toLowerCase()
       : base || 'My Preset';
     dom.presetNameInput.value = name;
   }
  if (dom.savedPresetName) dom.savedPresetName.style.display = 'none';

  const h = wizardState.hardware, m = wizardState.model;
  const arch = getSizingArch();

  const modelDisplay = m.source === 'hf'
    ? (m.hfFile ? m.hfFile.split('/').pop() : (m.hfRepo || '—'))
    : (m.path ? m.path.split(/[\\/]/).pop() || m.path : '—');

  const gpuDisplay = h.gpuLayers === 'manual'
    ? String(h.gpuLayersManual ?? '—')
    : h.gpuLayers;

  const kvK = (h.cacheTypeK || 'q8_0').toUpperCase();
  const kvV = (h.cacheTypeV || 'q8_0').toUpperCase();
  const fmtSampling = v => v != null ? String(parseFloat(Number(v).toFixed(4))) : '— (server default)';

  const sections = [
    {
      label: 'Model',
      rows: [
        { label: 'File', value: modelDisplay },
        ...(m.source === 'hf' ? [{ label: 'HF repo', value: m.hfRepo || '—' }] : []),
        ...(m.mmprojPath ? [{ label: 'mmproj', value: m.mmprojPath.split(/[\\/]/).pop() || m.mmprojPath }] : []),
        ...(wizardState.model.chatTemplatePath ? [{ label: 'Chat template', value: wizardState.model.chatTemplatePath.split(/[\\/]/).pop() || wizardState.model.chatTemplatePath }] : []),
      ],
    },
    {
      label: 'Hardware',
      rows: [
        { label: 'GPU layers', value: gpuDisplay },
        { label: 'Context size', value: `${h.contextSize.toLocaleString()} tokens` },
        { label: 'Batch / uBatch', value: `${h.batchSize} / ${h.ubatchSize}` },
        { label: 'Parallel slots', value: String(h.parallelSlots) },
        { label: 'KV cache K', value: kvK },
        { label: 'KV cache V', value: kvV },
        ...(h.flashAttn && h.flashAttn !== 'auto' ? [{ label: 'Flash Attn', value: h.flashAttn }] : []),
        ...(h.kvUnified ? [{ label: 'KV unified', value: 'Yes' }] : []),
        ...(h.mlock ? [{ label: 'mlock', value: 'Yes' }] : []),
        ...(h.prio != null ? [{ label: 'Priority', value: ['Normal', 'Medium', 'High', 'Realtime'][h.prio] ?? String(h.prio) }] : []),
        ...(h.nCpuMoe > 0 && arch.nExperts > 0 ? [{ label: 'MoE CPU offload', value: `${h.nCpuMoe} of ${arch.nLayers} layers` }] : []),
        ...(h.tensorSplit ? [{ label: 'Tensor split', value: h.tensorSplit }] : []),
        ...(h.fitTarget ? [{ label: '--fit-target', value: `${h.fitTarget} MB` }] : []),
        ...(h.cacheRam != null ? [{ label: '--cache-ram', value: h.cacheRam < 0 ? 'no limit' : h.cacheRam === 0 ? 'disabled' : `${h.cacheRam} MiB` }] : []),
      ],
    },
    {
      label: 'Sampling',
      rows: [
        { label: 'Temperature', value: fmtSampling(h.temperature) },
        { label: 'Top-P', value: fmtSampling(h.topP) },
        { label: 'Top-K', value: fmtSampling(h.topK) },
        { label: 'Min-P', value: fmtSampling(h.minP) },
        { label: 'Repeat penalty', value: fmtSampling(h.repeatPenalty) },
        { label: 'Presence penalty', value: fmtSampling(h.presencePenalty) },
        { label: 'Max tokens', value: h.maxTokens != null ? String(h.maxTokens) : '— (server default)' },
        { label: 'Seed', value: h.seed != null ? String(h.seed) : '— (random)' },
      ],
    },
  ];

  // Thinking section only when something is set
  const hasThinking = h.enableThinking != null || h.preserveThinking != null ||
                      h.reasoningMode != null || h.reasoningBudget != null;
  if (hasThinking) {
    const rows = [];
    if (h.enableThinking != null) rows.push({ label: 'Enable thinking', value: h.enableThinking ? 'Yes' : 'No' });
    if (h.preserveThinking != null) rows.push({ label: 'Preserve thinking', value: h.preserveThinking ? 'Yes' : 'No' });
    if (h.reasoningMode) rows.push({ label: 'Reasoning mode', value: h.reasoningMode });
    if (h.reasoningBudget != null) rows.push({ label: 'Reasoning budget', value: `${h.reasoningBudget} tokens` });
    if (h.reasoningBudgetMessage) rows.push({ label: 'Budget message', value: h.reasoningBudgetMessage });
    if (rows.length) sections.push({ label: 'Thinking & Reasoning', rows });
  }

  const outputMode = _structuredOutputMode();
  if (outputMode) {
    sections.push({
      label: 'Response Shaping',
      rows: [{
        label: outputMode === 'grammar' ? 'Grammar' : 'JSON schema',
        value: outputMode === 'grammar'
          ? ((h.grammar || '').split('\n')[0] || 'configured')
          : ((h.jsonSchema || '').split('\n')[0] || 'configured'),
      }],
    });
  }

  sections.push({
    label: 'Network & Identity',
    rows: [
      { label: 'Port', value: String(wizardState.access.port || 8001) },
      { label: 'Bind host', value: wizardState.access.bindHost === '0.0.0.0' ? '0.0.0.0 (LAN visible)' : '127.0.0.1 only' },
      { label: 'Alias', value: h.alias || '(derived from filename)' },
      { label: 'API key', value: wizardState.access.apiKey ? `${wizardState.access.apiKey.slice(0, 4)}…${wizardState.access.apiKey.slice(-4)}` : 'Not set' },
    ],
  });

  const specType = dom.specTypeSelect?.value || '';
  if (specType) {
      const rows = [{ label: 'Type', value: specType }];
      // Show draft model info for draft-model or MTP modes with external assistant
      if (dom.draftModelInput?.value) {
        const fileName = dom.draftModelInput.value.split(/[\\/]/).pop() || dom.draftModelInput.value;
        if (specType === 'draft-model') rows.push({ label: 'Draft model', value: fileName });
        else if (specType.includes('draft-mtp')) rows.push({ label: 'Draft model', value: fileName });
      }
      if (dom.specNgramSizeInput?.value) rows.push({ label: 'N-gram size', value: dom.specNgramSizeInput.value });
      sections.push({ label: 'Speculative Decoding', rows });
    }

  if (h.extraArgs) {
    sections.push({ label: 'Extra', rows: [{ label: 'Extra args', value: h.extraArgs }] });
  }

  container.innerHTML = '';

  for (const section of sections) {
    if (!section.rows.length) continue;
    const block = document.createElement('div');
    block.className = 'summary-list';
    block.style.marginTop = '8px';

    const hdr = document.createElement('div');
    hdr.className = 'summary-list-header';
    hdr.textContent = section.label;
    block.appendChild(hdr);

    for (const r of section.rows) {
      const row = document.createElement('div');
      row.className = 'summary-row';
      const lbl = document.createElement('span');
      lbl.className = 'summary-label';
      lbl.textContent = r.label;
      const val = document.createElement('span');
      val.className = 'summary-value';
      val.textContent = r.value;
      row.appendChild(lbl);
      row.appendChild(val);
      block.appendChild(row);
    }
    container.appendChild(block);
  }

  // Edit shortcuts
  const editRow = document.createElement('div');
  editRow.className = 'summary-edit-row';
  editRow.style.cssText = 'display:flex;gap:8px;flex-wrap:wrap;margin-top:10px;padding:0;';
  [
    { label: 'Edit model', step: 1 },
    { label: 'Edit hardware', step: 2 },
    { label: 'Edit sampling', step: 3, focusId: 'spawn-temperature' },
  ].forEach(({ label, step, focusId }) => {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'btn-wizard-tertiary';
    btn.textContent = label;
    btn.addEventListener('click', () => {
      showStep(step);
      if (focusId) setTimeout(() => document.getElementById(focusId)?.focus(), 50);
    });
    editRow.appendChild(btn);
  });
  container.appendChild(editRow);
}

// ── Spawn config preview card (step 6) ────────────────────────────────────────

async function _renderSpawnConfigCard() {
  const card = document.getElementById('spawn-config-card');
  const sidebar = document.getElementById('spawn-sidebar-config');
  const m = wizardState.model, hw = wizardState.hardware, acc = wizardState.access;

  const modelName = m.source === 'hf'
    ? (m.hfFile ? m.hfFile.split('/').pop() : (m.hfRepo || '—'))
    : (m.path ? (m.path.split(/[\\/]/).pop() || m.path) : '—');
  const port     = acc.port || 8001;
  const ctx      = hw.contextSize ? hw.contextSize.toLocaleString() + ' tok' : '—';
  const gpu      = hw.gpuLayers === 'manual'
    ? `${hw.gpuLayersManual ?? '?'} layers`
    : (hw.gpuLayers === 'all' ? 'All GPU' : 'Auto');
  const bind     = acc.bindHost === '0.0.0.0' ? '0.0.0.0 (LAN)' : '127.0.0.1';
  const kvStr    = `${(hw.cacheTypeK || 'q8_0').toUpperCase()} / ${(hw.cacheTypeV || 'q8_0').toUpperCase()}`;
  const alias    = hw.alias || modelName.replace(/\.gguf$/i, '').replace(/[^A-Za-z0-9._-]/g, '-');

  // Fetch tags for this model
  let modelTags = [];
  try {
    const modelPath = m.path || (m.localPath && m.localPath.trim());
    if (modelPath) {
      const headers = window.authHeaders ? window.authHeaders() : {};
      const resp = await fetch('/api/models/tags', { headers });
      if (resp.ok) {
        const data = await resp.json();
        modelTags = (data.tags && data.tags[modelPath]) || [];
      }
    }
  } catch { /* ignore */ }

  if (card) {
    card.style.display = '';
    const mk = (tag, cls, text) => {
      const el = document.createElement(tag);
      if (cls) el.className = cls;
      if (text !== undefined) el.textContent = text;
      return el;
    };
    card.innerHTML = '';

    const hdr = mk('div', 'spawn-config-card-header');
    hdr.appendChild(mk('span', 'spawn-config-card-title', 'Model'));
    hdr.appendChild(mk('span', 'spawn-config-card-model', modelName));
    card.appendChild(hdr);

    // Add tag pills if model has tags
    if (modelTags.length > 0) {
      const tagsRow = mk('div', 'spawn-config-card-tags');
      modelTags.forEach(tag => {
        const pill = mk('span', 'mm-tag-pill', tag);
        tagsRow.appendChild(pill);
      });
      card.appendChild(tagsRow);
    }

    const grid = mk('div', 'spawn-config-grid');
    const items = [
      ['Port',    String(port)],
      ['Host',    bind],
      ['Context', ctx],
      ['GPU',     gpu],
      ['KV quant', kvStr],
      ['Alias',   alias],
    ];
    items.forEach(([label, value]) => {
      const item = mk('div', 'spawn-config-item');
      item.appendChild(mk('div', 'spawn-config-item-label', label));
      item.appendChild(mk('div', 'spawn-config-item-value', value));
      grid.appendChild(item);
    });
    card.appendChild(grid);
  }

  if (sidebar) {
    sidebar.innerHTML = '';
    [
      ['Model', modelName],
      ['Port',  String(port)],
      ['Host',  bind],
    ].forEach(([label, value]) => {
      const stat = document.createElement('div');
      stat.className = 'spawn-sidebar-stat';
      const lbl = document.createElement('span');
      lbl.className = 'spawn-sidebar-stat-label';
      lbl.textContent = label;
      const val = document.createElement('span');
      val.className = 'spawn-sidebar-stat-value';
      val.textContent = value;
      stat.appendChild(lbl);
      stat.appendChild(val);
      sidebar.appendChild(stat);
    });
  }
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
    // /api/sessions/spawn requires the db-admin-token, not the llama-server API token.
    const tokenResp = await fetch('/api/db/admin-token', {
      headers: window.authHeaders ? window.authHeaders() : {},
    });
    const tokenData = tokenResp.ok ? await tokenResp.json().catch(() => ({})) : {};
    const adminToken = tokenData.token;
    if (!adminToken) { throw new Error('Authentication required. Could not retrieve admin token.'); }
    const headers = { 'Content-Type': 'application/json', 'Authorization': `Bearer ${adminToken}` };
    const resp = await fetch('/api/sessions/spawn', { method: 'POST', headers, body: JSON.stringify(payload) });
    setProgress(60);
    if (!resp.ok) {
      if (resp.status === 429) {
        // Cooldown: parse seconds_remaining and disable button
        const d = await resp.json().catch(() => null);
        const seconds = d?.seconds_remaining || 15;
        showErrorText(`Spawn request too soon. Please wait ${seconds} seconds.`);
        setStatusText('Cooldown active.');
        setProgress(0);
        // Disable button with countdown
        if (dom.spawnServerBtn) {
          dom.spawnServerBtn.disabled = true;
          const origText = dom.spawnServerBtn.textContent || '';
          let left = seconds;
          const iv = setInterval(() => {
            if (left <= 0) {
              clearInterval(iv);
              if (dom.spawnServerBtn) {
                dom.spawnServerBtn.disabled = false;
                dom.spawnServerBtn.textContent = origText;
              }
            } else {
              if (dom.spawnServerBtn) dom.spawnServerBtn.textContent = `Wait ${left}s`;
              left--;
            }
          }, 1000);
        }
        wizardState.spawn.inFlight = false;
        return;
      }
      const t = await resp.text().catch(()=>'Unknown error');
      throw new Error(t || `HTTP ${resp.status}`);
    }
    const data = await resp.json().catch(() => null);
    if (!data?.ok) throw new Error(data?.error || 'Spawn request failed.');
    setStatusText('Server process started. Waiting for endpoint…');
    setProgress(75);
    await waitForSpawnReadiness(payload.port);
    setProgress(100); setStatusText('Server started.');
    showSuccessText('Server is running.');
    showToast('Server started', 'success', '', { duration: 8000 });
    setTuneConfig(payload);
    setTimeout(() => {
      closeSpawnWizard();
      setHeaderMode('Spawn:' + (payload.port || 8001));
      if (document.body.classList.contains('setup-active')) {
        switchView('monitor');
      }
      showTunePanel();
      // Select the preset that was saved during this wizard run (if any)
      if (wizardState.savedPresetId) {
        import('./presets.js').then(({ loadPresets }) => {
          loadPresets(wizardState.savedPresetId);
        });
      }
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
  const specTypeUser = dom.specTypeSelect?.value || '';
  // Respect an explicit 'draft-mtp' choice from the advanced dropdown; only
  // default to 'draft-mtp,ngram-mod' when the user hasn't already chosen a
  // draft-mtp variant (covers the common case where the dropdown is empty).
  const specType = mtpActive
    ? (specTypeUser.includes('draft-mtp') ? specTypeUser : 'draft-mtp,ngram-mod')
    : specTypeUser;
  const parallelSlots = mtpActive ? 1 : h.parallelSlots;

  // Resolve draft assistant: prefer MTP section selection, fall back to
  // advanced draft-model input if user configured one.
  const assistantPath = m.selectedDraftPath || (dom.draftModelInput?.value || '').trim() || '';

  // Resolve mmproj local path: prefer mmprojPath (local file), fall back to
  // mmprojHfFile only if it looks like an absolute path (i.e. was set from
  // a directory scan, not from an HF file list).
  const mmprojLocal = m.mmprojPath && m.mmprojPath.startsWith('/') ? m.mmprojPath : null;

  // MTP n-max: 4 when assistant is present (recommended for Gemma4-style), 2 otherwise.
  // Use != null so an explicit 0 from the user is not treated as "unset".
  const mtpNMaxDefault = assistantPath ? 4 : 2;
  const mtpNMax =
    (mtpActive || (assistantPath && (specType.includes('draft-mtp') || specType.includes('draft-model'))))
      ? (h.mtpDraftNMax != null ? h.mtpDraftNMax : mtpNMaxDefault)
      : undefined;

  return {
    model_path: m.source !== 'hf' ? (m.path || '') : '',
    hf_repo: m.source === 'hf' ? (m.hfRepo || null) : null,
    hf_file: m.source === 'hf' ? (m.hfFile || null) : null,
    mmproj: mmprojLocal,
    port: wizardState.access.port || 8001,
    bind_host: wizardState.access.bindHost || '127.0.0.1',
    gpu_layers: gpuLayers,
    context_size: h.contextSize,
    batch_size: h.batchSize,
    ubatch_size: h.ubatchSize,
    parallel_slots: parallelSlots,
    ctk: h.cacheTypeK || '',
    ctv: h.cacheTypeV || '',
    n_cpu_moe: h.nCpuMoe || null,
    tensor_split: h.tensorSplit || '',
    no_mmap: true,
    ngram_spec: false,
    spec_type: specType,
    spec_draft_n_max: mtpNMax,
    spec_draft_n_min: mtpActive && h.mtpDraftNMin != null ? h.mtpDraftNMin : undefined,
    spec_draft_p_min: mtpActive && h.mtpDraftPMin != null ? h.mtpDraftPMin : undefined,
    draft_model: assistantPath || '',
    kv_unified: h.kvUnified || null,
    flash_attn: h.flashAttn || '',
    mlock: h.mlock || false,
    prio: h.prio != null ? h.prio : null,
    threads: h.threads != null ? h.threads : null,
    threads_batch: h.threadsBatch != null ? h.threadsBatch : null,
    fit_enabled: h.fitTarget ? true : null,
    fit_target: h.fitTarget || null,
    cache_ram_mib: (h.cacheRam !== null && h.cacheRam !== undefined) ? h.cacheRam : null,
    // Sampling defaults (null = use llama-server built-in defaults)
    temperature: h.temperature != null ? h.temperature : null,
    top_p: h.topP != null ? h.topP : null,
    top_k: h.topK != null ? h.topK : null,
    min_p: h.minP != null ? h.minP : null,
    repeat_penalty: h.repeatPenalty != null ? h.repeatPenalty : null,
    presence_penalty: h.presencePenalty != null && h.presencePenalty > 0 ? h.presencePenalty : null,
    max_tokens: h.maxTokens != null ? h.maxTokens : null,
    seed: h.seed != null ? h.seed : null,
    // Thinking / reasoning
    enable_thinking: h.enableThinking,
    preserve_thinking: h.preserveThinking,
    reasoning_budget: h.reasoningBudget,
    reasoning: h.reasoningMode || null,
    reasoning_budget_message: h.reasoningBudgetMessage || null,
    grammar: h.grammar.trim() ? h.grammar.trim() : null,
    json_schema: h.jsonSchema.trim() ? h.jsonSchema.trim() : null,
    // Image token budget — only passed when mmproj is active.
    // Values are derived from model family; user can override via extra_args.
    image_min_tokens: mmprojLocal ? _imageMinTokens(m) : null,
    image_max_tokens: mmprojLocal ? _imageMaxTokens(m) : null,
    api_key: wizardState.access.apiKey || null,
    alias: h.alias || null,
    extra_args: h.extraArgs || '',
    chat_template_file: wizardState.model.chatTemplatePath || null,
    profile: wizardState.profile,
    use_case: wizardState.useCase,
  };
}

function _modelNameLower(m) {
  return ((m.hfFile || '').split('/').pop() || m.path?.split(/[\\/]/).pop() || m.hfRepo || '').toLowerCase();
}

function _imageMinTokens(m) {
  const name = _modelNameLower(m);
  if (name.includes('gemma')) return 280;
  return 1024; // Qwen3.6 / default for other vision models
}

function _imageMaxTokens(m) {
  const name = _modelNameLower(m);
  if (name.includes('gemma')) return 560;
  return 4096; // Qwen3.6 / default for other vision models
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

  // Move focus out before hiding so it's not trapped inside aria-hidden
  const wasFocused = document.activeElement;
  if (dom.cardPanel.contains(wasFocused)) {
    wasFocused.blur();
    // Restore focus to a neutral ancestor (hardware step)
    const step = document.getElementById('wizard-step-2');
    if (step && step.focus) {
      step.focus({ preventScroll: true });
    }
  }

  dom.cardPanel.classList.remove('open');
  dom.cardPanel.setAttribute('aria-hidden', 'true');
}

// ── Binary prerequisite check & download ─────────────────────────────────────

let _binaryReady  = false;
let _platformInfo = null;   // cached result of /api/llama-binary/platform-info
let _selectedBackend = null;
let _mtpUserConfigured = false;

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

    // MTP performance is backend-dependent. Metal can regress even with high
    // draft acceptance, so require an explicit opt-in on Apple Silicon.
    if (!_mtpUserConfigured) {
      wizardState.hardware.mtpEnabled = _platformInfo?.auto_backend !== 'metal';
      const mtpCheck = document.getElementById('hw-use-mtp');
      const mtpDepthRow = document.getElementById('hw-mtp-depth-row');
      if (mtpCheck) mtpCheck.checked = wizardState.hardware.mtpEnabled;
      if (mtpDepthRow) {
        mtpDepthRow.style.display = wizardState.hardware.mtpEnabled ? '' : 'none';
      }
    }

    // For unified memory (Apple Silicon): default --cache-ram to 0 on first load.
    // The default of 8 GB wastes unified memory that is better used for model/context.
    // Only set if the user hasn't already configured a value.
    if (_platformInfo?.auto_backend === 'metal' && wizardState.hardware.cacheRam == null) {
      wizardState.hardware.cacheRam = 0;
      if (dom.cacheRamInput) dom.cacheRamInput.value = '0';
    }
    // Show unified memory note about -cram.
    if (_platformInfo?.auto_backend === 'metal') {
      const hint = document.getElementById('unified-cram-hint');
      if (hint) hint.style.display = '';
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
