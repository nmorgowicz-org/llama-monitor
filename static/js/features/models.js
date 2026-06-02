// ── Models ────────────────────────────────────────────────────────────────────
// Models modal: open, close, load, refresh, delete, and HF download tab.

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

let initialized = false;

// State for the HF download tab
let hfState = {
    selectedRepoId: null,
    selectedFile: null,
    currentDownloadId: null,
    initialized: false,
};

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
                return (a.model_name || a.filename || '').localeCompare(b.model_name || b.filename || '');
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

    const badge = document.createElement('span');
    badge.className = 'mm-quant-badge';
    badge.textContent = quant;
    top.appendChild(badge);

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

    // Stats row
    if (size || vramEst) {
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

    // Actions row
    const actions = document.createElement('div');
    actions.className = 'mm-card-actions';

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

    card.appendChild(actions);
    return card;
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
    filtersBtn.className = 'mm-lib-btn';
    filtersBtn.id = 'mm-lib-filters-toggle';
    filtersBtn.title = 'Filters';
    filtersBtn.innerHTML =
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="11" height="11"><line x1="4" y1="21" x2="4" y2="14"/><line x1="4" y1="10" x2="4" y2="3"/><line x1="12" y1="21" x2="12" y2="12"/><line x1="12" y1="8" x2="12" y2="3"/><line x1="20" y1="21" x2="20" y2="16"/><line x1="20" y1="12" x2="20" y2="3"/><line x1="1" y1="14" x2="7" y2="14"/><line x1="9" y1="8" x2="15" y2="8"/><line x1="17" y1="16" x2="23" y2="16"/></svg>';

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

    sortWrap.appendChild(sortSelect);
    right.appendChild(sortWrap);

    // View mode toggle
    const viewBtn = document.createElement('button');
    viewBtn.type = 'button';
    viewBtn.className = 'mm-lib-btn';
    viewBtn.id = 'mm-lib-view-toggle';
    viewBtn.title = prefs.viewMode === 'cards' ? 'Switch to list view' : 'Switch to cards view';
    // eslint-disable-next-line no-unsanitized/property -- static SVG, no user data
    viewBtn.innerHTML = prefs.viewMode === 'cards' ? ICON_LIST_VIEW : ICON_CARDS_VIEW;

    viewBtn.addEventListener('click', () => {
        prefs.viewMode = prefs.viewMode === 'cards' ? 'list' : 'cards';
        viewBtn.title = prefs.viewMode === 'cards' ? 'Switch to list view' : 'Switch to cards view';
        // eslint-disable-next-line no-unsanitized/property -- static SVG, no user data
        viewBtn.innerHTML = prefs.viewMode === 'cards' ? ICON_LIST_VIEW : ICON_CARDS_VIEW;
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

    // Render discover pills
    hfRenderDiscoverPills({
        container: discoverPills,
        quickpicksContainer: quickpicks,
        onPillClick: (cat) => {
            hfSearch({
                query: cat.params.query,
                sort: cat.params.sort,
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
            hfSearch({
                query: '',
                author,
                sort: 'downloads',
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
        const query = (searchInput.value || '').trim();
        const sort = sortSelect?.value || 'downloads';
        hfSearch({
            query,
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
    hfHideDownloadPanel(downloadPanel);

    await hfListFiles({
        repoId: model.id,
        container: filelistContainer,
        vramGb: 0,
        onSelectFile: (file, repoId) => onHfFileSelected(file, repoId, downloadPanel),
    });
}

async function onHfFileSelected(file, repoId, downloadPanel) {
    hfState.selectedFile = file;

    // Show download panel
    await hfShowDownloadPanel(downloadPanel, file.path || file.name || '');

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
