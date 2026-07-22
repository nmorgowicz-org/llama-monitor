// ── Models ────────────────────────────────────────────────────────────────────
// Models modal: open, close, load, refresh, delete, and HF download tab.

import { sessionState } from '../core/app-state.js';
import { escapeHtml } from '../core/format.js';
import { getPlatformInfo } from '../core/platform-info.js';
import { showToast, showToastWithActions } from './toast.js';
import Router from './router.js';
import { _showConfirm } from './presets.js';
import { openCardPanel } from './spawn-wizard.js';
import { buildEstimateBody } from './vram-estimate.js';
import {
    hfSearch,
    hfListFiles,
    hfStartCompanionDownload,
    hfStartDownload,
    hfPollDownload,
    hfCancelDownload,
    hfShowDownloadPanel,
    hfHideDownloadPanel,
    hfRenderDiscoverPills,
    hfLoadQuickPicks,
    getRecommendedMmproj,
    hfCreateScopeSelector,
    hfCreateSortSelector,
    HF_SCOPE,
    HF_SORT,
} from './hf-browse.js';

const PREFS_KEY = 'llama-monitor-models-prefs';

const ICON_LIST_VIEW = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="11" height="11"><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>';
const ICON_CARDS_VIEW = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="11" height="11"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></svg>';

const KNOWN_TAGS = ['coding', 'roleplay', 'general', 'art', 'fast', 'default'];

const INVENTORY_BADGES = {
    format: {
        gguf: ['GGUF', 'Format: GGUF'],
        mlx: ['MLX', 'Format: MLX'],
        transformers: ['Transformers', 'Format: Transformers / safetensors'],
        unknown: ['Unknown format', 'Format: Unknown'],
    },
    source: {
        local: ['Local', 'Source: Local library'],
        hugging_face: ['Hugging Face', 'Source: Hugging Face'],
        official_conversion: ['Official conversion', 'Source: Official MLX conversion'],
        recovered_gguf: ['Recovered from GGUF', 'Source: Experimental GGUF recovery'],
        requantized_mlx: ['Re-quantized MLX', 'Source: Experimental MLX re-quantization'],
        legacy: ['Legacy location', 'Source: Legacy model-library location'],
        unknown: ['Unknown source', 'Source: Unknown'],
    },
    lifecycle: {
        ready: ['Ready', 'Lifecycle: Ready'],
        incomplete: ['Incomplete', 'Lifecycle: Incomplete'],
        converting: ['Converting', 'Lifecycle: Converting'],
        invalid: ['Invalid', 'Lifecycle: Invalid'],
        unknown: ['Status unknown', 'Lifecycle: Unknown'],
    },
    compatibility: {
        verified: ['Verified', 'Compatibility: Verified'],
        experimental: ['Experimental', 'Compatibility: Experimental and not launchable'],
        provisional: ['Provisional', 'Compatibility: Provisional'],
        unsupported: ['Unsupported', 'Compatibility: Unsupported'],
        unknown: ['Unknown compatibility', 'Compatibility: Unknown'],
    },
};

const BACKEND_LABELS = {
    llama_cpp: 'llama.cpp',
    rapid_mlx: 'Rapid-MLX',
};

let modelCardSequence = 0;
let rapidMlxLocalAvailable = false;
let rapidMlxLocalRequirement = 'Rapid-MLX local execution requires macOS on Apple Silicon';

let initialized = false;
let inventoryCache = null;

// State for the HF download tab
let hfState = {
    selectedRepoId: null,
    selectedFile: null,
    currentDownloadIds: new Set(),
    initialized: false,
    // Wizard-like state
    paramB: 0,
    modelBytes: 0,
    nCtxTrain: 0,
    mmprojFiles: [],
    mmprojPath: '',
    mmprojRepoId: '',
    mmprojBytes: 0,
    // Active filters (mirrors Quick Start behavior)
    activeAuthor: null,          // e.g. "bartowski"
    activeDiscoverQuery: null,   // e.g. "qwen3" from a discover pill
    // Discovery scope + sort (Phase 8B1) — additive toggles: MLX and GGUF can both be active
    discoveryScopeMlx: false,
    discoveryScopeGguf: true, // default: GGUF always active; macOS will also activate MLX below
    discoverySort: HF_SORT.DOWNLOADS,
};

// Cached hardware
let cachedVram = 0;
// True when the GPU pool is unified memory (Apple Silicon / Metal) — selects the Metal
// overhead model in the backend estimator instead of the discrete-GPU one.
let cachedUnified = false;
let cachedRamTotal = 0;

// Library preferences
let prefs = loadPrefs();

function loadPrefs() {
    const def = {
        viewMode: 'cards',
        search: '',
        sort: 'name-asc',
        showMmproj: true,
        showMain: true,
        showSplit: true,
        showDraftModels: false,
        sizeMaxGb: 0,
        quantFilters: {},
        tagFilter: '',
        familyFilter: '',
        sizeClassFilter: '',
    };
    try {
        const raw = localStorage.getItem(PREFS_KEY);
        if (raw) {
            const saved = JSON.parse(raw);
            return { ...def, ...saved };
        }
    } catch {
        // ignore
    }
    return def;
}

function savePrefs() {
    try {
        localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
    } catch {
        // ignore
    }
}

export function openModelsModal() {
    document.getElementById('models-modal')?.classList.add('open');
    loadModels({ refresh: true });
}

function closeModelsModal() {
    document.getElementById('models-modal')?.classList.remove('open');
}

export { closeModelsModal };

export function invalidateModelInventory() {
    inventoryCache = null;
}

async function loadModels({ refresh = false } = {}) {
    const grid = document.getElementById('models-list');
    const summary = document.getElementById('models-summary');
    const tabCount = document.getElementById('models-tab-count');
    const dirLabel = document.getElementById('models-dir-label');
    if (!grid) return;

    const needsFetch = refresh || !inventoryCache;
    if (needsFetch) {
        if (summary) summary.textContent = 'Loading...';
        grid.innerHTML = '<div class="mm-loading">Scanning...</div>';
    }

    try {
        if (needsFetch) {
            const dirResp = await fetch('/api/hf/download-dir', {
                headers: window.authHeaders ? window.authHeaders() : {},
            });
            if (dirResp.ok && dirLabel) {
                const dirInfo = await dirResp.json();
                if (dirInfo.dir) {
                    if (dirInfo.configured) {
                        dirLabel.textContent = dirInfo.dir;
                    } else {
                        dirLabel.textContent = 'Using default: ' + dirInfo.dir;
                    }
                } else {
                    dirLabel.textContent = 'No models directory configured';
                }
            }
            const platform = await getPlatformInfo();
            rapidMlxLocalAvailable = platform.rapid_mlx_local_available === true;
            rapidMlxLocalRequirement = platform.rapid_mlx_local_requirement
                || rapidMlxLocalRequirement;
            const resp = await fetch('/api/models', {
                headers: window.authHeaders ? window.authHeaders() : {},
            });
            if (!resp.ok) throw new Error(`Model inventory failed (${resp.status})`);
            inventoryCache = await resp.json();
        }
        const models = inventoryCache || [];

        const count = models.length;
        if (tabCount) tabCount.textContent = count ? String(count) : '';

        // Build or refresh toolbar filter chips based on models list
        rebuildLibraryFilters(models);

        // Apply client-side filter/sort/search
        const filtered = applyFilters(models);
        const sorted = applySort(filtered);
        const result = applySearch(sorted);

        if (summary) {
            if (result.length === count) {
                summary.textContent = count
                    ? count + ' model' + (count === 1 ? '' : 's') + ' found'
                    : 'No models found';
            } else {
                summary.textContent = result.length + ' of ' + count + ' models shown';
            }
        }

        if (!count) {
            grid.innerHTML = '<div class="mm-empty">No models found in this directory. You can download one from the Download tab.</div>';
            grid.className = 'mm-model-grid';
            return;
        }

        if (!result.length) {
            grid.innerHTML = '<div class="mm-empty">No models match the current filters or search.</div>';
            grid.className = 'mm-model-grid';
            return;
        }

        grid.className = prefs.viewMode === 'list'
            ? 'mm-model-grid mm-model-grid--list'
            : 'mm-model-grid';

        grid.innerHTML = '';
        result.forEach(m => {
            grid.appendChild(buildModelCard(m));
        });
    } catch (err) {
        if (summary) summary.textContent = 'Failed to load models';
        const errDiv = document.createElement('div');
        errDiv.className = 'mm-empty';
        errDiv.textContent = 'Error: ' + err.message;
        grid.innerHTML = '';
        grid.appendChild(errDiv);
    }
}

function isMmproj(m) {
    if (m.companion_kind != null) return m.companion_kind === 'mmproj';
    const f = (m.filename || '').toLowerCase();
    return f.includes('mmproj') || f.includes('.mmproj.') || f.includes('-mmproj-');
}

function isDraftModel(m) {
    if (m.companion_kind != null) return m.companion_kind === 'draft';
    // Trust the backend's is_draft_assistant (size-guarded).
    // Fallback: quick client-side check using same logic as backend.
    if (m.is_draft_assistant !== undefined) return m.is_draft_assistant;
    const f = (m.filename || '').toLowerCase();
    const hasKeyword =
        f.includes('mtp-draft') ||
        f.includes('mtp_small') ||
        f.includes('mtp-heads') ||
        f.startsWith('mtp-') ||
        f.endsWith('-mtp.gguf') ||
        f.includes('mtp') ||
        f.includes('draft') ||
        f.includes('assistant') ||
        f.includes('draft-model');
    const size = m.size_bytes || 0;
    // Same tiered thresholds as backend:
    const isUnambiguous =
        f.includes('mtp-draft') ||
        f.includes('mtp_small') ||
        f.includes('mtp-heads') ||
        (f.startsWith('mtp-') && size <= 5_000_000_000);
    if (isUnambiguous && size <= 5_000_000_000) return true;
    if (hasKeyword && size > 0 && size <= 3_000_000_000) return true;
    return false;
}

function applyFilters(models) {
    return models.filter(m => {
        // mmproj vs main
        const mmproj = isMmproj(m);
        if (mmproj && !prefs.showMmproj) return false;
        if (!mmproj && !prefs.showMain) return false;

        // draft model vs main
        const draft = isDraftModel(m);

        if (draft && !prefs.showDraftModels) return false;
        // split
        if (m.is_split && !prefs.showSplit) return false;

        // size filter
        if (prefs.sizeMaxGb > 0) {
            if ((m.size_bytes || 0) > prefs.sizeMaxGb * 1024 ** 3) return false;
        }

        // quant filter
        const qt = (m.quant_type || '').toUpperCase();
        if (qt && Object.keys(prefs.quantFilters).length > 0) {
            if (!prefs.quantFilters[qt]) return false;
        }

        // tag filter
        if (prefs.tagFilter) {
            const tags = Array.isArray(m.tags) ? m.tags : [];
            if (!tags.includes(prefs.tagFilter)) return false;
        }

        // family filter (from backend classification)
        if (prefs.familyFilter) {
            const c = m.classification || {};
            if ((c.family || '') !== prefs.familyFilter) return false;
        }

        // size-class filter (from backend classification)
        if (prefs.sizeClassFilter) {
            const c = m.classification || {};
            if ((c.size_class || '') !== prefs.sizeClassFilter) return false;
        }

        return true;
    });
}

function applySort(models) {
    const mode = prefs.sort || 'name-asc';
    return [...models].sort((a, b) => {
        switch (mode) {
            case 'name-asc':
                return (a.model_name || a.filename || '').localeCompare(b.model_name || b.filename || '');
            case 'name-desc':
                return (b.model_name || b.filename || '').localeCompare(a.model_name || a.filename || '');
            case 'size-asc':
                return (a.size_bytes || 0) - (b.size_bytes || 0);
            case 'size-desc':
                return (b.size_bytes || 0) - (a.size_bytes || 0);
            case 'vram-asc':
                return (a.vram_est_gb || 0) - (b.vram_est_gb || 0);
            case 'vram-desc':
                return (b.vram_est_gb || 0) - (a.vram_est_gb || 0);
            case 'date-asc':
                return (a.last_modified || 0) - (b.last_modified || 0);
            case 'date-desc':
                return (b.last_modified || 0) - (a.last_modified || 0);
            default:
                return (a.model_name || a.filename || '').localeCompare(b.model_name || a.filename || '');
        }
    });
}

function applySearch(models) {
    const q = (prefs.search || '').trim().toLowerCase();
    if (!q) return models;
    return models.filter(m => {
        const haystack = [
            m.model_name,
            m.filename,
            m.path,
            m.quant_type,
        ]
            .filter(Boolean)
            .join(' ')
            .toLowerCase();
        return haystack.includes(q);
    });
}

function buildModelCard(m) {
    const name = m.model_name || m.filename || 'Unnamed model';
    const quant = m.quant_type || 'unknown';
    const size = m.size_display || '';
    const vramEst = m.vram_est_gb != null
        ? (typeof m.vram_estimate_display === 'string'
            ? m.vram_estimate_display
            : (m.vram_est_gb % 1 === 0 ? m.vram_est_gb + ' GB' : m.vram_est_gb.toFixed(1) + ' GB'))
        : '';
    const vramPct = m.vram_percent != null ? Math.min(100, m.vram_percent) : null;
    const isSplit = m.is_split;
    const inventory = normalizeInventory(m);
    const mmproj = inventory.companionKind === 'mmproj';
    const companion = inventory.companionKind !== null;
    const tags = Array.isArray(m.tags) ? m.tags : [];
    const relatedPresets = mmproj ? [] : findPresetsForModel(m);

    const card = document.createElement('article');
    card.className = 'mm-model-card';
    card.dataset.format = inventory.format;
    card.dataset.source = inventory.source;
    card.dataset.lifecycle = inventory.lifecycle;
    card.dataset.compatibility = inventory.compatibility;
    if (mmproj) card.classList.add('mm-model-card--mmproj');
    if (companion) card.classList.add('mm-model-card--companion');

    // Top row: name + quant badge
    const top = document.createElement('div');
    top.className = 'mm-card-top';

    const nameEl = document.createElement('div');
    nameEl.className = 'mm-card-name';
    modelCardSequence += 1;
    nameEl.id = `mm-model-name-${modelCardSequence}`;
    card.setAttribute('aria-labelledby', nameEl.id);
    nameEl.title = m.path || '';
    nameEl.textContent = name;
    top.appendChild(nameEl);

    // Only show quant badge when it's meaningful — skip for mmproj files with no known quant
    if (quant !== 'unknown') {
        const badge = document.createElement('span');
        badge.className = 'mm-quant-badge';
        badge.textContent = quant;
        top.appendChild(badge);
    }

    if (isSplit) {
        const splitBadge = document.createElement('span');
        splitBadge.className = 'mm-quant-badge mm-split-badge';
        splitBadge.textContent = 'split';
        top.appendChild(splitBadge);
    }

    // MTP / draft model badge
    const isDraft = m.is_draft_assistant || (m.classification && m.classification.has_mtp);
    if (isDraft && !mmproj) {
        const mtpBadge = document.createElement('span');
        mtpBadge.className = 'mm-quant-badge mm-mtp-badge';
        mtpBadge.textContent = 'MTP';
        mtpBadge.title = 'Multi-token prediction / draft model';
        top.appendChild(mtpBadge);
    }

    card.appendChild(top);

    card.appendChild(buildInventoryBadgeRail(inventory));

    // Meta row: filename (ellipsis + full tooltip)
    const meta = document.createElement('div');
    meta.className = 'mm-card-meta';
    const metaText = m.filename || '';
    meta.textContent = metaText;
    if (metaText) meta.title = metaText;
    card.appendChild(meta);

    // Stats row: size, tag pills (always include if any)
    if (size || tags.length > 0) {
        const stats = document.createElement('div');
        stats.className = 'mm-card-stats';
        if (size) {
            const sizeEl = document.createElement('span');
            sizeEl.className = 'mm-stat';
            sizeEl.textContent = size;
            stats.appendChild(sizeEl);
        }
        tags.forEach(tag => {
            const pill = document.createElement('span');
            pill.className = 'mm-tag-pill';
            pill.textContent = tag;
            pill.title = 'Click to remove tag';
            pill.addEventListener('click', e => {
                e.stopPropagation();
                removeModelTag(m.path, tag);
            });
            stats.appendChild(pill);
        });
        card.appendChild(stats);
    }

    // VRAM estimate: always show when available, regardless of presets.
    if (vramEst) {
        const vramEl = document.createElement('div');
        vramEl.className = 'mm-card-stats';
        const vramSpan = document.createElement('span');
        vramSpan.className = 'mm-stat mm-stat-vram';
        vramSpan.textContent = 'VRAM ~' + vramEst;
        vramEl.appendChild(vramSpan);
        card.appendChild(vramEl);
    }

    // VRAM bar: always show when available, regardless of presets.
    if (vramPct !== null) {
        const barWrap = document.createElement('div');
        barWrap.className = 'mm-vram-bar';
        const fill = document.createElement('div');
        fill.className = 'mm-vram-fill';
        fill.style.width = vramPct + '%';
        if (vramPct > 90) fill.classList.add('mm-vram-fill--warn');
        barWrap.appendChild(fill);
        card.appendChild(barWrap);
    }

    // Phase 8B2: Lineage info for HF-sourced models (repo/revision/original author/converter)
    const hfRepoId = m.hf_repo_id || m.originRepo || m.repo_id || '';
    const hfRevision = m.hf_revision || m.revision || '';
    const hfOriginalAuthor = m.original_author || m.hf_source_info?.original_author || '';
    const hfConverter = m.converter || m.hf_source_info?.converter || '';
    if (hfRepoId) {
        const lineageEl = document.createElement('div');
        lineageEl.className = 'mm-card-lineage';

        // Repo name
        const repoSpan = document.createElement('span');
        repoSpan.textContent = hfRepoId;
        lineageEl.appendChild(repoSpan);

        // Revision badge
        if (hfRevision && hfRevision !== 'main') {
            const revPrefix = hfRevision.length > 7 ? hfRevision.substring(0, 7) : hfRevision;
            const dot = document.createElement('span');
            dot.textContent = ' · ';
            lineageEl.appendChild(dot);
            const revSpan = document.createElement('span');
            revSpan.className = 'mm-lineage-rev';
            revSpan.textContent = hfRevision === revPrefix ? revPrefix : revPrefix + '…';
            lineageEl.appendChild(revSpan);
        }

        // Original author (distinct from converter)
        if (hfOriginalAuthor && hfOriginalAuthor !== hfConverter) {
            const authDot = document.createElement('span');
            authDot.textContent = ' · ';
            lineageEl.appendChild(authDot);
            const authSpan = document.createElement('span');
            authSpan.className = 'mm-lineage-author';
            authSpan.textContent = 'by ' + hfOriginalAuthor;
            lineageEl.appendChild(authSpan);
        }

        // Converter
        if (hfConverter) {
            const convDot = document.createElement('span');
            convDot.textContent = ' · ';
            lineageEl.appendChild(convDot);
            const convSpan = document.createElement('span');
            convSpan.className = 'mm-lineage-converter';
            const convLabel = m.format === 'mlx' ? 'MLX by' : 'via';
            convSpan.textContent = convLabel + ' ' + hfConverter;
            lineageEl.appendChild(convSpan);
        }

        card.appendChild(lineageEl);
    }

    if (relatedPresets.length) {
        const presetMeta = document.createElement('div');
        presetMeta.className = 'mm-card-meta';
        presetMeta.textContent = buildPresetSummary(relatedPresets);
        presetMeta.title = relatedPresets.map(formatPresetSummaryLine).join('\n');
        card.appendChild(presetMeta);
    }

    // Actions row
    const actions = document.createElement('div');
    actions.className = 'mm-card-actions';

    if (!companion && inventory.launchable) {
        const serverRunning = isLocalServerRunning();

        if (serverRunning) {
            // ── Server is running: primary action is Switch or Quick Load ──────

            if (relatedPresets.length === 1) {
                // One preset → simple Switch button
                const switchBtn = document.createElement('button');
                switchBtn.type = 'button';
                switchBtn.className = 'mm-action-btn mm-action-btn--switch';
                switchBtn.title = `Switch to preset: ${relatedPresets[0].name}`;
                switchBtn.innerHTML = '<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 16V4m0 0L3 8m4-4l4 4"/><path d="M17 8v12m0 0l4-4m-4 4l-4-4"/></svg> Switch';
                switchBtn.addEventListener('click', () => doSwitchToPreset(relatedPresets[0].id));
                actions.appendChild(switchBtn);

            } else if (relatedPresets.length > 1) {
                // Multiple presets → inline select + Switch button
                const switchWrap = document.createElement('div');
                switchWrap.className = 'mm-switch-wrap';

                const switchSelect = document.createElement('select');
                switchSelect.className = 'mm-switch-select';
                switchSelect.title = 'Pick which preset to load';
                relatedPresets.forEach(p => {
                    const opt = document.createElement('option');
                    opt.value = p.id;
                    const ctxLabel = p.context_size ? ' · ' + Math.round(p.context_size / 1024) + 'k' : '';
                    opt.textContent = p.name + ctxLabel;
                    switchSelect.appendChild(opt);
                });

                const switchBtn = document.createElement('button');
                switchBtn.type = 'button';
                switchBtn.className = 'mm-action-btn mm-action-btn--switch';
                switchBtn.title = 'Switch to selected preset';
                switchBtn.innerHTML = '<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 16V4m0 0L3 8m4-4l4 4"/><path d="M17 8v12m0 0l4-4m-4 4l-4-4"/></svg> Switch';
                switchBtn.addEventListener('click', () => doSwitchToPreset(switchSelect.value));

                switchWrap.appendChild(switchSelect);
                switchWrap.appendChild(switchBtn);
                actions.appendChild(switchWrap);

            } else if (inventory.supportedBackends.includes('llama_cpp')) {
                // No preset → Quick Load (inherits current server settings)
                const loadBtn = document.createElement('button');
                loadBtn.type = 'button';
                loadBtn.className = 'mm-action-btn mm-action-btn--switch';
                loadBtn.title = 'Load this model using current server settings (port, GPU layers, etc.)';
                loadBtn.innerHTML = '<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M7 16V4m0 0L3 8m4-4l4 4"/><path d="M17 8v12m0 0l4-4m-4 4l-4-4"/></svg> Quick Load';
                loadBtn.addEventListener('click', () => doQuickLoad(m));
                actions.appendChild(loadBtn);
            } else {
                appendRapidMlxConfigureButton(actions, m);
            }

            // Edit preset(s) — secondary action when running
            if (relatedPresets.length > 0) {
                const editBtn = document.createElement('button');
                editBtn.type = 'button';
                editBtn.className = 'mm-action-btn';
                editBtn.title = relatedPresets.length === 1
                    ? 'Edit the preset for this model'
                    : `Edit one of ${relatedPresets.length} presets`;
                editBtn.textContent = relatedPresets.length === 1 ? 'Edit' : 'Edit…';
                editBtn.addEventListener('click', () => _openEditPreset(relatedPresets));
                actions.appendChild(editBtn);
            }

        } else {
            // ── Server is not running: wizard / new-preset ───────────────────

            if (inventory.supportedBackends.includes('llama_cpp')) {
                const useBtn = document.createElement('button');
                useBtn.type = 'button';
                useBtn.className = 'mm-action-btn';
                useBtn.title = relatedPresets.length ? 'Build a new preset from this model' : 'Open this model in the spawn wizard';
                useBtn.textContent = relatedPresets.length ? 'New Preset' : 'Use in Wizard';
                useBtn.addEventListener('click', () => {
                    closeModelsModal();
                    window.__spawnWizardOpts = {
                        localPath: m.path || '',
                        localModel: m,
                    };
                    Router.navigate('/spawn');
                });
                actions.appendChild(useBtn);
            } else if (!relatedPresets.length) {
                appendRapidMlxConfigureButton(actions, m);
            }

            if (relatedPresets.length) {
                const editBtn = document.createElement('button');
                editBtn.type = 'button';
                editBtn.className = 'mm-action-btn';
                editBtn.title = relatedPresets.length === 1
                    ? 'Edit the saved preset that uses this model'
                    : `Edit one of the ${relatedPresets.length} presets using this model`;
                editBtn.textContent = relatedPresets.length === 1 ? 'Edit Preset' : 'Edit Presets…';
                editBtn.addEventListener('click', () => _openEditPreset(relatedPresets));
                actions.appendChild(editBtn);
            }
        }
    } else {
        const unavailable = document.createElement('div');
        unavailable.className = 'mm-card-action-note';
        unavailable.setAttribute('role', 'status');
        unavailable.textContent = inventoryActionNote(inventory);
        actions.appendChild(unavailable);
    }

    const copyBtn = document.createElement('button');
    copyBtn.type = 'button';
    copyBtn.className = 'mm-action-btn mm-action-copy';
    copyBtn.title = 'Copy path';
    copyBtn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="13" height="13"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg> Copy Path';
    // Typed HF sources retain a stable repo id while `path` points at a resolved
    // cache snapshot. Copy the source users can reuse in configuration.
    const pathToCopy = inventoryModelSourceValue(m) || m.path || '';
    copyBtn.addEventListener('click', () => {
        navigator.clipboard.writeText(pathToCopy).then(() => {
            showToast('Path copied', 'success');
        }).catch(() => {
            showToast('Copy failed', 'error');
        });
    });
    if (pathToCopy) actions.appendChild(copyBtn);

    // The current delete endpoint is deliberately file-only and validates a
    // .gguf suffix. Do not advertise directory deletion for MLX/Transformers.
    if (inventory.format === 'gguf' && (m.path || '').toLowerCase().endsWith('.gguf')) {
        const deleteBtn = document.createElement('button');
        deleteBtn.type = 'button';
        deleteBtn.className = 'mm-action-btn mm-action-delete';
        deleteBtn.title = 'Delete this model from library';
        deleteBtn.setAttribute('aria-label', 'Delete this model from library');
        deleteBtn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="13" height="13"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>';
        deleteBtn.addEventListener('click', () => deleteModel(m.path, m.filename || name));
        actions.appendChild(deleteBtn);
    }

    const tagBtn = document.createElement('button');
    tagBtn.type = 'button';
    tagBtn.className = 'mm-action-btn mm-action-tags';
    tagBtn.title = 'Add tag';
    tagBtn.setAttribute('aria-label', 'Add tag');
    tagBtn.textContent = '+';
    tagBtn.addEventListener('click', () => {
        openTagPicker(m.path, tags);
    });
    actions.appendChild(tagBtn);

    card.appendChild(actions);
    return card;
}

function normalizeInventory(model) {
    const format = Object.hasOwn(INVENTORY_BADGES.format, model.format) ? model.format : 'unknown';
    const source = Object.hasOwn(INVENTORY_BADGES.source, model.source) ? model.source : 'unknown';
    const lifecycle = Object.hasOwn(INVENTORY_BADGES.lifecycle, model.lifecycle) ? model.lifecycle : 'unknown';
    let compatibility = Object.hasOwn(INVENTORY_BADGES.compatibility, model.compatibility)
        ? model.compatibility
        : 'unknown';
    const advertisedBackends = Array.isArray(model.supported_backends)
        ? [...new Set(model.supported_backends.filter(backend => Object.hasOwn(BACKEND_LABELS, backend)))]
        : [];
    const rapidMlxPlatformBlocked = advertisedBackends.includes('rapid_mlx')
        && !rapidMlxLocalAvailable;
    const supportedBackends = rapidMlxPlatformBlocked
        ? advertisedBackends.filter(backend => backend !== 'rapid_mlx')
        : advertisedBackends;
    if (rapidMlxPlatformBlocked && supportedBackends.length === 0) compatibility = 'unsupported';
    const companionKind = ['mmproj', 'draft'].includes(model.companion_kind)
        ? model.companion_kind
        : null;
    return {
        format,
        source,
        lifecycle,
        compatibility,
        supportedBackends,
        rapidMlxPlatformBlocked,
        companionKind,
        launchable: lifecycle === 'ready' && compatibility !== 'unsupported' && supportedBackends.length > 0,
    };
}

function createInventoryBadge(category, value, descriptor) {
    const badge = document.createElement('span');
    badge.className = `mm-inventory-badge mm-inventory-badge--${category} mm-inventory-badge--${value}`;
    badge.textContent = descriptor[0];
    badge.title = descriptor[1];
    badge.setAttribute('aria-label', descriptor[1]);
    return badge;
}

function buildInventoryBadgeRail(inventory) {
    const rail = document.createElement('div');
    rail.className = 'mm-inventory-badges';
    rail.setAttribute('aria-label', 'Model inventory metadata');
    rail.appendChild(createInventoryBadge('format', inventory.format, INVENTORY_BADGES.format[inventory.format]));
    rail.appendChild(createInventoryBadge('source', inventory.source, INVENTORY_BADGES.source[inventory.source]));
    rail.appendChild(createInventoryBadge('lifecycle', inventory.lifecycle, INVENTORY_BADGES.lifecycle[inventory.lifecycle]));
    rail.appendChild(createInventoryBadge(
        'compatibility',
        inventory.compatibility,
        INVENTORY_BADGES.compatibility[inventory.compatibility],
    ));

    if (inventory.supportedBackends.length) {
        inventory.supportedBackends.forEach(backend => {
            rail.appendChild(createInventoryBadge(
                'backend',
                backend,
                [BACKEND_LABELS[backend], `Supported backend: ${BACKEND_LABELS[backend]}`],
            ));
        });
    } else if (inventory.rapidMlxPlatformBlocked) {
        rail.appendChild(createInventoryBadge(
            'backend',
            'platform-unavailable',
            ['Apple Silicon required', rapidMlxLocalRequirement],
        ));
    } else {
        rail.appendChild(createInventoryBadge('backend', 'none', ['No backend', 'Supported backend: None']));
    }

    if (inventory.companionKind) {
        const descriptor = inventory.companionKind === 'mmproj'
            ? ['Vision companion', 'Companion type: Multimodal projector']
            : ['Draft companion', 'Companion type: Draft / MTP model'];
        rail.appendChild(createInventoryBadge('companion', inventory.companionKind, descriptor));
    }
    return rail;
}

function inventoryActionNote(inventory) {
    if (inventory.companionKind === 'mmproj') return 'Vision companion — select it with a compatible primary model.';
    if (inventory.companionKind === 'draft') return 'Draft companion — select it from speculative decoding settings.';
    if (inventory.lifecycle === 'converting') return 'Conversion in progress. Launch will be available after validation.';
    if (inventory.lifecycle === 'incomplete') return 'Incomplete model. Finish the download or conversion before launch.';
    if (inventory.lifecycle === 'invalid') return 'Invalid model. Review its files or provenance before launch.';
    if (inventory.rapidMlxPlatformBlocked) return `${rapidMlxLocalRequirement}. You can still manage or copy this model.`;
    if (inventory.compatibility === 'unsupported') return 'No installed backend supports this model.';
    if (inventory.source === 'recovered_gguf') return 'Experimental GGUF recovery; launch is disabled pending profile promotion.';
    if (inventory.source === 'requantized_mlx') return 'Experimental MLX re-quantization; launch is disabled pending profile promotion.';
    return 'Backend compatibility has not been verified.';
}

function appendRapidMlxConfigureButton(actions, model) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'mm-action-btn mm-action-btn--switch';
    button.title = 'Create a Rapid-MLX preset for this model';
    button.textContent = 'Configure Rapid-MLX';
    button.addEventListener('click', async () => {
        const modelPath = inventoryModelSourceValue(model) || model.path || '';
        const modelSource = model.model_source || {
            kind: 'mlx_directory',
            path: model.path || modelPath,
        };
        const seed = {
            backend: 'rapid_mlx',
            name: `${model.model_name || model.filename || 'MLX model'} · Rapid-MLX`,
            model_path: '',
            port: 8000,
            rapid_mlx: {
                model_path: modelPath,
                model_source: modelSource,
                host: '127.0.0.1',
                port: 8000,
                log_level: 'INFO',
            },
        };
        closeModelsModal();
        const { openPresetModal } = await import('./presets.js');
        openPresetModal('new', 'model', seed);
    });
    actions.appendChild(button);
}

function inventoryModelSourceValue(model) {
    const source = model.model_source;
    if (!source || typeof source !== 'object') return '';
    if (source.kind === 'mlx_directory' || source.kind === 'gguf_file') return source.path || '';
    if (source.kind === 'hugging_face_repo') return source.repo_id || '';
    if (source.kind === 'alias') return source.value || '';
    if (source.kind === 'authoritative_safetensors') {
        return source.source?.path || source.source?.repo_id || '';
    }
    return '';
}

function isLocalServerRunning() {
    return !document.getElementById('btn-stop')?.disabled;
}

function findPresetsForModel(model) {
    const paths = new Set([model.path, inventoryModelSourceValue(model)].filter(Boolean));
    if (!paths.size) return [];
    return (sessionState.presets || []).filter(preset => {
        if (paths.has(preset.model_path)) return true;
        const rapidMlx = preset.rapid_mlx;
        if (!rapidMlx) return false;
        const candidate = rapidMlx.model_source_view?.canonical_identity
            || rapidMlx.model_source_view?.display_name
            || rapidMlx.model_path;
        return candidate ? paths.has(candidate) : false;
    });
}

async function doSwitchToPreset(presetId) {
    closeModelsModal();
    const { syncSelectedPresetSelection } = await import('./presets.js');
    syncSelectedPresetSelection(presetId, { userIntent: true, persist: true });
    const { doStart } = await import('./attach-detach.js');
    await doStart(null, { skipRunningConfirm: true });
}

async function doQuickLoad(model) {
    closeModelsModal();

    // Derive base config from the currently running preset
    const runningPreset = sessionState.presets?.find(p => p.id === sessionState.activeSessionPresetId) || {};

    const config = {
        preset_id: '',
        model_path: model.path || '',
        hf_repo: null,
        context_size: runningPreset.context_size || 32768,
        ctk: runningPreset.ctk || 'q8_0',
        ctv: runningPreset.ctv || 'f16',
        port: runningPreset.port || 8001,
        bind_host: runningPreset.bind_host || '127.0.0.1',
        api_key: runningPreset.api_key || null,
        gpu_layers: runningPreset.gpu_layers ?? null,
        threads: runningPreset.threads ?? null,
        threads_batch: runningPreset.threads_batch ?? null,
        batch_size: runningPreset.batch_size || 2048,
        ubatch_size: runningPreset.ubatch_size || runningPreset.batch_size || 2048,
        flash_attn: runningPreset.flash_attn || '',
        no_mmap: !!runningPreset.no_mmap,
        mlock: !!runningPreset.mlock,
        parallel_slots: runningPreset.parallel_slots || 1,
        tensor_split: runningPreset.tensor_split || '',
        split_mode: runningPreset.split_mode || '',
        main_gpu: runningPreset.main_gpu ?? null,
        kv_unified: runningPreset.kv_unified ?? null,
        cache_ram_mib: runningPreset.cache_ram_mib ?? null,
        // Clear model-specific fields — let llama-server auto-detect from GGUF
        mmproj: null,
        chat_template_file: null,
        reasoning: null,
        enable_thinking: null,
        preserve_thinking: null,
        reasoning_budget: null,
        reasoning_budget_message: null,
        draft_model: '',
        draft_min: null,
        draft_max: null,
        spec_type: null,
        spec_ngram_size: null,
        spec_draft_n_max: null,
        ngram_spec: false,
        seed: runningPreset.seed ?? null,
        alias: null,
        max_tokens: runningPreset.max_tokens ?? null,
        fit_enabled: null,
        fit_target: null,
        system_prompt_file: '',
        extra_args: runningPreset.extra_args || '',
        // Generation defaults — leave unset so llama-server uses its own defaults
        temperature: runningPreset.temperature,
        top_p: runningPreset.top_p,
        top_k: runningPreset.top_k,
        min_p: runningPreset.min_p,
        repeat_penalty: runningPreset.repeat_penalty,
        presence_penalty: runningPreset.presence_penalty ?? null,
        n_cpu_moe: runningPreset.n_cpu_moe,
        rope_scaling: runningPreset.rope_scaling || '',
        rope_freq_base: runningPreset.rope_freq_base ?? null,
        rope_freq_scale: runningPreset.rope_freq_scale ?? null,
    };

    // Cap context to the model's training limit if known
    if (model.n_ctx_train > 0 && config.context_size > model.n_ctx_train) {
        config.context_size = model.n_ctx_train;
    }

    const modelName = model.model_name || model.filename || 'model';
    showToast('Loading ' + modelName + '…', 'info', 'Stopping current server', { duration: 14000 });

    const { doStartWithConfig } = await import('./attach-detach.js');
    await doStartWithConfig(config, { skipRunningConfirm: true });
}

function formatPresetSummaryLine(preset) {
    const parts = [preset.name || 'Unnamed preset'];
    if (preset.context_size) parts.push(`${Math.round(preset.context_size / 1024)}k context`);
    const ctk = preset.ctk || '';
    const ctv = preset.ctv || '';
    if (ctk || ctv) parts.push(`KV ${ctk || 'default'}/${ctv || 'default'}`);
    if (preset.reasoning) parts.push(`reasoning ${preset.reasoning}`);
    else if (preset.enable_thinking != null) parts.push(preset.enable_thinking ? 'thinking on' : 'thinking off');
    if (preset.mmproj) parts.push('vision');
    if (preset.bind_host === '0.0.0.0') parts.push('LAN');
    if (preset.api_key) parts.push('API key');
    return parts.join(' · ');
}

function _openEditPreset(presets) {
    if (presets.length === 1) {
        const select = document.getElementById('preset-select');
        if (select) select.value = presets[0].id;
        closeModelsModal();
        import('./presets.js').then(({ openPresetModal }) => openPresetModal('edit'));
        return;
    }

    // Multiple presets: show an inline chooser via toast instead of prompt()
    showToastWithActions(
        'Which preset to edit?',
        'info',
        presets.map(p => p.name).join(' · '),
        presets.slice(0, 3).map((p, i) => ({
            id: 'p' + i,
            label: p.name,
            primary: i === 0,
            handler: () => {
                const select = document.getElementById('preset-select');
                if (select) select.value = p.id;
                closeModelsModal();
                import('./presets.js').then(({ openPresetModal }) => openPresetModal('edit'));
            },
        })),
        { duration: 12000 },
    );
}

function buildPresetSummary(presets) {
    if (!presets.length) return '';
    const first = presets[0];
    const summary = formatPresetSummaryLine(first);
    if (presets.length === 1) {
        return `Saved preset: ${summary}`;
    }
    return `Saved presets (${presets.length}): ${summary} +${presets.length - 1} more`;
}

async function deleteModel(path, filename) {
    const presets = findPresetsForModel({ path });
    const extra = presets.length
        ? `\nThis will break presets that use this model:\n- ${presets.map(p => p.name || 'Unnamed preset').join('\n- ')}\n`
        : '';
    const confirmed = await _showConfirm(
        'Delete model file',
        `"${escapeHtml(filename)}"\nPath: ${escapeHtml(path)}\n\nThis will permanently remove the file from disk.${extra}\nThis action cannot be undone.`
    );
    if (!confirmed) return;

    try {
        const resp = await fetch('/api/models/file', {
            method: 'DELETE',
            headers: window.authHeaders
                ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                : { 'Content-Type': 'application/json' },
            body: JSON.stringify({ path }),
        });
        if (!resp.ok) {
            const err = await resp.text().catch(() => 'Unknown error');
            showToast('Delete failed: ' + err, 'error');
            return;
        }
        showToast('Model deleted', 'success');
        invalidateModelInventory();
        await loadModels({ refresh: true });
    } catch (err) {
        showToast('Delete failed: ' + err.message, 'error');
    }
}

async function refreshModels() {
    const summary = document.getElementById('models-summary');
    if (summary) summary.textContent = 'Refreshing...';
    const btn = document.getElementById('models-refresh-btn');
    if (btn) btn.classList.add('spinning');
    try {
        const resp = await fetch('/api/models/refresh', {
            method: 'POST',
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        const data = await resp.json();
        if (!data.ok) showToast('Model refresh failed: ' + (data.error || 'unknown'), 'error');
    } catch (err) {
        showToast('Model refresh failed: ' + err.message, 'error');
    } finally {
        if (btn) btn.classList.remove('spinning');
    }
    invalidateModelInventory();
    await loadModels({ refresh: true });
}

// ── Model tags ────────────────────────────────────────────────────────────────

async function updateModelTags(modelPath, tags) {
    try {
        const resp = await fetch('/api/models/tags', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ model_path: modelPath, tags }),
        });
        const data = await resp.json();
        if (!data.ok) {
            showToast('Tag update failed: ' + (data.error || 'unknown'), 'error');
            return false;
        }
        return true;
    } catch (err) {
        showToast('Tag update failed: ' + err.message, 'error');
        return false;
    }
}

async function removeModelTag(modelPath, tag) {
    const resp = await fetch('/api/models/tags', {
        headers: window.authHeaders ? window.authHeaders() : {},
    });
    if (!resp.ok) return;
    const data = await resp.json();
    const currentTags = (data.tags[modelPath] || []).filter(t => t !== tag);
    await updateModelTags(modelPath, currentTags);
    invalidateModelInventory();
    await loadModels({ refresh: true });
}

function openTagPicker(modelPath, currentTags) {
    const existing = document.getElementById('mm-tag-picker');
    if (existing) existing.remove();

    const picker = document.createElement('div');
    picker.id = 'mm-tag-picker';
    picker.className = 'mm-tag-picker';

    const title = document.createElement('div');
    title.className = 'mm-tag-picker-title';
    title.textContent = 'Tags';
    picker.appendChild(title);

    const pillsWrap = document.createElement('div');
    pillsWrap.className = 'mm-tag-picker-pills';

    const allTags = new Set([...KNOWN_TAGS, ...currentTags]);
    allTags.forEach(tag => {
        const pill = document.createElement('span');
        const has = currentTags.includes(tag);
        pill.className = 'mm-tag-pill' + (has ? ' mm-tag-pill--active' : '');
        pill.textContent = tag;
        pill.addEventListener('click', () => {
            const newTags = has
                ? currentTags.filter(t => t !== tag)
                : [...currentTags, tag];
            updateModelTags(modelPath, newTags).then(ok => {
                if (ok) {
                    invalidateModelInventory();
                    loadModels({ refresh: true });
                }
            });
        });
        pillsWrap.appendChild(pill);
    });

    picker.appendChild(pillsWrap);
    document.body.appendChild(picker);

    const close = (e) => {
        if (!picker.contains(e.target)) {
            picker.remove();
            document.removeEventListener('mousedown', close);
        }
    };
    setTimeout(() => document.addEventListener('mousedown', close), 0);
}

// ── Library toolbar ───────────────────────────────────────────────────────────

// ── Library toolbar (stable container to avoid focus loss) ────────────────────

let _toolbarInitialized = false;

function ensureLibraryToolbar() {
    if (_toolbarInitialized) return;
    _toolbarInitialized = true;
    const container = document.getElementById('mm-library-toolbar');
    if (!container) return;
    container.innerHTML = '';

    // Search input
    const wrap = document.createElement('div');
    wrap.className = 'mm-lib-search-wrap';

    const searchIcon = document.createElement('span');
    searchIcon.className = 'mm-lib-search-icon';
    searchIcon.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="11" height="11"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>';

    const input = document.createElement('input');
    input.type = 'text';
    input.id = 'mm-lib-search-input';
    input.className = 'mm-lib-search-input';
    input.placeholder = 'Search models...';
    input.value = prefs.search || '';
    input.setAttribute('autocomplete', 'off');
    input.setAttribute('aria-label', 'Search models');

    wrap.appendChild(searchIcon);
    wrap.appendChild(input);
    container.appendChild(wrap);

    let lastSearch = null;
    input.addEventListener('input', () => {
        clearTimeout(lastSearch);
        lastSearch = setTimeout(() => {
            prefs.search = input.value;
            savePrefs();
            loadModels();
        }, 250);
    });

    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            clearTimeout(lastSearch);
            prefs.search = input.value;
            savePrefs();
            loadModels();
        }
    });

    // Right-side controls
    const right = document.createElement('div');
    right.className = 'mm-lib-controls';

    // Filters button
    const filtersWrap = document.createElement('div');
    filtersWrap.className = 'mm-lib-filters';

    const filtersBtn = document.createElement('button');
    filtersBtn.type = 'button';
    filtersBtn.className = 'mm-lib-btn mm-lib-btn--labeled';
    filtersBtn.id = 'mm-lib-filters-toggle';
    filtersBtn.title = 'Filter models by type, quantization, or tag';
    filtersBtn.innerHTML =
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="11" height="11"><line x1="4" y1="21" x2="4" y2="14"/><line x1="4" y1="10" x2="4" y2="3"/><line x1="12" y1="21" x2="12" y2="12"/><line x1="12" y1="8" x2="12" y2="3"/><line x1="20" y1="21" x2="20" y2="16"/><line x1="20" y1="12" x2="20" y2="3"/><line x1="1" y1="14" x2="7" y2="14"/><line x1="9" y1="8" x2="15" y2="8"/><line x1="17" y1="16" x2="23" y2="16"/></svg>'
        + '<span>Filter</span>';

    const filtersPanel = document.createElement('div');
    filtersPanel.className = 'mm-lib-filters-panel';
    filtersPanel.id = 'mm-lib-filters-panel';

    filtersWrap.appendChild(filtersBtn);
    filtersWrap.appendChild(filtersPanel);
    right.appendChild(filtersWrap);

    // Toggle filters panel
    filtersBtn.addEventListener('click', () => {
        filtersPanel.classList.toggle('open');
    });

    // Sort select
    const sortWrap = document.createElement('div');
    sortWrap.className = 'mm-lib-sort-wrap';

    const sortSelect = document.createElement('select');
    sortSelect.className = 'mm-lib-sort-select';
    sortSelect.id = 'mm-lib-sort-select';
    const sortOptions = [
        { value: 'name-asc', label: 'Name A–Z' },
        { value: 'name-desc', label: 'Name Z–A' },
        { value: 'size-desc', label: 'Size (largest)' },
        { value: 'size-asc', label: 'Size (smallest)' },
        { value: 'vram-desc', label: 'VRAM (highest)' },
        { value: 'vram-asc', label: 'VRAM (lowest)' },
        { value: 'date-desc', label: 'Date (newest)' },
        { value: 'date-asc', label: 'Date (oldest)' },
    ];
    sortOptions.forEach(o => {
        const opt = document.createElement('option');
        opt.value = o.value;
        opt.textContent = o.label;
        sortSelect.appendChild(opt);
    });
    sortSelect.value = prefs.sort || 'name-asc';

    sortSelect.addEventListener('change', () => {
        prefs.sort = sortSelect.value;
        savePrefs();
        loadModels();
    });

    const sortLabel = document.createElement('span');
    sortLabel.className = 'mm-lib-sort-label';
    sortLabel.textContent = 'Sort:';
    sortWrap.appendChild(sortLabel);
    sortWrap.appendChild(sortSelect);
    right.appendChild(sortWrap);

    // View mode toggle
    const viewBtn = document.createElement('button');
    viewBtn.type = 'button';
    viewBtn.className = 'mm-lib-btn mm-lib-btn--labeled';
    viewBtn.id = 'mm-lib-view-toggle';
    viewBtn.title = prefs.viewMode === 'cards' ? 'Switch to list view' : 'Switch to cards view';
    // eslint-disable-next-line no-unsanitized/property -- static SVG, no user data
    viewBtn.innerHTML = prefs.viewMode === 'cards'
        ? ICON_LIST_VIEW + '<span>List</span>'
        : ICON_CARDS_VIEW + '<span>Cards</span>';

    viewBtn.addEventListener('click', () => {
        prefs.viewMode = prefs.viewMode === 'cards' ? 'list' : 'cards';
        viewBtn.title = prefs.viewMode === 'cards' ? 'Switch to list view' : 'Switch to cards view';
        // eslint-disable-next-line no-unsanitized/property -- static SVG, no user data
        viewBtn.innerHTML = prefs.viewMode === 'cards'
            ? ICON_LIST_VIEW + '<span>List</span>'
            : ICON_CARDS_VIEW + '<span>Cards</span>';
        savePrefs();
        loadModels();
    });

    right.appendChild(viewBtn);
    container.appendChild(right);
}

// Called each time models list is loaded; only rebuilds dynamic filter chips.
function rebuildLibraryFilters(models) {
    const container = document.getElementById('mm-library-toolbar');
    if (!container) return;
    ensureLibraryToolbar();

    const filtersPanel = document.getElementById('mm-lib-filters-panel');
    const filtersBtn = document.getElementById('mm-lib-filters-toggle');
    if (!filtersPanel || !filtersBtn) return;

    // Update filter button active indicator
    const hasActiveFilters = !prefs.showMmproj || !prefs.showMain || !prefs.showSplit ||
        !prefs.showDraftModels || prefs.sizeMaxGb > 0 ||
        Object.values(prefs.quantFilters).some(v => v === false) ||
        !!prefs.tagFilter || !!prefs.familyFilter || !!prefs.sizeClassFilter;
    filtersBtn.classList.toggle('mm-lib-btn--active', hasActiveFilters);
    const spanEl = filtersBtn.querySelector('span');
    if (spanEl) spanEl.textContent = 'Filter' + (hasActiveFilters ? ' •' : '');

    // Clear previous filter rows
    filtersPanel.innerHTML = '';

    // Type filters
    const typeRow = document.createElement('div');
    typeRow.className = 'mm-lib-filter-row';
    const typeLabel = document.createElement('span');
    typeLabel.className = 'mm-lib-filter-label';
    typeLabel.textContent = 'Type';

    const mmprojChip = createChip('mmproj', prefs.showMmproj);
    const mainChip = createChip('Main', prefs.showMain);
    const splitChip = createChip('Split', prefs.showSplit);
    const draftChip = createChip('Draft / MTP', prefs.showDraftModels);

    mmprojChip.addEventListener('click', () => {
        prefs.showMmproj = !prefs.showMmproj;
        mmprojChip.classList.toggle('active', prefs.showMmproj);
        savePrefs(); loadModels();
    });
    mainChip.addEventListener('click', () => {
        prefs.showMain = !prefs.showMain;
        mainChip.classList.toggle('active', prefs.showMain);
        savePrefs(); loadModels();
    });
    splitChip.addEventListener('click', () => {
        prefs.showSplit = !prefs.showSplit;
        splitChip.classList.toggle('active', prefs.showSplit);
        savePrefs(); loadModels();
    });
    draftChip.addEventListener('click', () => {
        prefs.showDraftModels = !prefs.showDraftModels;
        draftChip.classList.toggle('active', prefs.showDraftModels);
        savePrefs(); loadModels();
    });

    typeRow.appendChild(typeLabel);
    typeRow.appendChild(mmprojChip);
    typeRow.appendChild(mainChip);
    typeRow.appendChild(splitChip);
    typeRow.appendChild(draftChip);
    filtersPanel.appendChild(typeRow);

    // Size filter row
    const sizeRow = document.createElement('div');
    sizeRow.className = 'mm-lib-filter-row';
    const sizeLabel = document.createElement('span');
    sizeLabel.className = 'mm-lib-filter-label';
    sizeLabel.textContent = 'Size';
    const sizeAnyChip = createChip('Any', prefs.sizeMaxGb === 0);
    const size3Chip = createChip('< 3 GB', prefs.sizeMaxGb === 3);
    const size8Chip = createChip('< 8 GB', prefs.sizeMaxGb === 8);
    const setSizeFilter = (gb) => {
        prefs.sizeMaxGb = gb;
        sizeAnyChip.classList.toggle('active', gb === 0);
        size3Chip.classList.toggle('active', gb === 3);
        size8Chip.classList.toggle('active', gb === 8);
        savePrefs(); loadModels();
    };
    sizeAnyChip.addEventListener('click', () => setSizeFilter(0));
    size3Chip.addEventListener('click', () => setSizeFilter(prefs.sizeMaxGb === 3 ? 0 : 3));
    size8Chip.addEventListener('click', () => setSizeFilter(prefs.sizeMaxGb === 8 ? 0 : 8));
    sizeRow.appendChild(sizeLabel);
    sizeRow.appendChild(sizeAnyChip);
    sizeRow.appendChild(size3Chip);
    sizeRow.appendChild(size8Chip);
    filtersPanel.appendChild(sizeRow);

    // Family filter (from classification)
    const familySet = new Set();
    models.forEach(m => {
        const f = (m.classification || {}).family || '';
        if (f && f !== 'other') familySet.add(f);
    });

    if (familySet.size > 0) {
        const familyRow = document.createElement('div');
        familyRow.className = 'mm-lib-filter-row';
        const familyLabel = document.createElement('span');
        familyLabel.className = 'mm-lib-filter-label';
        familyLabel.textContent = 'Family';

        const noneChip = createChip('All', !prefs.familyFilter);
        noneChip.addEventListener('click', () => {
            prefs.familyFilter = ''; savePrefs(); loadModels();
        });
        familyRow.appendChild(familyLabel);
        familyRow.appendChild(noneChip);

        const familyLabels = {
            qwen36: 'Qwen3.6', qwen35: 'Qwen3.5', qwen3: 'Qwen3',
            llama3: 'Llama 3.x', gemma4: 'Gemma4', gemma: 'Gemma',
            mistral: 'Mistral', exaone: 'EXAONE', heretic: 'Heretic',
        };

        familySet.forEach(fam => {
            const chip = createChip(familyLabels[fam] || fam, prefs.familyFilter === fam);
            chip.classList.toggle('active', prefs.familyFilter === fam);
            chip.addEventListener('click', () => {
                prefs.familyFilter = prefs.familyFilter === fam ? '' : fam;
                savePrefs(); loadModels();
            });
            familyRow.appendChild(chip);
        });

        filtersPanel.appendChild(familyRow);
    }

    // Size-class filter (from classification)
    const sizeClassSet = new Set();
    models.forEach(m => {
        const s = (m.classification || {}).size_class || '';
        if (s && s !== 'unknown') sizeClassSet.add(s);
    });

    if (sizeClassSet.size > 0) {
        const sizeClassRow = document.createElement('div');
        sizeClassRow.className = 'mm-lib-filter-row';
        const sizeClassLabel = document.createElement('span');
        sizeClassLabel.className = 'mm-lib-filter-label';
        sizeClassLabel.textContent = 'Size class';

        const noneChip = createChip('All', !prefs.sizeClassFilter);
        noneChip.addEventListener('click', () => {
            prefs.sizeClassFilter = ''; savePrefs(); loadModels();
        });
        sizeClassRow.appendChild(sizeClassLabel);
        sizeClassRow.appendChild(noneChip);

        ['tiny', 'small', 'medium', 'large', 'huge'].forEach(sc => {
            if (!sizeClassSet.has(sc)) return;
            const chip = createChip(sc.charAt(0).toUpperCase() + sc.slice(1), prefs.sizeClassFilter === sc);
            chip.classList.toggle('active', prefs.sizeClassFilter === sc);
            chip.addEventListener('click', () => {
                prefs.sizeClassFilter = prefs.sizeClassFilter === sc ? '' : sc;
                savePrefs(); loadModels();
            });
            sizeClassRow.appendChild(chip);
        });

        filtersPanel.appendChild(sizeClassRow);
    }

    // Quant filters (dynamic from models list)
    const quantRow = document.createElement('div');
    quantRow.className = 'mm-lib-filter-row';
    const quantLabel = document.createElement('span');
    quantLabel.className = 'mm-lib-filter-label';
    quantLabel.textContent = 'Quant';

    const quantSet = new Set();
    models.forEach(m => {
        const qt = (m.quant_type || '').toUpperCase();
        if (qt && qt !== 'UNKNOWN') quantSet.add(qt);
    });

    if (quantSet.size > 0 && quantSet.size <= 30) {
        quantSet.forEach(qt => {
            const chip = createChip(qt, prefs.quantFilters[qt] !== false);
            chip.addEventListener('click', () => {
                const active = !prefs.quantFilters[qt];
                prefs.quantFilters[qt] = active;
                chip.classList.toggle('active', active);
                savePrefs(); loadModels();
            });
            quantRow.appendChild(chip);
        });
    }

    filtersPanel.appendChild(quantRow);

    // Tag filter
    const tagRow = document.createElement('div');
    tagRow.className = 'mm-lib-filter-row';
    const tagLabel = document.createElement('span');
    tagLabel.className = 'mm-lib-filter-label';
    tagLabel.textContent = 'Tag';

    const allTags = new Set(KNOWN_TAGS);
    models.forEach(m => {
        (Array.isArray(m.tags) ? m.tags : []).forEach(t => allTags.add(t));
    });

    const allTagArr = Array.from(allTags);
    if (allTagArr.length > 0) {
        const noneChip = createChip('All', !prefs.tagFilter);
        noneChip.addEventListener('click', () => {
            prefs.tagFilter = ''; savePrefs(); loadModels();
        });
        tagRow.appendChild(noneChip);

        allTagArr.forEach(tag => {
            const chip = createChip(tag, prefs.tagFilter === tag);
            chip.classList.toggle('active', prefs.tagFilter === tag);
            chip.addEventListener('click', () => {
                prefs.tagFilter = prefs.tagFilter === tag ? '' : tag;
                savePrefs(); loadModels();
            });
            tagRow.appendChild(chip);
        });
    }

    tagRow.appendChild(tagLabel);
    filtersPanel.appendChild(tagRow);
}

function createChip(label, active) {
    const chip = document.createElement('button');
    chip.type = 'button';
    chip.className = 'mm-lib-chip' + (active ? ' active' : '');
    chip.textContent = label;
    return chip;
}

// ── Experimental Import Lab ─────────────────────────────────────────────────

let importLabInitialized = false;
let importLabAvailability = null;
let importLabPollTimer = null;
let importLabCompletedJobs = new Set();

function apiHeaders(json = false) {
    const headers = window.authHeaders ? { ...window.authHeaders() } : {};
    if (json) headers['Content-Type'] = 'application/json';
    return headers;
}

async function importLabJson(url, options = {}) {
    const response = await fetch(url, options);
    const data = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(data.error || `Request failed (${response.status})`);
    return data;
}

function formatImportBytes(bytes) {
    const value = Number(bytes || 0);
    if (value >= 1024 ** 3) return `${(value / 1024 ** 3).toFixed(1)} GiB`;
    if (value >= 1024 ** 2) return `${(value / 1024 ** 2).toFixed(0)} MiB`;
    return `${Math.round(value / 1024)} KiB`;
}

function exactImportProfile(path) {
    const name = String(path || '').split('/').pop();
    return [
        'SmolLM2-135M-Instruct-F16.gguf',
        'SmolLM2-135M-Instruct-Q8_0.gguf',
        'SmolLM2-135M-Instruct-Q6_K.gguf',
        'SmolLM2-135M-Instruct-Q4_K_M.gguf',
    ].includes(name);
}

async function initImportLab() {
    if (!importLabInitialized) {
        importLabInitialized = true;
        document.getElementById('mm-import-analyze')?.addEventListener('click', analyzeImportSource);
        document.getElementById('mm-import-source')?.addEventListener('keydown', event => {
            if (event.key === 'Enter') analyzeImportSource();
        });
        document.getElementById('mm-import-jobs-refresh')?.addEventListener('click', loadImportJobs);
    }
    try {
        importLabAvailability = await importLabJson('/api/models/import-lab/availability', {
            headers: apiHeaders(),
        });
        const status = document.getElementById('mm-import-platform');
        if (status) {
            status.classList.toggle('unavailable', !importLabAvailability.local_execution_available);
            status.textContent = importLabAvailability.local_execution_available
                ? 'Apple Silicon ready · Local recovery available'
                : 'Local recovery unavailable · Manage models or use llama.cpp';
        }
    } catch (error) {
        showToast(`Import Lab availability failed: ${error.message}`, 'error');
    }
    await loadImportJobs();
}

async function analyzeImportSource() {
    const input = document.getElementById('mm-import-source');
    const reportEl = document.getElementById('mm-import-report');
    const engineNote = document.getElementById('mm-import-engine-note');
    const path = input?.value.trim();
    if (!path || !reportEl) return;
    reportEl.replaceChildren(Object.assign(document.createElement('div'), {
        className: 'mm-import-empty-state', textContent: 'Reading bounded GGUF metadata…',
    }));
    try {
        const body = JSON.stringify({ path });
        const [report, resource] = await Promise.all([
            importLabJson('/api/models/gguf/import/compatibility/preview', {
                method: 'POST', headers: apiHeaders(true), body,
            }),
            importLabJson('/api/models/import-lab/resource-estimate', {
                method: 'POST', headers: apiHeaders(true), body,
            }),
        ]);
        renderImportReport(reportEl, report, resource, path);
        if (engineNote) {
            const recoverable = report.compatibility === 'experimental' && exactImportProfile(path);
            engineNote.textContent = recoverable
                ? 'Exact experimental profile found. llama.cpp remains available; recovery creates a separate non-launchable MLX copy.'
                : 'Recommended engine: llama.cpp. No safe MLX recovery profile is available for this model.';
        }
    } catch (error) {
        reportEl.replaceChildren(Object.assign(document.createElement('div'), {
            className: 'mm-import-empty-state', textContent: error.message,
        }));
    }
}

function appendImportFact(container, label, value) {
    const fact = document.createElement('div');
    fact.className = 'mm-import-fact';
    const labelEl = document.createElement('span');
    labelEl.textContent = label;
    const valueEl = document.createElement('strong');
    valueEl.textContent = value;
    fact.append(labelEl, valueEl);
    container.appendChild(fact);
}

function renderImportReport(container, report, resource, path) {
    container.replaceChildren();
    const verdict = document.createElement('div');
    verdict.className = 'mm-import-verdict';
    const title = document.createElement('strong');
    title.textContent = report.architecture
        ? `${report.architecture} · ${report.tensor_count || 0} tensors`
        : 'GGUF compatibility report';
    const badge = document.createElement('span');
    const compatibility = report.compatibility || 'unsupported';
    badge.className = `mm-import-verdict-badge ${compatibility}`;
    badge.textContent = compatibility;
    verdict.append(title, badge);
    container.appendChild(verdict);

    const facts = document.createElement('div');
    facts.className = 'mm-import-facts';
    appendImportFact(facts, 'Source', formatImportBytes(resource.source_bytes));
    appendImportFact(facts, 'Recovered FP16', formatImportBytes(resource.estimated_fp16_bytes));
    appendImportFact(facts, 'Disk required', formatImportBytes(resource.required_disk_bytes));
    appendImportFact(facts, 'Disk headroom', resource.disk_sufficient === true
        ? 'Available'
        : (resource.disk_sufficient === false ? 'Insufficient' : 'Unknown'));
    appendImportFact(facts, 'Memory', resource.ram_guidance?.replaceAll('_', ' ') || 'Unknown');
    appendImportFact(facts, 'Engine fallback', 'llama.cpp');
    container.appendChild(facts);

    const reasons = [
        ...(report.unsupported_reasons || []),
        ...(report.missing_profile_fields || []).map(value => `Missing profile: ${value}`),
        ...(report.missing_assets || []).map(value => `Missing asset: ${value}`),
        ...(report.warnings || []),
    ].slice(0, 8);
    if (reasons.length) {
        const list = document.createElement('ul');
        list.className = 'mm-import-reasons';
        reasons.forEach(reason => {
            const item = document.createElement('li');
            item.textContent = reason;
            list.appendChild(item);
        });
        container.appendChild(list);
    }

    const actions = document.createElement('div');
    actions.className = 'mm-import-actions';
    const start = document.createElement('button');
    start.type = 'button';
    start.className = 'mm-action-btn mm-action-btn--switch';
    start.textContent = 'Recover experimental FP16';
    const allowed = report.compatibility === 'experimental'
        && exactImportProfile(path)
        && importLabAvailability?.local_execution_available
        && resource.disk_sufficient === true;
    start.disabled = !allowed;
    start.title = allowed
        ? 'Create a separate non-launchable MLX recovery cache'
        : 'Recovery requires Apple Silicon, disk headroom, and an exact supported profile';
    start.addEventListener('click', () => startImportJob(path));
    actions.appendChild(start);
    container.appendChild(actions);
}

async function startImportJob(path) {
    try {
        await importLabJson('/api/models/import-lab/jobs', {
            method: 'POST', headers: apiHeaders(true), body: JSON.stringify({ source_path: path }),
        });
        showToast('Experimental recovery queued', 'success');
        await loadImportJobs();
    } catch (error) {
        showToast(`Recovery could not start: ${error.message}`, 'error');
    }
}

async function cancelImportJob(id) {
    try {
        await importLabJson(`/api/models/import-lab/jobs/${encodeURIComponent(id)}/cancel`, {
            method: 'POST', headers: apiHeaders(),
        });
        await loadImportJobs();
    } catch (error) {
        showToast(`Cancellation failed: ${error.message}`, 'error');
    }
}

async function forgetImportJob(id) {
    try {
        await importLabJson(`/api/models/import-lab/jobs/${encodeURIComponent(id)}`, {
            method: 'DELETE', headers: apiHeaders(),
        });
        await loadImportJobs();
    } catch (error) {
        showToast(`Cleanup failed: ${error.message}`, 'error');
    }
}

async function loadImportJobs() {
    const container = document.getElementById('mm-import-jobs-list');
    if (!container) return;
    try {
        const jobs = await importLabJson('/api/models/import-lab/jobs', { headers: apiHeaders() });
        renderImportJobs(container, jobs);
        const active = jobs.some(job => job.can_cancel);
        clearTimeout(importLabPollTimer);
        if (active) importLabPollTimer = setTimeout(loadImportJobs, 800);
        const newlyComplete = jobs.filter(job => job.state === 'complete' && !importLabCompletedJobs.has(job.id));
        if (newlyComplete.length) {
            newlyComplete.forEach(job => importLabCompletedJobs.add(job.id));
            invalidateModelInventory();
        }
    } catch (error) {
        container.replaceChildren(Object.assign(document.createElement('div'), {
            className: 'mm-import-empty-state', textContent: error.message,
        }));
    }
}

function renderImportJobs(container, jobs) {
    container.replaceChildren();
    if (!jobs.length) {
        container.appendChild(Object.assign(document.createElement('div'), {
            className: 'mm-import-empty-state', textContent: 'No recovery jobs yet.',
        }));
        return;
    }
    jobs.slice().reverse().forEach(job => {
        const card = document.createElement('article');
        card.className = 'mm-import-job';
        const top = document.createElement('div');
        top.className = 'mm-import-job-top';
        const message = document.createElement('strong');
        message.textContent = job.message;
        const state = document.createElement('span');
        state.className = 'mm-import-job-state';
        state.textContent = job.state;
        top.append(message, state);
        const progress = document.createElement('div');
        progress.className = 'mm-import-progress';
        const progressPercent = Math.max(0, Math.min(100, job.progress_percent || 0));
        progress.setAttribute('role', 'progressbar');
        progress.setAttribute('aria-label', `Recovery progress: ${job.phase || job.state}`);
        progress.setAttribute('aria-valuemin', '0');
        progress.setAttribute('aria-valuemax', '100');
        progress.setAttribute('aria-valuenow', String(progressPercent));
        const fill = document.createElement('span');
        fill.style.width = `${progressPercent}%`;
        progress.appendChild(fill);
        card.append(top, progress);
        if (Array.isArray(job.diagnostics) && job.diagnostics.length) {
            const diagnostics = document.createElement('div');
            diagnostics.className = 'mm-import-diagnostics';
            diagnostics.textContent = job.diagnostics.join('\n');
            card.appendChild(diagnostics);
        }
        const actions = document.createElement('div');
        actions.className = 'mm-import-actions';
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'mm-action-btn';
        if (job.can_cancel) {
            button.textContent = 'Cancel and clean staging';
            button.addEventListener('click', () => cancelImportJob(job.id));
        } else {
            button.textContent = 'Clear job';
            button.addEventListener('click', () => forgetImportJob(job.id));
        }
        actions.appendChild(button);
        card.appendChild(actions);
        container.appendChild(card);
    });
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initModels() {
    if (initialized) return;
    initialized = true;

    // Initialize toolbar structure once (search, sort, view controls)
    ensureLibraryToolbar();

    // Bind modal close buttons
    const closeBtn = document.getElementById('models-modal-close');
    if (closeBtn) closeBtn.addEventListener('click', closeModelsModal);

    const cancelBtn = document.getElementById('models-modal-cancel');
    if (cancelBtn) cancelBtn.addEventListener('click', closeModelsModal);

    const refreshBtn = document.getElementById('models-refresh-btn');
    if (refreshBtn) refreshBtn.addEventListener('click', refreshModels);

    // Tab switching
    document.querySelectorAll('#models-modal .mm-tab').forEach(tab => {
        tab.addEventListener('click', () => {
            const target = tab.dataset.tab;
            const summary = document.getElementById('models-summary');
            if (summary) summary.style.visibility = target === 'library' ? '' : 'hidden';
            document.querySelectorAll('#models-modal .mm-tab').forEach(t => t.classList.remove('active'));
            document.querySelectorAll('#models-modal .mm-tab-panel').forEach(p => p.classList.remove('active'));
            tab.classList.add('active');
            const panel = document.querySelector('#models-modal .mm-tab-panel[data-tab="' + target + '"]');
            if (panel) panel.classList.add('active');
            if (target === 'download') {
                initHfDownloadTab();
            } else if (target === 'import-lab') {
                initImportLab();
            } else if (target === 'library' && !inventoryCache) {
                loadModels();
            }
        });
    });

    // Configure settings link
    const settingsLink = document.getElementById('models-open-settings-link');
    if (settingsLink) {
        settingsLink.addEventListener('click', () => {
            closeModelsModal();
            Router.navigate('/settings#models');
        });
    }

    // Modal overlay click to close
    const modal = document.getElementById('models-modal');
    if (modal) {
        modal.addEventListener('click', e => {
            if (e.target === modal) closeModelsModal();
        });
    }
}

// ── HF Download tab (inside models modal) ─────────────────────────────────────

async function initHfDownloadTab() {
    if (hfState.initialized) return;
    hfState.initialized = true;

    const searchInput = document.getElementById('mm-hf-search-input');
    const discoverPills = document.getElementById('mm-hf-discover-pills');
    const quickpicks = document.getElementById('mm-hf-quickpicks');
    const resultsContainer = document.getElementById('mm-hf-search-results');
    const filelistContainer = document.getElementById('mm-hf-file-list');
    const downloadPanel = document.getElementById('mm-hf-download-panel');

    if (!searchInput || !resultsContainer || !filelistContainer || !downloadPanel) return;

    // Fetch hardware info
    await fetchGpuVram();
    await fetchSystemRam();

    // Helpers to build search params while preserving active filters
    const buildSearchParams = () => {
        const typedQuery = (searchInput.value || '').trim();
        const query = typedQuery || hfState.activeDiscoverQuery || '';
        const author = (typedQuery ? hfState.activeAuthor : (hfState.activeAuthor || null));
        const workloadProfile = sessionState.workloadProfile?.id || null;
        return { query: query || undefined, author: author || undefined, workloadProfile };
    };

    // Render discover pills
    hfRenderDiscoverPills({
        container: discoverPills,
        quickpicksContainer: quickpicks,
        onPillClick: (cat) => {
            // Track active discover query so sort changes still work
            hfState.activeDiscoverQuery = cat.params.query || null;
            hfState.activeAuthor = null;
            hfSearch({
                query: cat.params.query,
                mlxActive: hfState.discoveryScopeMlx,
                ggufActive: hfState.discoveryScopeGguf,
                hfSort: hfState.discoverySort,
                limit: cat.params.limit || 20,
                container: resultsContainer,
                filelistContainer,
                quickpicksContainer: quickpicks,
                discoverPillsContainerId: 'mm-hf-discover-pills',
                onOpenCardPanel: openCardPanel,
                onSelectModel: (m) => onHfModelSelected(m, filelistContainer, downloadPanel),
                workloadProfile: sessionState.workloadProfile?.id || null,
            });
        },
    });

    // Load quick-picks
    hfLoadQuickPicks({
        container: quickpicks,
        discoverPillsContainerId: 'mm-hf-discover-pills',
        onAuthorClick: (author) => {
            hfState.activeAuthor = author;
            hfState.activeDiscoverQuery = null;
            hfSearch({
                query: '',
                author,
                mlxActive: hfState.discoveryScopeMlx,
                ggufActive: hfState.discoveryScopeGguf,
                hfSort: hfState.discoverySort,
                limit: 20,
                container: resultsContainer,
                filelistContainer,
                quickpicksContainer: quickpicks,
                discoverPillsContainerId: 'mm-hf-discover-pills',
                onOpenCardPanel: openCardPanel,
                onSelectModel: (m) => onHfModelSelected(m, filelistContainer, downloadPanel),
                workloadProfile: sessionState.workloadProfile?.id || null,
            });
        },
    });

    // Search on input (debounced)
    let searchTimer = null;
    const doSearch = () => {
        const { query, author, sort, workloadProfile } = buildSearchParams();
        hfSearch({
            query,
            author,
            sort,
            mlxActive: hfState.discoveryScopeMlx,
            ggufActive: hfState.discoveryScopeGguf,
            hfSort: hfState.discoverySort,
            limit: 20,
            container: resultsContainer,
            filelistContainer,
            quickpicksContainer: quickpicks,
            discoverPillsContainerId: 'mm-hf-discover-pills',
            onOpenCardPanel: openCardPanel,
            onSelectModel: (m) => onHfModelSelected(m, filelistContainer, downloadPanel),
            workloadProfile,
        });
    };

    searchInput.addEventListener('input', () => {
        clearTimeout(searchTimer);
        searchTimer = setTimeout(doSearch, 3000);
    });

    searchInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            clearTimeout(searchTimer);
            doSearch();
        }
    });

    // Phase 8B1: create discovery scope selector and sort selector
    // Platform-smart defaults: macOS → MLX+GGUF; Win/Linux → GGUF
    const isMac = typeof navigator !== 'undefined' && navigator.platform?.includes('Mac');
    const defaultMlx = isMac;
    const defaultGguf = true;

    hfState.discoveryScopeMlx = defaultMlx;
    hfState.discoveryScopeGguf = defaultGguf;

    const scopeContainer = document.getElementById('mm-hf-scope-container');
    const sortContainer = document.getElementById('mm-hf-sort-container');

    if (scopeContainer) {
        // Set initial state via dataset for additive toggles
        scopeContainer.dataset.hfScopeMlx = defaultMlx ? '1' : '';
        scopeContainer.dataset.hfScopeGguf = '1';
        scopeContainer.dataset.hfScopeAll = '';

        hfCreateScopeSelector({
            container: scopeContainer,
            onChange: (mlxActive, ggufActive, allActive) => {
                hfState.discoveryScopeMlx = mlxActive || allActive;
                hfState.discoveryScopeGguf = ggufActive || allActive;
                clearTimeout(searchTimer);
                searchTimer = setTimeout(doSearch, 200);
            },
        });
    }

    if (sortContainer) {
        hfCreateSortSelector({
            container: sortContainer,
            defaultSort: HF_SORT.DOWNLOADS,
            onChange: (sort) => {
                hfState.discoverySort = sort;
                clearTimeout(searchTimer);
                searchTimer = setTimeout(doSearch, 200);
            },
        });
    }

    // Settings link from warning
    const settingsBtn = document.getElementById('mm-hf-dlp-open-settings');
    if (settingsBtn) {
        settingsBtn.addEventListener('click', () => {
            Router.navigate('/settings');
        });
    }
}

async function onHfModelSelected(model, filelistContainer, downloadPanel) {
    // Handle both raw model objects and selection payloads
    const repoId = model.repoId || model.id || '';
    const paramB = model.param_b || model._raw?.param_b || 0;

    hfState.selectedRepoId = repoId;
    hfState.selectedFile = null;
    hfState.paramB = paramB;
    hfState.mmprojFiles = [];
    hfState.mmprojPath = '';
    hfState.mmprojRepoId = '';
    hfState.mmprojBytes = 0;
    hfHideDownloadPanel(downloadPanel);
    hideQuantAdvisor();
    hideMmprojSection();
    hideVramPanel();
    hideCtxTrainWarning();

    // Hide the generic hardware info card — we'll show real estimates below
    hideHardwareInfoCard();

    // Show selected model info
    showSelectedModel(repoId, model);

    await hfListFiles({
        repoId,
        container: filelistContainer,
        vramGb: cachedVram > 0 ? cachedVram / (1024 * 1024 * 1024) : 0,
        onSelectFile: (file, repoId) => onHfFileSelected(file, repoId, downloadPanel),
    });

    // Trigger quant advisor if we have param count
    if (hfState.paramB > 0) {
        triggerQuantAdvisor();
    }
}

async function onHfFileSelected(file, repoId, downloadPanel) {
    hfState.selectedFile = file;
    hfState.modelBytes = Number(file.size) || 0;

    // Update selected model display with file info
    const nameEl = document.getElementById('mm-selected-model-name');
    const metaEl = document.getElementById('mm-selected-model-meta');
    if (nameEl) nameEl.textContent = (file.path || file.name || '').split('/').pop() || '';
    if (metaEl) {
        const parts = [];
        if (repoId) parts.push(repoId);
        if (file.size) parts.push(formatBytes(file.size));
        if (file.label) parts.push(file.label);
        metaEl.textContent = parts.join(' · ');
    }

    // Hide quant advisor — VRAM panel will show the precise estimate
    hideQuantAdvisor();

    // Show download panel
    await hfShowDownloadPanel(downloadPanel, file.path || file.name || '');

    // Update VRAM panel with live estimate
    if (hfState.paramB > 0 || hfState.modelBytes > 0) {
        scheduleVramUpdate(file);
    }

    // Check for mmproj companion in this repo
    await detectMmprojCompanion(repoId);

    // Wire download button with double-click guard
    const newBtn = document.getElementById('mm-hf-dlp-download-btn');
    if (newBtn) {
        newBtn.onclick = null;
    }
    if (newBtn) {
        newBtn.disabled = false;
        let clickLocked = false;
        newBtn.onclick = async () => {
            if (clickLocked) return;
            clickLocked = true;
            const modelFilePath = file.path || file.name;
            const started = await hfStartDownload({
                repoId,
                filePath: modelFilePath,
                panelEl: downloadPanel,
                onComplete: (downloadId, localPath) => {
                    hfState.currentDownloadIds.add(downloadId);
                    // Refresh library tab
                    invalidateModelInventory();
                    loadModels({ refresh: true });
                },
                onValidationError: (msg) => {
                    // hfStartDownload now shows its own toast for most cases.
                    // This handler is a fallback for non-standard messages.
                    const e = (msg || '').toLowerCase();
                    if (!e.includes('already downloading') &&
                        !e.includes('already exists') &&
                        !e.includes('too many downloads') &&
                        !e.includes('download started')) {
                        showToast(msg || 'Download failed', 'error');
                    }
                },
            });
            if (!started || !hfState.mmprojPath) {
                clickLocked = false;
                return;
            }

            const companion = await hfStartCompanionDownload({
                repoId: hfState.mmprojRepoId || repoId,
                filePath: hfState.mmprojPath,
                saveAs: deriveMmprojSaveName(modelFilePath, hfState.mmprojPath),
            });
            if (!companion) {
                showToast('Model download started, but the mmproj download could not start.', 'error');
            }
            // Unlock only after progress is actively showing (polling started).
            // hfStartDownload sets state to 'progress' internally; keep button
            // disabled visually via the progress panel, so clickLocked is only
            // to prevent a second click before that transition.
            clickLocked = false;
        };
    }

    // Wire cancel button
    const newCancel = document.getElementById('mm-hf-dlp-cancel-btn');
    if (newCancel) {
        newCancel.onclick = null;
    }
    if (newCancel) {
        newCancel.onclick = () => {
            if (hfState.currentDownloadIds.size > 0) {
                const downloadId = [...hfState.currentDownloadIds][0];
                hfCancelDownload({
                    downloadId,
                    panelEl: downloadPanel,
                });
                hfState.currentDownloadIds.delete(downloadId);
            }
        };
    }
}

// ── Selected model display (wizard-style) ────────────────────────────────────

function showSelectedModel(repoId, model) {
    const el = document.getElementById('mm-selected-model');
    const nameEl = document.getElementById('mm-selected-model-name');
    const metaEl = document.getElementById('mm-selected-model-meta');
    if (!el) return;

    el.style.display = '';
    if (nameEl) nameEl.textContent = repoId;
    if (metaEl) {
        const parts = [];
        if (model.param_b > 0) parts.push(formatParams(model.param_b));
        if (model.downloads > 0) parts.push(model.downloads + ' downloads');
        metaEl.textContent = parts.join(' · ');
    }
}

// ── Quant advisor (wizard-style) ─────────────────────────────────────────────

let quantAdvisorDebounce = null;

function triggerQuantAdvisor() {
    if (quantAdvisorDebounce) clearTimeout(quantAdvisorDebounce);
    quantAdvisorDebounce = setTimeout(loadQuantAdvisor, 200);
}

function hideQuantAdvisor() {
    const el = document.getElementById('mm-quant-advisor');
    if (el) el.style.display = 'none';
}

function hideHardwareInfoCard() {
    const el = document.getElementById('mm-hw-info');
    if (el) el.style.display = 'none';
}

async function loadQuantAdvisor() {
    const paramB = hfState.paramB;
    if (!paramB || paramB <= 0) return;

    // Unified memory handling: use current_safe_availability_bytes instead of total
    let availVram = cachedVram || 0;
    if (cachedUnified) {
        const snapshot = await fetchMemoryAvailability();
        if (snapshot && snapshot.current_safe_availability_bytes > 0) {
            availVram = snapshot.current_safe_availability_bytes;
        }
    }
    if (!availVram) return;

    try {
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };

        const body = {
            param_b: paramB,
            model_name: hfState.selectedRepoId || '',
            available_vram_bytes: availVram,
        };

        const resp = await fetch('/api/vram/quant-compare', { method: 'POST', headers, body: JSON.stringify(body) });
        if (!resp.ok) return;
        const data = await resp.json();
        if (!data.ok || !data.quants) return;

        renderQuantAdvisor(data.quants, availVram);
    } catch {
        // ignore
    }
}

function renderQuantAdvisor(quants, availVram) {
    const panel = document.getElementById('mm-quant-advisor');
    const tableEl = document.getElementById('mm-quant-advisor-table');
    const subtitleEl = document.getElementById('mm-quant-advisor-subtitle');
    if (!panel || !tableEl) return;
    if (!quants || quants.length === 0) { panel.style.display = 'none'; return; }

    const availGb = Math.round(availVram / (1024 ** 3));
    if (subtitleEl) subtitleEl.textContent = `Estimated VRAM available: ${availGb} GB`;

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
            badge.textContent = '\u2605 Rec';
            badge.style.marginLeft = '6px';
            nameTd.appendChild(badge);
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

        // Max ctx q8_0
        const ctxQ8Td = tr.insertCell();
        ctxQ8Td.className = 'qa-ctx';
        if (q.max_ctx_q8 > 0) {
            ctxQ8Td.textContent = formatCtx(q.max_ctx_q8);
            ctxQ8Td.classList.add('qa-ctx-q8');
        } else {
            ctxQ8Td.textContent = '\u2014'; ctxQ8Td.classList.add('qa-ctx-na');
        }

        // Max ctx q4_0
        const ctxQ4Td = tr.insertCell();
        ctxQ4Td.className = 'qa-ctx';
        if (q.max_ctx_q4 > 0) {
            ctxQ4Td.textContent = formatCtx(q.max_ctx_q4);
            ctxQ4Td.classList.add('qa-ctx-q4');
        } else {
            ctxQ4Td.textContent = '\u2014'; ctxQ4Td.classList.add('qa-ctx-na');
        }

        // Quality badge
        const qualTd = tr.insertCell();
        const qualBadge = document.createElement('span');
        const qClass = 'qa-quality-' + (q.quality || '').toLowerCase();
        qualBadge.className = `qa-quality-badge ${qClass}`;
        qualBadge.textContent = q.quality_label || q.quality;
        qualTd.appendChild(qualBadge);
    }

    tableEl.innerHTML = '';
    tableEl.appendChild(table);
    panel.style.display = '';
}

// ── VRAM estimation (wizard-style) ───────────────────────────────────────────

let vramDebounce = null;

function scheduleVramUpdate(file) {
    if (vramDebounce) clearTimeout(vramDebounce);
    vramDebounce = setTimeout(() => updateVramDisplay(file), 250);
}

function hideVramPanel() {
    const el = document.getElementById('mm-vram-panel');
    if (el) el.style.display = 'none';
}

async function fetchGpuVram() {
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const resp = await fetch('/metrics/gpu', { headers });
        if (!resp.ok) return;
        const data = await resp.json();
        let totalVram = 0;
        const gpus = Array.isArray(data) ? data : (data.gpus ? data.gpus : Object.values(data));
        for (const g of gpus) {
            const t = g.vram_total_mb || g.total_mb || g.total_memory_mb || g.vram_total || 0;
            totalVram += t * 1024 * 1024;
            const id = `${g.name || ''} ${g.vendor || ''} ${g.backend || ''}`;
            if (/apple|metal/i.test(id) || g.metal_gpu_limit_mb != null) cachedUnified = true;
        }
        if (totalVram > 0 && !cachedUnified) cachedVram = totalVram;
    } catch {
        // ignore
    }
}

async function fetchSystemRam() {
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const resp = await fetch('/metrics/system', { headers });
        if (!resp.ok) return;
        const d = await resp.json();
        cachedRamTotal = (d.ram_total_gb || 0) * 1024 * 1024 * 1024;
    } catch {
        // ignore
    }
}

/// Fetch MemoryAvailabilitySnapshot from the single backend source of truth.
/// Uses current_safe_availability_bytes for fit determination (never total unified memory).
async function fetchMemoryAvailability() {
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const resp = await fetch('/api/memory-availability', { headers });
        if (!resp.ok) return null;
        const data = await resp.json();
        if (!data.ok || !data.snapshot) return null;
        return data.snapshot;
    } catch {
        return null;
    }
}

async function updateVramDisplay(file) {
    const panel = document.getElementById('mm-vram-panel');
    if (!panel) return;

    // Phase 5b Part C: On unified memory (Apple Silicon), use current_safe_availability_bytes
    // from the MemoryAvailabilitySnapshot, NOT total unified memory.
    let availVram = cachedVram || 0;
    if (cachedUnified) {
        const snapshot = await fetchMemoryAvailability();
        if (snapshot && snapshot.current_safe_availability_bytes > 0) {
            availVram = snapshot.current_safe_availability_bytes;
        }
    }
    if (!availVram) {
        panel.style.display = 'none';
        return;
    }

    // Use model bytes from file or estimate from paramB
    let modelBytes = hfState.modelBytes;
    if (!modelBytes && hfState.paramB > 0) {
        const fname = (file?.path || file?.name || '').toLowerCase();
        const quant = guessQuantFromName(fname);
        const BPW = { q8_0: 8.5, q6_k: 6.5625, q5_k_m: 5.69, q4_k_m: 4.85, iq4_xs: 4.25, q3_k_m: 3.875, q2_k: 2.625, iq2_xxs: 2.0625, f16: 16, bf16: 16 };
        const bpw = BPW[quant] ?? 4.85;
        modelBytes = Math.round(hfState.paramB * 1e9 * bpw / 8);
    }
    if (!modelBytes) {
        panel.style.display = 'none';
        return;
    }

    // Estimate via the backend so the preview uses the SAME math as the spawn wizard /
    // preset editor. Pre-download, the backend range-fetches the GGUF header from HuggingFace
    // (hf_repo_id + hf_file_path) to introspect the model's real architecture — no name
    // guessing, no divergent client-side formula. We assume a modest preview context.
    const PREVIEW_CTX = 16384;
    const mmprojBytes = hfState.mmprojBytes || 0;
    let data;
    try {
        // Builder item 6: canonical body builder for cross-surface equality.
        const body = buildEstimateBody({
            backend: 'llama_cpp',
            hf_repo_id: hfState.selectedRepoId || null,
            hf_file_path: file?.path || file?.name || null,
            model_size_bytes: modelBytes,
            n_ctx: PREVIEW_CTX,
            parallel_slots: 1,
            ubatch_size: 512,
            ctk: 'q8_0',
            ctv: 'q8_0',
            available_vram_bytes: availVram,
            is_unified_memory: cachedUnified,
            mmproj_bytes: mmprojBytes,
        });
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };
        const resp = await fetch('/api/vram-estimate', { method: 'POST', headers, body: JSON.stringify(body) });
        if (resp.ok) data = await resp.json();
    } catch {
        // network error — fall through to hide
    }
    if (!data || !data.ok) {
        panel.style.display = 'none';
        return;
    }

    // Fold MTP + linear-attention state into the displayed "overhead" segment.
    const weightsBytes = data.weights_bytes || 0;
    const kvEstimate = data.kv_cache_bytes || 0;
    const overhead = (data.overhead_bytes || 0) + (data.mtp_bytes || 0) + (data.linear_attn_state_bytes || 0);
    const total = data.total_bytes || (weightsBytes + kvEstimate + overhead + mmprojBytes);
    const free = availVram - total;

    // Show panel
    panel.style.display = '';

    // Update header
    const labelEl = document.getElementById('mm-vram-panel-label');
    const totalEl = document.getElementById('mm-vram-panel-total');
    if (labelEl) labelEl.textContent = `VRAM @ ${PREVIEW_CTX / 1024}K ctx`;
    if (totalEl) totalEl.textContent = formatVramTotal(availVram) + ' total';

    // Update bar
    const denom = availVram > 0 ? availVram : total;
    const weightsPct = weightsBytes / denom;
    const kvPct = kvEstimate / denom;
    const mmprojPct = mmprojBytes / denom;
    const overheadPct = overhead / denom;
    const freePct = Math.max(0, free) / denom;

    setSegWidth(document.getElementById('mm-vseg-weights'), weightsPct);
    setSegWidth(document.getElementById('mm-vseg-kv'), kvPct);
    setSegWidth(document.getElementById('mm-vseg-mmproj'), mmprojPct);
    setSegWidth(document.getElementById('mm-vseg-overhead'), overheadPct);
    setSegWidth(document.getElementById('mm-vseg-free'), freePct);

    const barEl = document.getElementById('mm-vram-bar');
    if (barEl) {
        const ratio = availVram > 0 ? total / availVram : 0;
        barEl.classList.toggle('tight', ratio >= 0.88 && ratio < 1.0);
        barEl.classList.toggle('over', ratio >= 1.0);
    }

    const freeSeg = document.getElementById('mm-vseg-free');
    if (freeSeg) freeSeg.classList.toggle('over-budget', free < 0);

    // Update legend
    const weightsLabel = document.getElementById('mm-vleg-weights-label');
    const kvLabel = document.getElementById('mm-vleg-kv-label');
    const mmprojItem = document.getElementById('mm-vleg-mmproj');
    const mmprojLabel = document.getElementById('mm-vleg-mmproj-label');
    const overheadLabel = document.getElementById('mm-vleg-overhead-label');
    const freeLabel = document.getElementById('mm-vleg-free-label');
    const freeDot = document.querySelector('#mm-vleg-free .vram-legend-dot-free');

    if (weightsLabel) weightsLabel.textContent = 'Weights ' + formatGB(weightsBytes);
    if (kvLabel) kvLabel.textContent = 'KV ' + formatGB(kvEstimate);

    if (mmprojBytes > 0) {
        if (mmprojItem) mmprojItem.style.display = '';
        if (mmprojLabel) mmprojLabel.textContent = 'mmproj ' + formatGB(mmprojBytes);
    } else {
        if (mmprojItem) mmprojItem.style.display = 'none';
    }

    if (overheadLabel) overheadLabel.textContent = 'Overhead ' + formatGB(overhead);

    if (freeLabel) {
        const freeAbs = Math.abs(free);
        freeLabel.textContent = free >= 0 ? 'Free ' + formatGB(free) : 'Over ' + formatGB(freeAbs);
    }
    if (freeDot) freeDot.style.background = free >= 0 ? '' : 'var(--color-error)';

    // Update ctx train warning (if introspection revealed n_ctx_train)
    updateCtxTrainWarning();
}

// ── Context train warning (wizard-style) ─────────────────────────────────────

function hideCtxTrainWarning() {
    const el = document.getElementById('mm-ctx-train-warning');
    if (el) el.style.display = 'none';
}

function updateCtxTrainWarning() {
    const el = document.getElementById('mm-ctx-train-warning');
    if (!el) return;
    const nCtxTrain = hfState.nCtxTrain;
    if (!nCtxTrain) {
        el.style.display = 'none';
        return;
    }
    // Show warning if we're looking at a model with a known training limit
    // and the user might set context beyond it
    const fmtK = n => n >= 1024 ? Math.round(n / 1024) + 'k' : String(n);
    el.textContent = '';
    const strong = document.createElement('strong');
    strong.textContent = 'Training context: ' + fmtK(nCtxTrain) + ' tokens';
    el.appendChild(strong);
    el.appendChild(document.createTextNode(
        ' — setting context beyond this may degrade quality. Use --rope-scaling yarn if needed.'
    ));
    el.className = 'ctx-fit-warning';
    el.style.display = '';
}

// ── mmproj companion detection (wizard-style) ────────────────────────────────

function deriveMmprojSaveName(modelPath, mmprojPath) {
    const modelBase = (modelPath.split('/').pop() || modelPath).replace(/\.gguf$/i, '');
    const stem = modelBase.replace(/-?(Q\d[\w.]*|IQ\d[\w.]*|BF16|F16)$/i, '');
    const mmprojBase = mmprojPath.split('/').pop() || mmprojPath;
    return `${stem}-${mmprojBase}`;
}

async function detectMmprojCompanion(repoId) {
    const section = document.getElementById('mm-mmproj-section');
    const content = document.getElementById('mm-mmproj-content');
    if (!section || !content) return;

    if (!repoId) {
        section.style.display = 'none';
        return;
    }

    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const resp = await fetch('/api/hf/files', {
            method: 'POST',
            headers: { ...headers, 'Content-Type': 'application/json' },
            body: JSON.stringify({ repo_id: repoId }),
        });
        if (!resp.ok) { section.style.display = 'none'; return; }
        const data = await resp.json();
        const files = data.files || [];
        const mmprojFiles = files.filter(f => f.is_mmproj);

        if (!mmprojFiles.length) {
            section.style.display = 'none';
            hfState.mmprojFiles = [];
            hfState.mmprojBytes = 0;
            return;
        }

        hfState.mmprojFiles = mmprojFiles;
        section.style.display = '';
        const recommendedMmproj = getRecommendedMmproj(mmprojFiles);

        // Render mmproj options
        content.innerHTML = '';

        // Checkbox to enable mmproj
        const checkLabel = document.createElement('label');
        checkLabel.className = 'hw-toggle-label';
        const check = document.createElement('input');
        check.type = 'checkbox';
        check.checked = !!hfState.mmprojPath;
        checkLabel.appendChild(check);
        const span = document.createElement('span');
        span.textContent = 'Include vision projector (mmproj)';
        checkLabel.appendChild(span);
        content.appendChild(checkLabel);

        // Select for mmproj file
        const select = document.createElement('select');
        select.className = 'hw-mmproj-select';
        select.style.marginLeft = '22px';
        select.style.display = check.checked ? '' : 'none';

        const noneOpt = document.createElement('option');
        noneOpt.value = '';
        noneOpt.textContent = '(none — text-only)';
        select.appendChild(noneOpt);

        mmprojFiles.forEach(f => {
            const fpath = f.path || f.name || '';
            const fname = fpath.split('/').pop();
            const opt = document.createElement('option');
            opt.value = fpath;
            opt.dataset.repoId = f.repo_id || repoId;
            const sizeStr = f.size ? ' · ' + formatBytes(f.size) : '';
            opt.textContent = fname + sizeStr + (f.is_recommended_mmproj ? ' · Recommended' : '');
            if (f.is_recommended_mmproj) {
                opt.title = f.mmproj_recommendation || 'Preferred projector format for this model family';
            }
            if (fpath === hfState.mmprojPath) opt.selected = true;
            select.appendChild(opt);
        });

        select.addEventListener('change', () => {
            const fpath = select.value;
            hfState.mmprojPath = fpath;
            const f = mmprojFiles.find(x => (x.path || x.name) === fpath);
            hfState.mmprojRepoId = f?.repo_id || repoId;
            hfState.mmprojBytes = f?.size ? Number(f.size) : 0;
            scheduleVramUpdate(hfState.selectedFile);
        });

        check.addEventListener('change', () => {
            select.style.display = check.checked ? '' : 'none';
            if (!check.checked) {
                hfState.mmprojPath = '';
                hfState.mmprojRepoId = '';
                hfState.mmprojBytes = 0;
            } else if (!select.value && mmprojFiles.length) {
                const preferred = recommendedMmproj || mmprojFiles[0];
                select.value = preferred.path || preferred.name || '';
                hfState.mmprojPath = select.value;
                hfState.mmprojRepoId = preferred.repo_id || repoId;
                hfState.mmprojBytes = preferred.size ? Number(preferred.size) : 0;
            }
            scheduleVramUpdate(hfState.selectedFile);
        });

        content.appendChild(select);

        // Hint
        const hint = document.createElement('div');
        hint.className = 'mm-hint';
        const sizeFile = recommendedMmproj || mmprojFiles[0];
        const recommendation = recommendedMmproj?.mmproj_recommendation
            ? recommendedMmproj.mmproj_recommendation + '. '
            : '';
        hint.textContent = recommendation + 'Adds ~' + (sizeFile?.size ? formatBytes(sizeFile.size) : '0.5–1.5 GB') + ' VRAM. Required for multimodal inference.';
        content.appendChild(hint);

    } catch {
        section.style.display = 'none';
    }
}

function hideMmprojSection() {
    const el = document.getElementById('mm-mmproj-section');
    if (el) el.style.display = 'none';
}

// ── Utilities ────────────────────────────────────────────────────────────────

function formatBytes(bytes) {
    if (!bytes) return '';
    const b = Number(bytes);
    if (!isFinite(b)) return '';
    if (b >= 1024 ** 3) return (b / (1024 ** 3)).toFixed(1) + ' GiB';
    if (b >= 1024 ** 2) return (b / (1024 ** 2)).toFixed(1) + ' MiB';
    if (b >= 1024) return (b / 1024).toFixed(0) + ' KiB';
    return b + ' B';
}

function formatParams(paramB) {
    if (paramB >= 1000) return (paramB / 1000).toFixed(1) + 'T';
    return paramB + 'B';
}

function formatCtx(n) {
    if (n >= 1024) return Math.round(n / 1024) + 'k';
    return String(n);
}

function formatVramTotal(bytes) {
    const gb = bytes / (1024 ** 3);
    if (gb >= 100) return Math.round(gb) + ' GB';
    return gb.toFixed(1) + ' GB';
}

function formatGB(bytes) {
    if (!bytes || !isFinite(bytes)) return '0 MB';
    const gb = bytes / (1024 ** 3);
    if (gb >= 1) return gb.toFixed(1) + ' GB';
    return Math.round(bytes / (1024 ** 2)) + ' MB';
}

function setSegWidth(el, ratio) {
    if (!el) return;
    const pct = Math.max(0, Math.min(1, ratio)) * 100;
    el.style.width = pct + '%';
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
    return 'q4_k_m';
}
