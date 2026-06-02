// ── HF Browse Module ──────────────────────────────────────────────────────────
// Shared HuggingFace browse, search, file-list, and download utilities.
// Consumed by spawn-wizard.js and (future) models-modal.

import { showToast } from './toast.js';

// ── Discover categories ───────────────────────────────────────────────────────

export const HF_DISCOVER_CATEGORIES = [
  { id: 'trending',  label: 'Trending',      params: { query: '',           sort: 'trending',  limit: 30 } },
  { id: 'qwen3',     label: 'Qwen3',         params: { query: 'qwen3',      sort: 'downloads', limit: 30 } },
  { id: 'llama3',    label: 'Llama 3.x',     params: { query: 'llama-3',    sort: 'downloads', limit: 30 } },
  { id: 'mistral',   label: 'Mistral / MoE', params: { query: 'mistral',    sort: 'downloads', limit: 30 } },
  { id: 'gemma',     label: 'Gemma',         params: { query: 'gemma',      sort: 'downloads', limit: 30 } },
  { id: 'exaone',    label: 'EXAONE',        params: { query: 'exaone',     sort: 'downloads', limit: 30 } },
  { id: 'heretic',   label: 'Heretic',       params: { query: 'heretic',    sort: 'downloads', limit: 30 } },
];

// ── Small utilities ───────────────────────────────────────────────────────────

function escHtml(s) {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function hfRelativeAge(iso) {
  if (!iso) return '';
  const ms = Date.now() - new Date(iso).getTime();
  if (isNaN(ms) || ms < 0) return '';
  const mins  = Math.floor(ms / 60_000);
  const hours = Math.floor(ms / 3_600_000);
  const days  = Math.floor(ms / 86_400_000);
  const weeks = Math.floor(days / 7);
  const months = Math.floor(days / 30);
  const years  = Math.floor(days / 365);
  if (mins  <  60)  return `${mins}m ago`;
  if (hours < 24)   return `${hours}h ago`;
  if (days  <  7)   return `${days}d ago`;
  if (weeks <  5)   return `${weeks}w ago`;
  if (months < 12)  return `${months}mo ago`;
  return `${years}y ago`;
}

function formatBytes(bytes) {
  if (!bytes) return '';
  const b = Number(bytes);
  if (!isFinite(b)) return '';
  if (b >= 1e9) return (b / 1e9).toFixed(1) + ' GB';
  if (b >= 1e6) return (b / 1e6).toFixed(1) + ' MB';
  if (b >= 1e3) return (b / 1e3).toFixed(0) + ' KB';
  return b + ' B';
}

function getRecommendedQuant(vramGb) {
  if (vramGb < 8)  return 'Q4_K_M';
  if (vramGb <= 16) return 'Q5_K_M';
  if (vramGb <= 24) return 'Q5_K_M';
  return 'Q8_0';
}

function getAuthHeaders() {
  return window.authHeaders ? window.authHeaders() : {};
}

// ── hfSearch ──────────────────────────────────────────────────────────────────
// Search HuggingFace models and render results into container.
//
// params:
//   query, author, sort, limit          – search params
//   container                           – DOM element to render into
//   filelistContainer                   – optional element to hide when showing results
//   quickpicksContainer                 – optional element holding quick-pick buttons (for loading/active state)
//   discoverPillsContainerId            – optional id of discover-pills container (for loading/active state)
//   onOpenCardPanel                     – (repoId) => void
//   onSelectModel                       – (model) => void  (called when user clicks a result row)

export async function hfSearch({
  query,
  author,
  sort,
  limit,
  container,
  filelistContainer,
  quickpicksContainer,
  discoverPillsContainerId,
  onOpenCardPanel,
  onSelectModel,
}) {
  if (!container) return;

  container.innerHTML = '<div class="hf-search-loading">Searching HuggingFace…</div>';
  container.style.display = '';

  if (filelistContainer) {
    filelistContainer.innerHTML = '';
    filelistContainer.classList.remove('visible');
  }

  const clearPillLoading = () => {
    quickpicksContainer?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('loading'));
    document.getElementById(discoverPillsContainerId)
      ?.querySelectorAll('.hf-discover-pill').forEach(p => p.classList.remove('loading'));
  };

  const scrollToResults = () => {
    const scrollParent = container.closest('.wizard-body');
    if (!scrollParent) return;
    const cRect = container.getBoundingClientRect();
    const pRect = scrollParent.getBoundingClientRect();
    scrollParent.scrollTo({ top: scrollParent.scrollTop + cRect.top - pRect.top - 12, behavior: 'smooth' });
  };

  try {
    const body = {
      query: query || '',
      author: author || undefined,
      sort: sort || 'downloads',
      limit: limit || 20,
    };

    const headers = { ...getAuthHeaders(), 'Content-Type': 'application/json' };
    const resp = await fetch('/api/hf/search', { method: 'POST', headers, body: JSON.stringify(body) });
    if (!resp.ok) {
      clearPillLoading();
      container.innerHTML = '<div class="hf-search-empty">Search failed.</div>';
      return;
    }
    const data = await resp.json();
    const models = data.models || [];

    clearPillLoading();
    container.innerHTML = '';
    if (!models.length) {
      container.innerHTML = '<div class="hf-search-empty">No models found.</div>';
      return;
    }

    for (const m of models) {
      const row = document.createElement('div');
      row.className = 'hf-search-result';
      row.setAttribute('tabindex', '0');
      row.setAttribute('role', 'button');

      const nameEl = document.createElement('span');
      nameEl.className = 'hf-sr-name';
      nameEl.textContent = m.id || '';

      const meta = document.createElement('span');
      meta.className = 'hf-sr-meta';

      if (m.downloads > 0) {
        const dl = document.createElement('span');
        dl.textContent = m.downloads >= 1000 ? `${(m.downloads / 1000).toFixed(0)}k\u2193` : `${m.downloads}\u2193`;
        meta.appendChild(dl);
      }

      const ageStr = hfRelativeAge(m.last_modified || m.created_at || '');
      if (ageStr) {
        const age = document.createElement('span');
        age.className = 'hf-sr-age';
        age.textContent = ageStr;
        age.title = m.last_modified || m.created_at || '';
        meta.appendChild(age);
      }

      if (m.has_imatrix) {
        const b = document.createElement('span');
        b.className = 'hf-sr-badge hf-sr-badge-imatrix';
        b.textContent = 'imatrix';
        meta.appendChild(b);
      } else if ((m.quant_provider || '').toLowerCase() === 'unsloth') {
        const b = document.createElement('span');
        b.className = 'hf-sr-badge hf-sr-badge-ud';
        b.textContent = 'UD';
        meta.appendChild(b);
      }
      if (m.gated) {
        const b = document.createElement('span');
        b.className = 'hf-sr-badge hf-sr-badge-gated';
        b.textContent = 'gated';
        meta.appendChild(b);
      }
      const lowerTags = (m.tags || []).map(t => t.toLowerCase());
      if (lowerTags.some(t => t.includes('moe'))) {
        const b = document.createElement('span');
        b.className = 'hf-sr-badge hf-sr-badge-moe';
        b.textContent = 'MoE';
        meta.appendChild(b);
      }

      const cardLink = document.createElement('button');
      cardLink.type = 'button';
      cardLink.className = 'hf-sr-card-link';
      cardLink.title = 'View model card';
      cardLink.setAttribute('aria-label', `View model card for ${escHtml(m.id)}`);
      cardLink.innerHTML =
        '<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>';
      cardLink.addEventListener('click', e => {
        e.stopPropagation();
        if (onOpenCardPanel) onOpenCardPanel(m.id);
      });

      row.appendChild(nameEl);
      row.appendChild(meta);
      row.appendChild(cardLink);

      const selectRepo = () => {
        if (onSelectModel) onSelectModel(m);
      };
      row.addEventListener('click', selectRepo);
      row.addEventListener('keydown', e => {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectRepo(); }
      });

      container.appendChild(row);
    }

    scrollToResults();
  } catch (err) {
    clearPillLoading();
    const errEl = document.createElement('div');
    errEl.className = 'hf-search-empty';
    errEl.textContent = 'Error: ' + (err.message || String(err));
    container.appendChild(errEl);
  }
}

// ── hfListFiles ───────────────────────────────────────────────────────────────
// List GGUF files for a repo and render into container.
//
// params:
//   repoId                              – HF repo ID
//   container                           – DOM element to render into
//   vramGb                              – available VRAM in GiB (for recommendation badge)
//   onOpenCardPanel                     – (repoId) => void
//   onSelectFile                        – (file, repoId) => void

export async function hfListFiles({
  repoId,
  container,
  vramGb,
  onOpenCardPanel,
  onSelectFile,
}) {
  if (!container) return;

  container.innerHTML = '<div class="hf-file-loading">Loading GGUF files…</div>';
  container.classList.add('visible');

  try {
    const headers = { ...getAuthHeaders(), 'Content-Type': 'application/json' };
    const resp = await fetch('/api/hf/files', {
      method: 'POST',
      headers,
      body: JSON.stringify({ repo_id: repoId }),
    });
    if (!resp.ok) {
      container.innerHTML = '<div class="hf-file-empty">Failed to load files. Check the repo ID.</div>';
      return;
    }
    const data = await resp.json();
    const files = (data.files || []).filter(Boolean);

    container.innerHTML = '';
    if (!files.length) {
      container.innerHTML = '<div class="hf-file-empty">No GGUF files found in this repo.</div>';
      return;
    }

    let autoSelectFn = null;
    let firstSelectFn = null;

    for (const file of files) {
      const fname = file.path || file.name || '';
      if (!fname) continue;

      const item = document.createElement('div');
      item.className = 'hf-file-item';
      item.setAttribute('tabindex', '0');
      item.setAttribute('role', 'button');
      item.dataset.filename = fname;
      item.dataset.size = file.size || '';

      const nameSpan = document.createElement('span');
      nameSpan.className = 'hf-file-name';
      nameSpan.textContent = fname.split('/').pop() || fname;

      const metaSpan = document.createElement('span');
      metaSpan.className = 'hf-file-size';
      const parts = [];
      if (file.size) parts.push(formatBytes(file.size));
      if (file.label) {
        parts.push(file.label);
        if (vramGb > 0 && file.label === getRecommendedQuant(vramGb)) parts.push('\u2713 Recommended');
      }
      metaSpan.textContent = parts.join(' \u00b7 ');

      const qt = file.quant_type || '';
      if (qt === 'imatrix' || file.is_imatrix) {
        const b = document.createElement('span');
        b.className = 'hf-file-badge hf-file-badge-imatrix';
        b.textContent = 'imatrix';
        b.title = 'Importance-matrix calibrated — better quality at same bpw (mradermacher style)';
        nameSpan.appendChild(b);
      } else if (qt === 'unsloth_dynamic') {
        const b = document.createElement('span');
        b.className = 'hf-file-badge hf-file-badge-ud';
        b.textContent = 'UD';
        b.title = 'Unsloth Dynamic — mixed bits per layer, excellent quality/size tradeoff';
        nameSpan.appendChild(b);
      }
      if (file.is_mmproj) {
        const b = document.createElement('span');
        b.className = 'hf-file-badge hf-file-badge-mmproj';
        b.textContent = 'mmproj';
        b.title = 'Vision projector — load alongside the main model for multimodal inference';
        nameSpan.appendChild(b);
      }

      item.appendChild(nameSpan);
      item.appendChild(metaSpan);

      const selectFile = () => {
        if (onSelectFile) onSelectFile(file, repoId);
      };
      item.addEventListener('click', selectFile);
      item.addEventListener('keydown', e => {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectFile(); }
      });

      if (!file.is_mmproj) {
        if (!firstSelectFn) firstSelectFn = selectFile;
        if (!autoSelectFn && file.label && vramGb > 0 && file.label === getRecommendedQuant(vramGb)) {
          autoSelectFn = selectFile;
        }
      }

      container.appendChild(item);
    }

    const autoFn = autoSelectFn || firstSelectFn;
    if (autoFn) autoFn();
  } catch {
    container.innerHTML = '<div class="hf-file-empty">Error loading files. Check the repo ID and your HF token.</div>';
  }
}

// ── hfStartDownload ───────────────────────────────────────────────────────────
// Start a download and show progress in panelEl.
//
// params:
//   repoId, filePath                    – HF repo and file to download
//   panelEl                             – DOM element for the download panel
//   onComplete                          – (downloadId, localPath) => void
//   onValidationError                   – (msg) => void
//   onClearValidationError              – () => void (optional)

export async function hfStartDownload({
  repoId,
  filePath,
  panelEl,
  onComplete,
  onValidationError,
  onClearValidationError,
}) {
  if (!repoId || !filePath) {
    if (onValidationError) onValidationError('Select a GGUF file first.');
    return;
  }
  if (!panelEl) return;

  const btn = panelEl.querySelector('#hf-dlp-download-btn');
  if (btn) { btn.disabled = true; btn.textContent = 'Starting\u2026'; }

  try {
    const headers = { ...getAuthHeaders(), 'Content-Type': 'application/json' };
    const res = await fetch('/api/hf/download', {
      method: 'POST',
      headers,
      body: JSON.stringify({ repo_id: repoId, file_path: filePath, resume: true }),
    });
    if (btn) { btn.disabled = false; btn.textContent = 'Download to models folder'; }
    const data = await res.json().catch(() => ({}));
    if (!res.ok || !data.ok) {
      if (onValidationError) onValidationError(data.error || 'Download failed to start.');
      return;
    }

    const downloadId = data.download_id;

    const fileEl = panelEl.querySelector('#hf-dlp-progress-file');
    if (fileEl) fileEl.textContent = filePath.split('/').pop();
    _dlSetState(panelEl, 'progress');
    hfPollDownload(downloadId, panelEl, { onComplete, onValidationError, onClearValidationError });
  } catch (err) {
    if (btn) { btn.disabled = false; }
    if (onValidationError) onValidationError(err.message || 'Download request failed.');
  }
}

// ── hfPollDownload ────────────────────────────────────────────────────────────
// Poll download status and update progress in panelEl.
// Caller owns state updates via onComplete/onValidationError.

export function hfPollDownload(downloadId, panelEl, { onComplete, onValidationError, onClearValidationError }) {
  if (!panelEl) return;
  _dlCancelPoll(panelEl);

  const headers = getAuthHeaders();

  async function poll() {
    try {
      const res = await fetch(`/api/models/download/${downloadId}/status`, { headers });
      if (!res.ok) {
        _dlSchedulePoll(panelEl, poll, 1000);
        return;
      }
      const data = await res.json();
      const s = data.status;
      if (!s) {
        _dlSchedulePoll(panelEl, poll, 1000);
        return;
      }

      const { status, bytes_downloaded = 0, total_bytes = 0, speed = 0, eta = 0 } = s;
      const pct = total_bytes > 0 ? Math.round(bytes_downloaded / total_bytes * 100) : 0;

      const bar = panelEl.querySelector('#hf-dlp-bar');
      if (bar) bar.style.width = pct + '%';

      const pctEl = panelEl.querySelector('#hf-dlp-progress-pct');
      if (pctEl) pctEl.textContent = total_bytes > 0 ? `${pct}%` : '';

      const statsEl = panelEl.querySelector('#hf-dlp-stats');
      if (statsEl) {
        const mb = (bytes_downloaded / 1_048_576).toFixed(1);
        const tot = total_bytes > 0 ? ` / ${(total_bytes / 1_048_576).toFixed(0)} MB` : '';
        const spd = speed > 0 ? ` \u00b7 ${(speed / 1_048_576).toFixed(1)} MB/s` : '';
        const etaStr = eta > 0
          ? ` \u00b7 ETA ${eta < 60 ? eta + 's' : Math.round(eta / 60) + 'm'}`
          : '';
        statsEl.textContent = `${mb} MB${tot}${spd}${etaStr}`;
      }

      if (status === 'completed') {
        _dlCancelPoll(panelEl);
        _dlSetState(panelEl, 'complete');
        if (onClearValidationError) onClearValidationError();
        const localPath = data.status?.local_path || data.local_path;
        if (onComplete) onComplete(downloadId, localPath);
        return;
      }
      if (status === 'failed') {
        _dlCancelPoll(panelEl);
        _dlSetState(panelEl, 'idle');
        if (onValidationError) onValidationError(s.message || 'Download failed.');
        return;
      }
      if (status === 'cancelled') {
        _dlCancelPoll(panelEl);
        _dlSetState(panelEl, 'idle');
        return;
      }
    } catch { /* network glitch — keep polling */ }
    _dlSchedulePoll(panelEl, poll, 1000);
  }

  _dlSchedulePoll(panelEl, poll, 800);
}

// ── hfCancelDownload ──────────────────────────────────────────────────────────
// Cancel an active download.

export async function hfCancelDownload({ downloadId, panelEl }) {
  if (!downloadId || !panelEl) return;
  const headers = getAuthHeaders();
  await fetch(`/api/models/download/${downloadId}/cancel`, { method: 'POST', headers }).catch(() => {});
  _dlCancelPoll(panelEl);
  _dlSetState(panelEl, 'idle');
}

// ── hfShowDownloadPanel ───────────────────────────────────────────────────────
// Show the download panel and set the idle state + destination path.

export async function hfShowDownloadPanel(panelEl, fname) {
  if (!panelEl) return;
  _dlSetState(panelEl, 'idle');
  panelEl.style.display = '';

  try {
    const headers = getAuthHeaders();
    const res = await fetch('/api/hf/download-dir', { headers });
    const data = res.ok ? await res.json() : null;
    const dir = data?.dir || '~/.config/llama-monitor/models';
    const configured = data?.configured ?? false;
    const destPath = dir.replace(/\/$/, '') + '/' + (fname || '').split('/').pop();

    const destEl = panelEl.querySelector('#hf-dlp-dest-path');
    if (destEl) { destEl.textContent = destPath; destEl.title = destPath; }

    const warnEl = panelEl.querySelector('#hf-dlp-no-dir-warn');
    if (warnEl) warnEl.style.display = configured ? 'none' : '';
  } catch { /* ignore */ }
}

export function hfHideDownloadPanel(panelEl) {
  if (!panelEl) return;
  panelEl.style.display = 'none';
  _dlCancelPoll(panelEl);
}

// ── hfRenderDiscoverPills ─────────────────────────────────────────────────────
// Render discover pills into container.
//
// params:
//   container                           – DOM element to render into
//   quickpicksContainer                 – optional element holding quick-pick buttons
//   onPillClick                         – (cat, pillEl) => void  (called when a pill is clicked)

export function hfRenderDiscoverPills({ container, quickpicksContainer, onPillClick }) {
  if (!container) return;
  container.innerHTML = '';

  for (const cat of HF_DISCOVER_CATEGORIES) {
    const pill = document.createElement('button');
    pill.type = 'button';
    pill.className = 'hf-discover-pill';
    pill.textContent = cat.label;
    pill.dataset.catId = cat.id;

    pill.addEventListener('click', () => {
      container.querySelectorAll('.hf-discover-pill').forEach(p => p.classList.remove('active', 'loading'));
      quickpicksContainer?.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active', 'loading'));
      pill.classList.add('active', 'loading');
      if (onPillClick) onPillClick(cat, pill);
    });

    container.appendChild(pill);
  }
}

// ── hfLoadQuickPicks ──────────────────────────────────────────────────────────
// Load quantizer quick-picks and render into container.
//
// params:
//   container                           – DOM element to render into
//   discoverPillsContainerId            – optional id of discover-pills container
//   onAuthorClick                       – (author, btnEl) => void  (called when a quick-pick is clicked)

export async function hfLoadQuickPicks({ container, discoverPillsContainerId, onAuthorClick }) {
  if (!container) return;
  try {
    const headers = getAuthHeaders();
    const resp = await fetch('/api/hf/quantizers', { headers });
    if (!resp.ok) return;
    const data = await resp.json();
    if (!data.ok || !data.quantizers) return;

    container.innerHTML = '';
    for (const q of data.quantizers) {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'hf-qp-btn';
      if (q.quant_style === 'imatrix') btn.classList.add('hf-qp-imatrix');
      if (q.quant_style === 'ud') btn.classList.add('hf-qp-ud');
      btn.textContent = q.display_name;
      btn.title = q.description + (q.note ? `\n\n${q.note}` : '');
      btn.dataset.author = q.username;

      btn.addEventListener('click', () => {
        container.querySelectorAll('.hf-qp-btn').forEach(b => b.classList.remove('active', 'loading'));
        document.getElementById(discoverPillsContainerId)
          ?.querySelectorAll('.hf-discover-pill').forEach(p => p.classList.remove('active', 'loading'));
        btn.classList.add('active', 'loading');
        if (onAuthorClick) onAuthorClick(q.username, btn);
      });

      container.appendChild(btn);
    }
  } catch { /* non-fatal */ }
}

// ── Internal helpers (download panel state) ───────────────────────────────────

function _dlSetState(panelEl, state) {
  ['idle', 'progress', 'complete'].forEach(s => {
    const el = panelEl.querySelector(`#hf-dlp-${s}`);
    if (el) el.style.display = s === state ? '' : 'none';
  });
}

function _dlSchedulePoll(panelEl, fn, ms) {
  const existing = panelEl._hfDlPollTimer;
  if (existing) clearTimeout(existing);
  panelEl._hfDlPollTimer = setTimeout(fn, ms);
}

function _dlCancelPoll(panelEl) {
  if (panelEl._hfDlPollTimer) {
    clearTimeout(panelEl._hfDlPollTimer);
    panelEl._hfDlPollTimer = null;
  }
}
