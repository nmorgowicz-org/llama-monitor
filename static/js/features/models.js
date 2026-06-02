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

let initialized = false;

// State for the HF download tab
let hfState = {
    selectedRepoId: null,
    selectedFile: null,
    currentDownloadId: null,
    initialized: false,
};

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
        // Use HF download-dir endpoint for the effective models directory
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
        if (summary) summary.textContent = count
            ? count + ' model' + (count === 1 ? '' : 's') + ' found'
            : 'No models found';

        if (!count) {
            grid.innerHTML = '<div class="mm-empty">No models found in this directory. You can download one from the Download tab.</div>';
            return;
        }

        // Build cards using DOM to avoid innerHTML with user data
        grid.innerHTML = '';
        models.forEach(m => {
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

function buildModelCard(m) {
    const name = m.model_name || m.filename;
    const quant = m.quant_type || 'unknown';
    const size = m.size_display || '';
    const vramEst = m.vram_estimate_display || '';
    const vramPct = m.vram_percent != null ? Math.min(100, m.vram_percent) : null;
    const isSplit = m.is_split;

    const card = document.createElement('div');
    card.className = 'mm-model-card';

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
