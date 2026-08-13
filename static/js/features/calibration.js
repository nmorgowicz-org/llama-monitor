import { sessionState } from '../core/app-state.js';
import { showConfirmDialog, showToast } from './toast.js';

let currentJobId = null;
let currentPreflight = null;
let currentReceipt = null;
let pollTimer = null;

function apiHeaders(json = false) {
    const headers = window.authHeaders ? { ...window.authHeaders() } : {};
    if (json) headers['Content-Type'] = 'application/json';
    return headers;
}

function currentPreset() {
    const id = document.getElementById('modal-preset-id')?.value || '';
    return sessionState.presets.find(preset => preset.id === id) || null;
}

function setModalOpen(open) {
    const modal = document.getElementById('calibration-modal');
    if (!modal) return;
    modal.classList.toggle('open', open);
    modal.setAttribute('aria-hidden', String(!open));
    modal.inert = !open;
}

function setStatus(text) {
    const status = document.getElementById('calibration-status');
    if (status) status.textContent = text;
}

function setError(text = '') {
    const error = document.getElementById('calibration-error');
    if (!error) return;
    error.textContent = text;
    error.hidden = !text;
}

function renderCandidates(results = [], preflight = currentPreflight) {
    const container = document.getElementById('calibration-candidates');
    if (!container) return;
    container.replaceChildren();
    const rows = results.length ? results : (preflight?.candidate_ids || []).map(id => ({ candidate: { id } }));
    rows.forEach((result) => {
        const row = document.createElement('div');
        row.className = 'calibration-candidate';
        const label = document.createElement('span');
        label.className = 'calibration-candidate-label';
        label.textContent = result.candidate?.id || 'candidate';
        const value = document.createElement('span');
        value.className = 'calibration-candidate-value';
        const samples = result.measurement?.tg_tps_samples || [];
        value.textContent = samples.length ? `${samples[0].toFixed(1)} tok/s` : 'pending';
        row.append(label, value);
        container.appendChild(row);
    });
    container.hidden = rows.length === 0;
}

async function requestJson(url, options = {}) {
    const response = await fetch(url, options);
    const data = await response.json().catch(() => ({}));
    if (!response.ok || data.error || data.ok === false) {
        throw new Error(data.error || `Request failed (${response.status})`);
    }
    return data;
}

async function openCalibration() {
    const preset = currentPreset();
    if (!preset) {
        showToast('Open an existing preset before calibrating', 'warn');
        return;
    }
    if (preset.backend === 'rapid_mlx') {
        showToast('Calibration v1 is llama.cpp-only; Rapid-MLX remains informational', 'info');
        return;
    }
    setModalOpen(true);
    setError();
    setStatus('Validating preset, model library, and managed llama-bench…');
    document.getElementById('calibration-start').disabled = true;
    document.getElementById('calibration-apply').disabled = true;
    currentJobId = null;
    currentReceipt = null;
    try {
        const data = await requestJson('/api/calibrations/preflight', {
            method: 'POST',
            headers: apiHeaders(true),
            body: JSON.stringify({ preset_id: preset.id }),
        });
        currentPreflight = data.preflight;
        renderCandidates([], currentPreflight);
        setStatus(`Ready: ${currentPreflight.planned_trials} bounded Quick trial(s). The active session and source preset will remain unchanged.`);
        document.getElementById('calibration-start').disabled = false;
    } catch (error) {
        setError(error.message || String(error));
        setStatus('Preflight could not be completed.');
    }
}

async function startCalibration() {
    if (!currentPreflight) return;
    const context = Number(document.getElementById('modal-context-size')?.value || 4096);
    setError();
    setStatus('Starting bounded Quick calibration…');
    document.getElementById('calibration-start').disabled = true;
    try {
        const data = await requestJson('/api/calibrations', {
            method: 'POST',
            headers: apiHeaders(true),
            body: JSON.stringify({
                preset_id: currentPreflight.preset_id,
                expected_preset_fingerprint: currentPreflight.preset_fingerprint,
                workload: {
                    kind: 'interactive', prompt_tokens: 512, generation_tokens: 256,
                    parallel_requests: 1, minimum_context: Math.max(1, Math.min(context, 131072)),
                    objective: 'balanced', fixture_id: 'calibration-v1-interactive',
                },
                budget: 'quick', kv_quality_floor: 'q8_0',
                allow_stop_active_server: false,
                exact_confirmation: currentPreflight.confirmation,
            }),
        });
        currentJobId = data.job.id;
        setStatus('Calibration is running…');
        pollCalibration();
    } catch (error) {
        document.getElementById('calibration-start').disabled = false;
        setError(error.message || String(error));
        setStatus('Calibration could not be started.');
    }
}

async function pollCalibration() {
    if (!currentJobId) return;
    clearTimeout(pollTimer);
    try {
        const data = await requestJson(`/api/calibrations/${encodeURIComponent(currentJobId)}`, { headers: apiHeaders() });
        const job = data.job;
        setStatus(`${job.phase || job.state}: ${job.completed_trials || 0}/${job.planned_trials || 0} trial(s)`);
        if (job.state === 'complete') {
            const receipt = await requestJson(`/api/calibrations/${encodeURIComponent(currentJobId)}/receipt`, { headers: apiHeaders() });
            currentReceipt = receipt.receipt;
            renderCandidates(currentReceipt.candidate_results || []);
            document.getElementById('calibration-apply').disabled = !currentReceipt.selected_candidate;
            return;
        }
        if (['cancelled', 'failed'].includes(job.state)) {
            setError((job.diagnostics || []).join(' ') || 'Calibration did not complete.');
            return;
        }
        pollTimer = setTimeout(pollCalibration, 1000);
    } catch (error) {
        setError(error.message || String(error));
        pollTimer = setTimeout(pollCalibration, 2000);
    }
}

async function cancelCalibration() {
    clearTimeout(pollTimer);
    if (currentJobId) {
        try {
            await requestJson(`/api/calibrations/${encodeURIComponent(currentJobId)}/cancel`, {
                method: 'POST', headers: apiHeaders(),
            });
            setStatus('Cancellation requested; owned benchmark process is being cleaned up.');
        } catch (error) {
            setError(error.message || String(error));
        }
    }
    setModalOpen(false);
}

async function applyCalibration() {
    if (!currentJobId || !currentReceipt?.selected_candidate || !currentPreflight) return;
    const tokenResponse = await fetch('/api/db/admin-token', { headers: apiHeaders() });
    const tokenData = tokenResponse.ok ? await tokenResponse.json().catch(() => ({})) : {};
    if (!tokenData.token) {
        setError('The db-admin token is unavailable; no preset was changed.');
        return;
    }
    const preset = currentPreset();
    if (!preset) return;
    const confirmed = await showConfirmDialog(
        'Create derived preset',
        'Create a new preset from the measured Calibration winner? Your source preset will remain unchanged.',
        'Create preset',
    );
    if (!confirmed) return;
    try {
        await requestJson(`/api/calibrations/${encodeURIComponent(currentJobId)}/apply`, {
            method: 'POST',
            headers: { Authorization: `Bearer ${tokenData.token}`, 'Content-Type': 'application/json' },
            body: JSON.stringify({
                target_preset_id: preset.id,
                expected_target_fingerprint: currentPreflight.preset_fingerprint,
                candidate_id: currentReceipt.selected_candidate,
                create_derived: true,
                exact_confirmation: 'APPLY_CALIBRATION',
            }),
        });
        window.dispatchEvent(new CustomEvent('presets:reload'));
        showToast('Derived calibrated preset created', 'success');
        setModalOpen(false);
    } catch (error) {
        setError(error.message || String(error));
    }
}

export function initCalibrationUi() {
    document.getElementById('preset-modal-calibrate')?.addEventListener('click', openCalibration);
    document.getElementById('calibration-start')?.addEventListener('click', startCalibration);
    document.getElementById('calibration-apply')?.addEventListener('click', applyCalibration);
    document.getElementById('calibration-cancel')?.addEventListener('click', cancelCalibration);
    document.getElementById('calibration-modal-close')?.addEventListener('click', cancelCalibration);
    document.getElementById('calibration-modal')?.addEventListener('click', (event) => {
        if (event.target === event.currentTarget) cancelCalibration();
    });
}
