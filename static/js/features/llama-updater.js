// ── llama-server Binary Updater ───────────────────────────────────────────────
// Shows current binary version in nav pill. When a newer build is available,
// pill turns amber and clicking it confirms + triggers the update.

import { showToast } from './toast.js';

let _currentBuild = null;
let _latestBuild  = null;
let _updating     = false;

export function initLlamaUpdater() {
  const pill = document.getElementById('llama-pill');
  if (!pill) return;
  pill.addEventListener('click', onPillClick);
  checkVersion();
}

async function checkVersion() {
  const pill    = document.getElementById('llama-pill');
  const verSpan = document.getElementById('llama-pill-version');
  if (!pill || !verSpan) return;

  try {
    const headers = window.authHeaders ? window.authHeaders() : {};

    // Current installed version
    const vResp = await fetch('/api/llama-binary/version', { headers });
    if (!vResp.ok) return;
    const vData = await vResp.json();
    _currentBuild = vData.build ?? null;

    if (_currentBuild) {
      verSpan.textContent = `llama.cpp · b${_currentBuild}`;
      pill.style.display = 'flex';
    }

    // Latest available from GitHub
    const lResp = await fetch('/api/llama-binary/latest', { headers });
    if (!lResp.ok) return;
    const lData = await lResp.json();
    if (lData.error) return;
    _latestBuild = lData.build ?? null;

    if (_latestBuild && _currentBuild && _latestBuild > _currentBuild) {
      verSpan.textContent = `llama.cpp · ↑ b${_latestBuild}`;
      pill.classList.remove('llama-pill-idle');
      pill.classList.add('llama-pill-update');
      pill.title = `Update available: b${_currentBuild} → b${_latestBuild}. Click to update.`;
    }
  } catch (e) {
    // silently ignore — this is a background check
  }
}

async function onPillClick() {
  if (_updating) return;

  if (!_latestBuild || !_currentBuild || _latestBuild <= _currentBuild) {
    // No update — just show current info
    showToast(`llama.cpp binary is up to date (b${_currentBuild})`, 'success');
    return;
  }

  const confirmed = confirm(
    `Update llama-server from b${_currentBuild} to b${_latestBuild}?\n\nThe server will be replaced at its current path. Stop it first if it is running.`
  );
  if (!confirmed) return;

  await doUpdate();
}

async function doUpdate() {
  _updating = true;
  const pill    = document.getElementById('llama-pill');
  const verSpan = document.getElementById('llama-pill-version');

  if (verSpan) verSpan.textContent = 'Updating…';
  if (pill) {
    pill.classList.remove('llama-pill-update');
    pill.classList.add('llama-pill-busy');
    pill.disabled = true;
  }

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };

    const resp = await fetch('/api/llama-binary/update', {
      method: 'POST',
      headers,
      body: JSON.stringify({}),
    });
    const data = await resp.json();

    if (!data.ok) throw new Error(data.error || 'Update failed');

    _currentBuild = _latestBuild;
    if (verSpan) verSpan.textContent = `llama.cpp · b${_currentBuild}`;
    showToast(`Updated to b${_currentBuild}`, 'success', 'Restart the server to use the new binary.');
    if (pill) {
      pill.classList.remove('llama-pill-busy');
      pill.classList.add('llama-pill-idle');
      pill.disabled = false;
      pill.title = 'llama-server binary version';
    }
  } catch (err) {
    showToast('Update failed', 'error', err.message || String(err));
    if (verSpan) verSpan.textContent = `llama.cpp · b${_currentBuild ?? '?'}`;
    if (pill) {
      pill.classList.remove('llama-pill-busy');
      pill.classList.add('llama-pill-update');
      pill.disabled = false;
    }
  } finally {
    _updating = false;
  }
}
