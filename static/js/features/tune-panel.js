// ── Tune / Benchmark Panel ────────────────────────────────────────────────────
// Drives the Performance dropdown on the monitor view.
// Triggered by "Benchmark" pill in the Inference Metrics section header.
// Flow: idle → runBenchmark() → running → renderResults() → results
//       results → applyAndRetune() → running → results (updated)

import { showToast } from './toast.js';
import { renderSuggestionCards, suggestionPatch } from './tuning-cards.js';

// Last config used to spawn/start the server. Apply mutations build on this.
let _tuneConfig = null;

// ── Public API ────────────────────────────────────────────────────────────────

/** Call after a successful spawn or start so Apply knows what to modify. */
export function setTuneConfig(config) {
  _tuneConfig = config ? { ...config } : null;
}

/** Show the Benchmark pill (called when server becomes connected). */
export function showTunePanel() {
  const pill = document.getElementById('benchmark-pill');
  if (pill) pill.classList.add('show');
  const group = document.getElementById('inference-log-tail-group');
  if (group) group.style.display = '';
}

/** Hide the Benchmark pill and close dropdown (called when server disconnects). */
export function hideTunePanel() {
  const pill = document.getElementById('benchmark-pill');
  if (pill) pill.classList.remove('show');
  _closeDropdown();
  _resetToIdle();
}

export function initTunePanel() {
  document.getElementById('tune-run-btn')?.addEventListener('click', runBenchmark);
  document.getElementById('tune-rerun-btn')?.addEventListener('click', runBenchmark);

  // Pill toggle: open/close dropdown
  const pill = document.getElementById('benchmark-pill');
  const wrap = document.getElementById('benchmark-dropdown-wrap');
  if (pill && wrap) {
    pill.addEventListener('click', (e) => {
      e.stopPropagation();
      const isOpen = wrap.classList.toggle('open');
      pill.classList.toggle('is-active', isOpen);
      if (isOpen) _positionDropdown(pill, wrap);
    });

    // Close when clicking outside
    document.addEventListener('click', (e) => {
      if (wrap.classList.contains('open') &&
          !wrap.contains(e.target) &&
          !pill.contains(e.target)) {
        _closeDropdown();
      }
    });
  }
}

function _positionDropdown(trigger, dropdown) {
  const rect = trigger.getBoundingClientRect();
  dropdown.style.top = (rect.bottom + 8) + 'px';
  dropdown.style.left = Math.min(rect.left, window.innerWidth - 450) + 'px';
  // Reposition on scroll/resize
  const ro = new ResizeObserver(() => _positionDropdown(trigger, dropdown));
  ro.observe(trigger);
  window.addEventListener('scroll', () => _positionDropdown(trigger, dropdown), { passive: true });
}

function _closeDropdown() {
  const wrap = document.getElementById('benchmark-dropdown-wrap');
  const pill = document.getElementById('benchmark-pill');
  if (wrap) {
    wrap.classList.remove('open');
    wrap.style.top = '';
    wrap.style.left = '';
  }
  if (pill) pill.classList.remove('is-active');
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
    gradeEl.className   = `tune-grade-ring tune-grade-${grade.letter}`;
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
  renderSuggestionCards(container, suggestions, {
    onApply: _applyAndRetune,
    config: _tuneConfig,
    emptyMessage: (suggestions && suggestions.length === 0)
      ? 'Your config looks well-tuned for this hardware.'
      : 'No further automatic suggestions — all recommended settings are already applied.',
  });
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

  // Mutate a copy of the config (supports multi-field patches)
  const newConfig = { ..._tuneConfig, ...suggestionPatch(suggestion) };
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
