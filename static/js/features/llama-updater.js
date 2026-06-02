// ── llama-server Binary Updater ───────────────────────────────────────────────
// Shows current binary version in nav pill + setup toolbar button.
// Clicking opens a version picker modal with the last 8 releases;
// any build (including older ones) can be installed from there.

import { showToast } from './toast.js';

let _currentBuild = null;
let _latestBuild  = null;
let _updating     = false;

export function initLlamaUpdater() {
  const pill = document.getElementById('llama-pill');
  if (pill) pill.addEventListener('click', openVersionModal);

  const closeBtn = document.getElementById('llama-version-modal-close');
  if (closeBtn) closeBtn.addEventListener('click', closeVersionModal);

  const overlay = document.getElementById('llama-version-modal');
  if (overlay) {
    overlay.addEventListener('click', e => {
      if (e.target === overlay) closeVersionModal();
    });
  }

  checkVersion();
  setInterval(checkVersion, 30 * 60 * 1000);
}

async function checkVersion() {
  const pill    = document.getElementById('llama-pill');
  const verSpan = document.getElementById('llama-pill-version');

  try {
    const headers = window.authHeaders ? window.authHeaders() : {};

    const vResp = await fetch('/api/llama-binary/version', { headers });
    if (!vResp.ok) return;
    const vData = await vResp.json();
    _currentBuild = vData.build ?? null;

    if (_currentBuild) {
      if (verSpan) verSpan.textContent = `llama.cpp · b${_currentBuild}`;
      if (pill) pill.style.display = 'flex';
    }

    const lResp = await fetch('/api/llama-binary/latest', { headers });
    if (!lResp.ok) return;
    const lData = await lResp.json();
    if (lData.error) return;
    _latestBuild = lData.build ?? null;

    if (_latestBuild && _currentBuild && _latestBuild > _currentBuild) {
      if (verSpan) verSpan.textContent = `llama.cpp · ↑ b${_latestBuild}`;
      if (pill) {
        pill.classList.remove('llama-pill-idle');
        pill.classList.add('llama-pill-update');
        pill.title = `Update available: b${_currentBuild} → b${_latestBuild}. Click to manage.`;
      }
    }
  } catch (_) {
    // silently ignore — background check
  }
}

// ── Modal ──────────────────────────────────────────────────────────────────────

async function openVersionModal() {
  if (_updating) return;

  const modal = document.getElementById('llama-version-modal');
  if (!modal) return;

  // Show current build in the header
  const buildEl = document.getElementById('llama-version-current-build');
  if (buildEl) {
    buildEl.textContent = _currentBuild ? `b${_currentBuild}` : '—';
    if (_latestBuild && _currentBuild && _latestBuild > _currentBuild) {
      buildEl.textContent += ` → b${_latestBuild} available`;
      buildEl.classList.add('llama-version-has-update');
    } else {
      buildEl.classList.remove('llama-version-has-update');
    }
  }

  modal.style.display = 'flex';
  modal.classList.add('open');
  document.body.style.overflow = 'hidden';

  await loadReleaseList();
}

function closeVersionModal() {
  const modal = document.getElementById('llama-version-modal');
  if (!modal) return;
  modal.style.display = 'none';
  modal.classList.remove('open');
  document.body.style.overflow = '';
  // Reset notes pane for next open
  const empty = document.getElementById('llama-version-notes-empty');
  const content = document.getElementById('llama-version-notes-content');
  if (empty) empty.style.display = '';
  if (content) content.style.display = 'none';
}

async function loadReleaseList() {
  const listEl = document.getElementById('llama-version-list');
  if (!listEl) return;

  listEl.innerHTML = '<div class="llama-version-loading">Loading releases…</div>';

  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch('/api/llama-binary/releases', { headers });
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    const data = await resp.json();
    if (data.error) throw new Error(data.error);

    const releases = data.releases ?? [];
    if (!releases.length) {
      listEl.innerHTML = '<div class="llama-version-loading">No releases found.</div>';
      return;
    }

    listEl.innerHTML = '';
    releases.forEach((r, i) => {
      const row = buildReleaseRow(r, i === 0);
      listEl.appendChild(row);
    });

    // Auto-select the latest release to show notes on open
    if (releases.length > 0) showReleaseNotes(releases[0]);
  } catch (err) {
    listEl.textContent = '';
    const errEl = document.createElement('div');
    errEl.className = 'llama-version-loading llama-version-error';
    errEl.textContent = `Failed to load releases: ${err.message}`;
    listEl.appendChild(errEl);
  }
}

function showReleaseNotes(release) {
  const empty = document.getElementById('llama-version-notes-empty');
  const content = document.getElementById('llama-version-notes-content');
  const tagEl = document.getElementById('llama-version-notes-tag');
  const bodyEl = document.getElementById('llama-version-notes-body');

  if (!content || !tagEl || !bodyEl) return;

  // Highlight selected row
  document.querySelectorAll('.llama-version-row').forEach(r => r.classList.remove('llama-version-row--selected'));
  const tag = release.tag ?? `b${release.build}`;
  document.querySelectorAll('.llama-version-row').forEach(r => {
    if (r.dataset.tag === tag) r.classList.add('llama-version-row--selected');
  });

  tagEl.textContent = tag;
  if (empty) empty.style.display = 'none';
  content.style.display = '';

  if (release.body && release.body.trim()) {
    const md = typeof marked !== 'undefined' ? marked.parse(release.body) : release.body.replace(/\n/g, '<br>');
    // eslint-disable-next-line no-unsanitized/property
    bodyEl.innerHTML = typeof DOMPurify !== 'undefined' ? DOMPurify.sanitize(md) : md;
  } else {
    bodyEl.innerHTML = '<p class="llama-version-notes-none">No release notes for this build.</p>';
  }
}

function buildReleaseRow(release, isLatest) {
  const isCurrent = _currentBuild !== null && release.build === _currentBuild;
  const age = release.published_at ? relativeAge(release.published_at) : '';
  const tag = release.tag ?? `b${release.build}`;

  const row = document.createElement('div');
  row.className = 'llama-version-row' + (isCurrent ? ' llama-version-row--current' : '');
  row.dataset.tag = tag;
  row.title = 'Click to view release notes';
  row.style.cursor = 'pointer';
  row.addEventListener('click', (e) => {
    if (e.target.closest('.llama-version-install-btn')) return;
    showReleaseNotes(release);
  });

  const info = document.createElement('div');
  info.className = 'llama-version-row-info';

  const tagEl = document.createElement('span');
  tagEl.className = 'llama-version-row-tag';
  tagEl.textContent = tag;

  const badges = document.createElement('span');
  badges.className = 'llama-version-row-badges';
  if (isLatest) {
    const b = document.createElement('span');
    b.className = 'llama-version-badge llama-version-badge--latest';
    b.textContent = 'latest';
    badges.appendChild(b);
  }
  if (isCurrent) {
    const b = document.createElement('span');
    b.className = 'llama-version-badge llama-version-badge--installed';
    b.textContent = 'installed';
    badges.appendChild(b);
  }

  const meta = document.createElement('span');
  meta.className = 'llama-version-row-age';
  meta.textContent = age;

  info.append(tagEl, badges);

  const right = document.createElement('div');
  right.className = 'llama-version-row-right';
  right.appendChild(meta);

  if (!isCurrent) {
    const btn = document.createElement('button');
    btn.className = 'llama-version-install-btn';
    btn.textContent = 'Install';
    btn.dataset.tag = tag;
    btn.dataset.build = release.build ?? '';
    btn.addEventListener('click', () => installRelease(btn, release));
    right.appendChild(btn);
  }

  row.append(info, right);
  return row;
}

async function installRelease(btn, release) {
  if (_updating) return;
  _updating = true;

  const tag = release.tag ?? `b${release.build}`;
  btn.disabled = true;
  btn.textContent = 'Installing…';

  const startTime = Date.now();
  const timer = setInterval(() => {
    const s = Math.round((Date.now() - startTime) / 1000);
    btn.textContent = `${s}s…`;
  }, 1000);

  // Also update the nav pill to show progress
  const verSpan = document.getElementById('llama-pill-version');
  const pill    = document.getElementById('llama-pill');
  if (pill) { pill.classList.add('llama-pill-busy'); pill.disabled = true; }
  if (verSpan) verSpan.textContent = 'Installing…';

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };

    const resp = await fetch('/api/llama-binary/update', {
      method: 'POST',
      headers,
      body: JSON.stringify({ tag }),
    });
    clearInterval(timer);
    const data = await resp.json();
    if (!data.ok) throw new Error(data.error || 'Install failed');

    _currentBuild = release.build ?? _latestBuild;
    const sha = data.sha256 ? `SHA256: ${data.sha256}` : 'Restart the server to use the new binary.';
    showToast(`Installed ${tag}`, 'success', sha);

    // Refresh pill
    if (verSpan) verSpan.textContent = `llama.cpp · b${_currentBuild}`;
    if (pill) {
      pill.classList.remove('llama-pill-busy', 'llama-pill-update');
      pill.classList.add('llama-pill-idle');
      pill.disabled = false;
      pill.title = 'llama-server binary version';
    }
    // Refresh the list so badges update
    closeVersionModal();
  } catch (err) {
    clearInterval(timer);
    showToast('Install failed', 'error', err.message || String(err));
    btn.disabled = false;
    btn.textContent = 'Install';
    if (pill) {
      pill.classList.remove('llama-pill-busy');
      if (_latestBuild && _currentBuild && _latestBuild > _currentBuild) {
        pill.classList.add('llama-pill-update');
      } else {
        pill.classList.add('llama-pill-idle');
      }
      pill.disabled = false;
    }
    if (verSpan) verSpan.textContent = `llama.cpp · b${_currentBuild ?? '?'}`;
  } finally {
    _updating = false;
  }
}

function relativeAge(iso) {
  if (!iso) return '';
  const diff = Date.now() - new Date(iso).getTime();
  const mins  = Math.floor(diff / 60_000);
  const hours = Math.floor(diff / 3_600_000);
  const days  = Math.floor(diff / 86_400_000);
  if (mins < 60)  return `${mins}m ago`;
  if (hours < 24) return `${hours}h ago`;
  if (days < 30)  return `${days}d ago`;
  const months = Math.floor(days / 30);
  return `${months}mo ago`;
}
