// ── Attach / Detach / Start / Stop ─────────────────────────────────────────────
// LLM lifecycle: start, stop, attach, detach, kill.

import { sessionState, setupViewState } from '../core/app-state.js';
import { updateActiveSessionInfo } from './sessions.js';
import { showToast } from './toast.js';
import { saveSettings } from './settings.js';
import { hideConnectingState, saveLastSessionData, showConnectingState, switchView, restorePreviousPosition, savePreviousPosition } from './setup-view.js';
import { setTuneConfig, showTunePanel, hideTunePanel } from './tune-panel.js';
import { hideDisconnectedBanner } from './chat-transport.js';
import { monitorState } from '../core/app-state.js';
import { waitForSpawnReadiness } from './spawn-readiness.js';

// ── Config ─────────────────────────────────────────────────────────────────────

export function getConfig() {
    const id = document.getElementById('preset-select').value;
    const p = sessionState.presets.find(pr => pr.id === id) || {};

    return {
        preset_id: id,
        model_path: p.model_path || '',
        hf_repo: p.hf_repo || null,
        context_size: p.context_size || 128000,
        ctk: p.ctk || 'q8_0',
        ctv: p.ctv || 'f16',
        tensor_split: p.tensor_split || '',
        batch_size: p.batch_size || 2048,
        ubatch_size: p.ubatch_size || p.batch_size || 2048,
        no_mmap: !!p.no_mmap,
        port: p.port || 8001,
        ngram_spec: !!p.ngram_spec,
        parallel_slots: p.parallel_slots || 1,
        // Generation
        temperature: p.temperature,
        top_p: p.top_p,
        top_k: p.top_k,
        min_p: p.min_p,
        repeat_penalty: p.repeat_penalty,
        presence_penalty: p.presence_penalty ?? null,
        enable_thinking: p.enable_thinking ?? null,
        preserve_thinking: p.preserve_thinking ?? null,
        reasoning: p.reasoning || null,
        reasoning_budget: p.reasoning_budget ?? null,
        reasoning_budget_message: p.reasoning_budget_message || null,
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
        spec_type: p.spec_type || null,
        kv_unified: p.kv_unified ?? null,
        cache_ram_mib: p.cache_ram_mib ?? null,
        draft_model: p.draft_model || '',
        draft_min: p.draft_min ?? null,
        draft_max: p.draft_max ?? null,
        spec_ngram_size: p.spec_ngram_size ?? null,
        spec_draft_n_max: p.spec_draft_n_max ?? null,
        seed: p.seed ?? null,
        mmproj: p.mmproj || null,
        chat_template_file: p.chat_template_file || null,
        alias: p.alias || null,
        max_tokens: p.max_tokens ?? null,
        ignore_eos: !!p.ignore_eos,
        fit_enabled: p.fit_enabled ?? null,
        fit_target: p.fit_target || null,
        system_prompt_file: p.system_prompt_file || '',
        extra_args: p.extra_args || '',
        bind_host: p.bind_host || '127.0.0.1',
        api_key: p.api_key || null,
    };
}

// ── Start / Stop ───────────────────────────────────────────────────────────────

// Fetch the db-admin-token needed for V2 spawn endpoints.
async function _fetchDbAdminToken() {
    const tokenResp = await fetch('/api/db/admin-token', {
        headers: window.authHeaders ? window.authHeaders() : {},
    });
    const tokenData = tokenResp.ok ? await tokenResp.json().catch(() => ({})) : {};
    return tokenData.token || null;
}

// Disable start buttons for N seconds, showing countdown.
function applyCooldown(seconds, button) {
    if (!seconds || seconds <= 0 || !button) return;
    button.disabled = true;
    const remaining = seconds;
    const label = button.textContent || button.value || '';
    const orig = label;
    const interval = setInterval(() => {
        if (seconds <= 0) {
            clearInterval(interval);
            button.disabled = false;
            button.textContent = orig;
            return;
        }
        button.textContent = `Wait ${seconds}s`;
        seconds--;
    }, 1000);
}

export async function doStart(cooldownBtn) {
    const config = getConfig();
    if (!config.model_path && !config.hf_repo) {
        showToast('No model source set. Edit the preset to select a local model or HuggingFace repo.', 'error');
        return;
    }

    const btnStart = document.getElementById('btn-start');
    if (btnStart) btnStart.disabled = true;

    try {
        await doKillLlamaInternal();

        // V2 spawn endpoint requires db-admin-token
        const adminToken = await _fetchDbAdminToken();
        if (!adminToken) {
            showToast('Failed: authentication required', 'error');
            hideConnectingState();
            return;
        }

        const resp = await fetch('/api/sessions/spawn', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${adminToken}`,
            },
            body: JSON.stringify(config),
        });

        if (!resp.ok) {
            if (resp.status === 429) {
                try {
                    const data = await resp.json().catch(() => null);
                    const wait = data?.seconds_remaining || data?.error || '';
                    if (typeof wait === 'number') {
                        showToast('Start failed: Please wait ' + wait + 's', 'warning');
                        applyCooldown(wait, cooldownBtn || btnStart);
                    } else {
                        showToast('Start failed: too soon; please wait', 'warning');
                    }
                } catch {
                    showToast('Start failed: too soon; please wait', 'warning');
                }
                hideConnectingState();
                return;
            }

            const text = await resp.text().catch(() => 'Request failed');
            showToast('Start failed: ' + text, 'error');
            hideConnectingState();
            return;
        }

        const data = await resp.json().catch(() => ({}));

        if (!data.ok) {
            showToast('Start failed: ' + (data.error || 'server responded with an error'), 'error');
            hideConnectingState();
            return;
        }

        // Wait for the spawned server to become reachable
        // Show a "starting" toast so the user knows something is happening
        showToast('Starting llama-server…', 'info', 'Loading model on port ' + config.port, { duration: 12000 });
        await waitForSpawnReadiness(config.port);

        showToast('llama-server is running', 'success', '', { duration: 6000 });
        setTuneConfig(config);
        setHeaderMode('Spawn:' + config.port);
        switchView('monitor');
        hideConnectingState();
        showTunePanel();
        saveSettings();
        setTimeout(() => restorePreviousPosition(), 600);
    } catch (e) {
        const msg = (e.message || 'network or server error').split('\n')[0].trim();
        showToast('Start failed: ' + msg, 'error');
        hideConnectingState();
    } finally {
        if (btnStart) btnStart.disabled = false;
    }
}

export async function doStop() {
    const btnStop = document.getElementById('btn-stop');
    if (btnStop) btnStop.disabled = true;

    // V2: kill-llama kills the tracked child process and clears in-memory state
    await doKillLlamaInternal();
    hideTunePanel();
    setHeaderMode(null);

    // Instead of jumping straight to the welcome screen,
    // show a small modal and let the user choose what's next.
    const modal = document.createElement('div');
    modal.className = 'stop-choice-modal';
    modal.innerHTML = `
        <div class="stop-choice-card">
            <div class="stop-choice-title">Server stopped</div>
            <div class="stop-choice-actions">
                <button class="btn btn-stop-choice-welcome" id="stop-choice-welcome">
                    Go to welcome screen
                </button>
                <button class="btn btn-stop-choice-stay" id="stop-choice-stay">
                    Stay on dashboard
                </button>
            </div>
        </div>
    `;

    document.body.appendChild(modal);

    const welcomeBtn = modal.querySelector('#stop-choice-welcome');
    const stayBtn = modal.querySelector('#stop-choice-stay');

    const removeModal = () => {
        if (modal && modal.parentNode) {
            modal.parentNode.removeChild(modal);
        }
    };

    welcomeBtn.addEventListener('click', () => {
        // Go to welcome screen (previous behavior).
        if (document.body.classList.contains('setup-active') === false) {
            switchView('setup');
        }
        removeModal();
    });

    stayBtn.addEventListener('click', () => {
        // Stay on dashboard; disable buttons to avoid double-click.
        stayBtn.disabled = true;
        welcomeBtn.disabled = true;

        // Fade out, then remove.
        modal.style.transition = 'opacity 250ms ease';
        modal.style.opacity = '0.5';
        modal.style.pointerEvents = 'none';

        setTimeout(() => {
            removeModal();
        }, 3000);
    });

    // On any Escape: default to welcome screen.
    const onKey = (e) => {
        if (e.key === 'Escape') {
            e.preventDefault();
            if (document.body.classList.contains('setup-active') === false) {
                switchView('setup');
            }
            removeModal();
            document.removeEventListener('keydown', onKey);
        }
    };
    document.addEventListener('keydown', onKey);

    if (btnStop) btnStop.disabled = false;
}

// ── Kill ───────────────────────────────────────────────────────────────────────

export async function doKillLlama() {
    if (!confirm('Kill all running llama-server processes?')) return;

    const btnKill = document.getElementById('btn-kill');
    if (btnKill) btnKill.disabled = true;

    try {
        const tokenResp = await fetch('/api/db/admin-token', {
            headers: window.authHeaders ? window.authHeaders() : {},
        });
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
                const wait = data?.seconds_remaining
                    ? `Too soon; please wait ${data.seconds_remaining}s`
                    : 'Too soon; please wait';
                showToast('Kill failed: ' + wait, 'warning');
                if (data?.seconds_remaining) {
                    applyCooldown(data.seconds_remaining, btnKill);
                }
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
        const tokenResp = await fetch('/api/db/admin-token', {
            headers: window.authHeaders ? window.authHeaders() : {},
        });
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

        setHeaderMode('Attach:' + (document.getElementById('server-endpoint')?.value?.trim() || ''));

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
    doStart(document.getElementById('setup-start-btn'));
}

// ── Button init ────────────────────────────────────────────────────────────────

// Update the server header visibility based on session mode.
// Call this whenever session mode changes (spawn, attach, detach, init).
export function setHeaderMode(mode) {
    const serverHeader = document.getElementById('server-header');
    const btnAttach = document.getElementById('btn-attach');
    const btnDetach = document.getElementById('btn-detach');
    if (!serverHeader) return;
    if (mode && mode.startsWith('Spawn:')) {
        serverHeader.style.display = 'none';
    } else if (mode && mode.startsWith('Attach:')) {
        serverHeader.style.display = '';
        if (btnAttach) btnAttach.style.display = 'none';
        if (btnDetach) btnDetach.style.display = 'inline-block';
    } else {
        serverHeader.style.display = '';
        if (btnAttach) btnAttach.style.display = 'inline-block';
        if (btnDetach) btnDetach.style.display = 'none';
    }
}

export async function initAttachDetachButtons() {
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const resp = await fetch('/api/sessions/active', { headers });
        if (resp.status === 401) {
            console.error('initAttachDetachButtons: authentication required');
            return;
        }
        const data = await resp.json();
        setHeaderMode(data?.mode ?? null);
        // If a session is already running, restore the monitor view instead of
        // leaving the user stranded on the welcome screen after a hard refresh.
        if (data?.status === 'Running' && setupViewState.view === 'setup') {
            switchView('monitor');
            showTunePanel();
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

    // Bind Switch Model button
    const btnSwitchModel = document.getElementById('btn-switch-model');
    if (btnSwitchModel) btnSwitchModel.addEventListener('click', () => {
        import('./models.js').then(({ openModelsModalForSwitch }) => openModelsModalForSwitch());
    });

    // Bind control bar spawn button — opens wizard from monitor view
    const btnControlSpawn = document.getElementById('btn-control-spawn');
    if (btnControlSpawn) btnControlSpawn.addEventListener('click', () => {
        import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard());
    });

    // Bind logs empty state button — opens wizard
    const btnSpawnFromLogs = document.getElementById('btn-spawn-server');
    if (btnSpawnFromLogs) btnSpawnFromLogs.addEventListener('click', () => {
        import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard());
    });

    // Initialize button states
    initAttachDetachButtons();
}
