/* global DOMPurify */

// ── Rapid-MLX Runtime Manager ─────────────────────────────────────────────────
// Manages Rapid-MLX runtime lifecycle in Settings and nav pill.
// Uses existing API endpoints under /api/rapid-mlx/runtime/.

import { showToast } from './toast.js';

let _runtimeStatus = null;
let _releases = [];
let _mutationInflight = false;

export function initRapidMlxUpdater() {
  // Settings: Manage button
  const manageBtn = document.getElementById('rapid-mlx-manage-btn');
  if (manageBtn) {
    manageBtn.addEventListener('click', openRapidMlxModal);
  }

  // Nav pill (for when there is an active runtime)
  const pill = document.getElementById('rapid-mlx-pill');
  if (pill) {
    pill.addEventListener('click', openRapidMlxModal);
  }

  // Modal: Upgrade button
  const upgradeBtn = document.getElementById('rapid-mlx-upgrade-btn');
  if (upgradeBtn) {
    upgradeBtn.addEventListener('click', async () => {
      if (_mutationInflight) return;
      const latest = _releases[0];
      if (!latest) { showToast('No release info available.', 'info'); return; }
      const token = await getDbAdminToken();
      if (!token) { showToast('Upgrade requires authentication.', 'error'); return; }
      _mutationInflight = true;
      upgradeBtn.disabled = true;
      upgradeBtn.textContent = 'Upgrading…';
      try {
        const resp = await fetch('/api/rapid-mlx/runtime/upgrade', {
          method: 'POST',
          headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
          body: JSON.stringify({ version: latest.version, confirm: 'INSTALL_RAPID_MLX_RUNTIME' }),
        });
        if (!resp.ok) {
          const body = await resp.json().catch(() => ({}));
          throw new Error(body.error || 'Upgrade failed');
        }
        const data = await resp.json();
        showToast(`Rapid-MLX upgrade started to v${latest.version}`, 'success');
        if (data.job_id) pollJob(data.job_id);
      } catch (err) {
        showToast(`Rapid-MLX upgrade failed: ${err.message}`, 'error');
        upgradeBtn.disabled = false;
        upgradeBtn.textContent = 'Upgrade';
        _mutationInflight = false;
      }
    });
  }

  // Modal: Repair button
  const repairBtn = document.getElementById('rapid-mlx-repair-btn');
  if (repairBtn) {
    repairBtn.addEventListener('click', async () => {
      if (_mutationInflight) return;
      const token = await getDbAdminToken();
      if (!token) { showToast('Repair requires authentication.', 'error'); return; }
      _mutationInflight = true;
      repairBtn.disabled = true;
      repairBtn.textContent = 'Repairing…';
      try {
        const resp = await fetch('/api/rapid-mlx/runtime/repair', {
          method: 'POST',
          headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
          body: JSON.stringify({ confirm: 'REPAIR_RAPID_MLX_RUNTIME' }),
        });
        if (!resp.ok) {
          const body = await resp.json().catch(() => ({}));
          throw new Error(body.error || 'Repair failed');
        }
        const data = await resp.json();
        showToast('Rapid-MLX repair started', 'success');
        if (data.job_id) pollJob(data.job_id);
      } catch (err) {
        showToast(`Rapid-MLX repair failed: ${err.message}`, 'error');
        repairBtn.disabled = false;
        repairBtn.textContent = 'Repair';
        _mutationInflight = false;
      }
    });
  }

  // Modal: Rollback button
  const rollbackBtn = document.getElementById('rapid-mlx-rollback-btn');
  if (rollbackBtn) {
    rollbackBtn.addEventListener('click', async () => {
      if (_mutationInflight) return;
      const token = await getDbAdminToken();
      if (!token) { showToast('Rollback requires authentication.', 'error'); return; }
      _mutationInflight = true;
      rollbackBtn.disabled = true;
      rollbackBtn.textContent = 'Rolling back…';
      try {
        const resp = await fetch('/api/rapid-mlx/runtime/rollback', {
          method: 'POST',
          headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
          body: JSON.stringify({ confirm: 'ROLLBACK_RAPID_MLX_RUNTIME' }),
        });
        if (!resp.ok) {
          const body = await resp.json().catch(() => ({}));
          throw new Error(body.error || 'Rollback failed');
        }
        const data = await resp.json();
        showToast('Rapid-MLX rollback started', 'success');
        if (data.job_id) pollJob(data.job_id);
      } catch (err) {
        showToast(`Rapid-MLX rollback failed: ${err.message}`, 'error');
        rollbackBtn.disabled = false;
        rollbackBtn.textContent = 'Rollback';
        _mutationInflight = false;
      }
    });
  }

  // Close button
  const closeBtn = document.getElementById('rapid-mlx-modal-close');
  if (closeBtn) closeBtn.addEventListener('click', closeRapidMlxModal);

  const overlay = document.getElementById('rapid-mlx-modal');
  if (overlay) {
    overlay.addEventListener('click', e => {
      if (e.target === overlay) closeRapidMlxModal();
    });
  }

  // Initial status load
  setTimeout(() => {
    if (!document.hidden) {
      fetchRuntimeStatus();
      fetchReleases();
    }
  }, 2500);

  document.addEventListener('visibilitychange', () => {
    if (!document.hidden) {
      fetchRuntimeStatus();
      fetchReleases();
    }
  });
}

export async function fetchRuntimeStatus() {
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch('/api/rapid-mlx/runtime/status', { headers });
    if (!resp.ok) return;
    const data = await resp.json();
    _runtimeStatus = data.runtime || null;

    const supported = _runtimeStatus?.supported ?? false;
    const active = _runtimeStatus?.active ?? null;
    const mutating = _runtimeStatus?.mutation_in_progress ?? false;

    // Update nav pill (if present)
    const pill = document.getElementById('rapid-mlx-pill');
    const pillVer = document.getElementById('rapid-mlx-pill-version');
    if (pill && supported && active && !mutating) {
      pill.style.display = 'flex';
      if (pillVer) pillVer.textContent = `Rapid-MLX · v${active.version}`;
      pill.title = `Rapid-MLX runtime v${active.version}. Click to manage.`;
    } else if (pill) {
      pill.style.display = 'none';
    }

    // Update Settings card summary
    updateSettingsSummary();

    // If there's an active job, start polling
    if (mutating && _runtimeStatus?.jobs?.length > 0) {
      const jobId = _runtimeStatus.jobs[_runtimeStatus.jobs.length - 1]?.id;
      if (jobId) pollJob(jobId);
    }
  } catch {
    // silent
  }
}

export async function fetchReleases() {
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch('/api/rapid-mlx/runtime/releases', { headers });
    if (!resp.ok) return;
    const data = await resp.json();
    _releases = data.releases || [];
  } catch {
    // silent
  }
}

function updateSettingsSummary() {
  const summaryEl = document.getElementById('rapid-mlx-runtime-summary');
  if (!summaryEl) return;

  if (!_runtimeStatus) {
    summaryEl.textContent = 'Status unavailable.';
    return;
  }

  const supported = _runtimeStatus.supported ?? false;
  const active = _runtimeStatus.active ?? null;
  const mutating = _runtimeStatus.mutation_in_progress ?? false;
  const rollbackAvail = _runtimeStatus.rollback_available ?? false;

  if (!supported) {
    summaryEl.textContent = 'Rapid-MLX is only available on Apple Silicon macOS.';
    return;
  }

  if (mutating) {
    summaryEl.textContent = 'Runtime operation in progress…';
    return;
  }

  if (active) {
    const note = rollbackAvail ? ' · Rollback available' : '';
    summaryEl.textContent = `v${active.version} active${note}.`;
  } else {
    summaryEl.textContent = 'No Rapid-MLX runtime installed.';
  }
}

// ── Modal ────────────────────────────────────────────────────────────────────

let _previousFocus = null;

async function openRapidMlxModal() {
  if (_mutationInflight) return;

  await Promise.all([fetchRuntimeStatus(), fetchReleases()]);

  const modal = document.getElementById('rapid-mlx-modal');
  if (!modal) return;
  _previousFocus = document.activeElement;
  modal.classList.add('open');

  // Move focus to close button for accessibility.
  const closeBtn = document.getElementById('rapid-mlx-modal-close');
  if (closeBtn) closeBtn.focus();

  // Attach Escape key and tab-scope handlers.
  document.addEventListener('keydown', handleRapidMlxModalKeydown, true);

  const supported = _runtimeStatus?.supported ?? false;
  const active = _runtimeStatus?.active ?? null;
  const mutating = _runtimeStatus?.mutation_in_progress ?? false;
  const rollbackAvail = _runtimeStatus?.rollback_available ?? false;

  // Header: version info
  const statusEl = document.getElementById('rapid-mlx-status-text');
  if (statusEl) {
    if (!supported) {
      statusEl.textContent = 'Rapid-MLX is only available on Apple Silicon macOS.';
    } else if (mutating) {
      statusEl.textContent = 'Runtime operation in progress…';
    } else if (active) {
      statusEl.textContent = `Rapid-MLX v${active.version} is active.`;
    } else {
      statusEl.textContent = 'No Rapid-MLX runtime installed.';
    }
  }

  // Action buttons
  const upgradeBtn = document.getElementById('rapid-mlx-upgrade-btn');
  const repairBtn = document.getElementById('rapid-mlx-repair-btn');
  const rollbackBtn = document.getElementById('rapid-mlx-rollback-btn');

  if (upgradeBtn) {
    upgradeBtn.style.display = (supported && active && !mutating) ? '' : 'none';
  }
  if (repairBtn) {
    repairBtn.style.display = (supported && active && !mutating) ? '' : 'none';
  }
  if (rollbackBtn) {
    rollbackBtn.style.display = (supported && rollbackAvail && !mutating) ? '' : 'none';
  }

  // Render releases list
  const listEl = document.getElementById('rapid-mlx-releases-list');
  if (listEl && supported && !mutating) {
    listEl.innerHTML = '';
    const latestRelease = _releases[0] ?? null;

    _releases.slice(0, 10).forEach(release => {
      const isCurrent = active?.version === release.version;
      const isLatest = latestRelease?.version === release.version;
      const row = buildReleaseRow(release, isCurrent, isLatest);
      listEl.appendChild(row);
    });
  }
}

function closeRapidMlxModal() {
  const modal = document.getElementById('rapid-mlx-modal');
  if (modal) modal.classList.remove('open');
  document.removeEventListener('keydown', handleRapidMlxModalKeydown, true);
  // Restore focus to element that had focus before modal opened.
  if (_previousFocus && typeof _previousFocus.focus === 'function') {
    _previousFocus.focus();
    _previousFocus = null;
  }
}

function handleRapidMlxModalKeydown(e) {
  const modal = document.getElementById('rapid-mlx-modal');
  if (!modal || !modal.classList.contains('open')) return;

  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    closeRapidMlxModal();
    return;
  }

  if (e.key === 'Tab') {
    const focusable = [...modal.querySelectorAll(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    )].filter(el => {
      const rect = el.getBoundingClientRect();
      const style = getComputedStyle(el);
      return rect.width > 0 || rect.height > 0 || style.display !== 'none';
    });
    if (!focusable.length) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;

    if (e.shiftKey && active === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  }
}

function buildReleaseRow(release, isCurrent, isLatest) {
  const row = document.createElement('div');
  row.className = 'rapid-mlx-release-row';
  row.title = 'Click to view release details';
  row.style.cursor = 'pointer';
  row.addEventListener('click', e => {
    if (e.target.closest('.rapid-mlx-install-btn')) return;
    showReleaseNotes(release);
  });

  const info = document.createElement('div');
  info.className = 'rapid-mlx-release-row-info';

  const ver = document.createElement('span');
  ver.className = 'rapid-mlx-release-row-ver';
  ver.textContent = `v${release.version}`;

  const badges = document.createElement('span');
  badges.className = 'rapid-mlx-release-row-badges';
  if (isLatest) {
    const b = document.createElement('span');
    b.className = 'rapid-mlx-badge rapid-mlx-badge--latest';
    b.textContent = 'latest';
    badges.appendChild(b);
  }
  if (isCurrent) {
    const b = document.createElement('span');
    b.className = 'rapid-mlx-badge rapid-mlx-badge--installed';
    b.textContent = 'installed';
    badges.appendChild(b);
  }

  const meta = document.createElement('span');
  meta.className = 'rapid-mlx-release-row-meta';
  if (release.channel && release.channel !== 'stable') {
    meta.textContent = release.channel;
  } else {
    const ago = timeAgo(release.published_at);
    meta.textContent = ago || '';
  }

  info.append(ver, badges);

  const right = document.createElement('div');
  right.className = 'rapid-mlx-release-row-right';
  right.appendChild(meta);

  if (!isCurrent) {
    const btn = document.createElement('button');
    btn.className = 'rapid-mlx-install-btn';
    btn.textContent = 'Install';
    btn.dataset.version = release.version;
    btn.addEventListener('click', () => installVersion(btn, release));
    right.appendChild(btn);
  }

  row.append(info, right);
  return row;
}

// ── Actions (use db-admin-token for mutations) ──────────────────────────────

async function getDbAdminToken() {
  const tokenResp = await fetch('/api/db/admin-token', {
    headers: window.authHeaders ? window.authHeaders() : {},
  });
  if (!tokenResp.ok) return null;
  const data = await tokenResp.json().catch(() => ({}));
  return data.token || null;
}

async function installVersion(btn, release) {
  if (_mutationInflight) return;
  _mutationInflight = true;

  btn.disabled = true;
  btn.textContent = 'Installing…';

  const token = await getDbAdminToken();
  if (!token) {
    showToast('Rapid-MLX install requires authentication.', 'error');
    btn.disabled = false;
    btn.textContent = 'Install';
    _mutationInflight = false;
    return;
  }

  const actionName = release.version
    ? ('upgrade' in _runtimeStatus?.active ? 'upgrade' : 'install')
    : 'install';

  try {
    const resp = await fetch(
      `/api/rapid-mlx/runtime/${actionName}`,
      {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${token}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          version: release.version,
          channel: release.channel || 'stable',
          confirm: 'INSTALL_RAPID_MLX_RUNTIME',
        }),
      }
    );

    if (!resp.ok) {
      const body = await resp.json().catch(() => ({}));
      throw new Error(body.error || 'Install failed');
    }

    const data = await resp.json();
    const jobId = data.job_id;
    showToast(`Rapid-MLX install started for v${release.version}`, 'success');
    if (jobId) pollJob(jobId);
  } catch (err) {
    showToast(`Rapid-MLX install failed: ${err.message}`, 'error');
    btn.disabled = false;
    btn.textContent = 'Install';
    _mutationInflight = false;
  }
}

async function pollJob(jobId) {
  if (!jobId) return;
  const interval = setInterval(async () => {
    try {
      const headers = window.authHeaders ? window.authHeaders() : {};
      const resp = await fetch(`/api/rapid-mlx/runtime/jobs/${jobId}`, { headers });
      if (!resp.ok) return;
      const data = await resp.json();
      const job = data.job || null;
      if (!job) { clearInterval(interval); return; }

      if (job.state === 'completed') {
        clearInterval(interval);
        _mutationInflight = false;
        showToast('Rapid-MLX runtime operation completed.', 'success');
        fetchRuntimeStatus();
      } else if (job.state === 'failed' || job.state === 'cancelled') {
        clearInterval(interval);
        _mutationInflight = false;
        showToast(`Rapid-MLX operation ${job.state}: ${job.message || 'See logs'}`, 'error');
        fetchRuntimeStatus();
      }
    } catch {
      // keep polling
    }
  }, 2000);
}

function showReleaseNotes(release) {
  const detailsEl = document.getElementById('rapid-mlx-release-notes');
  if (!detailsEl) return;

  if (release.release_notes) {
    /* eslint-disable-next-line no-unsanitized/property */
    detailsEl.innerHTML = DOMPurify ? DOMPurify.sanitize(release.release_notes) : release.release_notes;
    detailsEl.style.display = '';
  } else {
    detailsEl.textContent = 'No release notes available.';
    detailsEl.style.display = '';
  }
}

function timeAgo(iso) {
  if (!iso) return '';
  try {
    const diff = Date.now() - new Date(iso).getTime();
    const days = Math.max(0, Math.round(diff / 86_400_000));
    if (days === 0) return 'today';
    if (days < 7) return `${days}d ago`;
    if (days < 30) return `${Math.floor(days / 7)}w ago`;
    return `${Math.floor(days / 30)}mo ago`;
  } catch {
    return '';
  }
}
