/* global DOMPurify */

// ── Rapid-MLX Runtime Manager ─────────────────────────────────────────────────
// Manages Rapid-MLX runtime lifecycle in Settings and nav pill.
// Uses existing API endpoints under /api/rapid-mlx/runtime/.

import { showToast } from './toast.js';
import { attachModalFocusTrap, detachModalFocusTrap, fetchReleaseList, buildReleaseBadges } from './updater-shared.js';

let _runtimeStatus = null;
let _releases = [];
let _mutationInflight = false;
let _changelogCache = Object.create(null);

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
          body: JSON.stringify({ version: latest.version, confirm: 'UPGRADE_RAPID_MLX_RUNTIME' }),
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

   // Changelog expand/collapse toggle
   const changelogToggle = document.getElementById('rapid-mlx-changelog-toggle');
   if (changelogToggle) {
     changelogToggle.addEventListener('click', toggleChangelog);
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
    const data = await fetchReleaseList('/api/rapid-mlx/runtime/releases');
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

async function openRapidMlxModal() {
  if (_mutationInflight) return;

  await Promise.all([fetchRuntimeStatus(), fetchReleases()]);

  const modal = document.getElementById('rapid-mlx-modal');
  if (!modal) return;
  modal.classList.add('open');

  // Move focus to close button for accessibility.
  const closeBtn = document.getElementById('rapid-mlx-modal-close');
  if (closeBtn) closeBtn.focus();

  // Attach Escape key and tab-scope handlers.
  attachModalFocusTrap(modal, closeRapidMlxModal);

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
  detachModalFocusTrap(modal);
  resetChangelogSection();
}

function resetChangelogSection() {
  const changelogEl = document.getElementById('rapid-mlx-changelog');
  const bodyEl = document.getElementById('rapid-mlx-changelog-body');
  const toggleBtn = document.getElementById('rapid-mlx-changelog-toggle');
  if (changelogEl) changelogEl.style.display = 'none';
  if (bodyEl) {
    bodyEl.style.display = 'none';
    bodyEl.innerHTML = '';
  }
  if (toggleBtn) toggleBtn.setAttribute('aria-expanded', 'false');
}

function selectReleaseRow(row) {
  document.querySelectorAll('.rapid-mlx-release-row--selected').forEach(el => el.classList.remove('rapid-mlx-release-row--selected'));
  row.classList.add('rapid-mlx-release-row--selected');
}

function buildReleaseRow(release, isCurrent, isLatest) {
  const row = document.createElement('div');
  row.className = 'rapid-mlx-release-row';
  row.dataset.version = release.version;
  row.title = 'Click to view release details';
  row.style.cursor = 'pointer';
  row.addEventListener('click', e => {
    if (e.target.closest('.rapid-mlx-install-btn')) return;
    selectReleaseRow(row);
    showReleaseNotes(release);
  });

  const info = document.createElement('div');
  info.className = 'rapid-mlx-release-row-info';

  const ver = document.createElement('span');
  ver.className = 'rapid-mlx-release-row-ver';
  ver.textContent = `v${release.version}`;

  const badges = buildReleaseBadges({
    wrapperClass: 'rapid-mlx-release-row-badges',
    badgeClass: 'rapid-mlx-badge',
    isLatest,
    isCurrent,
  });

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
          confirm: `${actionName.toUpperCase()}_RAPID_MLX_RUNTIME`,
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

const POLL_JOB_INTERVAL_MS = 2000;
const POLL_JOB_MAX_CONSECUTIVE_ERRORS = 5;
const POLL_JOB_MAX_DURATION_MS = 10 * 60 * 1000; // 10 minutes

async function pollJob(jobId) {
  if (!jobId) return;
  const startedAt = Date.now();
  let consecutiveErrors = 0;

  const giveUp = (interval, message) => {
    clearInterval(interval);
    _mutationInflight = false;
    showToast('Rapid-MLX operation status unknown', 'error', message);
    fetchRuntimeStatus();
  };

  const interval = setInterval(async () => {
    if (Date.now() - startedAt > POLL_JOB_MAX_DURATION_MS) {
      giveUp(interval, 'Timed out waiting for the operation to finish. Check server logs.');
      return;
    }

    try {
      const headers = window.authHeaders ? window.authHeaders() : {};
      const resp = await fetch(`/api/rapid-mlx/runtime/jobs/${jobId}`, { headers });
      if (!resp.ok) {
        consecutiveErrors += 1;
        if (consecutiveErrors >= POLL_JOB_MAX_CONSECUTIVE_ERRORS) {
          giveUp(interval, `Repeated failures checking job status (HTTP ${resp.status}).`);
        }
        return;
      }
      consecutiveErrors = 0;
      const data = await resp.json();
      const job = data.job || null;
      if (!job) {
        clearInterval(interval);
        _mutationInflight = false;
        return;
      }

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
    } catch (err) {
      consecutiveErrors += 1;
      if (consecutiveErrors >= POLL_JOB_MAX_CONSECUTIVE_ERRORS) {
        giveUp(interval, `Repeated failures checking job status (${err.message || err}).`);
      }
    }
  }, POLL_JOB_INTERVAL_MS);
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

  showChangelogSection(release);
}

function showChangelogSection(release) {
  const changelogEl = document.getElementById('rapid-mlx-changelog');
  if (!changelogEl) return;

  const active = _runtimeStatus?.active;
  const fromVersion = active?.version;

  if (!fromVersion || fromVersion === release.version) {
    changelogEl.style.display = 'none';
    return;
  }

  changelogEl.style.display = '';
  resetChangelogBody(fromVersion, release.version);
}

function resetChangelogBody(fromVersion, toVersion) {
  const bodyEl = document.getElementById('rapid-mlx-changelog-body');
  const toggleBtn = document.getElementById('rapid-mlx-changelog-toggle');
  if (!bodyEl || !toggleBtn) return;

   const cacheKey = `${fromVersion}...${toVersion}`;
  const cached = _changelogCache[cacheKey];

  toggleBtn.setAttribute('aria-expanded', 'false');
  bodyEl.style.display = 'none';

  if (cached) {
    /* eslint-disable-next-line no-unsanitized/property */
    bodyEl.innerHTML = cached;
  } else {
    bodyEl.textContent = 'Loading changelog…';
    bodyEl.className = 'rapid-mlx-changelog-loading';
  }
}

function toggleChangelog() {
  const bodyEl = document.getElementById('rapid-mlx-changelog-body');
  const toggleBtn = document.getElementById('rapid-mlx-changelog-toggle');
  if (!bodyEl || !toggleBtn) return;

  const isExpanded = toggleBtn.getAttribute('aria-expanded') === 'true';

  if (isExpanded) {
    toggleBtn.setAttribute('aria-expanded', 'false');
    bodyEl.style.display = 'none';
    return;
  }

  toggleBtn.setAttribute('aria-expanded', 'true');
  bodyEl.style.display = '';

  const cachedContent = bodyEl.innerHTML.trim();
  if (!cachedContent.includes('Loading changelog') && !cachedContent.includes('changelog-unavailable')) {
    return;
  }

  fetchChangelog();
}

async function fetchChangelog() {
  const bodyEl = document.getElementById('rapid-mlx-changelog-body');
  if (!bodyEl) return;

  const active = _runtimeStatus?.active;
  const fromVersion = active?.version;
  const selectedRelease = findSelectedRelease();
  if (!fromVersion || !selectedRelease) {
    bodyEl.innerHTML = '<div class="rapid-mlx-changelog-error">Changelog unavailable.</div>';
    return;
  }

  const toVersion = selectedRelease.version;
  const cacheKey = `${fromVersion}...${toVersion}`;

  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch(
      `/api/rapid-mlx/runtime/changelog?from=${encodeURIComponent(fromVersion)}&to=${encodeURIComponent(toVersion)}`,
      { headers }
    );

    if (!resp.ok) {
      handleChangelogError(resp, bodyEl);
      return;
    }

    const data = await resp.json();
    if (!data.ok) {
      handleChangelogError({ status: 500, json: () => Promise.resolve(data) }, bodyEl);
      return;
    }

    const html = renderChangelog(data.changelog);
    _changelogCache[cacheKey] = html;
    /* eslint-disable-next-line no-unsanitized/property */
    bodyEl.innerHTML = html;
  } catch {
    bodyEl.innerHTML = '<div class="rapid-mlx-changelog-error">Changelog unavailable (network error).</div>';
  }
}

function handleChangelogError(resp, bodyEl) {
  if (resp.status === 429 || resp.status === 503) {
    bodyEl.innerHTML = '<div class="rapid-mlx-changelog-error">Changelog unavailable (rate-limited).</div>';
    return;
  }
  resp.json().then(data => {
    if (data.kind === 'rate_limited') {
      bodyEl.innerHTML = '<div class="rapid-mlx-changelog-error">Changelog unavailable (rate-limited).</div>';
    } else if (data.kind === 'invalid_tag') {
      bodyEl.innerHTML = '<div class="rapid-mlx-changelog-error">Changelog unavailable (invalid version).</div>';
    } else {
      bodyEl.innerHTML = '<div class="rapid-mlx-changelog-error">Changelog unavailable.</div>';
    }
  }).catch(() => {
    bodyEl.innerHTML = '<div class="rapid-mlx-changelog-error">Changelog unavailable.</div>';
  });
}

function findSelectedRelease() {
  const selectedRow = document.querySelector('.rapid-mlx-release-row--selected');
  if (!selectedRow) return null;

  const versionText = selectedRow.querySelector('.rapid-mlx-release-row-ver')?.textContent;
  if (!versionText) return null;

  const version = versionText.replace(/^v/, '');
  return _releases.find(r => r.version === version) || null;
}

function renderChangelog(changelog) {
  if (!changelog || !changelog.commits || changelog.commits.length === 0) {
    return '<div class="rapid-mlx-changelog-error">No commit details available.</div>';
  }

  let html = '';
  const commits = changelog.commits.slice(0, 50);

  html += `<div class="rapid-mlx-changelog-summary">
    ${commits.length} commit${commits.length === 1 ? '' : 's'} in this release
  </div>`;

  commits.forEach(commit => {
    const message = escapeHtml(commit.message);
    const author = escapeHtml(commit.author || 'unknown');
    html += `<div class="rapid-mlx-changelog-commit">
      <div class="rapid-mlx-changelog-commit-message">${message}</div>
      <div class="rapid-mlx-changelog-commit-meta">
        <span class="rapid-mlx-changelog-commit-sha">${commit.sha}</span>
        <span class="rapid-mlx-changelog-commit-author">${author}</span>
      </div>
    </div>`;
  });

  if (changelog.html_url) {
    html += `<div style="margin-top:6px;">
      <a class="rapid-mlx-changelog-link" href="${escapeHtml(changelog.html_url)}" target="_blank" rel="noopener noreferrer">View full diff on GitHub ↗</a>
    </div>`;
  }

  return html;
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
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
