// ── Local Models Modal ────────────────────────────────────────────────────────
// Scans discovered local GGUF files and presents them as launch-ready cards.

import { escapeHtml } from '../core/format.js';
import { showToast } from './toast.js';
import { openSpawnWizard } from './spawn-wizard.js';

let initialized = false;

export function openModelsModal() {
    document.getElementById('models-modal')?.classList.add('open');
    loadModels();
}

function closeModelsModal() {
    document.getElementById('models-modal')?.classList.remove('open');
}

// ── Data loading ──────────────────────────────────────────────────────────────

async function loadModels() {
    const list = document.getElementById('models-list');
    const summary = document.getElementById('models-summary');
    const hint = document.getElementById('models-no-dir-hint');
    if (!list || !summary) return;

    summary.textContent = 'Scanning…';
    if (hint) hint.style.display = 'none';
    list.innerHTML = '<div class="models-loading">Scanning for GGUF files…</div>';

    try {
        const resp = await fetch('/api/models', {
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        const models = await resp.json();
        window.__lastDiscoveredModels = Array.isArray(models) ? models : [];

        if (!models.length) {
            summary.textContent = 'No models found';
            if (hint) hint.style.display = '';
            list.innerHTML = '<div class="models-empty">No GGUF files discovered. Configure a models directory in Settings or add model paths to a preset.</div>';
            return;
        }

        summary.textContent = `${models.length} model${models.length === 1 ? '' : 's'} found`;
        list.innerHTML = '';
        for (const m of models) {
            list.appendChild(buildModelCard(m));
        }
    } catch (err) {
        summary.textContent = 'Failed to scan';
        list.innerHTML = `<div class="models-empty">Error: ${escapeHtml(err.message)}</div>`;
    }
}

// ── Card rendering ────────────────────────────────────────────────────────────

function buildModelCard(m) {
    const card = document.createElement('div');
    card.className = 'model-card';

    const name = m.model_name || m.filename.replace(/\.gguf$/i, '');
    const quant = m.quant_type || null;
    const quantStyle = m.quant_style || 'standard';
    const paramStr = m.param_b != null ? formatParams(m.param_b) : null;
    const vramStr = m.vram_est_gb != null ? `~${m.vram_est_gb.toFixed(0)} GB VRAM` : null;

    // Header row: name + quant badge
    const header = document.createElement('div');
    header.className = 'model-card-header';

    const nameEl = document.createElement('div');
    nameEl.className = 'model-card-name';
    nameEl.textContent = name;
    nameEl.title = m.path || m.filename;
    header.appendChild(nameEl);

    if (quant) {
        const badge = document.createElement('span');
        badge.className = `model-card-quant quant-style-${quantStyle}`;
        badge.textContent = quant;
        header.appendChild(badge);
    }
    card.appendChild(header);

    // Meta row: file size + params + VRAM
    const meta = document.createElement('div');
    meta.className = 'model-card-meta';
    const metaParts = [m.size_display, paramStr, vramStr, m.is_split ? 'split' : null].filter(Boolean);
    meta.textContent = metaParts.join(' · ');
    card.appendChild(meta);

    // VRAM fit bar (if we have an estimate)
    if (m.vram_est_gb != null) {
        card.appendChild(buildVramBar(m.vram_est_gb));
    }

    // Actions
    const actions = document.createElement('div');
    actions.className = 'model-card-actions';

    const spawnBtn = document.createElement('button');
    spawnBtn.type = 'button';
    spawnBtn.className = 'btn-sm btn-model-spawn';
    spawnBtn.textContent = 'Spawn';
    spawnBtn.title = 'Open spawn wizard with this model pre-loaded';
    spawnBtn.addEventListener('click', () => spawnLocalModel(m.path || m.filename));
    actions.appendChild(spawnBtn);

    const pathBtn = document.createElement('button');
    pathBtn.type = 'button';
    pathBtn.className = 'btn-sm btn-preset';
    pathBtn.textContent = 'Copy path';
    pathBtn.addEventListener('click', () => {
        navigator.clipboard?.writeText(m.path || m.filename)
            .then(() => showToast('Path copied', 'success'))
            .catch(() => showToast('Copy failed', 'error'));
    });
    actions.appendChild(pathBtn);

    card.appendChild(actions);
    return card;
}

function formatParams(paramB) {
    if (paramB >= 1000) return `${(paramB / 1000).toFixed(1)}T`;
    if (paramB % 1 === 0) return `${paramB}B`;
    return `${paramB.toFixed(1)}B`;
}

function buildVramBar(vramGb) {
    const tiers = [8, 16, 24, 48, 80]; // common VRAM tiers
    const maxTier = tiers.find(t => t >= vramGb) || tiers[tiers.length - 1];
    const pct = Math.min(100, (vramGb / maxTier) * 100);
    const fits = vramGb <= 24; // conservative: show green if fits in 24 GB

    const wrap = document.createElement('div');
    wrap.className = 'model-vram-bar-wrap';

    const label = document.createElement('span');
    label.className = 'model-vram-label';
    label.textContent = `VRAM: ~${vramGb.toFixed(0)} GB`;
    wrap.appendChild(label);

    const track = document.createElement('div');
    track.className = 'model-vram-track';
    const fill = document.createElement('div');
    fill.className = `model-vram-fill ${fits ? 'fits' : 'large'}`;
    fill.style.width = `${pct}%`;
    track.appendChild(fill);
    wrap.appendChild(track);

    return wrap;
}

// ── Spawn integration ─────────────────────────────────────────────────────────

function spawnLocalModel(filePath) {
    closeModelsModal();
    const model = (window.__lastDiscoveredModels || []).find(m => (m.path || m.filename) === filePath) || null;
    // Open spawn wizard and pre-load the local file path into step 2.
    openSpawnWizard({ localPath: filePath, localModel: model });
}

// ── Refresh ───────────────────────────────────────────────────────────────────

async function refreshModels() {
    const summary = document.getElementById('models-summary');
    if (summary) summary.textContent = 'Refreshing…';
    try {
        const resp = await fetch('/api/models/refresh', {
            method: 'POST',
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        const data = await resp.json();
        if (!data.ok) showToast('Model refresh failed: ' + (data.error || 'unknown'), 'error');
    } catch (err) {
        showToast('Model refresh failed: ' + err.message, 'error');
    }
    await loadModels();
}

// ── Init ──────────────────────────────────────────────────────────────────────

export function initModels() {
    if (initialized) return;
    initialized = true;

    document.getElementById('models-modal-close')?.addEventListener('click', closeModelsModal);
    document.getElementById('models-modal-cancel')?.addEventListener('click', closeModelsModal);
    document.getElementById('models-refresh-btn')?.addEventListener('click', refreshModels);
    document.getElementById('models-open-settings-link')?.addEventListener('click', () => {
        closeModelsModal();
        // Open settings and activate the Models tab
        document.getElementById('settings-btn')?.click();
        setTimeout(() => {
            document.querySelector('.settings-tab[data-tab="models"]')?.click();
        }, 80);
    });

    // Auto-refresh when models_dir is updated via settings
    window.addEventListener('settings-applied', () => {
        if (document.getElementById('models-modal')?.classList.contains('open')) {
            loadModels();
        }
    });
}
