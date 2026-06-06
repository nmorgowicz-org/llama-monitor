// ── Tuning suggestion cards (shared renderer) ────────────────────────────────
// One renderer for every place we surface tuning advice: the live Tune/Benchmark
// panel, the Spawn Wizard performance advisor, and the Preset Editor advisor.
//
// Each suggestion follows the backend contract from spawn_wizard.rs:
//   { label, description, param, value, patch? }
// - param === ""        → informational-only card (no Apply button).
// - patch (object)      → multi-field change; merged wholesale on Apply.
// - param + value       → single-field change.

import { escapeHtml } from '../core/format.js';

/**
 * Request an n_cpu_moe recommendation from the backend.
 * Pass `verify: true` in the body to run the empirical llama-bench sweep
 * (requires no server running). Returns the parsed JSON
 * `{ recommended_n_cpu_moe, verified, estimate?, probes?, error? }`.
 */
export async function requestNcpuMoeTune(body) {
  const headers = window.authHeaders
    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
    : { 'Content-Type': 'application/json' };
  const r = await fetch('/api/tune/ncpumoe', {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
  });
  return r.json();
}

/**
 * Run a depth sweep via the backend (llama-bench). Returns parsed JSON
 * `{ points: [{depth, pp_tps, tg_tps}], error? }`.
 */
export async function requestDepthSweep(body) {
  const headers = window.authHeaders
    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
    : { 'Content-Type': 'application/json' };
  const r = await fetch('/api/bench/sweep', {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
  });
  return r.json();
}

/** Compact depth → tokens/s sparkline + table for a sweep result. */
export function renderDepthSweep(container, points) {
  if (!container) return;
  container.replaceChildren();
  if (!points || points.length === 0) {
    container.textContent = 'No sweep data.';
    return;
  }

  const fmtDepth = (d) => (d >= 1000 ? `${Math.round(d / 1000)}k` : String(d));
  const maxTg = Math.max(...points.map((p) => p.tg_tps), 1);

  // Sparkline (decode t/s vs depth)
  const w = 200;
  const h = 40;
  const stepX = points.length > 1 ? w / (points.length - 1) : 0;
  const coords = points.map((p, i) => {
    const x = (i * stepX).toFixed(1);
    const y = (h - (p.tg_tps / maxTg) * (h - 4) - 2).toFixed(1);
    return `${x},${y}`;
  });
  const svgNs = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(svgNs, 'svg');
  svg.setAttribute('viewBox', `0 0 ${w} ${h}`);
  svg.setAttribute('class', 'depth-sweep-spark');
  svg.setAttribute('preserveAspectRatio', 'none');
  const poly = document.createElementNS(svgNs, 'polyline');
  poly.setAttribute('points', coords.join(' '));
  poly.setAttribute('fill', 'none');
  poly.setAttribute('stroke', 'currentColor');
  poly.setAttribute('stroke-width', '2');
  svg.appendChild(poly);
  container.appendChild(svg);

  // Table
  const table = document.createElement('table');
  table.className = 'depth-sweep-table';
  const head = document.createElement('tr');
  ['Depth', 'Prefill t/s', 'Decode t/s'].forEach((t) => {
    const th = document.createElement('th');
    th.textContent = t;
    head.appendChild(th);
  });
  table.appendChild(head);
  points.forEach((p) => {
    const tr = document.createElement('tr');
    [fmtDepth(p.depth), p.pp_tps ? p.pp_tps.toFixed(0) : '—', p.tg_tps ? p.tg_tps.toFixed(1) : '—'].forEach((v) => {
      const td = document.createElement('td');
      td.textContent = v;
      tr.appendChild(td);
    });
    table.appendChild(tr);
  });
  container.appendChild(table);
}

/**
 * Resolve the full set of config keys a suggestion would change.
 * Returns a plain object suitable for Object.assign onto a config.
 */
export function suggestionPatch(suggestion) {
  if (suggestion.patch && typeof suggestion.patch === 'object') {
    return { ...suggestion.patch };
  }
  if (suggestion.param) {
    return { [suggestion.param]: suggestion.value };
  }
  return {};
}

/**
 * True if `config` already has the suggestion's *primary* field at the target
 * value. We key off `param` (the representative field) rather than every patch
 * key, so a card clears as soon as its headline change is applied — secondary
 * patch keys (e.g. spec_draft_n_max) don't keep it lingering.
 */
export function isSuggestionApplied(suggestion, config) {
  if (!config || !suggestion.param) return false; // informational cards never "applied"
  const cur = config[suggestion.param];
  if (cur === undefined || cur === null) return false;
  return String(cur) === String(suggestion.value);
}

/**
 * Render suggestion cards into `container`.
 *
 * @param {HTMLElement} container
 * @param {Array} suggestions
 * @param {Object} opts
 * @param {(s, cardEl) => void} [opts.onApply]  Apply handler (omit to hide buttons).
 * @param {Object}   [opts.config]              Current config, to filter applied advice.
 * @param {string}   [opts.emptyMessage]        Shown when nothing actionable remains.
 */
export function renderSuggestionCards(container, suggestions, opts = {}) {
  const { onApply, config, emptyMessage } = opts;
  if (!container) return;
  container.replaceChildren();

  const list = (suggestions || []).filter((s) => !isSuggestionApplied(s, config));

  if (list.length === 0) {
    if (emptyMessage) {
      const msg = document.createElement('p');
      msg.className = 'tune-no-suggestions';
      msg.textContent = emptyMessage;
      container.appendChild(msg);
    }
    return;
  }

  list.forEach((suggestion) => {
    const informational = !suggestion.param;

    const card = document.createElement('div');
    card.className = 'tune-suggestion-card' + (informational ? ' tune-suggestion-info' : '');

    const body = document.createElement('div');
    body.className = 'tune-suggestion-body';

    const label = document.createElement('div');
    label.className = 'tune-suggestion-label';
    label.innerHTML = escapeHtml(suggestion.label);

    const desc = document.createElement('div');
    desc.className = 'tune-suggestion-desc';
    desc.innerHTML = escapeHtml(suggestion.description);

    body.appendChild(label);
    body.appendChild(desc);
    card.appendChild(body);

    if (!informational && typeof onApply === 'function') {
      const btn = document.createElement('button');
      btn.className = 'tune-suggestion-apply';
      btn.type = 'button';
      btn.textContent = 'Apply →';
      btn.addEventListener('click', () => onApply(suggestion, card));
      card.appendChild(btn);
    }

    container.appendChild(card);
  });
}
