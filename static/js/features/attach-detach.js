// ── Attach / Detach / Start / Stop ─────────────────────────────────────────────
// LLM lifecycle: start, stop, attach, detach, kill.

import { sessionState } from '../core/app-state.js';
import { updateActiveSessionInfo } from './sessions.js';
import { showToast } from './toast.js';
import { hideConnectingState, saveLastSessionData, showConnectingState, switchView, restorePreviousPosition } from './setup-view.js';
import { setTuneConfig, showTunePanel, hideTunePanel } from './tune-panel.js';
import { hideDisconnectedBanner } from './chat-transport.js';
import { monitorState } from '../core/app-state.js';

// ── Config ─────────────────────────────────────────────────────────────────────

export function getConfig() {
    const id = document.getElementById('preset-select').value;
    const p = sessionState.presets.find(pr => pr.id === id) || {};

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
        bind_host: p.bind_host || '127.0.0.1',
        api_key: p.api_key || null,
    };
}

// ── Start / Stop ───────────────────────────────────────────────────────────────

export async function doStart() {
    const config = getConfig();
    if (!config.model_path) {
        showToast('No model path set. Edit the preset to select a model.', 'error');
        return;
    }

    const btnStart = document.getElementById('btn-start');
    if (btnStart) btnStart.disabled = true;

    await doKillLlamaInternal();

    const resp = await fetch('/api/start', {
        method: 'POST',
        headers: window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' },
        body: JSON.stringify(config),
    });
    const data = await resp.json();

    if (!data.ok) {
        showToast('Start failed: ' + (data.error || 'unknown'), 'error');
        hideConnectingState();
    } else {
        setTuneConfig(config);
        switchView('monitor');
        hideConnectingState();
        showTunePanel();
        setTimeout(() => restorePreviousPosition(), 600);
    }
}

export async function doStop() {
    const btnStop = document.getElementById('btn-stop');
    if (btnStop) btnStop.disabled = true;

    await fetch('/api/stop', {
            method: 'POST',
            headers: window.authHeaders ? window.authHeaders() : {},
        });
    await doKillLlamaInternal();
    hideTunePanel();
    if (btnStop) btnStop.disabled = false;
}

// ── Kill ───────────────────────────────────────────────────────────────────────

export async function doKillLlama() {
    if (!confirm('Kill all running llama-server processes?')) return;

    const btnKill = document.getElementById('btn-kill');
    if (btnKill) btnKill.disabled = true;

    try {
        const tokenResp = await fetch('/api/db/admin-token');
        const tokenData = await tokenResp.json();
        const token = tokenData.token;
        if (!token) {
            showToast('Kill failed: admin token not available', 'error');
            return;
        }

        const resp = await fetch('/api/kill-llama', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${token}`,
            },
            body: JSON.stringify({ confirm: 'kill' }),
        });

        const data = await resp.json();

        if (!data.ok) {
            if (resp.status === 429) {
                showToast('Kill failed: too soon; please wait', 'error');
            } else {
                showToast('Kill failed: ' + (data.error || 'unknown'), 'error');
            }
        } else {
            showToast('llama-server killed', 'success');
            hideTunePanel();
        }
    } catch (e) {
        showToast('Kill failed: ' + e.message, 'error');
    } finally {
        if (btnKill) btnKill.disabled = false;
    }
}

export async function doKillLlamaInternal() {
    try {
        const tokenResp = await fetch('/api/db/admin-token');
        const tokenData = await tokenResp.json();
        const token = tokenData.token;
        if (!token) return;

        await fetch('/api/kill-llama', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${token}`,
            },
            body: JSON.stringify({ confirm: 'kill' }),
        });
    } catch(e) {
        // Ignore errors from kill, just try to continue
    }
}

// ── Attach / Detach ────────────────────────────────────────────────────────────

export async function doAttach() {
    const endpointInput = document.getElementById('server-endpoint');
    const endpoint = endpointInput.value.trim();

    if (!endpoint) {
        showToast('Please enter a server endpoint', 'error');
        return;
    }

    // Read API key from welcome screen or monitor view input
    const apiKeyInput = document.getElementById('setup-endpoint-api-key');
    const apiKey = apiKeyInput ? apiKeyInput.value.trim() : '';

    const resp = await fetch('/api/attach', {
        method: 'POST',
        headers: (typeof window.authHeaders === 'function')
            ? window.authHeaders({ 'Content-Type': 'application/json' })
            : { 'Content-Type': 'application/json' },
        body: JSON.stringify({ endpoint, api_key: apiKey || undefined }),
    });
    const data = await resp.json();

    if (!data.ok) {
        showToast('Attach failed: ' + (data.error || 'unknown'), 'error');
        hideConnectingState();
    } else {
        showToast('Attached to server', 'success');
        hideConnectingState();

        if (data.warning) {
            showToast(data.warning, 'warning');
        }

        const serverHeader = document.getElementById('server-header');
        if (serverHeader) serverHeader.style.display = 'none';

        monitorState.speedMax = { prompt: 0, generation: 0 };
        hideDisconnectedBanner();
        switchView('monitor');
        showTunePanel();
        setTimeout(() => restorePreviousPosition(), 600);
    }

    updateActiveSessionInfo();
}

export async function doDetach() {
    try {
        const headers = window.authHeaders
            ? window.authHeaders({ 'Content-Type': 'application/json' })
            : { 'Content-Type': 'application/json' };
        const resp = await fetch('/api/detach', { method: 'POST', headers });
        if (resp.status === 401) {
            showToast('Detach failed: authentication required', 'error');
            return;
        }
        const data = await resp.json();

        if (!data.ok) {
            showToast('Detach failed: ' + (data.error || 'unknown'), 'error');
        } else {
            showToast('Detached from server', 'success');

            saveLastSessionData({
                promptRate: monitorState.speedMax.prompt > 0 ? monitorState.speedMax.prompt + ' t/s' : '—',
                genRate: monitorState.speedMax.generation > 0 ? monitorState.speedMax.generation + ' t/s' : '—',
                sessionName: sessionState.activeSessionId || '—',
                endpoint: document.getElementById('server-endpoint')?.value?.trim() || '',
                telemetryGrade: window.__telemetryGrade || '',
                telemetryLabel: document.getElementById('telemetry-grade-chip')?.textContent?.trim() || '',
            });

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

            monitorState.speedMax = { prompt: 0, generation: 0 };
            hideTunePanel();
            switchView('setup');
        }

        updateActiveSessionInfo();
    } catch (err) {
        showToast('Detach failed: ' + err.message, 'error');
    }
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
    showConnectingState();
    doAttach();
}

export function doStartFromSetup() {
    const select = document.getElementById('setup-preset-select');
    if (select) {
        const presetSelect = document.getElementById('preset-select');
        if (presetSelect) presetSelect.value = select.value;
    }
    showConnectingState();
    doStart();
}

// ── Button init ────────────────────────────────────────────────────────────────

export async function initAttachDetachButtons() {
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const resp = await fetch('/api/sessions/active', { headers });
        if (resp.status === 401) {
            console.error('initAttachDetachButtons: authentication required');
            return;
        }
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
    // Bind top detach button
    const detachTop = document.getElementById('btn-detach-top');
    if (detachTop) detachTop.addEventListener('click', doDetach);

    // Bind setup page buttons
    const setupAttach = document.getElementById('setup-attach-btn');
    if (setupAttach) setupAttach.addEventListener('click', doAttachFromSetup);

    // Setup wizard button — opens the spawn wizard overlay from the welcome screen
    const setupWizardBtn = document.getElementById('setup-spawn-wizard-btn');
    if (setupWizardBtn) setupWizardBtn.addEventListener('click', () => {
        import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard());
    });

    const setupStart = document.getElementById('setup-start-btn');
    if (setupStart) setupStart.addEventListener('click', doStartFromSetup);

    // Bind monitor view buttons
    const btnAttach = document.getElementById('btn-attach');
    if (btnAttach) btnAttach.addEventListener('click', doAttach);

    const btnDetach = document.getElementById('btn-detach');
    if (btnDetach) btnDetach.addEventListener('click', doDetach);

    const btnStart = document.getElementById('btn-start');
    if (btnStart) btnStart.addEventListener('click', doStart);

    const btnStop = document.getElementById('btn-stop');
    if (btnStop) btnStop.addEventListener('click', doStop);

    // Bind logs empty state button — opens wizard
    const btnSpawnFromLogs = document.getElementById('btn-spawn-server');
    if (btnSpawnFromLogs) btnSpawnFromLogs.addEventListener('click', () => {
        import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard());
    });

    // Initialize button states
    initAttachDetachButtons();
}
