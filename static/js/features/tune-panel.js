// ── Tune / Benchmark Panel ────────────────────────────────────────────────────
// Drives the Performance panel on the monitor view.
// Flow: idle → runBenchmark() → running → renderResults() → results
//       results → applyAndRetune() → running → results (updated)

import { showToast } from './toast.js';

// Last config used to spawn/start the server. Apply mutations build on this.
let _tuneConfig = null;

// ── Public API ────────────────────────────────────────────────────────────────

/** Call after a successful spawn or start so Apply knows what to modify. */
export function setTuneConfig(config) {
  _tuneConfig = config ? { ...config } : null;
}

/** Show the panel (called when server becomes connected). */
export function showTunePanel() {
  const panel = document.getElementById('tune-panel');
  if (panel) panel.style.display = '';
}

/** Hide and reset the panel (called when server disconnects). */
export function hideTunePanel() {
  const panel = document.getElementById('tune-panel');
  if (panel) panel.style.display = 'none';
  _resetToIdle();
}

export function initTunePanel() {
  document.getElementById('tune-run-btn')?.addEventListener('click', runBenchmark);
  document.getElementById('tune-rerun-btn')?.addEventListener('click', runBenchmark);
}

// ── Internal helpers ──────────────────────────────────────────────────────────

function _show(id) {
  const el = document.getElementById(id);
  if (el) el.style.display = '';
}

function _hide(id) {
  const el = document.getElementById(id);
  if (el) el.style.display = 'none';
}

function _resetToIdle() {
  _show('tune-idle');
  _hide('tune-running');
  _hide('tune-results');
}

// Homelab-calibrated grades. 8–15 t/s on a consumer GPU is solid — not "poor".
const GRADES = [
  { letter: 's', minTps: 25, label: 'Excellent' },
  { letter: 'a', minTps: 12, label: 'Good' },
  { letter: 'b', minTps: 6,  label: 'Usable' },
  { letter: 'c', minTps: 3,  label: 'Slow' },
  { letter: 'd', minTps: 0,  label: 'Very Slow' },
];

function _computeGrade(genTps) {
  return GRADES.find(g => genTps >= g.minTps) || GRADES[GRADES.length - 1];
}

// ── Benchmark ─────────────────────────────────────────────────────────────────

async function runBenchmark() {
  _hide('tune-idle');
  _hide('tune-results');
  _show('tune-running');

  // Disable both run buttons while benchmarking
  const runBtn   = document.getElementById('tune-run-btn');
  const rerunBtn = document.getElementById('tune-rerun-btn');
  if (runBtn)   runBtn.disabled   = true;
  if (rerunBtn) rerunBtn.disabled = true;

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };

    const resp = await fetch('/api/benchmark', {
      method: 'POST',
      headers,
      body: JSON.stringify({}),
    });

    const data = await resp.json();

    if (!resp.ok || data.error) {
      throw new Error(data.error || `HTTP ${resp.status}`);
    }

    _renderResults(data);
  } catch (err) {
    _resetToIdle();
    showToast('Benchmark failed', 'error', err.message || String(err));
  } finally {
    if (runBtn)   runBtn.disabled   = false;
    if (rerunBtn) rerunBtn.disabled = false;
  }
}

// ── Results rendering ─────────────────────────────────────────────────────────

function _renderResults(data) {
  const grade   = _computeGrade(data.gen_tokens_per_second ?? 0);
  const gradeEl = document.getElementById('tune-grade');
  if (gradeEl) {
    gradeEl.textContent = grade.letter.toUpperCase();
    gradeEl.className   = `tune-grade-chip tune-grade-${grade.letter}`;
  }
  const gradeLabelEl = document.getElementById('tune-grade-label');
  if (gradeLabelEl) gradeLabelEl.textContent = grade.label;

  const genTps   = data.gen_tokens_per_second   ?? 0;
  const promptTps = data.prompt_tokens_per_second ?? 0;
  const ttft      = data.time_to_first_token_ms   ?? 0;

  const genEl    = document.getElementById('tune-gen-tps');
  const promptEl = document.getElementById('tune-prompt-tps');
  const ttftEl   = document.getElementById('tune-ttft');

  if (genEl)    genEl.textContent    = genTps   >= 1 ? genTps.toFixed(1)   : '< 1';
  if (promptEl) promptEl.textContent = promptTps >= 1 ? promptTps.toFixed(0) : '< 1';
  if (ttftEl)   ttftEl.textContent   = ttft.toFixed(0);

  _renderSuggestions(data.suggestions || []);

  _hide('tune-running');
  _show('tune-results');
}

function _renderSuggestions(suggestions) {
  const container = document.getElementById('tune-suggestions');
  if (!container) return;
  container.innerHTML = '';

  // Filter out suggestions whose param is already set to the target value
  const applicable = suggestions.filter(s => !_isAlreadyApplied(s));

  if (applicable.length === 0) {
    const msg = document.createElement('p');
    msg.className = 'tune-no-suggestions';
    msg.textContent = applicable.length === 0 && suggestions.length === 0
      ? 'Your config looks well-tuned for this hardware.'
      : 'No further automatic suggestions — all recommended settings are already applied.';
    container.appendChild(msg);
    return;
  }

  applicable.forEach(suggestion => {
    const card = document.createElement('div');
    card.className = 'tune-suggestion-card';

    const body = document.createElement('div');
    body.className = 'tune-suggestion-body';

    const label = document.createElement('div');
    label.className = 'tune-suggestion-label';
    label.textContent = suggestion.label;

    const desc = document.createElement('div');
    desc.className = 'tune-suggestion-desc';
    desc.textContent = suggestion.description;

    body.appendChild(label);
    body.appendChild(desc);

    const btn = document.createElement('button');
    btn.className = 'tune-suggestion-apply';
    btn.type = 'button';
    btn.textContent = 'Apply →';
    btn.addEventListener('click', () => _applyAndRetune(suggestion, card));

    card.appendChild(body);
    card.appendChild(btn);
    container.appendChild(card);
  });
}

function _isAlreadyApplied(suggestion) {
  if (!_tuneConfig) return false;
  const current = _tuneConfig[suggestion.param];
  if (current === undefined || current === null) return false;
  // Loose comparison: "on" == "on", 8192 == 8192
  return String(current) === String(suggestion.value);
}

// Poll /api/sessions/active until status === "Running" or timeout expires.
async function _waitForServerReady(timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs;
  const headers = window.authHeaders ? window.authHeaders() : {};
  while (Date.now() < deadline) {
    try {
      const r = await fetch('/api/sessions/active', { headers });
      if (r.ok) {
        const d = await r.json();
        if (d.status === 'Running') return;
      }
    } catch { /* ignore transient errors */ }
    await new Promise(r => setTimeout(r, 800));
  }
  // Timeout — proceed anyway; benchmark will catch a still-loading server
}

// ── Apply & retune ────────────────────────────────────────────────────────────

async function _applyAndRetune(suggestion, cardEl) {
  if (!_tuneConfig) {
    showToast('No active config to modify', 'error', 'Try launching via Spawn New Server first.');
    return;
  }

  const applyBtn = cardEl?.querySelector('.tune-suggestion-apply');
  const rerunBtn = document.getElementById('tune-rerun-btn');

  if (applyBtn) { applyBtn.disabled = true; applyBtn.textContent = 'Applying…'; }
  if (rerunBtn) rerunBtn.disabled = true;

  // Mutate a copy of the config
  const newConfig = { ..._tuneConfig, [suggestion.param]: suggestion.value };
  _tuneConfig = newConfig;

  // Show running state with "Restarting…" hint
  const hint = document.querySelector('.tune-running-hint');
  if (hint) hint.textContent = 'Restarting server with new settings…';
  _hide('tune-results');
  _show('tune-running');

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };

    // Respawn with the modified config
    const startResp = await fetch('/api/start', {
      method: 'POST',
      headers,
      body: JSON.stringify(newConfig),
    });
    const startData = await startResp.json();
    if (!startResp.ok || !startData.ok) {
      throw new Error(startData.error || `Start failed (HTTP ${startResp.status})`);
    }

    // Poll until the session reports Running (max 60 s for large model loads)
    if (hint) hint.textContent = 'Waiting for server to become ready…';
    await _waitForServerReady(60_000);

    // Restore hint text and re-benchmark
    if (hint) hint.textContent = 'Sending a test prompt and measuring throughput…';
    await runBenchmark();
  } catch (err) {
    _resetToIdle();
    showToast('Apply failed', 'error', err.message || String(err));
  }
}
