function switchTab(name) {
    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
    document.getElementById('page-' + name).classList.add('active');
    document.getElementById('tab-' + name).classList.add('active');
}

const PARAM_HINTS = {
    // Model & Memory
    name: "Display name for this preset",
    model_path: "Absolute path to the .gguf model file on disk",
    gpu_layers: "Number of layers to offload to GPU (-ngl). 99 = all layers. Lower values keep some on CPU to save VRAM.",
    no_mmap: "Disable memory-mapped I/O (--no-mmap). Required for models >16GB or on network storage.",
    mlock: "Lock model in RAM (--mlock), preventing OS from swapping to disk. Requires enough physical RAM.",
    // Context & KV Cache
    context_size: "Maximum context length in tokens (-c). Larger = more VRAM. Auto-enables YaRN for >262144.",
    ctk: "KV cache key quantization (-ctk): f16 (best quality), q8_0, turbo3 (4.6x compression, fast)",
    ctv: "KV cache value quantization (-ctv): f16 (best quality), q8_0, turbo3 (4.6x compression, fast)",
    flash_attn: "Flash Attention (-fa): auto, on, off. Speeds up inference when supported.",
    // Batching
    batch_size: "Logical batch size (-b): max tokens processed in parallel during prompt evaluation.",
    ubatch_size: "Physical micro-batch size (-ub). Controls memory granularity. Usually same as batch_size.",
    parallel_slots: "Concurrent inference slots (-np). Each reserves context_size tokens of KV cache VRAM.",
    // GPU Distribution
    tensor_split: "VRAM distribution ratio across GPUs (-ts, e.g. '7,8,8,8'). Empty for single GPU.",
    split_mode: "Multi-GPU split strategy (--split-mode): layer (default) or row (split large tensors).",
    main_gpu: "GPU index for model computation (-mg, 0-based). Others used for offloading.",
    // Threading
    threads: "CPU threads for generation (-t). Leave empty = all available CPUs.",
    threads_batch: "CPU threads for batch/prompt processing (-tb). Leave empty = same as threads.",
    // Rope Scaling
    rope_scaling: "RoPE method (--rope-scaling): yarn (extended ctx), linear, none. Auto yarn for ctx >262144.",
    rope_freq_base: "RoPE base frequency (--rope-freq-base). Model default if not set.",
    rope_freq_scale: "RoPE frequency scale (--rope-freq-scale). Auto-calculated for YaRN if not set.",
    // Speculative Decoding
    ngram_spec: "Enable ngram-mod speculative decoding (--spec-type ngram-mod). +47% on pure transformers.",
    spec_ngram_size: "N-gram lookup table size (--spec-ngram-size-n). Larger = better predictions, more memory.",
    draft_min: "Min tokens to draft per step (--draft-min). Default 8.",
    draft_max: "Max tokens to draft per step (--draft-max). Default 24.",
    draft_model: "Draft model path (-md) for model-based speculative decoding (alternative to ngram).",
    // Advanced
    seed: "Random seed (-s). -1 = random. Set for reproducible generation.",
    system_prompt_file: "Path to text file containing system prompt (--system-prompt-file).",
    extra_args: "Additional llama-server CLI flags (space-separated). E.g. '--log-verbosity 2 --no-perf'",
};

let presets = [];
let serverRunning = false;
let prevLogLen = 0;

// --- Settings Persistence (backend) ---

let settingsSaveTimer = null;

function collectSettings() {
    return {
        preset_id: document.getElementById('preset-select').value,
        port: parseInt(document.getElementById('port').value) || 8080,
        ctx: document.getElementById('ctx').value,
        ctk: document.getElementById('ctk').value,
        ctv: document.getElementById('ctv').value,
        ts: document.getElementById('ts').value,
        batch: document.getElementById('batch').value,
        slots: document.getElementById('slots').value,
        no_mmap: document.getElementById('no_mmap').checked,
        ngram_spec: document.getElementById('ngram_spec').checked,
        llama_server_path: document.getElementById('set-server-path').value,
        llama_server_cwd: document.getElementById('set-server-cwd').value,
        models_dir: document.getElementById('set-models-dir').value,
    };
}

function saveSettings() {
    // Debounce: wait 400ms of inactivity before saving
    clearTimeout(settingsSaveTimer);
    settingsSaveTimer = setTimeout(() => {
        fetch('/api/settings', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(collectSettings()),
        }).catch(() => {});
    }, 400);
}

function applySettings(s) {
    if (!s) return;
    if (s.port) document.getElementById('port').value = s.port;
    if (s.ctx) document.getElementById('ctx').value = s.ctx;
    if (s.ctk) document.getElementById('ctk').value = s.ctk;
    if (s.ctv) document.getElementById('ctv').value = s.ctv;
    if (s.ts !== undefined) document.getElementById('ts').value = s.ts;
    if (s.batch) document.getElementById('batch').value = s.batch;
    if (s.slots) document.getElementById('slots').value = s.slots;
    if (s.no_mmap !== undefined) document.getElementById('no_mmap').checked = s.no_mmap;
    if (s.ngram_spec !== undefined) document.getElementById('ngram_spec').checked = s.ngram_spec;
    // Server paths
    if (s.llama_server_path !== undefined) document.getElementById('set-server-path').value = s.llama_server_path;
    if (s.llama_server_cwd !== undefined) document.getElementById('set-server-cwd').value = s.llama_server_cwd;
    if (s.models_dir !== undefined) document.getElementById('set-models-dir').value = s.models_dir;
    updateServerPathsInfo();
}

// Auto-save on any control bar change
document.getElementById('controls').addEventListener('input', saveSettings);
document.getElementById('controls').addEventListener('change', saveSettings);

// Load presets and populate dropdown
async function loadPresets(selectId) {
    const [presetsResp, settingsResp] = await Promise.all([
        fetch('/api/presets'),
        selectId === undefined ? fetch('/api/settings') : Promise.resolve(null),
    ]);
    presets = await presetsResp.json();
    const saved = settingsResp ? await settingsResp.json() : null;

    const sel = document.getElementById('preset-select');
    sel.innerHTML = '';
    presets.forEach(p => {
        const opt = document.createElement('option');
        opt.value = p.id;
        opt.textContent = p.name;
        sel.appendChild(opt);
    });

    // Determine which preset to select
    // Only restore overrides if settings were previously saved (preset_id non-empty)
    const hasSaved = saved && saved.preset_id;
    const targetId = selectId ?? (hasSaved ? saved.preset_id : null);

    if (targetId) {
        const exists = presets.find(p => p.id === targetId);
        if (exists) {
            sel.value = targetId;
            applyPresetById(targetId);
            if (selectId === undefined && hasSaved) applySettings(saved);
            return;
        }
    }
    // Default: select first
    if (presets.length > 0) {
        sel.value = presets[0].id;
        applyPresetById(presets[0].id);
        if (hasSaved) applySettings(saved);
    }
}

function applyPresetById(id) {
    const p = presets.find(pr => pr.id === id);
    if (!p) return;
    document.getElementById('ctx').value = p.context_size;
    document.getElementById('ctk').value = p.ctk;
    document.getElementById('ctv').value = p.ctv;
    document.getElementById('ts').value = p.tensor_split;
    document.getElementById('batch').value = p.batch_size;
    document.getElementById('slots').value = p.parallel_slots || 1;
    document.getElementById('no_mmap').checked = !!p.no_mmap;
    document.getElementById('ngram_spec').checked = !!p.ngram_spec;
    saveSettings();
}

// Initial load
loadPresets();
loadGpuEnv();

// --- GPU Environment ---

async function loadGpuEnv() {
    try {
        const resp = await fetch('/api/gpu-env');
        const data = await resp.json();
        const env = data.env;
        const archs = data.architectures;
        const detected = data.detected;

        // Populate architecture dropdown
        const sel = document.getElementById('gpu-env-arch');
        sel.innerHTML = '';
        archs.forEach(a => {
            const opt = document.createElement('option');
            opt.value = a.id;
            let label = a.name;
            if (detected && detected.arch === a.id) {
                label += ' (detected)';
            }
            opt.textContent = label;
            sel.appendChild(opt);
        });
        sel.value = env.arch;

        document.getElementById('gpu-env-devices').value = env.devices;
        document.getElementById('gpu-env-rocm-path').value = env.rocm_path || '/opt/rocm';

        // Show detection info
        const infoEl = document.getElementById('gpu-detected-info');
        const summaryInfo = document.getElementById('gpu-env-info');
        if (detected) {
            infoEl.textContent = 'Detected: ' + detected.count + 'x ' + detected.arch + ' (' + detected.names.join(', ') + ')';
            summaryInfo.textContent = ' \u2014 ' + detected.count + 'x ' + detected.arch;
        } else {
            infoEl.textContent = 'No GPU detected via rocminfo/nvidia-smi';
            summaryInfo.textContent = '';
        }
    } catch (err) {
        console.error('Failed to load GPU env:', err);
    }
}

async function saveGpuEnv() {
    const env = {
        arch: document.getElementById('gpu-env-arch').value,
        devices: document.getElementById('gpu-env-devices').value.trim(),
        rocm_path: document.getElementById('gpu-env-rocm-path').value.trim() || '/opt/rocm',
        extra_env: [],
    };
    try {
        const resp = await fetch('/api/gpu-env', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(env),
        });
        const data = await resp.json();
        if (data.ok) {
            document.getElementById('gpu-detected-info').textContent = 'Saved. Changes apply on next server start.';
        }
    } catch (err) {
        showToast('Failed to save GPU env: ' + err.message, 'error');
    }
}

// --- Server Paths ---

function updateServerPathsInfo() {
    const path = document.getElementById('set-server-path').value;
    const dir = document.getElementById('set-models-dir').value;
    const parts = [];
    if (path) parts.push(path.split('/').pop() || path);
    if (dir) parts.push('models: ' + (dir.split('/').pop() || dir));
    document.getElementById('server-paths-info').textContent = parts.length ? ' \u2014 ' + parts.join(', ') : '';
}

function saveServerPaths() {
    saveSettings();
    // Force immediate save (bypass debounce)
    clearTimeout(settingsSaveTimer);
    fetch('/api/settings', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(collectSettings()),
    }).then(() => {
        showToast('Server paths saved', 'success');
        updateServerPathsInfo();
    }).catch(() => showToast('Failed to save', 'error'));
}

// --- File Browser ---

let fbTargetId = '';
let fbFilter = '';
let fbCurrentPath = '';

function openFileBrowser(targetId, filter) {
    fbTargetId = targetId;
    fbFilter = filter === 'dir' ? '' : (filter || '');
    const modal = document.getElementById('file-browser-modal');
    // If target already has a path, start there; otherwise home
    const current = document.getElementById(targetId).value;
    let startPath = '';
    if (current) {
        // Use parent directory of current value
        const parts = current.split('/');
        parts.pop();
        startPath = parts.join('/') || '/';
    }
    // Show/hide "Select This Folder" for dir-mode
    const selectBtn = modal.querySelector('.btn-modal-save');
    selectBtn.style.display = filter === 'dir' ? '' : 'none';
    modal.classList.add('open');
    fileBrowserGo(startPath);
}

function closeFileBrowser() {
    document.getElementById('file-browser-modal').classList.remove('open');
}

document.getElementById('file-browser-modal').addEventListener('click', e => {
    if (e.target === e.currentTarget) closeFileBrowser();
});

async function fileBrowserGo(path) {
    const entriesEl = document.getElementById('fb-entries');
    entriesEl.innerHTML = '<div class="fb-empty">Loading...</div>';
    const params = new URLSearchParams();
    if (path) params.set('path', path);
    if (fbFilter) params.set('filter', fbFilter);
    try {
        const resp = await fetch('/api/browse?' + params);
        const data = await resp.json();
        if (data.error) {
            entriesEl.innerHTML = '<div class="fb-empty">' + data.error + '</div>';
            return;
        }
        fbCurrentPath = data.path;
        document.getElementById('fb-path-input').value = data.path;
        if (data.entries.length === 0) {
            entriesEl.innerHTML = '<div class="fb-empty">Empty directory</div>';
            return;
        }
        entriesEl.innerHTML = data.entries.map(e => {
            if (e.is_dir) {
                return '<div class="fb-entry fb-entry-dir" onclick="fileBrowserGo(\'' + e.path.replace(/'/g, "\\'") + '\')">' +
                    '<span class="fb-entry-icon">\u{1F4C1}</span>' +
                    '<span class="fb-entry-name">' + e.name + '</span></div>';
            } else {
                return '<div class="fb-entry fb-entry-file fb-match" onclick="fileBrowserSelect(\'' + e.path.replace(/'/g, "\\'") + '\')">' +
                    '<span class="fb-entry-icon">\u{1F4C4}</span>' +
                    '<span class="fb-entry-name">' + e.name + '</span>' +
                    '<span class="fb-entry-size">' + e.size_display + '</span></div>';
            }
        }).join('');
    } catch (err) {
        entriesEl.innerHTML = '<div class="fb-empty">Error: ' + err.message + '</div>';
    }
}

function fileBrowserUp() {
    if (fbCurrentPath && fbCurrentPath !== '/') {
        const parts = fbCurrentPath.split('/');
        parts.pop();
        fileBrowserGo(parts.join('/') || '/');
    }
}

function fileBrowserSelect(path) {
    document.getElementById(fbTargetId).value = path;
    document.getElementById(fbTargetId).dispatchEvent(new Event('input', { bubbles: true }));
    closeFileBrowser();
}

function fileBrowserSelectCurrent() {
    document.getElementById(fbTargetId).value = fbCurrentPath;
    document.getElementById(fbTargetId).dispatchEvent(new Event('input', { bubbles: true }));
    closeFileBrowser();
}

// Close file browser on Escape
document.addEventListener('keydown', e => {
    if (e.key === 'Escape' && document.getElementById('file-browser-modal').classList.contains('open')) {
        closeFileBrowser();
        e.stopImmediatePropagation();
    }
}, true);

// --- Preset Selection ---

document.getElementById('preset-select').addEventListener('change', e => {
    applyPresetById(e.target.value);
});

// --- Toast Notifications ---

function showToast(message, type = 'error') {
    const container = document.getElementById('toast-container');
    const toast = document.createElement('div');
    toast.className = 'toast toast-' + type;
    toast.textContent = message;
    container.appendChild(toast);
    requestAnimationFrame(() => { toast.classList.add('show'); });
    setTimeout(() => {
        toast.classList.remove('show');
        setTimeout(() => toast.remove(), 300);
    }, 3500);
}

// --- Preset Modal ---

function setVal(id, v) { document.getElementById(id).value = v ?? ''; }
function setChk(id, v) { document.getElementById(id).checked = !!v; }
function setOpt(id, v) { document.getElementById(id).value = v || ''; }
function numOrEmpty(id, v) { document.getElementById(id).value = v != null ? v : ''; }

function clearFieldErrors() {
    document.querySelectorAll('#preset-form .field-error').forEach(el => el.classList.remove('field-error'));
}

function openPresetModal(mode) {
    const modal = document.getElementById('preset-modal');
    const title = document.getElementById('modal-title');
    const badge = document.getElementById('modal-edit-badge');
    const form = document.getElementById('preset-form');
    form.reset();
    clearFieldErrors();

    if (mode === 'edit') {
        const id = document.getElementById('preset-select').value;
        const p = presets.find(pr => pr.id === id);
        if (!p) { showToast('No preset selected', 'warn'); return; }
        title.textContent = 'Edit Preset';
        badge.classList.add('visible');
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
        badge.classList.remove('visible');
        setVal('modal-preset-id', '');
        setVal('modal-context-size', 128000);
        setVal('modal-ctk', 'q8_0');
        setVal('modal-ctv', 'f16');
        setVal('modal-batch-size', 2048);
        setVal('modal-ubatch-size', 2048);
        setVal('modal-parallel-slots', 1);
    }

    modal.classList.add('open');
    // Scroll modal body to top
    const body = modal.querySelector('.modal-body');
    if (body) body.scrollTop = 0;
}

function closePresetModal() {
    const modal = document.getElementById('preset-modal');
    modal.classList.remove('open');
}

// Close modal on overlay click
document.getElementById('preset-modal').addEventListener('click', e => {
    if (e.target === e.currentTarget) closePresetModal();
});

// Close modal on Escape key
document.addEventListener('keydown', e => {
    if (e.key === 'Escape' && document.getElementById('preset-modal').classList.contains('open')) {
        closePresetModal();
    }
});

function intOrNull(id) { const v = document.getElementById(id).value; return v !== '' ? parseInt(v) : null; }
function floatOrNull(id) { const v = document.getElementById(id).value; return v !== '' ? parseFloat(v) : null; }
function strVal(id) { return document.getElementById(id).value.trim(); }

async function savePreset(event) {
    event.preventDefault();
    clearFieldErrors();

    const id = document.getElementById('modal-preset-id').value;
    const preset = {
        // Model & Memory
        name: strVal('modal-name'),
        model_path: strVal('modal-model-path'),
        gpu_layers: intOrNull('modal-gpu-layers'),
        no_mmap: document.getElementById('modal-no-mmap').checked,
        mlock: document.getElementById('modal-mlock').checked,
        // Context & KV
        context_size: parseInt(document.getElementById('modal-context-size').value) || 128000,
        ctk: strVal('modal-ctk') || 'q8_0',
        ctv: strVal('modal-ctv') || 'f16',
        flash_attn: strVal('modal-flash-attn'),
        // Batching
        batch_size: parseInt(document.getElementById('modal-batch-size').value) || 2048,
        ubatch_size: parseInt(document.getElementById('modal-ubatch-size').value) || 2048,
        parallel_slots: parseInt(document.getElementById('modal-parallel-slots').value) || 1,
        // GPU
        tensor_split: strVal('modal-tensor-split'),
        split_mode: strVal('modal-split-mode'),
        main_gpu: intOrNull('modal-main-gpu'),
        // Threading
        threads: intOrNull('modal-threads'),
        threads_batch: intOrNull('modal-threads-batch'),
        // Rope
        rope_scaling: strVal('modal-rope-scaling'),
        rope_freq_base: floatOrNull('modal-rope-freq-base'),
        rope_freq_scale: floatOrNull('modal-rope-freq-scale'),
        // Spec decoding
        ngram_spec: document.getElementById('modal-ngram-spec').checked,
        spec_ngram_size: intOrNull('modal-spec-ngram-size'),
        draft_min: intOrNull('modal-draft-min'),
        draft_max: intOrNull('modal-draft-max'),
        draft_model: strVal('modal-draft-model'),
        // Advanced
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
                headers: { 'Content-Type': 'application/json' },
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
                headers: { 'Content-Type': 'application/json' },
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

async function copyPreset() {
    const id = document.getElementById('preset-select').value;
    const p = presets.find(pr => pr.id === id);
    if (!p) { showToast('No preset selected', 'warn'); return; }

    const copy = Object.assign({}, p);
    delete copy.id;
    copy.name = p.name + ' (copy)';

    try {
        const resp = await fetch('/api/presets', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
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

async function deletePreset() {
    const id = document.getElementById('preset-select').value;
    const p = presets.find(pr => pr.id === id);
    if (!p) { showToast('No preset selected', 'warn'); return; }
    if (!confirm('Delete preset "' + p.name + '"?')) return;

    try {
        const resp = await fetch('/api/presets/' + encodeURIComponent(id), { method: 'DELETE' });
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

async function resetPresets() {
    if (!confirm('Reset all presets to built-in defaults? Custom presets will be removed.')) return;
    try {
        const resp = await fetch('/api/presets/reset', { method: 'POST' });
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

// Clear field errors on input
['modal-name', 'modal-model-path'].forEach(id => {
    document.getElementById(id).addEventListener('input', function() {
        this.classList.remove('field-error');
    });
});

// --- End Preset Modal ---

function getConfig() {
    const id = document.getElementById('preset-select').value;
    const p = presets.find(pr => pr.id === id) || {};
    // Quick-start bar overrides basic fields; all other params come from preset
    const batchVal = parseInt(document.getElementById('batch').value) || 2048;
    return {
        model_path: p.model_path || '',
        context_size: parseInt(document.getElementById('ctx').value) || 128000,
        ctk: document.getElementById('ctk').value || 'q8_0',
        ctv: document.getElementById('ctv').value || 'f16',
        tensor_split: document.getElementById('ts').value || '',
        batch_size: batchVal,
        ubatch_size: batchVal,
        no_mmap: document.getElementById('no_mmap').checked,
        port: parseInt(document.getElementById('port').value) || 8080,
        ngram_spec: document.getElementById('ngram_spec').checked,
        parallel_slots: parseInt(document.getElementById('slots').value) || 1,
        // Pass-through from preset (not editable in quick-start bar)
        gpu_layers: p.gpu_layers ?? null,
        mlock: p.mlock || false,
        flash_attn: p.flash_attn || '',
        split_mode: p.split_mode || '',
        main_gpu: p.main_gpu ?? null,
        threads: p.threads ?? null,
        threads_batch: p.threads_batch ?? null,
        rope_scaling: p.rope_scaling || '',
        rope_freq_base: p.rope_freq_base ?? null,
        rope_freq_scale: p.rope_freq_scale ?? null,
        draft_model: p.draft_model || '',
        draft_min: p.draft_min ?? null,
        draft_max: p.draft_max ?? null,
        spec_ngram_size: p.spec_ngram_size ?? null,
        seed: p.seed ?? null,
        system_prompt_file: p.system_prompt_file || '',
        extra_args: p.extra_args || '',
    };
}

async function doStart() {
    document.getElementById('btn-start').disabled = true;
    const resp = await fetch('/api/start', {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify(getConfig()),
    });
    const data = await resp.json();
    if (!data.ok) showToast('Start failed: ' + (data.error || 'unknown'), 'error');
}

async function doStop() {
    document.getElementById('btn-stop').disabled = true;
    await fetch('/api/stop', { method: 'POST' });
}

// WebSocket
const ws = new WebSocket((location.protocol === 'https:' ? 'wss://' : 'ws://') + location.host + '/ws');
ws.onmessage = e => {
    const d = JSON.parse(e.data);

    // Server state
    serverRunning = d.server_running;
    const dot = document.getElementById('status-dot');
    const txt = document.getElementById('status-text');
    dot.className = 'status-dot ' + (serverRunning ? 'running' : 'stopped');
    txt.textContent = serverRunning ? 'Running' : 'Stopped';
    document.getElementById('btn-start').disabled = serverRunning;
    document.getElementById('btn-stop').disabled = !serverRunning;

    // Inference
    const l = d.llama;
    document.getElementById('m-prompt').textContent = l.prompt_tokens_per_sec > 0 ? l.prompt_tokens_per_sec.toFixed(1) + ' t/s' : '\u2014';
    document.getElementById('m-gen').textContent = l.generation_tokens_per_sec > 0 ? l.generation_tokens_per_sec.toFixed(1) + ' t/s' : '\u2014';
    if (l.kv_cache_max > 0) {
        const pct = ((l.kv_cache_tokens / l.kv_cache_max) * 100).toFixed(1);
        document.getElementById('m-ctx').textContent = l.kv_cache_tokens + ' / ' + l.kv_cache_max + ' (' + pct + '%)';
    } else {
        document.getElementById('m-ctx').textContent = '\u2014';
    }
    document.getElementById('m-slots').textContent = l.slots_idle + l.slots_processing > 0 ? l.slots_idle + ' idle / ' + l.slots_processing + ' busy' : '\u2014';

    const statusEl = document.getElementById('m-status');
    statusEl.textContent = l.status || '\u2014';
    statusEl.className = 'metric-value ' + (l.status === 'ok' ? 'status-ok' : l.status === 'no slot available' ? 'status-busy' : 'status-err');

    // GPU table
    const tbody = document.getElementById('gpu-rows');
    tbody.innerHTML = Object.entries(d.gpu).map(([card, m]) => {
        const capped = m.power_consumption >= m.power_limit && m.power_limit > 0;
        const pcls = capped ? 'value capped' : 'value power';
        const ptxt = capped ? m.power_consumption.toFixed(1) + 'W!' : m.power_consumption.toFixed(1) + 'W / ' + m.power_limit + 'W';
        const vpct = m.vram_total > 0 ? Math.round((m.vram_used / m.vram_total) * 100) : 0;
        return '<tr>' +
            '<td class="card value">' + card + '</td>' +
            '<td class="value temp">' + Math.round(m.temp) + 'C</td>' +
            '<td class="value load">' + m.load + '%</td>' +
            '<td class="value vram">' + vpct + '%</td>' +
            '<td class="' + pcls + '">' + ptxt + '</td>' +
            '<td class="value sclk">' + m.sclk_mhz + 'MHz</td>' +
            '<td class="value mclk">' + m.mclk_mhz + 'MHz</td>' +
            '</tr>';
    }).join('');

    // Logs (update both panels)
    const logs = d.logs || [];
    if (logs.length !== prevLogLen) {
        const logText = logs.join('\n');
        ['log-panel-server', 'log-panel-monitor'].forEach(id => {
            const el = document.getElementById(id);
            const wasAtBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
            el.textContent = logText;
            if (wasAtBottom) el.scrollTop = el.scrollHeight;
        });
        prevLogLen = logs.length;
    }

    // Tab badges
    document.getElementById('badge-server').textContent = serverRunning ? ' Running' : ' Stopped';

    const genTxt = l.generation_tokens_per_sec > 0 ? l.generation_tokens_per_sec.toFixed(1) + 't/s' : '';
    const gpuEntries = Object.entries(d.gpu);
    let gpuBadge = '';
    if (gpuEntries.length > 0) {
        const maxTemp = Math.max(...gpuEntries.map(([,m]) => m.temp));
        gpuBadge = maxTemp.toFixed(0) + 'C';
    }
    document.getElementById('badge-monitor').textContent = [genTxt, gpuBadge].filter(Boolean).join(' \u00b7 ') ? ' ' + [genTxt, gpuBadge].filter(Boolean).join(' \u00b7 ') : '';

    document.getElementById('badge-chat').textContent = chatHistory.length > 0 ? ' ' + chatHistory.length + ' msg' : '';
};
ws.onerror = e => console.error('WebSocket error:', e);
ws.onclose = () => { document.getElementById('status-text').textContent = 'Disconnected'; };

// Markdown
if (typeof marked !== 'undefined') {
    marked.setOptions({ breaks: true, gfm: true });
}
function renderMd(src) {
    if (typeof marked !== 'undefined') {
        try { return marked.parse(src); } catch(_) {}
    }
    return src.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/\n/g,'<br>');
}

// Chat
let chatHistory = [];
let chatBusy = false;

document.getElementById('chat-input').addEventListener('keydown', e => {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendChat(); }
});

function clearChat() {
    chatHistory = [];
    document.getElementById('chat-messages').innerHTML = '';
}

function chatScroll() {
    const c = document.getElementById('chat-messages');
    c.scrollTop = c.scrollHeight;
}

function appendMsg(role, text) {
    const el = document.createElement('div');
    el.className = 'msg msg-' + role;
    el.textContent = text;
    document.getElementById('chat-messages').appendChild(el);
    chatScroll();
    return el;
}

async function sendChat() {
    if (chatBusy) return;
    const input = document.getElementById('chat-input');
    const text = input.value.trim();
    if (!text) return;
    input.value = '';

    chatHistory.push({ role: 'user', content: text });
    appendMsg('user', text);

    const chatPort = document.getElementById('port').value || '8080';
    const url = '/api/chat?port=' + encodeURIComponent(chatPort);

    chatBusy = true;
    document.getElementById('btn-send').disabled = true;

    let thinkEl = null;
    let thinkContent = '';
    const msgEl = appendMsg('assistant', '');
    let msgContent = '';

    try {
        const resp = await fetch(url, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                messages: chatHistory,
                stream: true,
                temperature: 1.0,
                top_p: 0.95,
                top_k: 40,
                min_p: 0.01,
                repeat_penalty: 1.0,
            }),
        });

        const reader = resp.body.getReader();
        const decoder = new TextDecoder();
        let buf = '';

        while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            buf += decoder.decode(value, { stream: true });

            const lines = buf.split('\n');
            buf = lines.pop() || '';

            for (const line of lines) {
                if (!line.startsWith('data: ')) continue;
                const payload = line.slice(6).trim();
                if (payload === '[DONE]') continue;
                try {
                    const obj = JSON.parse(payload);
                    const delta = obj.choices && obj.choices[0] && obj.choices[0].delta;
                    if (!delta) continue;

                    // Reasoning / thinking content
                    const rc = delta.reasoning_content || '';
                    if (rc) {
                        thinkContent += rc;
                        if (!thinkEl) {
                            thinkEl = document.createElement('details');
                            thinkEl.className = 'msg msg-thinking';
                            thinkEl.innerHTML = '<summary>thinking...</summary><span></span>';
                            document.getElementById('chat-messages').insertBefore(thinkEl, msgEl);
                        }
                        thinkEl.querySelector('span').textContent = thinkContent;
                    }

                    // Regular content
                    const c = delta.content || '';
                    if (c) {
                        msgContent += c;
                        msgEl.innerHTML = renderMd(msgContent);
                    }
                } catch (_) {}
            }
            chatScroll();
        }
    } catch (err) {
        msgEl.textContent = '[error] ' + err.message;
        msgEl.style.color = '#bf616a';
    }

    if (msgContent) {
        chatHistory.push({ role: 'assistant', content: msgContent });
    }
    chatBusy = false;
    document.getElementById('btn-send').disabled = false;
}
if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js').catch(() => {});
}
