// ── Models ────────────────────────────────────────────────────────────────────
// Models modal: open, close, load, refresh, delete, and HF download tab.

import { sessionState } from '../core/app-state.js';
import { escapeHtml } from '../core/format.js';
import { showToast } from './toast.js';
import {
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

const PREFS_KEY = 'llama-monitor-models-prefs';

const ICON_LIST_VIEW = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="11" height="11"><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>';
const ICON_CARDS_VIEW = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="11" height="11"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></svg>';

const KNOWN_TAGS = ['coding', 'roleplay', 'general', 'art', 'fast', 'default'];

let initialized = false;

// State for the HF download tab
let hfState = {
    selectedRepoId: null,
    selectedFile: null,
    currentDownloadId: null,
    initialized: false,
    // Wizard-like state
    paramB: 0,
    modelBytes: 0,
    nCtxTrain: 0,
    mmprojFiles: [],
    mmprojPath: '',
    mmprojBytes: 0,
    // Active filters (mirrors Quick Start behavior)
    activeAuthor: null,          // e.g. "bartowski"
    activeDiscoverQuery: null,   // e.g. "qwen3" from a discover pill
};

// Cached hardware
let cachedVram = 0;
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
        quantFilters: {},
        tagFilter: '',
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
    loadModels();
}

export function openModelsModalForSwitch() {
    openModelsModal();
}

function closeModelsModal() {
    document.getElementById('models-modal')?.classList.remove('open');
}

export { closeModelsModal };

async function loadModels() {
    const grid = document.getElementById('models-list');
    const summary = document.getElementById('models-summary');
    const tabCount = document.getElementById('models-tab-count');
    const dirLabel = document.getElementById('models-dir-label');
    if (!grid) return;

    if (summary) summary.textContent = 'Loading...';
    grid.innerHTML = '<div class="mm-loading">Scanning...</div>';

    try {
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

        const resp = await fetch('/api/models', {
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        const models = await resp.json();

        const count = models.length;
        if (tabCount) tabCount.textContent = count ? String(count) : '';

        // Build toolbar with models list for quant chips
        buildLibraryToolbar(models);

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
    const f = (m.filename || '').toLowerCase();
    return f.includes('mmproj') || f.includes('.mmproj.') || f.includes('-mmproj-');
}

function applyFilters(models) {
    return models.filter(m => {
        // mmproj vs main
        const mmproj = isMmproj(m);
        if (mmproj && !prefs.showMmproj) return false;
        if (!mmproj && !prefs.showMain) return false;

        // split
        if (m.is_split && !prefs.showSplit) return false;

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
    const name = m.model_name || m.filename;
    const quant = m.quant_type || 'unknown';
    const size = m.size_display || '';
    const vramEst = m.vram_est_gb != null
        ? (typeof m.vram_estimate_display === 'string'
            ? m.vram_estimate_display
            : (m.vram_est_gb % 1 === 0 ? m.vram_est_gb + ' GB' : m.vram_est_gb.toFixed(1) + ' GB'))
        : '';
    const vramPct = m.vram_percent != null ? Math.min(100, m.vram_percent) : null;
    const isSplit = m.is_split;
    const mmproj = isMmproj(m);
    const tags = Array.isArray(m.tags) ? m.tags : [];
    const relatedPresets = mmproj ? [] : findPresetsForModel(m);

    const card = document.createElement('div');
    card.className = 'mm-model-card';
    if (mmproj) card.classList.add('mm-model-card--mmproj');

    // Top row: name + quant badge
    const top = document.createElement('div');
    top.className = 'mm-card-top';

    const nameEl = document.createElement('div');
    nameEl.className = 'mm-card-name';
    nameEl.title = m.path || '';
    nameEl.textContent = name;
    top.appendChild(nameEl);

    // Only show quant badge when it's meaningful — skip for mmproj files with no known quant
    if (!(mmproj && quant === 'unknown')) {
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

    if (mmproj) {
        const mmBadge = document.createElement('span');
        mmBadge.className = 'mm-quant-badge mm-mmproj-badge';
        mmBadge.textContent = 'mmproj';
        top.appendChild(mmBadge);
    }

    card.appendChild(top);

    // Meta row: filename
    const meta = document.createElement('div');
    meta.className = 'mm-card-meta';
    meta.textContent = m.filename || '';
    card.appendChild(meta);

    // Stats row: size, VRAM, and tag pills all in one row
    if (size || vramEst || tags.length > 0) {
        const stats = document.createElement('div');
        stats.className = 'mm-card-stats';
        if (size) {
            const sizeEl = document.createElement('span');
            sizeEl.className = 'mm-stat';
            sizeEl.textContent = size;
            stats.appendChild(sizeEl);
        }
        if (vramEst) {
            const vramEl = document.createElement('span');
            vramEl.className = 'mm-stat mm-stat-vram';
            vramEl.textContent = 'VRAM ~' + vramEst;
            stats.appendChild(vramEl);
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

    // VRAM bar
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

    if (!mmproj) {
        const useBtn = document.createElement('button');
        useBtn.type = 'button';
        useBtn.className = 'mm-action-btn';
        useBtn.title = relatedPresets.length ? 'Build a new preset from this model' : 'Open this model in the spawn wizard';
        useBtn.textContent = relatedPresets.length ? 'New Preset' : 'Use in Wizard';
        useBtn.addEventListener('click', () => {
            closeModelsModal();
            import('./spawn-wizard.js').then(({ openSpawnWizard }) => {
                openSpawnWizard({ localPath: m.path || '', localModel: m });
            });
        });
        actions.appendChild(useBtn);

        if (relatedPresets.length) {
            const editBtn = document.createElement('button');
            editBtn.type = 'button';
            editBtn.className = 'mm-action-btn';
            editBtn.title = 'Edit the saved preset that already uses this model';
            editBtn.textContent = 'Edit Preset';
            editBtn.addEventListener('click', () => {
                const preset = relatedPresets[0];
                const select = document.getElementById('preset-select');
                if (select) select.value = preset.id;
                closeModelsModal();
                import('./presets.js').then(({ openPresetModal }) => openPresetModal('edit'));
            });
            actions.appendChild(editBtn);
        }
    }

    const copyBtn = document.createElement('button');
    copyBtn.type = 'button';
    copyBtn.className = 'mm-action-btn mm-action-copy';
    copyBtn.title = 'Copy path';
    copyBtn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="13" height="13"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg> Copy Path';
    const pathToCopy = m.path || '';
    copyBtn.addEventListener('click', () => {
        navigator.clipboard.writeText(pathToCopy).then(() => {
            showToast('Path copied', 'success');
        }).catch(() => {
            showToast('Copy failed', 'error');
        });
    });
    actions.appendChild(copyBtn);

    const deleteBtn = document.createElement('button');
    deleteBtn.type = 'button';
    deleteBtn.className = 'mm-action-btn mm-action-delete';
    deleteBtn.title = 'Delete model file';
    deleteBtn.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="13" height="13"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>';
    deleteBtn.addEventListener('click', () => deleteModel(m.path, m.filename || name));
    actions.appendChild(deleteBtn);

    const tagBtn = document.createElement('button');
    tagBtn.type = 'button';
    tagBtn.className = 'mm-action-btn mm-action-tags';
    tagBtn.title = 'Add tag';
    tagBtn.textContent = '+';
    tagBtn.addEventListener('click', () => {
        openTagPicker(m.path, tags);
    });
    actions.appendChild(tagBtn);

    card.appendChild(actions);
    return card;
}

function findPresetsForModel(model) {
    const path = model.path || '';
    if (!path) return [];
    return (sessionState.presets || []).filter(preset => preset.model_path === path);
}

function formatPresetSummaryLine(preset) {
    const parts = [preset.name || 'Unnamed preset'];
    if (preset.context_size) parts.push(`${Math.round(preset.context_size / 1024)}k ctx`);
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
    if (!confirm('Delete ' + filename + '?\n\nThis will permanently remove the file from disk.')) return;

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
        await loadModels();
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
    await loadModels();
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
    await loadModels();
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
                if (ok) loadModels();
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

function buildLibraryToolbar(models) {
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

    // Filters button (collapsible on small screens)
    const filtersWrap = document.createElement('div');
    filtersWrap.className = 'mm-lib-filters';

    const filtersBtn = document.createElement('button');
    filtersBtn.type = 'button';
    filtersBtn.className = 'mm-lib-btn mm-lib-btn--labeled';
    filtersBtn.id = 'mm-lib-filters-toggle';
    filtersBtn.title = 'Filter models by type, quantization, or tag';
    const hasActiveFilters = !prefs.showMmproj || !prefs.showMain || !prefs.showSplit ||
        Object.values(prefs.quantFilters).some(v => v === false) || !!prefs.tagFilter;
    if (hasActiveFilters) filtersBtn.classList.add('mm-lib-btn--active');
    // eslint-disable-next-line no-unsanitized/property -- static SVG, no user data
    filtersBtn.innerHTML =
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="11" height="11"><line x1="4" y1="21" x2="4" y2="14"/><line x1="4" y1="10" x2="4" y2="3"/><line x1="12" y1="21" x2="12" y2="12"/><line x1="12" y1="8" x2="12" y2="3"/><line x1="20" y1="21" x2="20" y2="16"/><line x1="20" y1="12" x2="20" y2="3"/><line x1="1" y1="14" x2="7" y2="14"/><line x1="9" y1="8" x2="15" y2="8"/><line x1="17" y1="16" x2="23" y2="16"/></svg>'
        + '<span>Filter' + (hasActiveFilters ? ' •' : '') + '</span>';

    const filtersPanel = document.createElement('div');
    filtersPanel.className = 'mm-lib-filters-panel';
    filtersPanel.id = 'mm-lib-filters-panel';

    // Type filters
    const typeRow = document.createElement('div');
    typeRow.className = 'mm-lib-filter-row';

    const typeLabel = document.createElement('span');
    typeLabel.className = 'mm-lib-filter-label';
    typeLabel.textContent = 'Type';

    const mmprojChip = createChip('mmproj', prefs.showMmproj);
    const mainChip = createChip('Main', prefs.showMain);
    const splitChip = createChip('Split', prefs.showSplit);

    mmprojChip.addEventListener('click', () => {
        prefs.showMmproj = !prefs.showMmproj;
        mmprojChip.classList.toggle('active', prefs.showMmproj);
        savePrefs();
        loadModels();
    });
    mainChip.addEventListener('click', () => {
        prefs.showMain = !prefs.showMain;
        mainChip.classList.toggle('active', prefs.showMain);
        savePrefs();
        loadModels();
    });
    splitChip.addEventListener('click', () => {
        prefs.showSplit = !prefs.showSplit;
        splitChip.classList.toggle('active', prefs.showSplit);
        savePrefs();
        loadModels();
    });

    typeRow.appendChild(typeLabel);
    typeRow.appendChild(mmprojChip);
    typeRow.appendChild(mainChip);
    typeRow.appendChild(splitChip);
    filtersPanel.appendChild(typeRow);

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
                savePrefs();
                loadModels();
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
            prefs.tagFilter = '';
            savePrefs();
            loadModels();
        });
        tagRow.appendChild(noneChip);

        allTagArr.forEach(tag => {
            const chip = createChip(tag, prefs.tagFilter === tag);
            chip.classList.toggle('active', prefs.tagFilter === tag);
            chip.addEventListener('click', () => {
                prefs.tagFilter = prefs.tagFilter === tag ? '' : tag;
                savePrefs();
                loadModels();
            });
            tagRow.appendChild(chip);
        });
    }

    tagRow.appendChild(tagLabel);
    filtersPanel.appendChild(tagRow);

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
        if (o.value === prefs.sort) opt.selected = true;
        sortSelect.appendChild(opt);
    });

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

function createChip(label, active) {
    const chip = document.createElement('button');
    chip.type = 'button';
    chip.className = 'mm-lib-chip' + (active ? ' active' : '');
    chip.textContent = label;
    return chip;
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initModels() {
    if (initialized) return;
    initialized = true;

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
            document.querySelectorAll('#models-modal .mm-tab').forEach(t => t.classList.remove('active'));
            document.querySelectorAll('#models-modal .mm-tab-panel').forEach(p => p.classList.remove('active'));
            tab.classList.add('active');
            const panel = document.querySelector('#models-modal .mm-tab-panel[data-tab="' + target + '"]');
            if (panel) panel.classList.add('active');
            if (target === 'download') {
                initHfDownloadTab();
            }
        });
    });

    // Configure settings link
    const settingsLink = document.getElementById('models-open-settings-link');
    if (settingsLink) {
        settingsLink.addEventListener('click', () => {
            closeModelsModal();
            if (typeof window.openSettingsModal === 'function') window.openSettingsModal('models');
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
    const sortSelect = document.getElementById('mm-hf-sort');
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
        const sort = sortSelect?.value || 'downloads';
        const typedQuery = (searchInput.value || '').trim();
        const query = typedQuery || hfState.activeDiscoverQuery || '';
        const author = (typedQuery ? hfState.activeAuthor : (hfState.activeAuthor || null));
        return { query: query || undefined, author: author || undefined, sort };
    };

    // Render discover pills
    hfRenderDiscoverPills({
        container: discoverPills,
        quickpicksContainer: quickpicks,
        onPillClick: (cat) => {
            const sort = cat.params.query
                ? (sortSelect?.value || cat.params.sort)
                : cat.params.sort;
            // Track active discover query so sort changes still work
            hfState.activeDiscoverQuery = cat.params.query || null;
            hfState.activeAuthor = null;
            hfSearch({
                query: cat.params.query,
                sort,
                limit: cat.params.limit || 20,
                container: resultsContainer,
                filelistContainer,
                quickpicksContainer: quickpicks,
                discoverPillsContainerId: 'mm-hf-discover-pills',
                onSelectModel: (m) => onHfModelSelected(m, filelistContainer, downloadPanel),
            });
        },
    });

    // Load quick-picks
    hfLoadQuickPicks({
        container: quickpicks,
        discoverPillsContainerId: 'mm-hf-discover-pills',
        onAuthorClick: (author) => {
            const sort = sortSelect?.value || 'downloads';
            hfState.activeAuthor = author;
            hfState.activeDiscoverQuery = null;
            hfSearch({
                query: '',
                author,
                sort,
                limit: 20,
                container: resultsContainer,
                filelistContainer,
                quickpicksContainer: quickpicks,
                discoverPillsContainerId: 'mm-hf-discover-pills',
                onSelectModel: (m) => onHfModelSelected(m, filelistContainer, downloadPanel),
            });
        },
    });

    // Search on input (debounced)
    let searchTimer = null;
    const doSearch = () => {
        const { query, author, sort } = buildSearchParams();
        hfSearch({
            query,
            author,
            sort,
            limit: 20,
            container: resultsContainer,
            filelistContainer,
            quickpicksContainer: quickpicks,
            discoverPillsContainerId: 'mm-hf-discover-pills',
            onSelectModel: (m) => onHfModelSelected(m, filelistContainer, downloadPanel),
        });
    };

    searchInput.addEventListener('input', () => {
        clearTimeout(searchTimer);
        searchTimer = setTimeout(doSearch, 350);
    });

    searchInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            clearTimeout(searchTimer);
            doSearch();
        }
    });

    sortSelect?.addEventListener('change', () => {
        clearTimeout(searchTimer);
        const sort = sortSelect.value;

        // If browsing a specific author, re-run with new sort (like Quick Start)
        if (hfState.activeAuthor) {
            hfState.activeAuthor = hfState.activeAuthor;
            searchTimer = setTimeout(() => {
                hfSearch({
                    query: '',
                    author: hfState.activeAuthor,
                    sort,
                    limit: 20,
                    container: resultsContainer,
                    filelistContainer,
                    quickpicksContainer: quickpicks,
                    discoverPillsContainerId: 'mm-hf-discover-pills',
                    onSelectModel: (m) => onHfModelSelected(m, filelistContainer, downloadPanel),
                });
            }, 200);
            return;
        }

        // If active discover pill, re-fire with new sort
        const activePill = document.querySelector('#mm-hf-discover-pills .hf-discover-pill.active');
        if (activePill) {
            const cat = window.HF_DISCOVER_CATEGORIES?.find(c => c.id === activePill.dataset.catId);
            if (cat) {
                searchTimer = setTimeout(() => {
                    hfSearch({
                        query: cat.params.query,
                        sort,
                        limit: cat.params.limit || 20,
                        container: resultsContainer,
                        filelistContainer,
                        quickpicksContainer: quickpicks,
                        discoverPillsContainerId: 'mm-hf-discover-pills',
                        onSelectModel: (m) => onHfModelSelected(m, filelistContainer, downloadPanel),
                    });
                }, 200);
                return;
            }
        }

        // Fallback: use typed query (if any) + new sort
        searchTimer = setTimeout(doSearch, 200);
    });

    // Settings link from warning
    const settingsBtn = document.getElementById('mm-hf-dlp-open-settings');
    if (settingsBtn) {
        settingsBtn.addEventListener('click', () => {
            if (typeof window.openSettingsModal === 'function') {
                window.openSettingsModal('models');
            }
        });
    }
}

async function onHfModelSelected(model, filelistContainer, downloadPanel) {
    hfState.selectedRepoId = model.id;
    hfState.selectedFile = null;
    hfState.paramB = model.param_b || 0;
    hfHideDownloadPanel(downloadPanel);
    hideQuantAdvisor();
    hideMmprojSection();
    hideVramPanel();
    hideCtxTrainWarning();

    // Show selected model info
    showSelectedModel(model.id, model);

    await hfListFiles({
        repoId: model.id,
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

    // Show download panel
    await hfShowDownloadPanel(downloadPanel, file.path || file.name || '');

    // Update VRAM panel with live estimate
    if (hfState.paramB > 0 || hfState.modelBytes > 0) {
        scheduleVramUpdate(file);
    }

    // Check for mmproj companion in this repo
    await detectMmprojCompanion(repoId);

    // Wire download button
    const downloadBtn = document.getElementById('mm-hf-dlp-download-btn');
    if (downloadBtn) {
        downloadBtn.replaceWith(downloadBtn.cloneNode(true));
    }
    const newBtn = document.getElementById('mm-hf-dlp-download-btn');
    if (newBtn) {
        newBtn.onclick = () => {
            hfStartDownload({
                repoId,
                filePath: file.path || file.name,
                panelEl: downloadPanel,
                onComplete: (downloadId, localPath) => {
                    hfState.currentDownloadId = downloadId;
                    showToast('Model downloaded', 'success');
                    // Refresh library tab
                    loadModels();
                },
                onValidationError: (msg) => {
                    showToast(msg || 'Download failed', 'error');
                },
            });
        };
    }

    // Wire cancel button
    const cancelBtn = document.getElementById('mm-hf-dlp-cancel-btn');
    if (cancelBtn) {
        cancelBtn.replaceWith(cancelBtn.cloneNode(true));
    }
    const newCancel = document.getElementById('mm-hf-dlp-cancel-btn');
    if (newCancel) {
        newCancel.onclick = () => {
            if (hfState.currentDownloadId) {
                hfCancelDownload({
                    downloadId: hfState.currentDownloadId,
                    panelEl: downloadPanel,
                });
                hfState.currentDownloadId = null;
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
    quantAdvisorDebounce = setTimeout(loadQuantAdvisor, 600);
}

function hideQuantAdvisor() {
    const el = document.getElementById('mm-quant-advisor');
    if (el) el.style.display = 'none';
}

async function loadQuantAdvisor() {
    const paramB = hfState.paramB;
    if (!paramB || paramB <= 0) return;

    const availVram = cachedVram || 0;
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
            im.style.cssText = 'margin-left:4px; font-size:10px; color:#94a3b8;';
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
        }
        if (totalVram > 0) cachedVram = totalVram;
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

async function updateVramDisplay(file) {
    const panel = document.getElementById('mm-vram-panel');
    if (!panel) return;

    const availVram = cachedVram || 0;
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

    // Simple VRAM estimate: weights + KV cache + overhead
    const kvEstimate = estimateKvBytes(hfState.paramB, 4096); // assume 4k ctx for download preview
    const overhead = 200 * 1024 * 1024; // 200 MB overhead
    const mmprojBytes = hfState.mmprojBytes || 0;
    const total = modelBytes + kvEstimate + overhead + mmprojBytes;
    const free = availVram - total;

    // Show panel
    panel.style.display = '';

    // Update header
    const labelEl = document.getElementById('mm-vram-panel-label');
    const totalEl = document.getElementById('mm-vram-panel-total');
    if (labelEl) labelEl.textContent = 'VRAM budget';
    if (totalEl) totalEl.textContent = formatVramTotal(availVram) + ' total';

    // Update bar
    const denom = availVram > 0 ? availVram : total;
    const weightsPct = modelBytes / denom;
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

    if (weightsLabel) weightsLabel.textContent = 'Weights ' + formatGB(modelBytes);
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
            const sizeStr = f.size ? ' · ' + formatBytes(f.size) : '';
            opt.textContent = fname + sizeStr;
            if (fpath === hfState.mmprojPath) opt.selected = true;
            select.appendChild(opt);
        });

        select.addEventListener('change', () => {
            const fpath = select.value;
            hfState.mmprojPath = fpath;
            const f = mmprojFiles.find(x => (x.path || x.name) === fpath);
            hfState.mmprojBytes = f?.size ? Number(f.size) : 0;
            scheduleVramUpdate(hfState.selectedFile);
        });

        check.addEventListener('change', () => {
            select.style.display = check.checked ? '' : 'none';
            if (!check.checked) {
                hfState.mmprojPath = '';
                hfState.mmprojBytes = 0;
            } else if (!select.value && mmprojFiles.length) {
                select.value = mmprojFiles[0].path || mmprojFiles[0].name || '';
                hfState.mmprojPath = select.value;
                const f = mmprojFiles[0];
                hfState.mmprojBytes = f.size ? Number(f.size) : 0;
            }
            scheduleVramUpdate(hfState.selectedFile);
        });

        content.appendChild(select);

        // Hint
        const hint = document.createElement('div');
        hint.style.cssText = 'font-size:11px; color:var(--color-text-muted,#94a3b8); margin-left:22px; line-height:1.5;';
        hint.textContent = 'Adds ~' + (mmprojFiles[0]?.size ? formatBytes(mmprojFiles[0].size) : '0.5–1.5 GB') + ' VRAM. Required for multimodal inference.';
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
    if (b >= 1e9) return (b / 1e9).toFixed(1) + ' GB';
    if (b >= 1e6) return (b / 1e6).toFixed(1) + ' MB';
    if (b >= 1e3) return (b / 1e3).toFixed(0) + ' KB';
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

function estimateKvBytes(paramB, ctxTokens) {
    // Rough KV cache estimate: 2 * ctx * (n_layers * n_kv_heads * head_dim * 2 bytes)
    // Use heuristic arch based on param count
    let nLayers, nKvHeads, headDim;
    if (paramB < 2)       { nLayers = 22; nKvHeads = 4; headDim = 64; }
    else if (paramB < 5)  { nLayers = 28; nKvHeads = 4; headDim = 128; }
    else if (paramB < 10) { nLayers = 32; nKvHeads = 8; headDim = 128; }
    else if (paramB < 18) { nLayers = 40; nKvHeads = 8; headDim = 128; }
    else if (paramB < 35) { nLayers = 40; nKvHeads = 8; headDim = 128; }
    else if (paramB < 55) { nLayers = 60; nKvHeads = 8; headDim = 128; }
    else                  { nLayers = 80; nKvHeads = 8; headDim = 128; }

    return 2 * ctxTokens * nLayers * nKvHeads * headDim * 2; // *2 for K+V, *2 for f16
}
