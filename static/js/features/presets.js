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
function nullableBoolOpt(id) {
    const v = document.getElementById(id).value;
    if (v === 'true') return true;
    if (v === 'false') return false;
    return null;
}

function isWindowsAbsolutePath(value) {
    return /^[A-Za-z]:[\\/]/.test(value);
}

function looksLikeLocalModelSource(value) {
    const v = (value || '').trim();
    if (!v) return false;
    const lower = v.toLowerCase();
    return v.startsWith('/') ||
        v.startsWith('./') ||
        v.startsWith('../') ||
        v.startsWith('~') ||
        v.includes('\\') ||
        isWindowsAbsolutePath(v) ||
        lower.endsWith('.gguf');
}

function normalizeModelSourceInput(value) {
    const input = (value || '').trim();
    if (!input) {
        return { model_path: '', hf_repo: null };
    }
    if (looksLikeLocalModelSource(input)) {
        return { model_path: input, hf_repo: null };
    }
    return { model_path: '', hf_repo: input };
}

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

    // Keep the setup view preset dropdown and launch grid in sync
    import('./setup-view.js').then(m => m.syncSetupPresetSelect?.()).catch(() => {});
}

// ── Modal ──────────────────────────────────────────────────────────────────────

export function openPresetModal(mode) {
    const modal = document.getElementById('preset-modal');
    const title = document.getElementById('modal-title');
    const subtitle = document.getElementById('preset-editor-subtitle');
    const form = document.getElementById('preset-form');
    form.reset();
    clearFieldErrors();

    if (mode === 'edit') {
        const id = document.getElementById('preset-select').value;
        const p = sessionState.presets.find(pr => pr.id === id);
        if (!p) { showToast('No preset selected', 'warn'); return; }
        title.textContent = 'Edit Preset';
        if (subtitle) subtitle.textContent = p.name;
        setVal('modal-preset-id', p.id);
        // Model & Memory
        setVal('modal-name', p.name);
        // Prefill model field:
        // - If model_path present, treat as local file.
        // - Else if hf_repo present, treat as HF repo.
        const modelValue = p.model_path || p.hf_repo || '';
        setVal('modal-model-path', modelValue);
        setVal('modal-alias', p.alias || '');
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
        numOrEmpty('modal-presence-penalty', p.presence_penalty);
        setOpt('modal-enable-thinking', p.enable_thinking == null ? '' : String(!!p.enable_thinking));
        setOpt('modal-preserve-thinking', p.preserve_thinking == null ? '' : String(!!p.preserve_thinking));
        setOpt('modal-reasoning', p.reasoning || '');
        numOrEmpty('modal-reasoning-budget', p.reasoning_budget);
        setVal('modal-reasoning-budget-message', p.reasoning_budget_message || '');
        // GPU
        setVal('modal-tensor-split', p.tensor_split);
        setOpt('modal-split-mode', p.split_mode);
        numOrEmpty('modal-main-gpu', p.main_gpu);
        // Threading
        numOrEmpty('modal-threads', p.threads);
        numOrEmpty('modal-threads-batch', p.threads_batch);
        numOrEmpty('modal-n-cpu-moe', p.n_cpu_moe);
        // Rope
        setOpt('modal-rope-scaling', p.rope_scaling);
        numOrEmpty('modal-rope-freq-base', p.rope_freq_base);
        numOrEmpty('modal-rope-freq-scale', p.rope_freq_scale);
        // Spec decoding — use spec_type; fallback: ngram_spec bool → ngram-mod
        const specType = p.spec_type || (p.ngram_spec ? 'ngram-mod' : '');
        setOpt('modal-spec-type', specType);
        numOrEmpty('modal-spec-ngram-size', p.spec_ngram_size);
        numOrEmpty('modal-draft-min', p.draft_min);
        numOrEmpty('modal-draft-max', p.draft_max);
        numOrEmpty('modal-spec-draft-n-max', p.spec_draft_n_max);
        setVal('modal-draft-model', p.draft_model);
        _toggleSpecFields(specType);
        // Context extras
        setChk('modal-kv-unified', p.kv_unified ?? false);
        // Model extras
        setVal('modal-mmproj', p.mmproj || '');
        // Advanced
        setOpt('modal-bind-host', p.bind_host || '');
        setVal('modal-api-key', p.api_key || '');
        numOrEmpty('modal-max-tokens', p.max_tokens);
        numOrEmpty('modal-seed', p.seed);
        setChk('modal-ignore-eos', p.ignore_eos ?? false);
        setChk('modal-fit-enabled', p.fit_enabled ?? false);
        setOpt('modal-fit-target', p.fit_target || '');
        _toggleFitTarget(p.fit_enabled ?? false);
        setVal('modal-system-prompt-file', p.system_prompt_file);
        setVal('modal-extra-args', p.extra_args);
    } else {
        title.textContent = 'New Preset';
        if (subtitle) subtitle.textContent = 'New configuration';
        setVal('modal-preset-id', '');
        setVal('modal-context-size', 128000);
        setVal('modal-ctk', 'q8_0');
        setVal('modal-ctv', 'f16');
        setVal('modal-batch-size', 2048);
        setVal('modal-ubatch-size', 2048);
        setVal('modal-parallel-slots', 1);
        _toggleFitTarget(false);
        _toggleSpecFields('');
    }

    const presetModel = document.getElementById('modal-model-path')?.value.trim();
    if (mode !== 'edit' && presetModel) _suggestGenerationDefaults(presetModel);
    else _renderGenerationPresetPills([]);

    // Reset change-summary state
    _hideSummary();

    // Show "Delete preset" button only when editing
    const deleteBtn = document.getElementById('preset-modal-delete');
    if (mode === 'edit') {
        if (deleteBtn) deleteBtn.style.display = '';
    } else {
        if (deleteBtn) deleteBtn.style.display = 'none';
    }

    modal.classList.add('open');
    // Reset nav to first section
    document.querySelector('.preset-nav-item[data-section="model"]')?.click();
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
    overlay.classList.add('open');
    _renderPresetsPanel();
    document.getElementById('presets-panel-wizard-btn')?.addEventListener('click', () => {
        closePresetsPanel();
        import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard());
    }, { once: true });
}

export function closePresetsPanel() {
    const overlay = document.getElementById('presets-panel-overlay');
    if (!overlay) return;
    overlay.classList.remove('open');
    overlay.style.display = 'none';
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
                }
            } catch (err) {
                console.error('Delete preset failed:', err);
            }
        });
        actions.appendChild(delBtn);

        card.appendChild(actions);

        // Top-right trash icon (subtle)
        const trashBtn = document.createElement('button');
        trashBtn.type = 'button';
        trashBtn.className = 'preset-panel-card-trash';
        trashBtn.title = 'Delete preset';
        trashBtn.innerHTML =
            '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" ' +
            'stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' +
            '<path d="M3 6h18"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>' +
            '<path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/>' +
            '<line x1="10" y1="11" x2="10" y2="17"/><line x1="14" y1="11" x2="14" y2="17"/>' +
            '</svg>';
        trashBtn.addEventListener('click', async (e) => {
            e.stopPropagation();
            if (!confirm(`Delete preset "${preset.name}"? This cannot be undone.`)) return;
            try {
                const headers = window.authHeaders ? { ...window.authHeaders() } : {};
                const resp = await fetch(`/api/presets/${preset.id}`, { method: 'DELETE', headers });
                if (resp.ok) {
                    await loadPresets();
                    _renderPresetsPanel();
                }
            } catch (err) {
                console.error('Delete preset failed:', err);
            }
        });
        card.appendChild(trashBtn);

        body.appendChild(card);
    });
}

// ── Change summary ────────────────────────────────────────────────────────────

function _toggleFitTarget(enabled) {
    const wrap = document.getElementById('modal-fit-target-wrap');
    if (wrap) wrap.style.display = enabled ? '' : 'none';
}

function _toggleSpecFields(specType) {
    const hasNgram = specType.includes('ngram');
    const hasMtp   = specType.includes('draft-mtp');
    const hasDraft = specType === 'draft-model';
    const ngWrap  = document.getElementById('spec-ngram-params-wrap');
    const mtpWrap = document.getElementById('spec-mtp-wrap');
    const dmWrap  = document.getElementById('spec-draft-model-wrap');
    const hint    = document.getElementById('spec-type-hint');
    if (ngWrap)  ngWrap.style.display  = hasNgram ? '' : 'none';
    if (mtpWrap) mtpWrap.style.display = hasMtp   ? '' : 'none';
    if (dmWrap)  dmWrap.style.display  = hasDraft ? '' : 'none';
    const hints = {
        'ngram-mod': 'Best for server deployments with multiple slots. Uses a shared hash pool — requires no extra files or VRAM.',
        'ngram-simple': 'Lightest-weight option. Scans recent history for matching n-grams. Good for single-slot use.',
        'ngram-map-k': 'Hash-map based pattern matching. Works well for repetitive content like code or structured data.',
        'ngram-map-k4v': 'Experimental. Tracks up to 4 candidate tokens per n-gram key. May outperform ngram-map-k on long repetitive content.',
        'draft-mtp,ngram-mod': 'Recommended for Qwen3, DeepSeek V3, and other models with built-in MTP heads. MTP handles main predictions; ngram-mod fills gaps. Forces --parallel 1.',
        'draft-mtp': 'Pure MTP — uses built-in prediction heads with no n-gram fallback. Forces --parallel 1.',
        'draft-model': 'Needs a separate draft model (same family, smaller size). Highest potential speedup but requires downloading and managing an additional file.',
    };
    if (hint) {
        const text = hints[specType] || '';
        hint.textContent = text;
        hint.style.display = text ? '' : 'none';
    }
}

function _hideSummary() {
    const summary = document.getElementById('preset-change-summary');
    const back = document.getElementById('preset-modal-back');
    const cancel = document.getElementById('preset-modal-cancel');
    const saveBtn = document.getElementById('btn-modal-save');
    if (summary) summary.style.display = 'none';
    if (back) back.style.display = 'none';
    if (cancel) cancel.style.display = '';
    if (saveBtn) { saveBtn.textContent = 'Save'; saveBtn.dataset.confirmed = ''; }
}

function _buildFormPreset(existing) {
    const modelSource = normalizeModelSourceInput(strVal('modal-model-path'));
    return {
        // Spread ALL existing fields first — preserves wizard-set values not shown in the editor
        ...existing,
        // Override only what the editor manages
        name: strVal('modal-name'),
        model_path: modelSource.model_path,
        hf_repo: modelSource.hf_repo,
        alias: strVal('modal-alias') || null,
        mmproj: strVal('modal-mmproj') || null,
        gpu_layers: intOrNull('modal-gpu-layers'),
        no_mmap: document.getElementById('modal-no-mmap').checked,
        mlock: document.getElementById('modal-mlock').checked,
        context_size: parseInt(document.getElementById('modal-context-size').value) || 128000,
        ctk: strVal('modal-ctk') || 'q8_0',
        ctv: strVal('modal-ctv') || 'f16',
        flash_attn: strVal('modal-flash-attn'),
        kv_unified: document.getElementById('modal-kv-unified').checked || null,
        batch_size: parseInt(document.getElementById('modal-batch-size').value) || 2048,
        ubatch_size: parseInt(document.getElementById('modal-ubatch-size').value) || 2048,
        parallel_slots: parseInt(document.getElementById('modal-parallel-slots').value) || 1,
        temperature: floatOrNull('modal-temperature'),
        top_p: floatOrNull('modal-top-p'),
        top_k: intOrNull('modal-top-k'),
        min_p: floatOrNull('modal-min-p'),
        repeat_penalty: floatOrNull('modal-repeat-penalty'),
        presence_penalty: floatOrNull('modal-presence-penalty'),
        enable_thinking: nullableBoolOpt('modal-enable-thinking'),
        preserve_thinking: nullableBoolOpt('modal-preserve-thinking'),
        reasoning: strVal('modal-reasoning') || null,
        reasoning_budget: intOrNull('modal-reasoning-budget'),
        reasoning_budget_message: document.getElementById('modal-reasoning-budget-message').value || null,
        tensor_split: strVal('modal-tensor-split'),
        split_mode: strVal('modal-split-mode'),
        main_gpu: intOrNull('modal-main-gpu'),
        threads: intOrNull('modal-threads'),
        threads_batch: intOrNull('modal-threads-batch'),
        n_cpu_moe: intOrNull('modal-n-cpu-moe'),
        rope_scaling: strVal('modal-rope-scaling'),
        rope_freq_base: floatOrNull('modal-rope-freq-base'),
        rope_freq_scale: floatOrNull('modal-rope-freq-scale'),
        spec_type: strVal('modal-spec-type') || null,
        ngram_spec: false,
        spec_ngram_size: intOrNull('modal-spec-ngram-size'),
        draft_min: intOrNull('modal-draft-min'),
        draft_max: intOrNull('modal-draft-max'),
        spec_draft_n_max: intOrNull('modal-spec-draft-n-max'),
        draft_model: strVal('modal-draft-model'),
        bind_host: strVal('modal-bind-host') || null,
        api_key: strVal('modal-api-key') || null,
        max_tokens: intOrNull('modal-max-tokens'),
        seed: intOrNull('modal-seed'),
        ignore_eos: document.getElementById('modal-ignore-eos').checked,
        fit_enabled: document.getElementById('modal-fit-enabled').checked || null,
        fit_target: strVal('modal-fit-target') || null,
        system_prompt_file: strVal('modal-system-prompt-file'),
        extra_args: strVal('modal-extra-args'),
    };
}

const CHANGE_LABELS = {
    name: 'Name', model_path: 'Model (local path or HF repo)', hf_repo: 'HuggingFace Repo', alias: 'Server Alias', mmproj: 'Multimodal Projector',
    gpu_layers: 'GPU Layers', no_mmap: 'no-mmap', mlock: 'mlock',
    context_size: 'Context Size', ctk: 'KV Key Type', ctv: 'KV Value Type',
    flash_attn: 'Flash Attn', kv_unified: 'KV Unified',
    fit_enabled: 'Fit to VRAM', fit_target: 'Fit Target',
    batch_size: 'Batch Size', ubatch_size: 'Micro-batch', parallel_slots: 'Parallel Slots',
    temperature: 'Temperature', top_p: 'Top-P', top_k: 'Top-K',
    min_p: 'Min-P', repeat_penalty: 'Repeat Penalty', presence_penalty: 'Presence Penalty',
    enable_thinking: 'Thinking Mode', preserve_thinking: 'Preserve Thinking',
    reasoning: 'Reasoning', reasoning_budget: 'Reasoning Budget',
    reasoning_budget_message: 'Reasoning Budget Message',
    tensor_split: 'Tensor Split', split_mode: 'Split Mode', main_gpu: 'Main GPU',
    threads: 'Threads', threads_batch: 'Threads Batch', n_cpu_moe: 'CPU MoE Threads',
    rope_scaling: 'RoPE Scaling', rope_freq_base: 'RoPE Freq Base', rope_freq_scale: 'RoPE Freq Scale',
    spec_type: 'Speculative Mode', spec_ngram_size: 'N-gram Size',
    draft_min: 'Draft Min', draft_max: 'Draft Max', spec_draft_n_max: 'MTP Depth', draft_model: 'Draft Model',
    bind_host: 'Bind Host', api_key: 'API Key', max_tokens: 'Max Tokens',
    seed: 'Seed', ignore_eos: 'Ignore EOS',
    system_prompt_file: 'System Prompt File', extra_args: 'Extra Args',
};

function _buildChangeSummary(existing, incoming) {
    const changes = [];
    const fmt = v => v == null || v === '' ? '(none)' : String(v);
    for (const key of Object.keys(CHANGE_LABELS)) {
        const prev = existing[key] ?? null;
        const next = incoming[key] ?? null;
        if (JSON.stringify(prev) !== JSON.stringify(next)) {
            changes.push({ label: CHANGE_LABELS[key], from: fmt(prev), to: fmt(next) });
        }
    }
    return changes;
}

// ── CRUD ───────────────────────────────────────────────────────────────────────

export async function savePreset(event) {
    event.preventDefault();
    clearFieldErrors();

    const id = document.getElementById('modal-preset-id').value;
    const saveBtn = document.getElementById('btn-modal-save');
    const existing = id ? (sessionState.presets.find(p => p.id === id) || {}) : {};
    const preset = _buildFormPreset(existing);

    // Inline validation
    let valid = true;
    if (!preset.name) {
        document.getElementById('modal-name').classList.add('field-error');
        valid = false;
    }
    if (!preset.model_path && !preset.hf_repo) {
        document.getElementById('modal-model-path').classList.add('field-error');
        valid = false;
    }
    if (!valid) {
        showToast('Please fill in all required fields', 'error');
        return;
    }

    // For edits: show change summary and require confirmation
    if (id && saveBtn.dataset.confirmed !== 'yes') {
        const changes = _buildChangeSummary(existing, preset);
        if (changes.length > 0) {
            const summary = document.getElementById('preset-change-summary');
            const list = document.getElementById('preset-change-summary-list');
            const back = document.getElementById('preset-modal-back');
            const cancel = document.getElementById('preset-modal-cancel');
            if (summary && list) {
                list.innerHTML = '';
                changes.forEach(({ label, from, to }) => {
                    const li = document.createElement('li');
                    li.className = 'preset-change-item';
                    li.innerHTML = `<span class="preset-change-field">${escapeHtml(label)}</span> <span class="preset-change-from">${escapeHtml(from)}</span><span class="preset-change-arrow">→</span><span class="preset-change-to">${escapeHtml(to)}</span>`;
                    list.appendChild(li);
                });
                summary.style.display = '';
                if (back) back.style.display = '';
                if (cancel) cancel.style.display = 'none';
                saveBtn.textContent = 'Confirm Save';
                saveBtn.dataset.confirmed = 'yes';
            }
            return;
        }
    }

    saveBtn.classList.add('saving');
    saveBtn.textContent = 'Saving...';
    _hideSummary();

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

// ── Preset Editor Nav ─────────────────────────────────────────────────────────

function initPresetEditorNav() {
    const navItems = document.querySelectorAll('.preset-nav-item');
    const sections = document.querySelectorAll('.preset-editor-section');

    navItems.forEach(btn => {
        btn.addEventListener('click', () => {
            const target = btn.dataset.section;
            // Deactivate all
            navItems.forEach(b => b.classList.remove('active'));
            sections.forEach(s => s.classList.remove('active'));
            // Activate clicked
            btn.classList.add('active');
            const activeSection = document.querySelector('.preset-editor-section[data-section="' + target + '"]');
            if (activeSection) activeSection.classList.add('active');
        });
    });
}

// ── Model-family generation defaults ─────────────────────────────────────────

async function _suggestGenerationDefaults(modelPath) {
    const modelName = modelPath.split(/[/\\]/).pop() || modelPath;
    try {
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };
        const resp = await fetch('/api/model-defaults', {
            method: 'POST',
            headers,
            body: JSON.stringify({ model_name_or_repo: modelName, size_bytes: 0, tags: [] }),
        });
        if (!resp.ok) return;
        const d = await resp.json();
        if (d.error) return;
        const defaults = d.defaults || d;

        // Only fill fields the user hasn't already set
        const fill = (id, val) => {
            const el = document.getElementById(id);
            if (el && el.value === '') numOrEmpty(id, val);
        };
        fill('modal-temperature', defaults.temperature ?? null);
        fill('modal-top-p', defaults.top_p ?? null);
        fill('modal-top-k', defaults.top_k ?? null);
        fill('modal-min-p', defaults.min_p ?? null);
        fill('modal-repeat-penalty', defaults.repeat_penalty ?? null);
        fill('modal-presence-penalty', defaults.presence_penalty ?? null);
        _fillSelectIfEmpty('modal-enable-thinking', defaults.enable_thinking);
        _fillSelectIfEmpty('modal-preserve-thinking', defaults.preserve_thinking);
        _fillSelectIfEmpty('modal-reasoning', defaults.reasoning ? 'on' : 'off');
        fill('modal-reasoning-budget', defaults.reasoning_budget ?? null);
        const msgEl = document.getElementById('modal-reasoning-budget-message');
        if (msgEl && msgEl.value === '' && defaults.reasoning_budget_message != null) {
            msgEl.value = defaults.reasoning_budget_message;
        }
        _renderGenerationPresetPills(d.presets || []);
    } catch (_) {
        // Silent — best-effort only
    }
}

function _fillSelectIfEmpty(id, value) {
    const el = document.getElementById(id);
    if (!el || el.value !== '' || value == null) return;
    el.value = typeof value === 'boolean' ? String(value) : String(value);
}

function _renderGenerationPresetPills(presets) {
    const container = document.getElementById('modal-generation-presets');
    if (!container) return;
    if (!presets || presets.length <= 1) {
        container.style.display = 'none';
        container.innerHTML = '';
        return;
    }

    container.style.display = 'flex';
    container.style.cssText = 'display:flex;align-items:center;gap:6px;flex-wrap:wrap;margin-bottom:12px;';
    container.innerHTML = '';

    const label = document.createElement('span');
    label.style.cssText = 'font-size:11px;color:var(--color-text-muted);flex-shrink:0;';
    label.textContent = 'Mode:';
    container.appendChild(label);

    presets.forEach((preset, index) => {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'sampling-preset-pill' + (index === 0 ? ' active' : '');
        btn.textContent = preset.name;
        if (preset.description) btn.title = preset.description;
        btn.addEventListener('click', () => {
            container.querySelectorAll('.sampling-preset-pill').forEach(p => p.classList.remove('active'));
            btn.classList.add('active');
            _applyGenerationPreset(preset);
        });
        container.appendChild(btn);
    });
}

function _applyGenerationPreset(preset) {
    numOrEmpty('modal-temperature', preset.temperature);
    numOrEmpty('modal-top-p', preset.top_p);
    numOrEmpty('modal-top-k', preset.top_k);
    numOrEmpty('modal-min-p', preset.min_p);
    numOrEmpty('modal-repeat-penalty', preset.repeat_penalty);
    numOrEmpty('modal-presence-penalty', preset.presence_penalty);
    setOpt('modal-enable-thinking', preset.enable_thinking == null ? '' : String(!!preset.enable_thinking));
    setOpt('modal-preserve-thinking', preset.preserve_thinking == null ? '' : String(!!preset.preserve_thinking));
    setOpt('modal-reasoning', preset.reasoning ? 'on' : 'off');
    numOrEmpty('modal-reasoning-budget', preset.reasoning_budget);
    setVal('modal-reasoning-budget-message', preset.reasoning_budget_message || '');
}

// ── Init ───────────────────────────────────────────────────────────────────────

export function initPresets() {
    // Init preset editor nav
    initPresetEditorNav();

    // Bind preset action buttons
    document.getElementById('preset-new-btn')?.addEventListener('click', () => openPresetModal('new'));
    document.getElementById('preset-edit-btn')?.addEventListener('click', () => openPresetModal('edit'));
    document.getElementById('preset-copy-btn')?.addEventListener('click', copyPreset);
    document.getElementById('preset-delete-btn')?.addEventListener('click', deletePreset);
    document.getElementById('preset-reset-btn')?.addEventListener('click', resetPresets);

    // Bind preset modal buttons
    document.getElementById('preset-modal-close')?.addEventListener('click', closePresetModal);
    document.getElementById('preset-modal-cancel')?.addEventListener('click', closePresetModal);
    document.getElementById('preset-modal-back')?.addEventListener('click', _hideSummary);

    // Delete preset from within the modal (only visible in edit mode)
    document.getElementById('preset-modal-delete')?.addEventListener('click', async () => {
        const id = document.getElementById('modal-preset-id').value;
        const p = sessionState.presets.find(pr => pr.id === id);
        if (!p) { showToast('No preset selected', 'warn'); return; }
        if (!confirm(`Delete preset "${p.name}"? This cannot be undone.`)) return;
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
            closePresetModal();
            await loadPresets();
            showToast('Preset deleted', 'success');
        } catch (err) {
            showToast('Delete failed: ' + err.message, 'error');
        }
    });
    document.getElementById('preset-browse-model-btn')?.addEventListener('click', () => openDeferredFileBrowser('modal-model-path', 'gguf'));
    document.getElementById('preset-browse-mmproj-btn')?.addEventListener('click', () => openDeferredFileBrowser('modal-mmproj', 'gguf'));
    document.getElementById('preset-browse-draft-model-btn')?.addEventListener('click', () => openDeferredFileBrowser('modal-draft-model', 'gguf'));

    // Fit-to-VRAM toggle shows/hides fit target
    document.getElementById('modal-fit-enabled')?.addEventListener('change', function() {
        _toggleFitTarget(this.checked);
    });

    // Spec type dropdown shows/hides relevant fields
    document.getElementById('modal-spec-type')?.addEventListener('change', function() {
        _toggleSpecFields(this.value);
    });

    // Bind preset form submit
    const presetForm = document.getElementById('preset-form');
    if (presetForm) presetForm.addEventListener('submit', savePreset);

    // Bind setup view link
    document.getElementById('setup-manage-presets-link')?.addEventListener('click', (e) => {
        e.preventDefault();
        openPresetModal('new');
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

    // When model path changes, suggest model-family generation defaults (only fills empty fields)
    let _modelDefaultsTimer = null;
    document.getElementById('modal-model-path')?.addEventListener('input', function() {
        clearTimeout(_modelDefaultsTimer);
        const path = this.value.trim();
        if (!path) return;
        _modelDefaultsTimer = setTimeout(() => _suggestGenerationDefaults(path), 600);
    });
    // Initial load
    loadPresets();
}
