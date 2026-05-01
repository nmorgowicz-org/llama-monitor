// ── Attach / Detach / Start / Stop ─────────────────────────────────────────────
// LLM lifecycle: start, stop, attach, detach, kill.

// ── Config ─────────────────────────────────────────────────────────────────────

export function getConfig() {
    const id = document.getElementById('preset-select').value;
    const p = window.presets.find(pr => pr.id === id) || {};

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

// ── Start / Stop ───────────────────────────────────────────────────────────────

export async function doStart() {
    const config = getConfig();
    if (!config.model_path) {
        window.showToast('No model path set. Edit the preset to select a model.', 'error');
        return;
    }

    const btnStart = document.getElementById('btn-start');
    if (btnStart) btnStart.disabled = true;

    await doKillLlamaInternal();

    const resp = await fetch('/api/start', {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify(config),
    });
    const data = await resp.json();

    if (!data.ok) {
        window.showToast('Start failed: ' + (data.error || 'unknown'), 'error');
        if (window.hideConnectingState) window.hideConnectingState();
    } else {
        if (window.switchView) window.switchView('monitor');
        if (window.hideConnectingState) window.hideConnectingState();
    }
}

export async function doStop() {
    const btnStop = document.getElementById('btn-stop');
    if (btnStop) btnStop.disabled = true;

    await fetch('/api/stop', { method: 'POST' });
    await doKillLlamaInternal();
}

// ── Kill ───────────────────────────────────────────────────────────────────────

export async function doKillLlama() {
    if (!confirm('Kill all running llama-server processes?')) return;

    const btnKill = document.getElementById('btn-kill');
    if (btnKill) btnKill.disabled = true;

    try {
        const resp = await fetch('/api/kill-llama', { method: 'POST' });
        const data = await resp.json();

        if (!data.ok) window.showToast('Kill failed: ' + (data.error || 'unknown'), 'error');
        else window.showToast('llama-server killed', 'success');
    } catch (e) {
        window.showToast('Kill failed: ' + e.message, 'error');
    } finally {
        if (btnKill) btnKill.disabled = false;
    }
}

export async function doKillLlamaInternal() {
    try {
        await fetch('/api/kill-llama', { method: 'POST' });
    } catch(e) {
        // Ignore errors from kill, just try to continue
    }
}

// ── Attach / Detach ────────────────────────────────────────────────────────────

export async function doAttach() {
    const endpointInput = document.getElementById('server-endpoint');
    const endpoint = endpointInput.value.trim();

    if (!endpoint) {
        window.showToast('Please enter a server endpoint', 'error');
        return;
    }

    const resp = await fetch('/api/attach', {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({ endpoint }),
    });
    const data = await resp.json();

    if (!data.ok) {
        window.showToast('Attach failed: ' + (data.error || 'unknown'), 'error');
        if (window.hideConnectingState) window.hideConnectingState();
    } else {
        window.showToast('Attached to server', 'success');
        if (window.hideConnectingState) window.hideConnectingState();

        if (data.warning) {
            window.showToast(data.warning, 'warning');
        }

        const serverHeader = document.getElementById('server-header');
        if (serverHeader) serverHeader.style.display = 'none';

        window.speedMax = { prompt: 0, generation: 0 };
        if (window.switchView) window.switchView('monitor');
    }

    if (window.updateActiveSessionInfo) window.updateActiveSessionInfo();
}

export async function doDetach() {
    const resp = await fetch('/api/detach', { method: 'POST' });
    const data = await resp.json();

    if (!data.ok) {
        window.showToast('Detach failed: ' + (data.error || 'unknown'), 'error');
    } else {
        window.showToast('Detached from server', 'success');

        if (window.saveLastSessionData) {
            window.saveLastSessionData({
                promptRate: window.speedMax.prompt > 0 ? window.speedMax.prompt + ' t/s' : '—',
                genRate: window.speedMax.generation > 0 ? window.speedMax.generation + ' t/s' : '—',
                sessionName: window.currentSessionId || '—'
            });
        }

        const btnAttach = document.getElementById('btn-attach');
        const btnDetach = document.getElementById('btn-detach');
        const btnDetachTop = document.getElementById('btn-detach-top');

        if (btnAttach && btnDetach) {
            btnAttach.style.display = 'inline-block';
            btnDetach.style.display = 'none';
        }
        if (btnDetachTop) btnDetachTop.style.display = 'none';

        const serverHeader = document.getElementById('server-header');
        if (serverHeader) serverHeader.style.display = '';

        const historicBadge = document.getElementById('inference-historic-badge');
        if (historicBadge) historicBadge.style.display = 'inline-block';

        window.speedMax = { prompt: 0, generation: 0 };
        if (window.switchView) window.switchView('setup');
    }

    if (window.updateActiveSessionInfo) window.updateActiveSessionInfo();
}

// ── Setup page helpers ─────────────────────────────────────────────────────────

export function doAttachFromSetup() {
    const input = document.getElementById('setup-endpoint-url');
    const url = input ? input.value.trim() : '';
    if (url) {
        const serverEndpoint = document.getElementById('server-endpoint');
        if (serverEndpoint) serverEndpoint.value = url;
        localStorage.setItem('llama-monitor-last-endpoint', url);
    }
    if (window.showConnectingState) window.showConnectingState();
    doAttach();
}

export function doStartFromSetup() {
    const select = document.getElementById('setup-preset-select');
    if (select) {
        const presetSelect = document.getElementById('preset-select');
        if (presetSelect) presetSelect.value = select.value;
    }
    if (window.showConnectingState) window.showConnectingState();
    doStart();
}

// ── Button init ────────────────────────────────────────────────────────────────

export async function initAttachDetachButtons() {
    try {
        const resp = await fetch('/api/sessions/active');
        const data = await resp.json();
        const btnAttach = document.getElementById('btn-attach');
        const btnDetach = document.getElementById('btn-detach');
        if (data && data.mode && data.mode.startsWith('Attach:') && btnAttach && btnDetach) {
            btnAttach.style.display = 'none';
            btnDetach.style.display = 'inline-block';
        } else if (btnAttach && btnDetach) {
            btnAttach.style.display = 'inline-block';
            btnDetach.style.display = 'none';
        }
    } catch (err) {
        console.error('Failed to initialize attach/detach buttons:', err);
    }
}

// ── Init ───────────────────────────────────────────────────────────────────────

export function initAttachDetach() {
    // Put on window for inline handlers
    window.doStart = doStart;
    window.doStop = doStop;
    window.doKillLlama = doKillLlama;
    window.doAttach = doAttach;
    window.doDetach = doDetach;
    window.doAttachFromSetup = doAttachFromSetup;
    window.doStartFromSetup = doStartFromSetup;
    window.getConfig = getConfig;

    // Initialize button states
    initAttachDetachButtons();
}
