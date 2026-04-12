function switchTab(name) {
    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
    document.getElementById('page-' + name).classList.add('active');
    document.getElementById('tab-' + name).classList.add('active');
}

let lastServerState = null;
let lastLlamaMetrics = null;
let lastSystemMetrics = null;
let lastGpuMetrics = null;
let currentPollInterval = 5000;

let presets = [];
let serverRunning = false;
let prevLogLen = 0;

// --- Settings Persistence (backend) ---

let settingsSaveTimer = null;

   function collectSettings() {
        const endpoint = document.getElementById('server-endpoint').value.trim();
        let port = 8001;
        if (endpoint) {
            try {
                const url = new URL(endpoint);
                port = parseInt(url.port) || 8001;
            } catch(e) {
                // invalid URL, use default
            }
        }
        return {
            preset_id: document.getElementById('preset-select').value,
            port: port,
            llama_server_path: document.getElementById('set-server-path').value,
            llama_server_cwd: document.getElementById('set-server-cwd').value,
            models_dir: '',
            server_endpoint: endpoint,
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
        if (s.llama_server_path !== undefined) document.getElementById('set-server-path').value = s.llama_server_path;
        if (s.llama_server_cwd !== undefined) document.getElementById('set-server-cwd').value = s.llama_server_cwd;
        if (s.server_endpoint) document.getElementById('server-endpoint').value = s.server_endpoint;
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

    const targetId = selectId ?? (saved?.preset_id || null);
    if (targetId && presets.find(p => p.id === targetId)) {
        sel.value = targetId;
    } else if (presets.length > 0) {
        sel.value = presets[0].id;
    }

    if (selectId === undefined && saved) applySettings(saved);
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

        const sel = document.getElementById('gpu-env-arch');
        sel.innerHTML = '';
        archs.forEach(a => {
            const opt = document.createElement('option');
            opt.value = a.id;
            let label = a.name;
            if (detected && detected.arch === a.id) label += ' (detected)';
            opt.textContent = label;
            sel.appendChild(opt);
        });
        sel.value = env.arch;

        document.getElementById('gpu-env-devices').value = env.devices;
        document.getElementById('gpu-env-rocm-path').value = env.rocm_path || '/opt/rocm';

        const infoEl = document.getElementById('gpu-detected-info');
        const summaryInfo = document.getElementById('gpu-env-info');
        if (detected) {
            infoEl.textContent = 'Detected: ' + detected.count + 'x ' + detected.arch + ' (' + detected.names.join(', ') + ')';
            summaryInfo.textContent = '\u2014 ' + detected.count + 'x ' + detected.arch;
        } else {
            infoEl.textContent = 'No GPU detected via rocminfo/nvidia-smi';
            summaryInfo.textContent = '';
        }
    } catch (err) {
        console.error('Failed to load GPU env:', err);
    }
}

// --- Config Modal ---

function openConfigModal() {
    document.getElementById('config-modal').classList.add('open');
}

function closeConfigModal() {
    document.getElementById('config-modal').classList.remove('open');
}

document.getElementById('config-modal').addEventListener('click', e => {
    if (e.target === e.currentTarget) closeConfigModal();
});

function saveConfig() {
    // Save server paths via settings
    clearTimeout(settingsSaveTimer);
    fetch('/api/settings', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(collectSettings()),
    }).catch(() => {});

    // Save GPU env
    const env = {
        arch: document.getElementById('gpu-env-arch').value,
        devices: document.getElementById('gpu-env-devices').value.trim(),
        rocm_path: document.getElementById('gpu-env-rocm-path').value.trim() || '/opt/rocm',
        extra_env: [],
    };
    fetch('/api/gpu-env', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(env),
    }).catch(() => {});

    closeConfigModal();
    showToast('Configuration saved', 'success');
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
    document.getElementById(fbTargetId).value = path || fbCurrentPath;
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

document.getElementById('preset-select').addEventListener('change', () => saveSettings());

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
    const form = document.getElementById('preset-form');
    form.reset();
    clearFieldErrors();

    if (mode === 'edit') {
        const id = document.getElementById('preset-select').value;
        const p = presets.find(pr => pr.id === id);
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

// Close modals on Escape key
document.addEventListener('keydown', e => {
    if (e.key === 'Escape' && document.getElementById('config-modal').classList.contains('open')) {
        closeConfigModal();
    } else if (e.key === 'Escape' && document.getElementById('preset-modal').classList.contains('open')) {
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
         // Generation
         temperature: floatOrNull('modal-temperature'),
         top_p: floatOrNull('modal-top-p'),
         top_k: floatOrNull('modal-top-k'),
         min_p: floatOrNull('modal-min-p'),
         repeat_penalty: floatOrNull('modal-repeat-penalty'),
         n_cpu_moe: intOrNull('modal-n-cpu-moe'),
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
    return {
        model_path: p.model_path || '',
        context_size: p.context_size || 128000,
        ctk: p.ctk || 'q8_0',
        ctv: p.ctv || 'f16',
        tensor_split: p.tensor_split || '',
      batch_size: p.batch_size || 2048,
         ubatch_size: p.ubatch_size || p.batch_size || 2048,
         no_mmap: !!p.no_mmap,
         port: parseInt(document.getElementById('port').value) || 8001,
         ngram_spec: !!p.ngram_spec,
         parallel_slots: p.parallel_slots || 1,
         // Generation
         temperature: p.temperature,
         top_p: p.top_p,
         top_k: p.top_k,
         min_p: p.min_p,
         repeat_penalty: p.repeat_penalty,
         n_cpu_moe: p.n_cpu_moe,
         gpu_layers: p.gpu_layers ?? null,
        mlock: !!p.mlock,
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
    const config = getConfig();
    if (!config.model_path) {
        showToast('No model path set. Edit the preset to select a model.', 'error');
        return;
    }
    document.getElementById('btn-start').disabled = true;
    await doKillLlamaInternal();
    const resp = await fetch('/api/start', {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify(config),
    });
    const data = await resp.json();
    if (!data.ok) showToast('Start failed: ' + (data.error || 'unknown'), 'error');
}

async function doKillLlamaInternal() {
    try {
        await fetch('/api/kill-llama', { method: 'POST' });
    } catch(e) {
        // Ignore errors from kill, just try to continue
    }
}

async function doAttach() {
    document.getElementById('btn-attach').disabled = true;
    const endpoint = document.getElementById('server-endpoint').value.trim();
    if (!endpoint) {
        showToast('Please enter a server endpoint', 'error');
        document.getElementById('btn-attach').disabled = false;
        return;
    }
    const resp = await fetch('/api/attach', {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({ endpoint }),
    });
    const data = await resp.json();
    if (!data.ok) showToast('Attach failed: ' + (data.error || 'unknown'), 'error');
    else showToast('Attached to server', 'success');
    document.getElementById('btn-attach').disabled = false;
}

async function doKillLlamaInternal() {
    try {
        await fetch('/api/kill-llama', { method: 'POST' });
    } catch(e) {
        // Ignore errors from kill, just try to continue
    }
}

async function doKillLlama() {
    if (!confirm('Kill all running llama-server processes?')) return;
    document.getElementById('btn-kill').disabled = true;
    try {
        const resp = await fetch('/api/kill-llama', { method: 'POST' });
        const data = await resp.json();
        if (!data.ok) showToast('Kill failed: ' + (data.error || 'unknown'), 'error');
        else showToast('llama-server killed', 'success');
    } catch (e) {
        showToast('Kill failed: ' + e.message, 'error');
    }
    document.getElementById('btn-kill').disabled = false;
}

async function doStop() {
    document.getElementById('btn-stop').disabled = true;
    await fetch('/api/stop', { method: 'POST' });
    await doKillLlamaInternal();
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

    lastServerState = d.server_running;
    lastLlamaMetrics = d.llama;
    lastSystemMetrics = d.system || null;
    lastGpuMetrics = d.gpu || {};

    // Inference metrics
    const l = lastLlamaMetrics;
    document.getElementById('m-prompt').textContent = l && l.prompt_tokens_per_sec > 0 ? l.prompt_tokens_per_sec.toFixed(1) + ' t/s' : '\u2014';
    document.getElementById('m-gen').textContent = l && l.generation_tokens_per_sec > 0 ? l.generation_tokens_per_sec.toFixed(1) + ' t/s' : '\u2014';
    if (l && l.kv_cache_max > 0) {
        const pct = ((l.kv_cache_tokens / l.kv_cache_max) * 100).toFixed(1);
        document.getElementById('m-ctx').textContent = l.kv_cache_tokens + ' / ' + l.kv_cache_max + ' (' + pct + '%)';
    } else {
        document.getElementById('m-ctx').textContent = '\u2014';
    }
    if (l) {
        document.getElementById('m-slots').textContent = (l.slots_idle || 0) + (l.slots_processing || 0) > 0 ? (l.slots_idle || 0) + ' idle / ' + (l.slots_processing || 0) + ' busy' : '\u2014';
    } else {
        document.getElementById('m-slots').textContent = '\u2014';
    }

    const statusEl = document.getElementById('m-status');
    statusEl.textContent = l && l.status ? l.status : '\u2014';
    statusEl.className = 'metric-value ' + (l && l.status === 'ok' ? 'status-ok' : l && l.status === 'no slot available' ? 'status-busy' : 'status-err');

    // System table
    const sys = lastSystemMetrics;
    const sysRowsEl = document.getElementById('system-rows');
    if (sysRowsEl) {
        sysRowsEl.innerHTML = '<tr>' +
            '<td class="card value">System</td>' +
            '<td class="value temp">' + (sys && sys.cpu_temp > 0 ? Math.round(sys.cpu_temp) + 'C' : '\u2014') + '</td>' +
            '<td class="value load">' + (sys && sys.cpu_load > 0 ? sys.cpu_load + '%' : '\u2014') + '</td>' +
            '<td class="value sclk">' + (sys && sys.cpu_clock_mhz > 0 ? sys.cpu_clock_mhz + 'MHz' : '\u2014') + '</td>' +
            '<td class="value vram">' + (sys && sys.ram_total_gb > 0 ? ((sys.ram_used_gb / sys.ram_total_gb) * 100).toFixed(0) + '% (' + (sys.ram_used_gb / 1024).toFixed(1) + ' GB)' : '\u2014') + '</td>' +
            '<td class="value">' + (sys && sys.cpu_name ? sys.cpu_name.substring(0, 20) : '\u2014') + '</td>' +
            '</tr>';
    }

    // GPU table
    const tbody = document.getElementById('gpu-rows');
    tbody.innerHTML = Object.entries(d.gpu).map(([card, m]) => {
        const capped = m.power_consumption >= m.power_limit && m.power_limit > 0;
        const pcls = capped ? 'value capped' : 'value power';
        const ptxt = capped ? m.power_consumption.toFixed(1) + 'W!' : m.power_consumption.toFixed(1) + 'W / ' + m.power_limit + 'W';
        const vpct = m.vram_total > 0 ? Math.round((m.vram_used / m.vram_total) * 100) : 0;
        const vgb = m.vram_total > 0 ? (m.vram_used / 1024).toFixed(1) : 0;
        return '<tr>' +
            '<td class="card value">' + card + '</td>' +
            '<td class="value temp">' + Math.round(m.temp) + 'C</td>' +
            '<td class="value load">' + m.load + '%</td>' +
            '<td class="value vram">' + vpct + '% (' + vgb + ' GB)</td>' +
            '<td class="' + pcls + '">' + ptxt + '</td>' +
            '<td class="value sclk">' + m.sclk_mhz + 'MHz</td>' +
            '<td class="value mclk">' + m.mclk_mhz + 'MHz</td>' +
            '</tr>';
    }).join('');

    // System table
    const sys = lastSystemMetrics;
    const sysRowsEl = document.getElementById('system-rows');
    if (sysRowsEl) {
        sysRowsEl.innerHTML = '<tr>' +
            '<td class="card value">System</td>' +
            '<td class="value temp">' + (sys && sys.cpu_temp > 0 ? Math.round(sys.cpu_temp) + 'C' : '\u2014') + '</td>' +
            '<td class="value load">' + (sys && sys.cpu_load > 0 ? sys.cpu_load + '%' : '\u2014') + '</td>' +
            '<td class="value sclk">' + (sys && sys.cpu_clock_mhz > 0 ? sys.cpu_clock_mhz + 'MHz' : '\u2014') + '</td>' +
            '<td class="value vram">' + (sys && sys.ram_total_gb > 0 ? ((sys.ram_used_gb / sys.ram_total_gb) * 100).toFixed(0) + '% (' + (sys.ram_used_gb / 1024).toFixed(1) + ' GB)' : '\u2014') + '</td>' +
            '<td class="value">' + (sys && sys.cpu_name ? sys.cpu_name.substring(0, 20) : '\u2014') + '</td>' +
            '</tr>';
    }

    // Logs
    const logs = d.logs || [];
    if (logs.length !== prevLogLen) {
        const el = document.getElementById('log-panel');
        const wasAtBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
        el.textContent = logs.join('\n');
        if (wasAtBottom) el.scrollTop = el.scrollHeight;
        prevLogLen = logs.length;
    }

    // Tab badges
    const badgeParts = [];
    if (serverRunning) badgeParts.push('Running');
    if (l.generation_tokens_per_sec > 0) badgeParts.push(l.generation_tokens_per_sec.toFixed(1) + 't/s');
    const gpuEntries = Object.entries(d.gpu);
    if (gpuEntries.length > 0) badgeParts.push(Math.max(...gpuEntries.map(([,m]) => m.temp)).toFixed(0) + 'C');
    document.getElementById('badge-server').textContent = badgeParts.length ? ' ' + badgeParts.join(' \u00b7 ') : ' Stopped';

    document.getElementById('badge-chat').textContent = chatHistory.length > 0 ? ' ' + chatHistory.length + ' msg' : '';
    document.getElementById('badge-logs').textContent = logs.length > 0 ? ' ' + logs.length : '';
};
ws.onerror = e => console.error('WebSocket error:', e);
ws.onclose = () => { 
    document.getElementById('status-text').textContent = 'Disconnected'; 
    prevLogLen = 0;
};

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
