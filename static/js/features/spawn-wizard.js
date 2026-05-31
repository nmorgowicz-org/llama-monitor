// ── Spawn Wizard Module ───────────────────────────────────────────────────────
// Spawn Llama-Server V2: step-based wizard for configuring and launching
// llama-server with a guided UX.
//
// Exports:
//   initSpawnWizard()
//   openSpawnWizard()
//   closeSpawnWizard()

import { openDeferredFileBrowser } from './file-browser-launcher.js';
import { showToast } from './toast.js';

// ── State ─────────────────────────────────────────────────────────────────────

const STEP_LABELS = ['Profile', 'Model', 'Hardware', 'Summary', 'Spawn'];

const wizardState = {
    currentStep: 0,
    profile: 'balanced',
    mode: 'guided', // 'guided' | 'raw'
    model: {
        source: 'local',
        path: '',
        hfRepo: '',
        hfFile: '',
    },
    hardware: {
        gpuLayers: 'auto',
        gpuLayersManual: null,
        contextSize: 8192,
        batchSize: 2048,
        ubatchSize: 512,
        parallelSlots: 1,
        cacheTypeK: '',
        cacheTypeV: '',
        nCpuMoe: null,
        tensorSplit: '',
    },
    vram: {
        estimated: null,
        available: null,
        status: null, // fit | tight | risk | wont-fit
    },
    spawn: {
        inFlight: false,
        error: '',
    },
};

// ── DOM refs (cached on init) ─────────────────────────────────────────────────

let dom = {};

// ── Public API ────────────────────────────────────────────────────────────────

export function initSpawnWizard() {
    cacheDom();
    applyReducedMotion();
    bindEvents();
    restoreProfile();
    applyProfileVisibility();
}

function applyReducedMotion() {
    const prefersReduced = window.matchMedia?.('(prefers-reduced-motion: reduce)');
    if (prefersReduced?.matches) {
        document.documentElement.classList.add('reduce-motion');
    }
}

export function openSpawnWizard() {
    if (!dom.overlay) return;
    dom.overlay.classList.add('open');
    updateModelInputVisibility();
    showStep(0);
}

export function closeSpawnWizard() {
    if (!dom.overlay) return;
    dom.overlay.classList.remove('open');
    resetSpawnStatus();
}

// ── DOM Caching ───────────────────────────────────────────────────────────────

function cacheDom() {
    dom.overlay = document.getElementById('spawn-wizard-overlay');
    dom.closeBtn = document.getElementById('spawn-wizard-close');
    dom.stepLabel = document.getElementById('wizard-step-label');
    dom.stepBadges = dom.overlay?.querySelectorAll('.step-badge[data-step]');
    dom.steps = dom.overlay?.querySelectorAll('.wizard-step[id^="wizard-step-"]');

    // Footer
    dom.backBtn = document.getElementById('wizard-back-btn');
    dom.nextBtn = document.getElementById('wizard-next-btn');

    // Step 1
    dom.profileCards = dom.overlay?.querySelectorAll('.profile-card[data-profile]');

    // Step 2
    dom.modelSourceCards = dom.overlay?.querySelectorAll('.model-source-card[data-source]');
    dom.modelInputLocal = document.getElementById('model-input-local');
    dom.modelInputHf = document.getElementById('model-input-hf');
    dom.modelInputImport = document.getElementById('model-input-import');
    dom.modelPathInput = document.getElementById('spawn-model-path');
    dom.hfRepoInput = document.getElementById('spawn-hf-repo');
    dom.importPathInput = document.getElementById('spawn-import-path');
    dom.browseModelBtn = document.getElementById('spawn-browse-model-btn');
    dom.importBrowseBtn = document.getElementById('spawn-import-browse-btn');
    dom.selectedModel = document.getElementById('spawn-selected-model');
    dom.selectedModelName = document.getElementById('spawn-selected-model-name');
    dom.selectedModelMeta = document.getElementById('spawn-selected-model-meta');
    dom.hfFileList = document.getElementById('spawn-hf-file-list');

    // Step 3
    dom.modeToggle = document.getElementById('spawn-mode-toggle');
    dom.modeGuidedBtn = document.getElementById('spawn-mode-guided');
    dom.modeRawBtn = document.getElementById('spawn-mode-raw');
    dom.rawCodeArea = document.getElementById('spawn-raw-script');
    // Guided/raw section containers — may be absent if raw mode not in HTML yet
    dom.wizardGuidedSection = document.getElementById('spawn-wizard-guided-section');
    dom.wizardRawSection = document.getElementById('spawn-wizard-raw-section');
    dom.gpuLayersSelect = document.getElementById('spawn-gpu-layers');
    dom.gpuLayersManualWrap = document.getElementById('spawn-gpu-layers-manual-wrap');
    dom.gpuLayersManualInput = document.getElementById('spawn-gpu-layers-manual');
    dom.contextSizeInput = document.getElementById('spawn-context-size');
    dom.batchSizeInput = document.getElementById('spawn-batch-size');
    dom.advancedFields = document.getElementById('spawn-advanced-fields');
    dom.ubatchSizeInput = document.getElementById('spawn-ubatch-size');
    dom.parallelSlotsInput = document.getElementById('spawn-parallel-slots');
    dom.cacheTypeKSelect = document.getElementById('spawn-cache-type-k');
    dom.cacheTypeVSelect = document.getElementById('spawn-cache-type-v');
    dom.nCpuMoeInput = document.getElementById('spawn-n-cpu-moe');
    dom.tensorSplitInput = document.getElementById('spawn-tensor-split');
    dom.moeNote = document.getElementById('spawn-moe-note');
    dom.vramEstimateText = document.getElementById('spawn-vram-estimate-text');
    dom.vramPill = document.getElementById('spawn-vram-pill');
    dom.specTypeSelect = document.getElementById('spawn-spec-type');
    dom.draftModelWrap = document.getElementById('spawn-draft-model-wrap');
    dom.draftModelInput = document.getElementById('spawn-draft-model');
    dom.kvUnifiedCheck = document.getElementById('spawn-kv-unified');
    dom.ignoreEosCheck = document.getElementById('spawn-ignore-eos');

    // Step 4
    dom.summaryList = document.getElementById('spawn-summary-list');
    dom.summaryWarnings = document.getElementById('spawn-summary-warnings');
    dom.savePresetBtn = document.getElementById('spawn-save-preset-btn');
    dom.healthCheckBtn = document.getElementById('spawn-health-check-btn');

    // Step 5
    dom.spawnServerBtn = document.getElementById('spawn-server-btn');
    dom.statusText = document.getElementById('spawn-status-text');
    dom.progressFill = document.getElementById('spawn-progress-fill');
    dom.errorText = document.getElementById('spawn-error-text');
    dom.successText = document.getElementById('spawn-success-text');

    // Validation error area (inserted dynamically if needed)
    dom.validationError = null;
}

// ── Events ────────────────────────────────────────────────────────────────────

function bindEvents() {
    // Close
    dom.closeBtn?.addEventListener('click', closeSpawnWizard);
    dom.overlay?.addEventListener('click', (e) => {
        if (e.target === dom.overlay) closeSpawnWizard();
    });

    // Keyboard: Escape closes wizard
    document.addEventListener('keydown', (e) => {
        if (!dom.overlay?.classList.contains('open')) return;
        if (e.key === 'Escape') closeSpawnWizard();
    });

    // Back/Next
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

    // Profile cards (click + Enter/Space)
    dom.profileCards?.forEach(card => {
        card.setAttribute('tabindex', '0');
        card.setAttribute('role', 'button');
        card.addEventListener('click', () => {
            wizardState.profile = card.dataset.profile;
            dom.profileCards.forEach(c => c.classList.remove('selected'));
            card.classList.add('selected');
            persistProfile();
            applyProfileVisibility();
        });
        card.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); card.click(); }
        });
    });

    // Model source cards (click + Enter/Space)
    dom.modelSourceCards?.forEach(card => {
        card.setAttribute('tabindex', '0');
        card.setAttribute('role', 'button');
        card.addEventListener('click', () => {
            wizardState.model.source = card.dataset.source;
            dom.modelSourceCards.forEach(c => c.classList.remove('selected'));
            card.classList.add('selected');
            updateModelInputVisibility();
            clearValidationError();
            // Auto-scan import sources when switching to that tab
            if (card.dataset.source === 'import') loadThirdPartyModels();
        });
        card.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); card.click(); }
        });
    });

    // Browse local model
    dom.browseModelBtn?.addEventListener('click', () => {
        openDeferredFileBrowser('spawn-model-path', 'gguf');
    });

    // Browse import path
    dom.importBrowseBtn?.addEventListener('click', () => {
        openDeferredFileBrowser('spawn-import-path', 'gguf');
    });

    // Model path change
    dom.modelPathInput?.addEventListener('input', () => {
        wizardState.model.path = dom.modelPathInput.value.trim();
        wizardState.model.source = 'local';
        updateSelectedModelDisplay();
        scheduleVramEstimate();
        clearValidationError();
    });

    // Import path change
    dom.importPathInput?.addEventListener('input', () => {
        wizardState.model.path = dom.importPathInput.value.trim();
        wizardState.model.source = 'import';
        updateSelectedModelDisplay();
        scheduleVramEstimate();
        clearValidationError();
    });

    // HF repo: fetch file list on blur or Enter
    dom.hfRepoInput?.addEventListener('blur', () => {
        const repo = dom.hfRepoInput.value.trim();
        if (!repo) return;
        wizardState.model.hfRepo = repo;
        fetchHfFiles(repo);
    });
    dom.hfRepoInput?.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            const repo = dom.hfRepoInput.value.trim();
            if (repo) { wizardState.model.hfRepo = repo; fetchHfFiles(repo); }
        }
    });

    // Hardware fields change
    [
        dom.gpuLayersSelect,
        dom.gpuLayersManualInput,
        dom.contextSizeInput,
        dom.batchSizeInput,
        dom.ubatchSizeInput,
        dom.parallelSlotsInput,
        dom.cacheTypeKSelect,
        dom.cacheTypeVSelect,
        dom.nCpuMoeInput,
        dom.tensorSplitInput,
        dom.specTypeSelect,
        dom.draftModelInput,
        dom.kvUnifiedCheck,
        dom.ignoreEosCheck,
    ].forEach(el => {
        el?.addEventListener('input', onHardwareChange);
        el?.addEventListener('change', onHardwareChange);
    });

    // GPU layers mode toggle
    dom.gpuLayersSelect?.addEventListener('change', () => {
        const v = dom.gpuLayersSelect.value;
        wizardState.hardware.gpuLayers = v;
        if (dom.gpuLayersManualWrap) {
            dom.gpuLayersManualWrap.style.display = v === 'manual' ? '' : 'none';
        }
    });

    // Speculative decoding type
    dom.specTypeSelect?.addEventListener('change', () => {
        const v = dom.specTypeSelect.value;
        if (dom.draftModelWrap) {
            dom.draftModelWrap.style.display = v === 'draft-model' ? '' : 'none';
        }
    });

    // Save as Preset
    dom.savePresetBtn?.addEventListener('click', saveAsPreset);

    // Health Check
    dom.healthCheckBtn?.addEventListener('click', runHealthCheck);

    // Spawn Server
    dom.spawnServerBtn?.addEventListener('click', spawnServer);

    // Mode toggle (Guided/Raw)
    dom.modeGuidedBtn?.addEventListener('click', () => setMode('guided'));
    dom.modeRawBtn?.addEventListener('click', () => setMode('raw'));

    // Raw code area: parse common flags on change
    dom.rawCodeArea?.addEventListener('input', onRawCodeChange);
}

// ── Validation ────────────────────────────────────────────────────────────────

function validateStep(step) {
    if (step === 1) {
        // Model step: require a model path or HF repo selection
        const { source, path, hfRepo, hfFile } = wizardState.model;
        if (source === 'local' || source === 'import') {
            if (!path) {
                showValidationError('Please select or enter a model path before continuing.');
                return false;
            }
        } else if (source === 'hf') {
            if (!hfRepo) {
                showValidationError('Please enter a HuggingFace repo ID (e.g. user/repo).');
                return false;
            }
            if (!hfFile) {
                showValidationError('Please select a GGUF file from the list below the repo field.');
                return false;
            }
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
    dom.overlay?.querySelectorAll('.wizard-validation-error').forEach(el => {
        el.style.display = 'none';
    });
}

// ── Mode Toggle (Guided / Raw) ────────────────────────────────────────────────

function setMode(mode) {
    wizardState.mode = mode;

    if (dom.wizardGuidedSection) {
        dom.wizardGuidedSection.classList.toggle('hidden', mode !== 'guided');
    }
    if (dom.wizardRawSection) {
        dom.wizardRawSection.classList.toggle('hidden', mode !== 'raw');
    }
    dom.modeGuidedBtn?.classList.toggle('active', mode === 'guided');
    dom.modeRawBtn?.classList.toggle('active', mode !== 'guided');

    if (mode === 'raw') updateRawScript();
}

function updateRawScript() {
    const el = dom.rawCodeArea;
    if (!el) return;

    const s = wizardState;
    const hw = s.hardware;
    const model = s.model;
    const bin = 'llama-server';
    const args = [];

    if (model.source === 'hf' && model.hfRepo) {
        args.push('-hf', model.hfRepo);
        if (model.hfFile) args.push('--hf-file', model.hfFile);
    } else if (model.path) {
        args.push('-m', `"${model.path}"`);
    }

    if (hw.gpuLayers === 'auto' || hw.gpuLayers === 'all') {
        args.push('-ngl', '9999');
    } else if (hw.gpuLayersManual != null) {
        args.push('-ngl', String(hw.gpuLayersManual));
    }

    if (hw.contextSize) args.push('-c', String(hw.contextSize));
    if (hw.batchSize) args.push('-b', String(hw.batchSize));
    if (hw.ubatchSize) args.push('-ub', String(hw.ubatchSize));
    if (hw.parallelSlots > 1) args.push('--parallel', String(hw.parallelSlots));
    if (hw.cacheTypeK) args.push('-ctk', hw.cacheTypeK);
    if (hw.cacheTypeV) args.push('-ctv', hw.cacheTypeV);
    if (hw.nCpuMoe != null && hw.nCpuMoe > 0) args.push('--n-cpu-moe', String(hw.nCpuMoe));
    if (hw.tensorSplit) args.push('--tensor-split', hw.tensorSplit);

    const specType = dom.specTypeSelect?.value || '';
    if (specType) {
        args.push('--spec-type', specType);
        if (specType === 'draft-model') {
            const dp = (dom.draftModelInput?.value || '').trim();
            if (dp) args.push('-md', `"${dp}"`);
        }
    }
    if (dom.kvUnifiedCheck?.checked) args.push('--kv-unified');
    if (dom.ignoreEosCheck?.checked) args.push('--ignore-eos');

    el.textContent = bin + ' \\\n  ' + args.join(' \\\n  ');
}

function onRawCodeChange() {
    const el = dom.rawCodeArea;
    if (!el) return;
    const text = el.textContent || '';
    const hw = wizardState.hardware;

    const readFlag = (flag) => {
        const idx = text.indexOf(flag + ' ');
        if (idx === -1) return null;
        const token = text.slice(idx + flag.length).trim().split(/[\s\\]+/)[0];
        return token || null;
    };

    const ctx = readFlag('-c') || readFlag('--ctx-size');
    if (ctx) { const n = Number(ctx); if (n > 0) hw.contextSize = n; }

    const ngl = readFlag('-ngl') || readFlag('--gpu-layers');
    if (ngl) { const n = Number(ngl); if (!isNaN(n)) { hw.gpuLayersManual = n; hw.gpuLayers = 'manual'; } }

    const bs = readFlag('-b') || readFlag('--batch-size');
    if (bs) { const n = Number(bs); if (n > 0) hw.batchSize = n; }

    const ubs = readFlag('-ub') || readFlag('--ubatch-size');
    if (ubs) { const n = Number(ubs); if (n > 0) hw.ubatchSize = n; }

    const moe = readFlag('--n-cpu-moe');
    if (moe) { const n = Number(moe); if (!isNaN(n) && n >= 0) hw.nCpuMoe = n; }
}

// ── Step Management ───────────────────────────────────────────────────────────

function showStep(index) {
    wizardState.currentStep = index;
    clearValidationError();

    if (dom.steps) {
        dom.steps.forEach(step => step.classList.remove('active'));
        const stepEl = document.getElementById(`wizard-step-${index}`);
        if (stepEl) stepEl.classList.add('active');
    }

    if (dom.stepBadges) {
        dom.stepBadges.forEach(badge => {
            const s = Number(badge.dataset.step);
            badge.classList.remove('active', 'completed');
            if (s === index) badge.classList.add('active');
            else if (s < index) badge.classList.add('completed');
        });
    }

    if (dom.stepLabel) dom.stepLabel.textContent = STEP_LABELS[index] || '';

    if (dom.backBtn) dom.backBtn.style.display = index === 0 ? 'none' : '';
    if (dom.nextBtn) dom.nextBtn.style.display = index === STEP_LABELS.length - 1 ? 'none' : '';

    // Step-specific side effects
    if (index === 2) {
        // Entering hardware step — kick off a VRAM estimate immediately
        scheduleVramEstimate();
    }
    if (index === 3) {
        // Entering summary — ensure VRAM estimate is fresh then render
        estimateVram().then(() => renderSummary());
    }
}

// ── Profile Persistence / Visibility ─────────────────────────────────────────

function persistProfile() {
    try { localStorage.setItem('spawn_wizard_profile', wizardState.profile); } catch { /* ignore */ }
}

function restoreProfile() {
    try {
        const stored = localStorage.getItem('spawn_wizard_profile');
        if (stored && ['quick', 'balanced', 'advanced'].includes(stored)) {
            wizardState.profile = stored;
        }
    } catch { /* ignore */ }
    if (dom.profileCards) {
        dom.profileCards.forEach(card => {
            card.classList.toggle('selected', card.dataset.profile === wizardState.profile);
        });
    }
}

function applyProfileVisibility() {
    const isAdvanced = wizardState.profile === 'advanced';
    const isQuick = wizardState.profile === 'quick';

    if (dom.advancedFields) dom.advancedFields.classList.toggle('visible', isAdvanced);

    if (isQuick) {
        if (dom.contextSizeInput) dom.contextSizeInput.disabled = true;
        if (dom.batchSizeInput) dom.batchSizeInput.disabled = true;
        if (dom.gpuLayersSelect) { dom.gpuLayersSelect.value = 'auto'; dom.gpuLayersSelect.disabled = true; }
    } else {
        if (dom.contextSizeInput) dom.contextSizeInput.disabled = false;
        if (dom.batchSizeInput) dom.batchSizeInput.disabled = false;
        if (dom.gpuLayersSelect) dom.gpuLayersSelect.disabled = false;
    }
}

// ── Model Source Visibility ───────────────────────────────────────────────────

function updateModelInputVisibility() {
    const source = wizardState.model.source;
    if (dom.modelInputLocal) dom.modelInputLocal.classList.toggle('visible', source === 'local');
    if (dom.modelInputHf) dom.modelInputHf.classList.toggle('visible', source === 'hf');
    if (dom.modelInputImport) dom.modelInputImport.classList.toggle('visible', source === 'import');
}

function updateSelectedModelDisplay() {
    if (!dom.selectedModel) return;
    const { source, path, hfRepo, hfFile } = wizardState.model;

    let displayName = '';
    let meta = '';

    if (source === 'hf' && hfRepo) {
        displayName = hfFile ? hfFile.split('/').pop() : hfRepo;
        meta = hfFile ? `${hfRepo} · HuggingFace` : 'HuggingFace repo';
    } else if (path) {
        // Show only the filename, not the full path
        displayName = path.split(/[\\/]/).pop() || path;
        meta = path;
    }

    if (!displayName) {
        dom.selectedModel.classList.remove('visible');
        return;
    }

    dom.selectedModel.classList.add('visible');
    if (dom.selectedModelName) dom.selectedModelName.textContent = displayName;
    if (dom.selectedModelMeta) dom.selectedModelMeta.textContent = meta;
}

// ── HF Search ─────────────────────────────────────────────────────────────────

async function hfSearchModels(query) {
    if (!query || query.length < 2) return [];
    try {
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };
        const resp = await fetch('/api/hf/search', {
            method: 'POST',
            headers,
            body: JSON.stringify({ query, limit: 20 }),
        });
        if (!resp.ok) {
            if (resp.status === 429) showToast('Too many search requests. Please wait.', 'warning');
            else if (resp.status === 401 || resp.status === 403) showToast('Authentication required to search models.', 'error');
            return [];
        }
        const data = await resp.json();
        return (data.models || []).filter(Boolean);
    } catch {
        return [];
    }
}

async function hfListFiles(repoId) {
    if (!repoId) return [];
    try {
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };
        const resp = await fetch('/api/hf/files', {
            method: 'POST',
            headers,
            body: JSON.stringify({ repo_id: repoId }),
        });
        if (!resp.ok) return [];
        const data = await resp.json();
        return (data.files || []).filter(Boolean);
    } catch {
        return [];
    }
}

async function hfStartDownload(repoId, filePath) {
    try {
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };
        const resp = await fetch('/api/hf/download', {
            method: 'POST',
            headers,
            body: JSON.stringify({ repo_id: repoId, file_path: filePath, resume: true }),
        });
        if (!resp.ok) {
            const text = await resp.text().catch(() => 'Download failed');
            throw new Error(text || 'Download failed');
        }
        const data = await resp.json();
        return data.download_id || null;
    } catch (err) {
        showToast('HF download failed: ' + (err.message || String(err)), 'error');
        return null;
    }
}

async function pollDownloadStatus(downloadId) {
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const resp = await fetch(`/api/models/download/${encodeURIComponent(downloadId)}/status`, { headers });
        if (!resp.ok) return null;
        const data = await resp.json();
        return data;
    } catch {
        return null;
    }
}

// ── HF File Listing ───────────────────────────────────────────────────────────

async function fetchHfFiles(repo) {
    if (!dom.hfFileList) return;

    dom.hfFileList.innerHTML = '<div class="hf-file-loading">Loading files…</div>';
    dom.hfFileList.classList.add('visible');

    try {
        const files = await hfListFiles(repo);
        dom.hfFileList.innerHTML = '';

        if (!Array.isArray(files) || files.length === 0) {
            dom.hfFileList.innerHTML = '<div class="hf-file-empty">No GGUF files found in this repo.</div>';
            return;
        }

        const vramGb = wizardState.vram.available ? wizardState.vram.available / (1024 ** 3) : 0;

        files.forEach(file => {
            const filename = file.path || file.name || '';
            if (!filename) return;

            const item = document.createElement('div');
            item.className = 'hf-file-item';
            item.setAttribute('tabindex', '0');
            item.setAttribute('role', 'button');
            item.dataset.filename = filename;
            item.dataset.size = file.size || '';

            const nameSpan = document.createElement('span');
            nameSpan.className = 'hf-file-name';
            nameSpan.textContent = filename.split('/').pop() || filename;

            const metaSpan = document.createElement('span');
            metaSpan.className = 'hf-file-size';
            const parts = [];
            if (file.size) parts.push(formatBytes(file.size));
            if (file.label) parts.push(file.label);
            if (vramGb > 0 && file.label && file.label === getRecommendedQuant(vramGb)) {
                parts.push('✓ Recommended');
            }
            metaSpan.textContent = parts.join(' · ');

            item.appendChild(nameSpan);
            item.appendChild(metaSpan);

            const selectFile = () => {
                dom.hfFileList.querySelectorAll('.hf-file-item.selected')
                    .forEach(el => el.classList.remove('selected'));
                item.classList.add('selected');
                wizardState.model.hfFile = filename;
                wizardState.model.path = ''; // HF path is not a local path
                updateSelectedModelDisplay();
                scheduleVramEstimate();
                clearValidationError();
            };

            item.addEventListener('click', selectFile);
            item.addEventListener('keydown', (e) => {
                if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectFile(); }
            });

            dom.hfFileList.appendChild(item);
        });
    } catch {
        dom.hfFileList.innerHTML = '<div class="hf-file-empty">Failed to load files. Check the repo ID and your HF token.</div>';
    }
}

function getRecommendedQuant(vramGb) {
    if (vramGb < 8) return 'Q4_K_M';
    if (vramGb <= 16) return 'Q5_K_M';
    return 'Q8_0';
}

// ── Third-Party Model Import ──────────────────────────────────────────────────

async function loadThirdPartyModels() {
    if (!dom.importPathInput) return;
    try {
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };
        const resp = await fetch('/api/third-party-models', {
            method: 'POST',
            headers,
            body: JSON.stringify({ include_subdirs: true }),
        });
        if (!resp.ok) return;
        const data = await resp.json();
        const models = (data.models || []).filter(Boolean);
        if (models.length === 0) return;

        // Populate a datalist for the import path input
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
            opt.textContent = m.path.split(/[\\/]/).pop() || m.path;
            dl.appendChild(opt);
        });
    } catch { /* ignore */ }
}

// ── Model Introspection ───────────────────────────────────────────────────────

async function introspectModel(modelPath) {
    if (!modelPath) return null;
    try {
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };
        const resp = await fetch('/api/model/introspect', {
            method: 'POST',
            headers,
            body: JSON.stringify({ model_path: modelPath }),
        });
        if (!resp.ok) return null;
        const data = await resp.json();
        return data.metadata || null;
    } catch {
        return null;
    }
}

function formatBytes(bytes) {
    if (!bytes) return '';
    const b = Number(bytes);
    if (!b || !isFinite(b)) return '';
    if (b >= 1_073_741_824) return (b / 1_073_741_824).toFixed(1) + ' GB';
    if (b >= 1_048_576) return (b / 1_048_576).toFixed(1) + ' MB';
    if (b >= 1024) return (b / 1024).toFixed(0) + ' KB';
    return b + ' B';
}

// ── Hardware Change Handler ───────────────────────────────────────────────────

let vramDebounce = null;

function onHardwareChange() {
    readHardwareState();
    scheduleVramEstimate();
}

function readHardwareState() {
    const h = wizardState.hardware;
    h.gpuLayers = dom.gpuLayersSelect?.value ?? 'auto';
    if (dom.gpuLayersManualInput) {
        const v = dom.gpuLayersManualInput.value;
        h.gpuLayersManual = v !== '' ? Number(v) : null;
    }
    if (dom.contextSizeInput) {
        const v = Number(dom.contextSizeInput.value);
        h.contextSize = v > 0 ? v : 8192;
    }
    if (dom.batchSizeInput) {
        const v = Number(dom.batchSizeInput.value);
        h.batchSize = v > 0 ? v : 2048;
    }
    if (dom.ubatchSizeInput) {
        const v = Number(dom.ubatchSizeInput.value);
        h.ubatchSize = v > 0 ? v : 512;
    }
    if (dom.parallelSlotsInput) {
        const v = Number(dom.parallelSlotsInput.value);
        h.parallelSlots = v > 0 ? v : 1;
    }
    if (dom.cacheTypeKSelect) h.cacheTypeK = dom.cacheTypeKSelect.value || '';
    if (dom.cacheTypeVSelect) h.cacheTypeV = dom.cacheTypeVSelect.value || '';
    if (dom.nCpuMoeInput) {
        const v = dom.nCpuMoeInput.value;
        h.nCpuMoe = v !== '' ? Number(v) : null;
    }
    if (dom.tensorSplitInput) h.tensorSplit = dom.tensorSplitInput.value.trim() || '';
}

function scheduleVramEstimate() {
    if (vramDebounce) clearTimeout(vramDebounce);
    vramDebounce = setTimeout(estimateVram, 400);
}

async function estimateVram() {
    const h = wizardState.hardware;
    const m = wizardState.model;

    // For local/import models, pass the path so the backend can stat the file.
    // For HF models, we don't have a local path yet; the estimate will be skipped gracefully.
    const modelPath = (m.source !== 'hf') ? m.path : '';
    if (!modelPath) {
        if (dom.vramEstimateText) dom.vramEstimateText.textContent = '—';
        setVramPill(null);
        return;
    }

    const payload = {
        model: modelPath,
        context_length: h.contextSize,
        batch_size: h.batchSize,
        ubatch_size: h.ubatchSize,
        kv_quant: h.cacheTypeK || 'q8_0',
        n_cpu_moe: h.nCpuMoe ?? undefined,
        speculative_decoding: !!(dom.specTypeSelect?.value),
    };

    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const resp = await fetch('/api/vram/estimate', {
            method: 'POST',
            headers: { ...headers, 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
        });

        if (!resp.ok) {
            if (dom.vramEstimateText) dom.vramEstimateText.textContent = 'Unable to estimate';
            setVramPill(null);
            return;
        }

        const data = await resp.json();
        if (!data.ok) {
            if (dom.vramEstimateText) dom.vramEstimateText.textContent = 'Unable to estimate';
            setVramPill(null);
            return;
        }

        const estimated = data.estimated_vram_bytes ?? null;
        const available = data.available_vram_bytes ?? null;
        // Backend returns snake_case enum: "fit" | "tight" | "risk" | "wont_fit"
        // Normalize to frontend keys: "fit" | "tight" | "risk" | "wont-fit"
        const status = (data.recommendation || '').toLowerCase().replace('_', '-');

        wizardState.vram = { estimated, available, status };

        if (dom.vramEstimateText) {
            if (estimated != null && available != null && available > 0) {
                dom.vramEstimateText.textContent =
                    `${formatBytes(estimated)} / ${formatBytes(available)}`;
            } else if (estimated != null) {
                dom.vramEstimateText.textContent = formatBytes(estimated);
            } else {
                dom.vramEstimateText.textContent = '—';
            }
        }

        setVramPill(status);
    } catch {
        if (dom.vramEstimateText) dom.vramEstimateText.textContent = '—';
        setVramPill(null);
    }
}

function setVramPill(status) {
    if (!dom.vramPill) return;
    const map = {
        'fit':      { cls: 'vram-pill-fit',      label: 'Fits' },
        'tight':    { cls: 'vram-pill-tight',     label: 'Tight' },
        'risk':     { cls: 'vram-pill-risk',      label: 'At risk' },
        'wont-fit': { cls: 'vram-pill-wont-fit',  label: "Won't fit" },
    };
    const cfg = status ? map[status.toLowerCase()] : null;
    dom.vramPill.className = cfg ? cfg.cls : '';
    dom.vramPill.textContent = cfg ? cfg.label : '';
}

// ── Summary Rendering (Step 4) ────────────────────────────────────────────────

function renderSummary() {
    if (!dom.summaryList) return;
    dom.summaryList.innerHTML = '';

    const m = wizardState.model;
    const h = wizardState.hardware;
    const v = wizardState.vram;

    const modelDisplay = m.source === 'hf'
        ? (m.hfFile ? `${m.hfRepo} / ${m.hfFile.split('/').pop()}` : m.hfRepo || '(none)')
        : (m.path ? m.path.split(/[\\/]/).pop() || m.path : '(none)');

    const gpuLayersDisplay = h.gpuLayers === 'manual'
        ? String(h.gpuLayersManual ?? '—')
        : h.gpuLayers;

    const vramDisplay = v.estimated != null && v.available != null && v.available > 0
        ? `${formatBytes(v.estimated)} / ${formatBytes(v.available)}`
        : (v.estimated != null ? formatBytes(v.estimated) : '—');

    const rows = [
        { label: 'Profile', value: wizardState.profile },
        { label: 'Model', value: modelDisplay },
        { label: 'Source', value: m.source === 'hf' ? 'HuggingFace' : m.source === 'import' ? 'Third-party import' : 'Local file' },
        { label: 'Context size', value: String(h.contextSize) },
        { label: 'GPU layers', value: gpuLayersDisplay },
        { label: 'Batch size', value: `${h.batchSize} / ${h.ubatchSize}` },
        { label: 'VRAM estimate', value: vramDisplay },
    ];

    if (h.nCpuMoe != null) rows.push({ label: 'MoE CPU experts', value: String(h.nCpuMoe) });
    if (h.tensorSplit) rows.push({ label: 'Tensor split', value: h.tensorSplit });

    const specType = dom.specTypeSelect?.value || '';
    if (specType) {
        const specLabels = { 'ngram-mod': 'N-gram (fast)', 'draft-model': 'Draft model' };
        let specVal = specLabels[specType] || specType;
        if (specType === 'draft-model' && dom.draftModelInput?.value) {
            specVal += ` (${dom.draftModelInput.value.split(/[\\/]/).pop()})`;
        }
        rows.push({ label: 'Speculative decoding', value: specVal });
    }
    if (dom.kvUnifiedCheck?.checked) rows.push({ label: 'KV cache unified', value: 'Yes' });
    if (dom.ignoreEosCheck?.checked) rows.push({ label: 'Ignore EOS', value: 'Yes' });

    rows.forEach(r => {
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
        dom.summaryList.appendChild(row);
    });

    // Warnings
    if (dom.summaryWarnings) {
        const warnings = [];
        if (!modelDisplay || modelDisplay === '(none)') warnings.push('No model selected. Go back and choose a model.');
        if (v.status === 'tight') warnings.push('VRAM is tight — consider reducing context size or using a smaller quantization.');
        if (v.status === 'risk') warnings.push('VRAM is at risk — reducing context size or enabling KV quantization is recommended.');
        if (v.status === 'wont-fit') warnings.push("Configuration won't fit in VRAM. Use a smaller model or reduce context.");

        if (warnings.length > 0) {
            dom.summaryWarnings.style.display = '';
            dom.summaryWarnings.innerHTML = '';
            warnings.forEach(w => {
                const p = document.createElement('div');
                p.textContent = w;
                dom.summaryWarnings.appendChild(p);
            });
        } else {
            dom.summaryWarnings.style.display = 'none';
        }
    }

    // Show health check button only if a server endpoint is reachable
    if (dom.healthCheckBtn) {
        dom.healthCheckBtn.style.display = '';
    }
}

// ── Save as Preset (Step 4) ───────────────────────────────────────────────────

async function saveAsPreset() {
    const preset = buildPresetPayload();

    // Ask for a name
    const name = window.prompt('Preset name:', 'Spawn Wizard Preset');
    if (!name) return;
    preset.name = name;

    try {
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };
        const resp = await fetch('/api/presets', {
            method: 'POST',
            headers,
            body: JSON.stringify(preset),
        });
        if (!resp.ok) {
            const err = await resp.text().catch(() => 'Unknown error');
            showToast('Save preset failed: ' + err, 'error');
            return;
        }
        showToast('Preset saved as "' + name + '"', 'success');
    } catch (err) {
        showToast('Save preset failed: ' + (err.message || String(err)), 'error');
    }
}

function buildPresetPayload() {
    const h = wizardState.hardware;
    const m = wizardState.model;

    const gpuLayers = h.gpuLayers === 'manual'
        ? (h.gpuLayersManual ?? -1)
        : (h.gpuLayers === 'all' ? -1 : null); // null = auto (let llama-server decide)

    return {
        name: 'Spawn Wizard Preset',
        // Only set model_path for local/import sources; HF uses hf_repo
        model_path: (m.source !== 'hf') ? (m.path || '') : '',
        hf_repo: m.source === 'hf' ? (m.hfRepo || null) : null,
        gpu_layers: gpuLayers,
        context_size: h.contextSize,
        batch_size: h.batchSize,
        ubatch_size: h.ubatchSize,
        parallel_slots: h.parallelSlots,
        ctk: h.cacheTypeK || '',
        ctv: h.cacheTypeV || '',
        n_cpu_moe: h.nCpuMoe,
        tensor_split: h.tensorSplit || '',
        spec_type: dom.specTypeSelect?.value || '',
        draft_model: (dom.draftModelInput?.value || '').trim() || '',
        kv_unified: dom.kvUnifiedCheck?.checked || false,
        ignore_eos: dom.ignoreEosCheck?.checked || false,
    };
}

// ── Health Check (Step 4) ─────────────────────────────────────────────────────

async function runHealthCheck() {
    if (!dom.healthCheckBtn) return;
    const btn = dom.healthCheckBtn;
    const origText = btn.textContent;
    btn.disabled = true;
    btn.textContent = 'Running…';

    try {
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };
        const resp = await fetch('/api/benchmark', { method: 'POST', headers });

        if (!resp.ok) {
            const err = await resp.text().catch(() => 'Unknown error');
            showToast('Health check failed: ' + err, 'error');
        } else {
            const data = await resp.json();
            const verdict = (data.verdict || '').toLowerCase();
            const details = [
                data.prompt_tokens_per_second ? `Prompt: ${data.prompt_tokens_per_second.toFixed(1)} t/s` : '',
                data.gen_tokens_per_second ? `Gen: ${data.gen_tokens_per_second.toFixed(1)} t/s` : '',
                data.time_to_first_token_ms ? `TTFT: ${data.time_to_first_token_ms.toFixed(0)} ms` : '',
            ].filter(Boolean).join(' · ');

            const level = verdict === 'good' ? 'success' : verdict === 'poor' ? 'error' : 'warning';
            showToast(
                `Health: ${verdict || 'complete'}`,
                level,
                details || (data.hints?.[0] ?? '')
            );
        }
    } catch (err) {
        showToast('Health check failed: ' + (err.message || String(err)), 'error');
    } finally {
        btn.disabled = false;
        btn.textContent = origText;
    }
}

// ── Spawn Server (Step 5) ─────────────────────────────────────────────────────

async function spawnServer() {
    if (wizardState.spawn.inFlight) return;
    wizardState.spawn.inFlight = true;
    wizardState.spawn.error = '';

    if (!dom.spawnServerBtn) return;
    dom.spawnServerBtn.disabled = true;

    setStatusText('Preparing configuration…');
    setProgress(10);
    clearStatusMessages();

    try {
        const payload = buildSpawnPayload();

        setStatusText('Starting llama-server…');
        setProgress(30);

        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };

        // Try sessions/spawn first; fall back to /api/start
        let resp;
        try {
            resp = await fetch('/api/sessions/spawn', {
                method: 'POST',
                headers,
                body: JSON.stringify(payload),
            });
        } catch {
            resp = await fetch('/api/start', {
                method: 'POST',
                headers,
                body: JSON.stringify(payload),
            });
        }

        setProgress(60);

        if (!resp.ok) {
            const errText = await resp.text().catch(() => 'Unknown error');
            throw new Error(errText || `Server responded ${resp.status}`);
        }

        setProgress(90);
        setStatusText('Server is starting up…');
        await new Promise(r => setTimeout(r, 1500));
        setProgress(100);
        setStatusText('Server started successfully.');
        showSuccessText('Server is running.');
        showToast('Server started', 'success', 'Llama-server is now running.');
        setTimeout(() => closeSpawnWizard(), 1200);
    } catch (err) {
        const msg = (err.message || String(err)).split('\n')[0].trim();
        showErrorText(msg || 'Failed to start server.');
        setStatusText('Spawn failed.');
        wizardState.spawn.error = msg;
        showToast('Spawn failed', 'error', msg || 'Check logs for details.');
    } finally {
        wizardState.spawn.inFlight = false;
        if (dom.spawnServerBtn) dom.spawnServerBtn.disabled = false;
    }
}

function buildSpawnPayload() {
    const h = wizardState.hardware;
    const m = wizardState.model;

    const gpuLayers = h.gpuLayers === 'manual'
        ? (h.gpuLayersManual ?? -1)
        : (h.gpuLayers === 'all' ? -1 : null);

    return {
        // Local/import: set model_path. HF: set hf_repo + hf_file, leave model_path empty.
        model_path: (m.source !== 'hf') ? (m.path || null) : null,
        hf_repo: m.source === 'hf' ? (m.hfRepo || null) : null,
        hf_file: m.source === 'hf' ? (m.hfFile || null) : null,
        gpu_layers: gpuLayers,
        context_size: h.contextSize,
        batch_size: h.batchSize,
        ubatch_size: h.ubatchSize,
        parallel_slots: h.parallelSlots,
        ctk: h.cacheTypeK || null,
        ctv: h.cacheTypeV || null,
        n_cpu_moe: h.nCpuMoe,
        tensor_split: h.tensorSplit || null,
        spec_type: dom.specTypeSelect?.value || '',
        draft_model: (dom.draftModelInput?.value || '').trim() || null,
        kv_unified: dom.kvUnifiedCheck?.checked || null,
        ignore_eos: dom.ignoreEosCheck?.checked || null,
        profile: wizardState.profile,
    };
}

// ── Spawn Status Helpers ──────────────────────────────────────────────────────

function setStatusText(text) {
    if (dom.statusText) dom.statusText.textContent = text;
}

function setProgress(pct) {
    if (dom.progressFill) dom.progressFill.style.width = Math.min(100, Math.max(0, pct)) + '%';
}

function showErrorText(text) {
    if (dom.errorText) dom.errorText.textContent = text || '';
}

function showSuccessText(text) {
    if (dom.successText) dom.successText.textContent = text || '';
}

function clearStatusMessages() {
    if (dom.errorText) dom.errorText.textContent = '';
    if (dom.successText) dom.successText.textContent = '';
}

function resetSpawnStatus() {
    wizardState.spawn = { inFlight: false, error: '' };
    setStatusText('Ready to spawn.');
    setProgress(0);
    clearStatusMessages();
}
