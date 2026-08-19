import { sessionState } from '../core/app-state.js';
import { showConfirmDialog, showToast } from './toast.js';

let currentJobId = null;
let currentPreflight = null;
let currentReceipt = null;
let pollTimer = null;
let lastApply = null;
let currentResults = [];
let calibrationStartedAt = null;
let calibrationBudget = 'quick';

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

function formatDuration(seconds) {
    if (!Number.isFinite(seconds) || seconds < 0) return '—';
    if (seconds < 60) return `${Math.round(seconds)}s`;
    return `${Math.floor(seconds / 60)}m ${Math.round(seconds % 60)}s`;
}

function modelLabel(preset) {
    const source = preset?.model_path || preset?.name || 'Selected preset';
    return source.split(/[\\/]/).pop() || source;
}

function addBadge(parent, text, tone = 'neutral') {
    const badge = document.createElement('span');
    badge.className = `calibration-badge calibration-badge--${tone}`;
    badge.textContent = text;
    parent.appendChild(badge);
    return badge;
}

function ensureCalibrationSurface(preset = currentPreset()) {
    const body = document.querySelector('.calibration-modal-body');
    if (!body || document.getElementById('calibration-hero')) return;
    const hero = document.createElement('section');
    hero.id = 'calibration-hero';
    hero.className = 'calibration-hero';
    hero.innerHTML = '<div class="calibration-hero-copy"><div class="calibration-eyebrow">CALIBRATION 2.0 · EVIDENCE-LED TUNING</div><h3 id="calibration-hero-title">A faster preset, measured for this machine</h3><p id="calibration-hero-subtitle"></p></div><div class="calibration-hero-badges" id="calibration-hero-badges"></div>';
    body.prepend(hero);
    const subtitle = hero.querySelector('#calibration-hero-subtitle');
    subtitle.textContent = `${modelLabel(preset)} · ${preset?.name || 'Source preset'} · llama.cpp`;
    const badges = hero.querySelector('#calibration-hero-badges');
    addBadge(badges, 'Source preserved', 'success');
    addBadge(badges, 'llama.cpp only', 'accent');

    const plan = document.createElement('section');
    plan.id = 'calibration-plan';
    plan.className = 'calibration-plan';
    plan.innerHTML = '<div><div class="calibration-section-kicker">Choose a bounded run</div><h3>How much evidence do you want?</h3><p>Quick is the recommended first pass. Balanced adds a wider comparison without changing your active server.</p></div><div class="calibration-budget-options" role="radiogroup" aria-label="Calibration budget"><label class="calibration-budget-option is-selected"><input type="radio" name="calibration-budget" value="quick" checked><span><strong>Quick</strong><small>Fast signal · 2–4 trials</small></span></label><label class="calibration-budget-option"><input type="radio" name="calibration-budget" value="balanced"><span><strong>Balanced</strong><small>More confidence · bounded run</small></span></label></div><div class="calibration-plan-meta"><span id="calibration-plan-trials">Preflight will confirm trial count</span><span>Expected duration <strong id="calibration-plan-duration">about 1–3 min</strong></span></div></section>';
    body.insertBefore(plan, document.getElementById('calibration-track-options'));
    plan.querySelectorAll('input[name="calibration-budget"]').forEach((input) => input.addEventListener('change', () => {
        calibrationBudget = input.value;
        plan.querySelectorAll('.calibration-budget-option').forEach((option) => option.classList.toggle('is-selected', option.querySelector('input')?.checked));
        const duration = document.getElementById('calibration-plan-duration');
        if (duration) duration.textContent = calibrationBudget === 'balanced' ? 'about 4–8 min' : 'about 1–3 min';
    }));

    const progress = document.createElement('section');
    progress.id = 'calibration-progress';
    progress.className = 'calibration-progress';
    progress.hidden = true;
    progress.innerHTML = '<div class="calibration-progress-head"><div><div class="calibration-section-kicker">Live run</div><strong id="calibration-progress-phase">Preparing</strong></div><span id="calibration-progress-count">0 / 0</span></div><div class="calibration-progress-track" role="progressbar" aria-label="Calibration progress" aria-valuemin="0" aria-valuemax="100" aria-valuenow="0"><span id="calibration-progress-fill"></span></div><div class="calibration-progress-meta"><span>Elapsed <strong id="calibration-progress-elapsed">0s</strong></span><span>ETA <strong id="calibration-progress-eta">—</strong></span><span id="calibration-progress-live">Waiting for first sample</span></div>';
    body.insertBefore(progress, document.getElementById('calibration-status'));

    const results = document.createElement('section');
    results.id = 'calibration-results-panel';
    results.className = 'calibration-results-panel';
    results.hidden = true;
    results.innerHTML = '<div class="calibration-results-heading"><div><div class="calibration-section-kicker">Measured outcome</div><h3>Choose the best supported trade-off</h3></div><div class="calibration-results-tools"><label>Filter <input id="calibration-result-filter" type="search" placeholder="Search candidate" aria-label="Filter candidates"></label><label>Sort <select id="calibration-result-sort" aria-label="Sort candidates"><option value="throughput">Throughput</option><option value="memory">Memory</option><option value="confidence">Confidence</option></select></label></div></div><div id="calibration-recommendation" class="calibration-recommendation"></div><div id="calibration-candidate-table" class="calibration-candidate-table"></div><div id="calibration-track-results" class="calibration-track-results"></div></section>';
    body.insertBefore(results, document.getElementById('calibration-candidates'));
    results.querySelector('#calibration-result-filter')?.addEventListener('input', () => renderCandidates(currentResults));
    results.querySelector('#calibration-result-sort')?.addEventListener('change', () => renderCandidates(currentResults));

    const applyPreview = document.createElement('section');
    applyPreview.id = 'calibration-apply-preview';
    applyPreview.className = 'calibration-apply-preview';
    applyPreview.hidden = true;
    applyPreview.innerHTML = '<div class="calibration-section-kicker">Safe hand-off</div><h3>Review before creating the derived preset</h3><p id="calibration-apply-preview-copy"></p><div id="calibration-apply-preview-values" class="calibration-apply-preview-values"></div>';
    body.insertBefore(applyPreview, document.getElementById('calibration-error'));
}

function updateProgress(job) {
    const progress = document.getElementById('calibration-progress');
    if (!progress) return;
    progress.hidden = !job || job.state === 'complete';
    if (!job) return;
    const total = Number(job.planned_trials || 0);
    const completed = Number(job.completed_trials || 0);
    const percent = total ? Math.min(100, Math.round((completed / total) * 100)) : 0;
    const fill = document.getElementById('calibration-progress-fill');
    const bar = progress.querySelector('[role="progressbar"]');
    if (fill) fill.style.width = `${percent}%`;
    if (bar) bar.setAttribute('aria-valuenow', String(percent));
    const phase = document.getElementById('calibration-progress-phase');
    const count = document.getElementById('calibration-progress-count');
    const elapsed = document.getElementById('calibration-progress-elapsed');
    const eta = document.getElementById('calibration-progress-eta');
    const live = document.getElementById('calibration-progress-live');
    if (phase) phase.textContent = String(job.phase || job.state || 'running').replaceAll('_', ' ');
    if (count) count.textContent = `${completed} / ${total || '—'}`;
    const seconds = calibrationStartedAt ? (Date.now() - calibrationStartedAt) / 1000 : 0;
    if (elapsed) elapsed.textContent = formatDuration(seconds);
    if (eta) eta.textContent = completed > 0 && total > completed ? formatDuration((seconds / completed) * (total - completed)) : 'calculating…';
    if (live) live.textContent = job.live_metrics?.throughput_tps ? `${Number(job.live_metrics.throughput_tps).toFixed(1)} tok/s · live sample` : 'Collecting stable samples';
}

function setResultMode(completed) {
    const plan = document.getElementById('calibration-plan');
    const tracks = document.getElementById('calibration-track-options');
    const description = document.getElementById('calibration-modal-description');
    const status = document.getElementById('calibration-status');
    [plan, tracks, description, status].forEach((element) => {
        if (element) element.hidden = completed;
    });
}

function renderCandidates(results = [], preflight = currentPreflight) {
    const container = document.getElementById('calibration-candidates');
    if (!container) return;
    currentResults = results;
    container.replaceChildren();
    const rows = results.length ? results : (preflight?.candidate_ids || []).map(id => ({ candidate: { id } }));
    const panel = document.getElementById('calibration-results-panel');
    const table = document.getElementById('calibration-candidate-table');
    const filter = document.getElementById('calibration-result-filter')?.value?.trim().toLowerCase() || '';
    const sort = document.getElementById('calibration-result-sort')?.value || 'throughput';
    const visibleRows = rows.filter((result) => !filter || String(result.candidate?.id || '').toLowerCase().includes(filter));
    visibleRows.sort((a, b) => {
        if (sort === 'memory') return Number(a.measurement?.memory_bytes || Infinity) - Number(b.measurement?.memory_bytes || Infinity);
        if (sort === 'confidence') return Number(b.measurement?.confidence || 0) - Number(a.measurement?.confidence || 0);
        return Number(b.measurement?.tg_tps_samples?.[0] || 0) - Number(a.measurement?.tg_tps_samples?.[0] || 0);
    });
    if (table) table.replaceChildren();
    visibleRows.forEach((result) => {
        const row = document.createElement('div');
        row.className = 'calibration-candidate';
        const label = document.createElement('span');
        label.className = 'calibration-candidate-label';
        label.textContent = result.candidate?.id || 'candidate';
        const details = document.createElement('span');
        details.className = 'calibration-candidate-details';
        const value = document.createElement('span');
        value.className = 'calibration-candidate-value';
        const samples = result.measurement?.tg_tps_samples || [];
        value.textContent = samples.length ? `${samples[0].toFixed(1)} tok/s` : 'pending';
        const confidence = Number(result.measurement?.confidence);
        if (Number.isFinite(confidence)) addBadge(details, `${Math.round(confidence * 100)}% confidence`, confidence >= 0.8 ? 'success' : 'warn');
        if (result.measurement?.status && result.measurement.status !== 'ok') addBadge(details, result.measurement.status, 'warn');
        row.append(label, details, value);
        container.appendChild(row);
        if (table) {
            const card = document.createElement('details');
            card.className = `calibration-candidate-card${result.candidate?.id === currentReceipt?.selected_candidate ? ' is-recommended' : ''}`;
            const summary = document.createElement('summary');
            const title = document.createElement('span');
            title.className = 'calibration-card-title';
            title.textContent = result.candidate?.id || 'candidate';
            const metric = document.createElement('span');
            metric.className = 'calibration-card-metric';
            metric.textContent = samples.length ? `${samples[0].toFixed(1)} tok/s` : 'Pending';
            summary.append(title, metric);
            if (result.candidate?.id === currentReceipt?.selected_candidate) addBadge(summary, 'Recommended', 'success');
            card.appendChild(summary);
            const body = document.createElement('div');
            body.className = 'calibration-card-body';
            const config = result.candidate?.config || result.candidate?.parameters || {};
            Object.entries(config).forEach(([name, configured]) => {
                const span = document.createElement('span');
                span.textContent = `${name.replaceAll('_', ' ')} ${configured}`;
                body.appendChild(span);
            });
            if (!body.childElementCount) body.textContent = 'Measured against the effective source preset. No unsupported runtime flags were introduced.';
            card.appendChild(body);
            table.appendChild(card);
        }
    });
    container.hidden = results.length > 0 || visibleRows.length === 0;
    if (panel) panel.hidden = rows.length === 0;
    const recommendation = document.getElementById('calibration-recommendation');
    if (recommendation && currentReceipt?.selected_candidate) {
        const winner = rows.find((result) => result.candidate?.id === currentReceipt.selected_candidate);
        recommendation.replaceChildren();
        const copy = document.createElement('div');
        copy.innerHTML = '<div class="calibration-recommendation-kicker">TOP RECOMMENDATION</div><h4></h4><p>Measured as the strongest supported trade-off for this preset. Your source preset remains untouched until you approve the derived copy.</p>';
        copy.querySelector('h4').textContent = winner?.candidate?.id || currentReceipt.selected_candidate;
        recommendation.appendChild(copy);
        addBadge(recommendation, 'Evidence-backed', 'success');
    }
}

function renderApplyPreview(receipt) {
    const preview = document.getElementById('calibration-apply-preview');
    if (!preview || !receipt?.selected_candidate) return;
    const copy = document.getElementById('calibration-apply-preview-copy');
    const values = document.getElementById('calibration-apply-preview-values');
    const winner = (receipt.candidate_results || []).find((result) => result.candidate?.id === receipt.selected_candidate);
    if (copy) copy.textContent = `Create a new preset from “${receipt.selected_candidate}”. The active session and original preset will remain unchanged.`;
    values?.replaceChildren();
    const config = winner?.candidate?.config || winner?.candidate?.parameters || {};
    Object.entries(config).forEach(([name, value]) => {
        const item = document.createElement('span');
        item.textContent = `${name.replaceAll('_', ' ')}: ${value}`;
        values?.appendChild(item);
    });
    if (values && !values.childElementCount) values.textContent = 'Only measured, supported values are carried forward.';
    preview.hidden = false;
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

function renderServerQualification(receipt) {
  const body = document.querySelector('.calibration-modal-body');
  if (!body || !receipt) return;
  let section = document.getElementById('calibration-server-qualification');
  if (!section) {
    section = document.createElement('section');
    section.id = 'calibration-server-qualification';
    section.className = 'calibration-baseline';
    const title = document.createElement('h3');
    title.textContent = 'Real-server qualification';
    section.appendChild(title);
    const details = document.createElement('dl');
    details.id = 'calibration-server-qualification-values';
    details.className = 'calibration-baseline-values';
    section.appendChild(details);
    body.insertBefore(section, document.getElementById('calibration-candidates'));
  }
  const details = document.getElementById('calibration-server-qualification-values');
  details.replaceChildren();
  const trackResults = document.getElementById('calibration-track-results');
  if (trackResults) trackResults.replaceChildren();
  receipt.tracks?.forEach((track) => {
    const term = document.createElement('dt');
    term.textContent = track.track.replaceAll('_', ' ');
    const value = document.createElement('dd');
    const latency = track.latency;
    const tool = track.tool;
    value.textContent = latency?.time_to_first_token_ms != null
      ? `${track.status}; TTFT ${latency.time_to_first_token_ms.toFixed(0)} ms`
      : tool?.tool_call_observed
        ? `${track.status}; tool call observed`
        : track.status;
    details.append(term, value);
    if (trackResults) {
      const card = document.createElement('article');
      card.className = 'calibration-track-result';
      const heading = document.createElement('strong');
      heading.textContent = track.track.replaceAll('_', ' ');
      const summary = document.createElement('span');
      summary.textContent = latency?.time_to_first_token_ms != null
        ? `${track.status} · TTFT ${latency.time_to_first_token_ms.toFixed(0)} ms`
        : tool?.tool_call_observed ? `${track.status} · tool call observed` : track.status;
      card.append(heading, summary);
      trackResults.appendChild(card);
    }
  });
  if (receipt.memory?.process_rss_peak_bytes != null) {
    const term = document.createElement('dt');
    term.textContent = 'process RSS peak';
    const value = document.createElement('dd');
    value.textContent = `${(receipt.memory.process_rss_peak_bytes / (1024 ** 2)).toFixed(0)} MiB`;
    details.append(term, value);
  }
  if (receipt.server_log_tail?.trim()) {
    const term = document.createElement('dt');
    term.textContent = 'server log tail';
    const value = document.createElement('dd');
    const pre = document.createElement('pre');
    pre.className = 'calibration-server-log-tail';
    pre.textContent = receipt.server_log_tail;
    value.appendChild(pre);
    details.append(term, value);
  }
  if (receipt.baseline?.tracks?.length) {
    const baselineLatency = receipt.baseline.tracks.find((track) => track.latency)?.latency;
    if (baselineLatency?.time_to_first_token_ms != null) {
      const term = document.createElement('dt');
      term.textContent = 'no-spec baseline TTFT';
      const value = document.createElement('dd');
      value.textContent = `${baselineLatency.time_to_first_token_ms.toFixed(0)} ms`;
      details.append(term, value);
    }
  }
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
    ensureCalibrationSurface(preset);
    setError();
    setStatus('Validating preset, model library, and managed llama-bench…');
    document.getElementById('calibration-start').disabled = true;
    document.getElementById('calibration-apply').disabled = true;
    document.getElementById('calibration-rollback').hidden = true;
    document.getElementById('calibration-rollback').disabled = true;
    currentJobId = null;
    currentReceipt = null;
    lastApply = null;
    currentResults = [];
    calibrationStartedAt = null;
    document.getElementById('calibration-results-panel').hidden = true;
    document.getElementById('calibration-apply-preview').hidden = true;
    document.getElementById('calibration-progress').hidden = true;
    setResultMode(false);
  document.getElementById('calibration-baseline')?.remove();
  document.getElementById('calibration-server-qualification')?.remove();
    try {
        const data = await requestJson('/api/calibrations/preflight', {
            method: 'POST',
            headers: apiHeaders(true),
            body: JSON.stringify({ preset_id: preset.id }),
        });
        currentPreflight = data.preflight;
        renderCandidates([], currentPreflight);
        const trials = document.getElementById('calibration-plan-trials');
        if (trials) trials.textContent = `${currentPreflight.planned_trials} bounded ${calibrationBudget === 'balanced' ? 'Balanced' : 'Quick'} trial(s) planned`;
        setStatus(`Ready: ${currentPreflight.planned_trials} bounded ${calibrationBudget === 'balanced' ? 'Balanced' : 'Quick'} trial(s). The active session and source preset will remain unchanged.`);
        document.getElementById('calibration-start').disabled = false;
    } catch (error) {
        setError(error.message || String(error));
        setStatus('Preflight could not be completed.');
    }
}

function updateCalibrationConcurrencyControls() {
  const checkbox = document.getElementById('calibration-track-concurrency');
  const count = document.getElementById('calibration-concurrency-count');
  if (count) count.hidden = !checkbox?.checked;
}

function selectedServerQualification() {
  const concurrency = document.getElementById('calibration-track-concurrency')?.checked === true;
  const tool = document.getElementById('calibration-track-tool')?.checked !== false;
  const parallel = concurrency
    ? Math.max(2, Math.min(8, Number(document.getElementById('calibration-parallel-requests')?.value || 2)))
    : 1;
  const tracks = ['latency_memory'];
  if (tool) tracks.push('tool_correctness');
  if (concurrency) tracks.push('concurrency');
  return {
    tracks,
    parallel_requests: parallel,
    allow_concurrency: concurrency,
    prompt: 'Reply with one short sentence describing a calibration check.',
    generation_tokens: 256,
    timeout_ms: 30000,
    capability_evidence: [],
  };
}

async function startCalibration() {
    if (!currentPreflight) return;
    const context = Number(document.getElementById('modal-context-size')?.value || 4096);
    setError();
    calibrationStartedAt = Date.now();
    setStatus(`Starting bounded ${calibrationBudget === 'balanced' ? 'Balanced' : 'Quick'} calibration…`);
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
                budget: calibrationBudget, kv_quality_floor: 'q8_0',
                allow_stop_active_server: false,
    exact_confirmation: currentPreflight.confirmation,
    server_qualification: selectedServerQualification(),
            }),
        });
        currentJobId = data.job.id;
        updateProgress(data.job);
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
        updateProgress(job);
        setStatus(`${job.phase || job.state}: ${job.completed_trials || 0}/${job.planned_trials || 0} trial(s)`);
        if (job.state === 'complete') {
            const receipt = await requestJson(`/api/calibrations/${encodeURIComponent(currentJobId)}/receipt`, { headers: apiHeaders() });
        currentReceipt = receipt.receipt;
        renderBaseline(currentReceipt.baseline);
        renderCandidates(currentReceipt.candidate_results || []);
        renderServerQualification(currentReceipt.server_qualification);
        renderApplyPreview(currentReceipt);
        updateProgress(job);
        setResultMode(true);
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
        document.getElementById('calibration-status').hidden = false;
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
        document.getElementById('calibration-status').hidden = false;
        setStatus('Calibration apply rolled back. The source preset is restored.');
        showToast('Calibration apply rolled back', 'success');
        window.dispatchEvent(new CustomEvent('presets:reload'));
    } catch (error) {
        setError(error.message || String(error));
    }
}

export function initCalibrationUi() {
  document.getElementById('calibration-track-concurrency')?.addEventListener('change', updateCalibrationConcurrencyControls);
  updateCalibrationConcurrencyControls();
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
