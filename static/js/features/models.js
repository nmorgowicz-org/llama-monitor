// ── Models ────────────────────────────────────────────────────────────────────
// Models modal: open, close, load, refresh.

import { escapeHtml } from '../core/format.js';
import { showToast } from './toast.js';

let initialized = false;

export function openModelsModal() {
    document.getElementById('models-modal')?.classList.add('open');
    loadModels();
}

function closeModelsModal() {
    document.getElementById('models-modal')?.classList.remove('open');
}

async function loadModels() {
    const list = document.getElementById('models-list');
    const summary = document.getElementById('models-summary');
    if (!list || !summary) return;

    summary.textContent = 'Loading models...';
    list.innerHTML = '';

    try {
        const resp = await fetch('/api/models');
        const models = await resp.json();
        summary.textContent = models.length ? models.length + ' model' + (models.length === 1 ? '' : 's') + ' found' : 'No models found';
        // eslint-disable-next-line no-unsanitized/property -- all server strings (path, name, filename, meta) wrapped in escapeHtml()
        list.innerHTML = models.length ? models.map(model => {
            const name = model.model_name || model.filename;
            const meta = [
                model.quant_type || 'unknown quant',
                model.size_display || '',
                model.is_split ? 'split model' : ''
            ].filter(Boolean).join(' · ');
            return '<div class="model-item">' +
                '<div><div class="model-name" title="' + escapeHtml(model.path) + '">' + escapeHtml(name) + '</div>' +
                '<div class="model-meta">' + escapeHtml(model.filename) + '</div></div>' +
                '<div class="model-meta">' + escapeHtml(meta) + '</div>' +
                '</div>';
        }).join('') : '<div class="model-item"><div class="model-name">No models discovered</div><div class="model-meta">Configure --models-dir or model paths in presets.</div></div>';
    } catch (err) {
        summary.textContent = 'Failed to load models';
        list.innerHTML = '<div class="model-item"><div class="model-name">Error</div><div class="model-meta">' + escapeHtml(err.message) + '</div></div>';
    }
}

async function refreshModels() {
    const summary = document.getElementById('models-summary');
    if (summary) summary.textContent = 'Refreshing...';
    try {
        const resp = await fetch('/api/models/refresh', { method: 'POST' });
        const data = await resp.json();
        if (!data.ok) showToast('Model refresh failed: ' + (data.error || 'unknown'), 'error');
    } catch (err) {
        showToast('Model refresh failed: ' + err.message, 'error');
    }
    await loadModels();
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initModels() {
    if (initialized) return;
    initialized = true;

    // Bind modal buttons (sidebar button also bound by nav.js with data-tab="models")
    const closeBtn = document.getElementById('models-modal-close');
    if (closeBtn) closeBtn.addEventListener('click', closeModelsModal);

    const cancelBtn = document.getElementById('models-modal-cancel');
    if (cancelBtn) cancelBtn.addEventListener('click', closeModelsModal);

    const refreshBtn = document.getElementById('models-refresh-btn');
    if (refreshBtn) refreshBtn.addEventListener('click', refreshModels);
}
