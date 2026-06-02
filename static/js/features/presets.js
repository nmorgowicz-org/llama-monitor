// ── Presets ────────────────────────────────────────────────────────────────────
// Preset CRUD: load, save, copy, delete, reset. Modal management.

import { sessionState } from '../core/app-state.js';
import { escapeHtml } from '../core/format.js';
import { openDeferredFileBrowser } from './file-browser-launcher.js';
import { applySettings, saveSettings } from './settings.js';
import { showToast } from './toast.js';

// ── Helpers ────────────────────────────────────────────────────────────────────

function setVal(id, v) { document.getElementById(id).value = v ?? ''; }
function setChk(id, v) { document.getElementById(id).checked = !!v; }
function setOpt(id, v) { document.getElementById(id).value = v || ''; }
function numOrEmpty(id, v) { document.getElementById(id).value = v != null ? v : ''; }
function intOrNull(id) { const v = document.getElementById(id).value; return v !== '' ? parseInt(v) : null; }
function floatOrNull(id) { const v = document.getElementById(id).value; return v !== '' ? parseFloat(v) : null; }
function strVal(id) { return document.getElementById(id).value.trim(); }

function clearFieldErrors() {
    document.querySelectorAll('#preset-form .field-error').forEach(el => el.classList.remove('field-error'));
}

// ── Load ───────────────────────────────────────────────────────────────────────

export async function loadPresets(selectId) {
    const auth = window.authHeaders ? window.authHeaders() : {};

    const [presetsResp, settingsResp] = await Promise.all([
        fetch('/api/presets', { headers: auth }),
        selectId === undefined ? fetch('/api/settings', { headers: auth }) : Promise.resolve(null),
    ]);

    if (presetsResp.status === 401) {
        showToast('Unauthorized: API token missing or invalid', 'error');
        return;
    }

    sessionState.presets = await presetsResp.json();
    let saved = null;
    if (settingsResp) {
        if (settingsResp.status === 401) {
            console.warn('[presets] /api/settings returned 401');
        } else {
            saved = await settingsResp.json();
        }
    }

    const sel = document.getElementById('preset-select');
    sel.innerHTML = '';
    sessionState.presets.forEach(p => {
        const opt = document.createElement('option');
        opt.value = p.id;
        opt.textContent = p.name;
        sel.appendChild(opt);
    });

    const targetId = selectId ?? (saved?.preset_id || null);
    if (targetId && sessionState.presets.find(p => p.id === targetId)) {
        sel.value = targetId;
    } else if (sessionState.presets.length > 0) {
        sel.value = sessionState.presets[0].id;
    }

    if (selectId === undefined && saved) {
        applySettings(saved);
    }
    if (selectId === undefined) {
        saveSettings();
    }
}

// ── Modal ──────────────────────────────────────────────────────────────────────

export function openPresetModal(mode) {
    const modal = document.getElementById('preset-modal');
    const title = document.getElementById('modal-title');
    const form = document.getElementById('preset-form');
    form.reset();
    clearFieldErrors();

    if (mode === 'edit') {
        const id = document.getElementById('preset-select').value;
        const p = sessionState.presets.find(pr => pr.id === id);
        if (!p) { showToast('No preset selected', 'warn'); return; }
        title.textContent = 'Edit Preset';
        setVal('modal-preset-id', p.id);
        // Model & Memory
        setVal('modal-name', p.name);
        setVal('modal-model-path', p.model_path);
        numOrEmpty('modal-gpu-layers', p.gpu_layers);
        setChk('modal-no-mmap', p.no_mmap);
        setChk('modal-mlock', p.mlock);
        // Context & KV
        setVal('modal-context-size', p.context_size || 128000);
        setVal('modal-ctk', p.ctk || 'q8_0');
        setVal('modal-ctv', p.ctv || 'f16');
        setOpt('modal-flash-attn', p.flash_attn);
        // Batching
        setVal('modal-batch-size', p.batch_size || 2048);
        setVal('modal-ubatch-size', p.ubatch_size || p.batch_size || 2048);
        setVal('modal-parallel-slots', p.parallel_slots || 1);
        // Generation
        numOrEmpty('modal-temperature', p.temperature);
        numOrEmpty('modal-top-p', p.top_p);
        numOrEmpty('modal-top-k', p.top_k);
        numOrEmpty('modal-min-p', p.min_p);
        numOrEmpty('modal-repeat-penalty', p.repeat_penalty);
        numOrEmpty('modal-n-cpu-moe', p.n_cpu_moe);
        // GPU
        setVal('modal-tensor-split', p.tensor_split);
        setOpt('modal-split-mode', p.split_mode);
        numOrEmpty('modal-main-gpu', p.main_gpu);
        // Threading
        numOrEmpty('modal-threads', p.threads);
        numOrEmpty('modal-threads-batch', p.threads_batch);
        // Rope
        setOpt('modal-rope-scaling', p.rope_scaling);
        numOrEmpty('modal-rope-freq-base', p.rope_freq_base);
        numOrEmpty('modal-rope-freq-scale', p.rope_freq_scale);
        // Spec decoding
        setChk('modal-ngram-spec', p.ngram_spec);
        numOrEmpty('modal-spec-ngram-size', p.spec_ngram_size);
        numOrEmpty('modal-draft-min', p.draft_min);
        numOrEmpty('modal-draft-max', p.draft_max);
        setVal('modal-draft-model', p.draft_model);
        // Advanced
        numOrEmpty('modal-seed', p.seed);
        setVal('modal-system-prompt-file', p.system_prompt_file);
        setVal('modal-extra-args', p.extra_args);
    } else {
        title.textContent = 'New Preset';
        setVal('modal-preset-id', '');
        setVal('modal-context-size', 128000);
        setVal('modal-ctk', 'q8_0');
        setVal('modal-ctv', 'f16');
        setVal('modal-batch-size', 2048);
        setVal('modal-ubatch-size', 2048);
        setVal('modal-parallel-slots', 1);
        setVal('modal-temperature', 1.0);
        setVal('modal-top-p', 0.95);
        numOrEmpty('modal-top-k', 40);
        numOrEmpty('modal-min-p', 0.01);
        numOrEmpty('modal-repeat-penalty', 1.0);
        numOrEmpty('modal-n-cpu-moe', 16);
    }

    modal.classList.add('open');
    const body = modal.querySelector('.modal-body');
    if (body) body.scrollTop = 0;
}

export function closePresetModal() {
    document.getElementById('preset-modal').classList.remove('open');
}

// ── Presets Panel ──────────────────────────────────────────────────────────────

export function openPresetsPanel() {
    const overlay = document.getElementById('presets-panel-overlay');
    if (!overlay) return;
    overlay.style.display = '';
    _renderPresetsPanel();
    document.getElementById('presets-panel-wizard-btn')?.addEventListener('click', () => {
        closePresetsPanel();
        import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard());
    }, { once: true });
}

export function closePresetsPanel() {
    const overlay = document.getElementById('presets-panel-overlay');
    if (overlay) overlay.style.display = 'none';
}

function _renderPresetsPanel() {
    const body = document.getElementById('presets-panel-body');
    if (!body) return;
    body.innerHTML = '';

    const presets = sessionState.presets || [];
    if (!presets.length) {
        const empty = document.createElement('div');
        empty.className = 'presets-panel-empty';
        empty.textContent = 'No presets saved yet. Use the Spawn Wizard to create one.';
        body.appendChild(empty);
        return;
    }

    presets.forEach(preset => {
        const card = document.createElement('div');
        card.className = 'preset-panel-card';

        const icon = document.createElement('div');
        icon.className = 'preset-panel-card-icon';
        icon.textContent = '🧠';
        card.appendChild(icon);

        const info = document.createElement('div');
        info.className = 'preset-panel-card-info';

        const name = document.createElement('div');
        name.className = 'preset-panel-card-name';
        name.textContent = preset.name || 'Unnamed preset';
        info.appendChild(name);

        const metaParts = [];
        if (preset.model_path) metaParts.push(preset.model_path.split(/[/\\]/).pop() || preset.model_path);
        else if (preset.hf_repo) metaParts.push(preset.hf_repo);
        if (preset.bind_host === '0.0.0.0') metaParts.push('LAN');
        if (preset.context_size) metaParts.push(`${Math.round(preset.context_size / 1024)}k ctx`);
        const ctk = preset.ctk || 'q8_0';
        const ctv = preset.ctv || 'q8_0';
        if (ctk || ctv) metaParts.push(`KV: ${ctk}/${ctv}`);

        const meta = document.createElement('div');
        meta.className = 'preset-panel-card-meta';
        meta.textContent = metaParts.join(' · ') || 'No details';
        meta.title = metaParts.join(' · ');
        info.appendChild(meta);
        card.appendChild(info);

        const actions = document.createElement('div');
        actions.className = 'preset-panel-card-actions';

        const startBtn = document.createElement('button');
        startBtn.type = 'button';
        startBtn.className = 'btn-preset-quick-start';
        startBtn.textContent = '▶ Quick Start';
        startBtn.title = 'Spawn this server configuration now';
        startBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            const mainSelect = document.getElementById('preset-select');
            if (mainSelect) {
                mainSelect.value = preset.id;
                mainSelect.dispatchEvent(new Event('change', { bubbles: true }));
            }
            closePresetsPanel();
            import('./attach-detach.js').then(({ doStartFromSetup }) => {
                const setupSelect = document.getElementById('setup-preset-select');
                if (setupSelect) setupSelect.value = preset.id;
                doStartFromSetup();
            });
        });
        actions.appendChild(startBtn);

        const delBtn = document.createElement('button');
        delBtn.type = 'button';
        delBtn.className = 'btn-preset-delete';
        delBtn.title = 'Delete preset';
        delBtn.textContent = '✕';
        delBtn.addEventListener('click', async (e) => {
            e.stopPropagation();
            if (!confirm(`Delete preset "${preset.name}"?`)) return;
            try {
                const headers = window.authHeaders ? { ...window.authHeaders() } : {};
                const resp = await fetch(`/api/presets/${preset.id}`, { method: 'DELETE', headers });
                if (resp.ok) {
                    await loadPresets();
                    _renderPresetsPanel();
                    const { syncSetupPresetSelect } = await import('./setup-view.js');
                    syncSetupPresetSelect();
                }
            } catch (err) {
                console.error('Delete preset failed:', err);
            }
        });
        actions.appendChild(delBtn);

        card.appendChild(actions);
        body.appendChild(card);
    });
}

// ── CRUD ───────────────────────────────────────────────────────────────────────

export async function savePreset(event) {
    event.preventDefault();
    clearFieldErrors();

    const id = document.getElementById('modal-preset-id').value;
    const preset = {
        name: strVal('modal-name'),
        model_path: strVal('modal-model-path'),
        gpu_layers: intOrNull('modal-gpu-layers'),
        no_mmap: document.getElementById('modal-no-mmap').checked,
        mlock: document.getElementById('modal-mlock').checked,
        context_size: parseInt(document.getElementById('modal-context-size').value) || 128000,
        ctk: strVal('modal-ctk') || 'q8_0',
        ctv: strVal('modal-ctv') || 'f16',
        flash_attn: strVal('modal-flash-attn'),
        batch_size: parseInt(document.getElementById('modal-batch-size').value) || 2048,
        ubatch_size: parseInt(document.getElementById('modal-ubatch-size').value) || 2048,
        parallel_slots: parseInt(document.getElementById('modal-parallel-slots').value) || 1,
        temperature: floatOrNull('modal-temperature'),
        top_p: floatOrNull('modal-top-p'),
        top_k: intOrNull('modal-top-k'),
        min_p: floatOrNull('modal-min-p'),
        repeat_penalty: floatOrNull('modal-repeat-penalty'),
        n_cpu_moe: intOrNull('modal-n-cpu-moe'),
        tensor_split: strVal('modal-tensor-split'),
        split_mode: strVal('modal-split-mode'),
        main_gpu: intOrNull('modal-main-gpu'),
        threads: intOrNull('modal-threads'),
        threads_batch: intOrNull('modal-threads-batch'),
        rope_scaling: strVal('modal-rope-scaling'),
        rope_freq_base: floatOrNull('modal-rope-freq-base'),
        rope_freq_scale: floatOrNull('modal-rope-freq-scale'),
        ngram_spec: document.getElementById('modal-ngram-spec').checked,
        spec_ngram_size: intOrNull('modal-spec-ngram-size'),
        draft_min: intOrNull('modal-draft-min'),
        draft_max: intOrNull('modal-draft-max'),
        draft_model: strVal('modal-draft-model'),
        seed: intOrNull('modal-seed'),
        system_prompt_file: strVal('modal-system-prompt-file'),
        extra_args: strVal('modal-extra-args'),
    };

    // Inline validation
    let valid = true;
    if (!preset.name) {
        document.getElementById('modal-name').classList.add('field-error');
        valid = false;
    }
    if (!preset.model_path) {
        document.getElementById('modal-model-path').classList.add('field-error');
        valid = false;
    }
    if (!valid) {
        showToast('Please fill in all required fields', 'error');
        return;
    }

    const saveBtn = document.getElementById('btn-modal-save');
    saveBtn.classList.add('saving');
    saveBtn.textContent = 'Saving...';

    try {
        let resp;
        let savedId;
        if (id) {
            resp = await fetch('/api/presets/' + encodeURIComponent(id), {
                method: 'PUT',
                headers: window.authHeaders
                    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                    : { 'Content-Type': 'application/json' },
                body: JSON.stringify(preset),
            });
            if (!resp.ok) {
                const err = await resp.text().catch(() => 'Unknown error');
                showToast('Save failed: ' + err, 'error');
                return;
            }
            savedId = id;
        } else {
            resp = await fetch('/api/presets', {
                method: 'POST',
                headers: window.authHeaders
                    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                    : { 'Content-Type': 'application/json' },
                body: JSON.stringify(preset),
            });
            if (!resp.ok) {
                const err = await resp.text().catch(() => 'Unknown error');
                showToast('Save failed: ' + err, 'error');
                return;
            }
            const data = await resp.json();
            savedId = data.id || null;
        }
        closePresetModal();
        await loadPresets(savedId);
        showToast('Preset saved', 'success');
    } catch (err) {
        showToast('Save failed: ' + err.message, 'error');
    } finally {
        saveBtn.classList.remove('saving');
        saveBtn.textContent = 'Save';
    }
}

export async function copyPreset() {
    const id = document.getElementById('preset-select').value;
    const p = sessionState.presets.find(pr => pr.id === id);
    if (!p) { showToast('No preset selected', 'warn'); return; }

    const copy = Object.assign({}, p);
    delete copy.id;
    copy.name = p.name + ' (copy)';

    try {
        const resp = await fetch('/api/presets', {
            method: 'POST',
            headers: window.authHeaders
                ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                : { 'Content-Type': 'application/json' },
            body: JSON.stringify(copy),
        });
        if (!resp.ok) {
            const err = await resp.text().catch(() => 'Unknown error');
            showToast('Copy failed: ' + err, 'error');
            return;
        }
        const data = await resp.json();
        await loadPresets(data.preset?.id || null);
        showToast('Preset copied', 'success');
    } catch (err) {
        showToast('Copy failed: ' + err.message, 'error');
    }
}

export async function deletePreset() {
    const id = document.getElementById('preset-select').value;
    const p = sessionState.presets.find(pr => pr.id === id);
    if (!p) { showToast('No preset selected', 'warn'); return; }
    if (!confirm('Delete preset "' + p.name + '"?')) return;

    try {
        const resp = await fetch('/api/presets/' + encodeURIComponent(id), {
            method: 'DELETE',
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        if (!resp.ok) {
            const err = await resp.text().catch(() => 'Unknown error');
            showToast('Delete failed: ' + err, 'error');
            return;
        }
        await loadPresets();
        showToast('Preset deleted', 'success');
    } catch (err) {
        showToast('Delete failed: ' + err.message, 'error');
    }
}

export async function resetPresets() {
    if (!confirm('Reset all presets to built-in defaults? Custom presets will be removed.')) return;
    try {
        const resp = await fetch('/api/presets/reset', {
            method: 'POST',
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        if (!resp.ok) {
            const err = await resp.text().catch(() => 'Unknown error');
            showToast('Reset failed: ' + err, 'error');
            return;
        }
        await loadPresets();
        showToast('Presets reset to defaults', 'success');
    } catch (err) {
        showToast('Reset failed: ' + err.message, 'error');
    }
}

// ── Init ───────────────────────────────────────────────────────────────────────

export function initPresets() {
    // Bind preset action buttons
    document.getElementById('preset-new-btn')?.addEventListener('click', () => openPresetModal('new'));
    document.getElementById('preset-edit-btn')?.addEventListener('click', () => openPresetModal('edit'));
    document.getElementById('preset-copy-btn')?.addEventListener('click', copyPreset);
    document.getElementById('preset-delete-btn')?.addEventListener('click', deletePreset);
    document.getElementById('preset-reset-btn')?.addEventListener('click', resetPresets);

    // Bind preset modal buttons
    document.getElementById('preset-modal-close')?.addEventListener('click', closePresetModal);
    document.getElementById('preset-modal-cancel')?.addEventListener('click', closePresetModal);
    document.getElementById('preset-browse-model-btn')?.addEventListener('click', () => openDeferredFileBrowser('modal-model-path', 'gguf'));

    // Bind preset form submit
    const presetForm = document.getElementById('preset-form');
    if (presetForm) presetForm.addEventListener('submit', savePreset);

    // Bind setup view link
    document.getElementById('setup-manage-presets-link')?.addEventListener('click', (e) => {
        e.preventDefault();
        openPresetsPanel();
    });

    // Modal overlay click
    const modal = document.getElementById('preset-modal');
    if (modal) {
        modal.addEventListener('click', e => {
            if (e.target === e.currentTarget) closePresetModal();
        });
    }

    window.closePresetsPanel = closePresetsPanel;

    // Clear field errors on input
    ['modal-name', 'modal-model-path'].forEach(id => {
        const el = document.getElementById(id);
        if (el) {
            el.addEventListener('input', function() {
                this.classList.remove('field-error');
            });
        }
    });
    // Initial load
    loadPresets();
}
