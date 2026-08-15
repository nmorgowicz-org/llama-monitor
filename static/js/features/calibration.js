import { sessionState } from '../core/app-state.js';
import { showConfirmDialog, showToast } from './toast.js';

let currentJobId = null;
let currentPreflight = null;
let currentReceipt = null;
let pollTimer = null;
let lastApply = null;

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

function renderBaseline(baseline) {
    const body = document.querySelector('.calibration-modal-body');
    if (!body || !baseline) return;
    let section = document.getElementById('calibration-baseline');
    if (!section) {
        section = document.createElement('section');
        section.id = 'calibration-baseline';
        section.className = 'calibration-baseline';
        const title = document.createElement('h3');
        title.textContent = 'Measured baseline';
        section.appendChild(title);
        const description = document.createElement('p');
        description.id = 'calibration-baseline-description';
        section.appendChild(description);
        const values = document.createElement('dl');
        values.id = 'calibration-baseline-values';
        values.className = 'calibration-baseline-values';
        section.appendChild(values);
        const helpTitle = document.createElement('h4');
        helpTitle.textContent = 'Managed llama-server defaults';
        section.appendChild(helpTitle);
        const helpNote = document.createElement('p');
        helpNote.id = 'calibration-baseline-help-note';
        helpNote.className = 'calibration-baseline-help-note';
        section.appendChild(helpNote);
        const helpValues = document.createElement('dl');
        helpValues.id = 'calibration-baseline-help-values';
        helpValues.className = 'calibration-baseline-values';
        section.appendChild(helpValues);
        body.insertBefore(section, document.getElementById('calibration-candidates'));
    }
    const valueList = document.getElementById('calibration-baseline-values');
    const helpList = document.getElementById('calibration-baseline-help-values');
    const description = document.getElementById('calibration-baseline-description');
    const helpNote = document.getElementById('calibration-baseline-help-note');
    valueList.replaceChildren();
    helpList.replaceChildren();
    const effective = baseline.effective || {};
    Object.entries(effective).forEach(([name, detail]) => {
        const term = document.createElement('dt');
        term.textContent = name.replaceAll('_', ' ');
        const value = document.createElement('dd');
        value.textContent = `${detail.value} (${detail.source.replaceAll('_', ' ')})`;
        valueList.append(term, value);
    });
    const defaults = baseline.llama_server_help_defaults || {};
    Object.entries(defaults).forEach(([name, value]) => {
        const term = document.createElement('dt');
        term.textContent = name.replaceAll('_', ' ');
        const detail = document.createElement('dd');
        detail.textContent = value;
        helpList.append(term, detail);
    });
    description.textContent = effective.context_size
        ? 'This is the effective preset configuration measured as the control. It is not a claim that these are llama.cpp compiled defaults.'
        : 'The managed llama-server defaults were captured during preflight; the measured preset baseline appears when the run completes.';
    const hash = baseline.llama_server_help_sha256 ? ` Help hash: ${baseline.llama_server_help_sha256}.` : '';
    helpNote.textContent = `Captured from the configured managed llama-server --help output.${hash}`;
    section.hidden = false;
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
    document.getElementById('calibration-rollback').hidden = true;
    document.getElementById('calibration-rollback').disabled = true;
    currentJobId = null;
    currentReceipt = null;
    lastApply = null;
    document.getElementById('calibration-baseline')?.remove();
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
            renderBaseline(currentReceipt.baseline);
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
        const applyData = await requestJson(`/api/calibrations/${encodeURIComponent(currentJobId)}/apply`, {
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
        lastApply = applyData.apply;
        window.dispatchEvent(new CustomEvent('presets:reload'));
        document.getElementById('calibration-apply').disabled = true;
        const rollbackButton = document.getElementById('calibration-rollback');
        rollbackButton.hidden = false;
        rollbackButton.disabled = false;
        setStatus(`Applied preset validated: ${applyData.apply.validation || 'complete'}. You can roll it back before closing.`);
        showToast('Derived calibrated preset created', 'success');
    } catch (error) {
        setError(error.message || String(error));
    }
}

async function rollbackCalibration() {
    if (!currentJobId || !lastApply) return;
    const confirmed = await showConfirmDialog(
        'Rollback calibrated preset',
        'Restore the exact preset state captured before Calibration? The active session will remain unchanged.',
        'Rollback preset',
    );
    if (!confirmed) return;
    try {
        const tokenResponse = await fetch('/api/db/admin-token', { headers: apiHeaders() });
        const tokenData = tokenResponse.ok ? await tokenResponse.json().catch(() => ({})) : {};
        if (!tokenData.token) throw new Error('The db-admin token is unavailable; no preset was changed.');
        await requestJson(`/api/calibrations/${encodeURIComponent(currentJobId)}/rollback`, {
            method: 'POST',
            headers: { Authorization: `Bearer ${tokenData.token}`, 'Content-Type': 'application/json' },
            body: JSON.stringify({
                expected_target_fingerprint: lastApply.after_fingerprint,
                exact_confirmation: 'ROLLBACK_CALIBRATION',
            }),
        });
        document.getElementById('calibration-rollback').disabled = true;
        setStatus('Calibration apply rolled back. The source preset is restored.');
        showToast('Calibration apply rolled back', 'success');
        window.dispatchEvent(new CustomEvent('presets:reload'));
    } catch (error) {
        setError(error.message || String(error));
    }
}

export function initCalibrationUi() {
    document.getElementById('preset-modal-calibrate')?.addEventListener('click', openCalibration);
    document.getElementById('calibration-start')?.addEventListener('click', startCalibration);
    document.getElementById('calibration-apply')?.addEventListener('click', applyCalibration);
    document.getElementById('calibration-rollback')?.addEventListener('click', rollbackCalibration);
    document.getElementById('calibration-cancel')?.addEventListener('click', cancelCalibration);
    document.getElementById('calibration-modal-close')?.addEventListener('click', cancelCalibration);
    document.getElementById('calibration-modal')?.addEventListener('click', (event) => {
        if (event.target === event.currentTarget) cancelCalibration();
    });
}
