// Shared chat-template lifecycle UI: status line copy, the "Manage template…"
// modal (Current / Updates / Version history / Community fixes), and the
// create-fix (jinja editor) flow. Used by both the llama.cpp Spawn Wizard
// (spawn-wizard-chat-template.js), the Rapid-MLX Spawn Wizard
// (spawn-wizard-rapid-mlx.js) and the Preset Editor (presets.js) so the
// lifecycle logic exists exactly once.
//
// Backend contract (do not change without updating src/web/api/spawn_wizard.rs):
//   GET  /api/chat-template/releases?name=<name>
//     -> { ok, releases: [{ sha256, revision, source_url, fetch_url, installed_at, file }], active_sha256 }
//   POST /api/chat-template/activate { name, sha256 } -> { ok }
//   POST /api/chat-template/check-update { path, fetch_url?, source_url? } -> { ok, changed }
//   GET  /api/chat-template/discussions?name=<name> -> { ok, discussions: [...], source_repo }
//   POST /api/chat-template/install-discussion {...} -> { ok, release_name, file_path }
//   POST /api/chat-template/smoke-test { name, model } -> { ok, summary }
//   GET  /api/chat-template/read?path=<path> -> raw text

import { showToast } from './toast.js';

export const CT_LABELS = {
  useRecommended: 'Use recommended template',
  revertToBuiltIn: 'Revert to built-in',
  manage: 'Manage template…',
  checkUpdates: 'Check for updates',
  useThisVersion: 'Use this version',
  editInstallFix: 'Edit and install this fix',
  previewAndInstall: 'Preview and install',
};

export function chatTemplateHelperText() {
  return 'Chat templates control how your messages are formatted before the model sees them. '
    + "The built-in one usually works; a fixed template can repair broken tool-calling or thinking tags.";
}

export function chatTemplateStatusText({ mode, tplDisplay, installedAt }) {
  if (mode === 'custom' || mode === 'installed' || mode === 'auto') {
    const when = installedAt ? new Date(installedAt).toLocaleString() : null;
    const name = tplDisplay || 'custom';
    return when
      ? `Chat template: ${name} — custom (installed ${when})`
      : `Chat template: ${name} — custom`;
  }
  return "Chat template: built-in — using the model's built-in template";
}

function _headers(extra) {
  const base = window.authHeaders ? window.authHeaders() : {};
  return extra ? { ...base, ...extra } : base;
}

function _jsonHeaders() {
  return _headers({ 'Content-Type': 'application/json' });
}

// ── Data fetchers — field names below are the exact contract, this is the
// regression guard target for the bug that motivated this module. ─────────

export async function fetchReleases(name) {
  const resp = await fetch(`/api/chat-template/releases?name=${encodeURIComponent(name)}`, { headers: _headers() });
  const result = resp.ok ? await resp.json() : { ok: false };
  return result;
}

export async function fetchDiscussions(name) {
  const resp = await fetch(`/api/chat-template/discussions?name=${encodeURIComponent(name)}`, { headers: _headers() });
  const result = resp.ok ? await resp.json() : { ok: false };
  return result;
}

export async function activateRelease(name, sha256) {
  const resp = await fetch('/api/chat-template/activate', {
    method: 'POST',
    headers: _jsonHeaders(),
    body: JSON.stringify(sha256 ? { name, sha256 } : { name }),
  });
  return resp.ok ? await resp.json() : { ok: false };
}

export async function checkForUpdate({ path, fetchUrl, sourceUrl }) {
  const resp = await fetch('/api/chat-template/check-update', {
    method: 'POST',
    headers: _jsonHeaders(),
    body: JSON.stringify({ path, fetch_url: fetchUrl, source_url: sourceUrl }),
  });
  return resp.ok ? await resp.json() : { ok: false };
}

// Extracts the "owner/repo" HF id from a chat-template source_url, used to
// pre-fill the create-fix editor.
export function repoFromSourceUrl(sourceUrl) {
  if (!sourceUrl) return '';
  const rest = sourceUrl.replace('https://huggingface.co/', '');
  const parts = rest.split('/');
  return parts.length >= 2 ? `${parts[0]}/${parts[1]}` : '';
}

// ── The "Manage template…" modal ────────────────────────────────────────
// Renders into the shared #chat-template-lifecycle-modal DOM (declared once
// in static/index.html) so both surfaces reuse the same markup/CSS.

export async function openChatTemplateManageModal({ tplName, tplRepo, currentPath, onActivated }) {
  const modal = document.getElementById('chat-template-lifecycle-modal');
  const nameEl = document.getElementById('chat-template-lifecycle-name');
  const versionEl = document.getElementById('chat-template-lifecycle-version');
  const updatesEl = document.getElementById('chat-template-lifecycle-updates');
  const historyEl = document.getElementById('chat-template-lifecycle-history');
  const discussionsEl = document.getElementById('chat-template-lifecycle-discussions');
  if (!modal || !nameEl || !versionEl || !historyEl || !discussionsEl) return;

  nameEl.textContent = (tplName || '').replace(/-/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
  versionEl.textContent = 'Loading…';
  if (updatesEl) updatesEl.textContent = '';
  historyEl.textContent = '';
  discussionsEl.textContent = '';
  modal.classList.add('open');

  if (!tplName) {
    versionEl.textContent = 'No template selected yet.';
    historyEl.textContent = 'Select or install a template first.';
    discussionsEl.textContent = '';
    return;
  }

  try {
    const [releasesResult, discussionsResult] = await Promise.all([
      fetchReleases(tplName),
      fetchDiscussions(tplName),
    ]);

    _renderCurrentSection(versionEl, releasesResult);
    _renderUpdatesSection(updatesEl, { path: currentPath, onChecked: () => {} });
    _renderVersionHistorySection(historyEl, releasesResult, tplName, async () => {
      if (onActivated) await onActivated();
      // Re-render so "active" markers reflect the new state without closing the modal.
      const refreshed = await fetchReleases(tplName);
      _renderCurrentSection(versionEl, refreshed);
      _renderVersionHistorySection(historyEl, refreshed, tplName, () => {});
    });
    _renderCommunityFixesSection(discussionsEl, discussionsResult, {
      tplName,
      tplRepo: tplRepo || repoFromSourceUrl(releasesResult?.releases?.[0]?.source_url),
      currentPath,
      onInstalled: async () => {
        modal.classList.remove('open');
        if (onActivated) await onActivated();
      },
    });
  } catch (err) {
    versionEl.textContent = 'Failed to load: ' + (err.message || String(err));
  }
}

export function closeChatTemplateManageModal() {
  document.getElementById('chat-template-lifecycle-modal')?.classList.remove('open');
}

export function bindChatTemplateManageModalChrome() {
  document.getElementById('chat-template-lifecycle-close')?.addEventListener('click', closeChatTemplateManageModal);
  document.getElementById('chat-template-lifecycle-modal')?.addEventListener('click', (e) => {
    if (e.target.id === 'chat-template-lifecycle-modal') closeChatTemplateManageModal();
  });
}

function _renderCurrentSection(versionEl, releasesResult) {
  const releases = releasesResult?.releases || [];
  if (!releasesResult?.ok || releases.length === 0) {
    versionEl.textContent = releasesResult?.error || 'This template was installed directly (no release index).';
    return;
  }
  const latest = releases[0];
  const activeSha = releasesResult.active_sha256 || null;
  const active = releases.find(r => r.sha256 === activeSha) || latest;
  versionEl.innerHTML = '';
  const rows = [
    { label: 'Active revision', value: active.revision ? active.revision.slice(0, 10) : (active.sha256 || '').slice(0, 10) },
    { label: 'Source', value: active.source_url || '—' },
    { label: 'SHA-256', value: (active.sha256 || '').substring(0, 16) + '…' },
    { label: 'Installed', value: active.installed_at ? new Date(active.installed_at).toLocaleString() : '—' },
  ];
  rows.forEach(item => {
    const row = document.createElement('div');
    row.className = 'chat-template-lifecycle-version-row';
    const label = document.createElement('span');
    label.className = 'chat-template-lifecycle-version-label';
    label.textContent = item.label + ':';
    const value = document.createElement('span');
    value.className = 'chat-template-lifecycle-version-value';
    value.textContent = item.value;
    row.appendChild(label);
    row.appendChild(value);
    versionEl.appendChild(row);
  });
}

function _renderUpdatesSection(updatesEl, { path }) {
  if (!updatesEl) return;
  updatesEl.innerHTML = '';
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.className = 'btn-sm btn-preset';
  btn.textContent = CT_LABELS.checkUpdates;
  const resultSpan = document.createElement('span');
  resultSpan.style.marginLeft = '8px';
  resultSpan.style.fontSize = '10px';
  resultSpan.style.color = 'var(--color-text-muted)';
  btn.addEventListener('click', async () => {
    if (!path) {
      showToast('No template selected', 'warn');
      return;
    }
    const orig = btn.textContent;
    btn.disabled = true;
    btn.textContent = 'Checking…';
    try {
      const result = await checkForUpdate({ path });
      if (result.ok && result.changed) {
        resultSpan.textContent = 'Upstream has changed since install.';
        showToast('Upstream template has changed', 'warn', 'Use "Use recommended template" to re-download', 6000);
      } else if (result.ok) {
        resultSpan.textContent = 'Up to date.';
        showToast('Template is up to date', 'success', null, 2400);
      } else {
        resultSpan.textContent = '';
        showToast(result.error || 'Check failed', 'error');
      }
    } catch (err) {
      showToast('Check failed: ' + (err.message || String(err)), 'error');
    } finally {
      btn.disabled = false;
      btn.textContent = orig;
    }
  });
  updatesEl.appendChild(btn);
  updatesEl.appendChild(resultSpan);
}

function _renderVersionHistorySection(historyEl, releasesResult, tplName, onActivated) {
  const releases = releasesResult?.releases || [];
  historyEl.innerHTML = '';
  if (!releasesResult?.ok || releases.length === 0) {
    historyEl.textContent = 'No prior releases retained.';
    return;
  }
  const activeSha = releasesResult.active_sha256 || null;
  const header = document.createElement('div');
  header.style.fontSize = '9px';
  header.style.color = 'var(--color-text-muted)';
  header.style.marginBottom = '4px';
  header.textContent = `${releases.length} retained release${releases.length === 1 ? '' : 's'}`;
  historyEl.appendChild(header);

  releases.forEach((rel) => {
    const row = document.createElement('div');
    row.className = 'chat-template-lifecycle-history-item';
    const isActive = rel.sha256 === activeSha;
    const when = rel.installed_at ? new Date(rel.installed_at).toLocaleString() : 'unknown date';
    const rev = rel.revision ? ` (${rel.revision.slice(0, 8)})` : '';
    const label = document.createElement('span');
    label.textContent = `${when}${rev} — ${(rel.sha256 || '').slice(0, 10)}` + (isActive ? ' (active)' : '');
    label.style.color = isActive ? 'var(--color-success)' : 'var(--color-text-muted)';
    row.appendChild(label);
    if (!isActive) {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'btn-sm btn-preset';
      btn.style.marginLeft = '6px';
      btn.textContent = CT_LABELS.useThisVersion;
      btn.addEventListener('click', async () => {
        btn.disabled = true;
        try {
          const result = await activateRelease(tplName, rel.sha256);
          if (result.ok) {
            showToast('Activated release ' + rel.sha256.slice(0, 10), 'success', 'You can undo this from Version history', 2600);
            await onActivated();
          } else {
            showToast(result.error || 'Failed to activate release', 'error');
            btn.disabled = false;
          }
        } catch (err) {
          showToast('Activate failed: ' + (err.message || String(err)), 'error');
          btn.disabled = false;
        }
      });
      row.appendChild(btn);
    }
    historyEl.appendChild(row);
  });
}

function _renderCommunityFixesSection(discussionsEl, discussionsResult, { tplName, tplRepo, currentPath, onInstalled }) {
  discussionsEl.innerHTML = '';
  const subtitle = document.createElement('div');
  subtitle.style.fontSize = '9px';
  subtitle.style.color = 'var(--color-text-muted)';
  subtitle.style.marginBottom = '4px';
  subtitle.textContent = "Fixes other people have posted for this model's template on Hugging Face.";
  discussionsEl.appendChild(subtitle);

  const discussions = discussionsResult?.discussions || [];
  if (!discussionsResult?.ok || discussions.length === 0) {
    const empty = document.createElement('div');
    empty.textContent = discussionsResult?.error || 'No community fixes found for this template.';
    discussionsEl.appendChild(empty);
  } else {
    discussions.forEach((d) => {
      const row = document.createElement('div');
      row.className = 'chat-template-lifecycle-discussion-item';
      const statusBadge = d.status === 'open' ? '●' : '○';
      const prLabel = d.is_pull_request ? 'PR' : '';
      const label = document.createElement('span');
      label.textContent = `${statusBadge} ${prLabel ? prLabel + ' ' : ''}${d.title} (${d.num_comments})`;
      row.appendChild(label);
      const link = document.createElement('a');
      link.href = `https://huggingface.co/${discussionsResult.source_repo}/discussions/${d.number}`;
      link.target = '_blank';
      link.rel = 'noopener noreferrer';
      link.textContent = ' ↗';
      link.style.color = 'var(--color-accent)';
      link.style.marginLeft = '4px';
      row.appendChild(link);
      discussionsEl.appendChild(row);
    });
  }

  const editBtn = document.createElement('button');
  editBtn.type = 'button';
  editBtn.className = 'btn-sm btn-preset';
  editBtn.style.marginTop = '6px';
  editBtn.textContent = CT_LABELS.editInstallFix;
  editBtn.title = 'Paste a proposed template fix, test it, and install it as a new release';
  editBtn.addEventListener('click', () => {
    openCreateFixEditor({
      container: document.body,
      tplName,
      tplRepo,
      currentPath,
      onInstalled,
    });
  });
  discussionsEl.appendChild(editBtn);
}

// ── "Edit and install this fix" — the jinja editor flow ────────────────────
// Paste a template fix, create a release, smoke-test it, activate on pass.

export function openCreateFixEditor({ container, tplName, tplRepo, currentPath, onInstalled }) {
  const modal = document.createElement('div');
  modal.className = 'chat-template-create-fix-overlay';
  modal.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.25);display:flex;align-items:center;justify-content:center;z-index:21000;backdrop-filter:blur(3px);';
  const panel = document.createElement('div');
  panel.style.cssText = 'background:var(--pe-panel-bg);border:1px solid var(--pe-panel-border);border-radius:8px;padding:16px;min-width:550px;max-width:600px;width:90%;max-height:80vh;overflow-y:auto;box-shadow:0 12px 48px rgba(0,0,0,0.5),0 2px 12px rgba(0,0,0,0.35);flex-shrink:0;';

  const title = document.createElement('strong');
  title.textContent = CT_LABELS.editInstallFix;
  title.style.fontSize = '13px';
  panel.appendChild(title);

  const desc = document.createElement('div');
  desc.style.fontSize = '10px';
  desc.style.color = 'var(--color-text-muted)';
  desc.style.marginTop = '4px';
  desc.textContent = 'Paste a proposed template fix from a Hugging Face discussion. It will be stored as a separate release and smoke-tested for tool calls before it is activated.';
  panel.appendChild(desc);

  const repoInput = document.createElement('input');
  repoInput.type = 'text';
  repoInput.placeholder = 'HF repo (e.g., Qwen/Qwen3.5-0.5B)';
  repoInput.style.cssText = 'width:100%;margin-top:8px;padding:5px 8px;font-size:11px;background:var(--color-bg-primary);border:1px solid var(--color-border);border-radius:4px;color:var(--color-text);box-sizing:border-box;';
  if (tplRepo) repoInput.value = tplRepo;

  const idInput = document.createElement('input');
  idInput.type = 'text';
  idInput.placeholder = 'Discussion ID (number)';
  idInput.style.cssText = 'width:100%;margin-top:6px;padding:5px 8px;font-size:11px;background:var(--color-bg-primary);border:1px solid var(--color-border);border-radius:4px;color:var(--color-text);box-sizing:border-box;';

  const titleInput = document.createElement('input');
  titleInput.type = 'text';
  titleInput.placeholder = 'Discussion title (brief)';
  titleInput.style.cssText = 'width:100%;margin-top:6px;padding:5px 8px;font-size:11px;background:var(--color-bg-primary);border:1px solid var(--color-border);border-radius:4px;color:var(--color-text);box-sizing:border-box;';

  const editorWrap = document.createElement('div');
  editorWrap.style.cssText = 'position:relative;width:100%;margin-top:8px;border:1px solid var(--color-border);border-radius:4px;background:var(--color-bg-primary);overflow:hidden;';
  const toolbarRow = document.createElement('div');
  toolbarRow.style.cssText = 'display:flex;justify-content:space-between;align-items:center;padding:2px 6px;background:var(--color-bg-secondary);border-bottom:1px solid var(--color-border);';
  const toolbarLeft = document.createElement('span');
  toolbarLeft.style.cssText = 'font-size:9px;color:var(--color-text-muted);';
  toolbarLeft.textContent = 'Jinja template editor';
  const toolbarRight = document.createElement('div');
  toolbarRight.style.cssText = 'display:flex;gap:6px;align-items:center;';
  const wrapToggle = document.createElement('label');
  wrapToggle.style.cssText = 'font-size:9px;color:var(--color-text-secondary);cursor:pointer;display:flex;align-items:center;gap:3px;';
  const wrapCheck = document.createElement('input');
  wrapCheck.type = 'checkbox';
  wrapCheck.checked = false;
  wrapCheck.style.cssText = 'transform:scale(0.7);';
  wrapToggle.appendChild(wrapCheck);
  wrapToggle.appendChild(document.createTextNode('Wrap'));
  toolbarRight.appendChild(wrapToggle);
  toolbarRow.appendChild(toolbarLeft);
  toolbarRow.appendChild(toolbarRight);

  const linesWrap = document.createElement('div');
  linesWrap.style.cssText = 'display:flex;height:250px;min-height:250px;overflow:hidden;';
  const lineNumbers = document.createElement('div');
  // min-width + padding avoid the numbers colliding with code text (the bug fixed this session).
  lineNumbers.style.cssText = 'flex:0 0 auto;min-width:32px;padding:6px 8px 6px 6px;font-size:9px;font-family:monospace;line-height:1.5;color:var(--color-text-muted);text-align:right;user-select:none;background:var(--color-bg-secondary);border-right:1px solid var(--color-border);overflow:hidden;white-space:nowrap;box-sizing:border-box;';
  const contentTextarea = document.createElement('textarea');
  contentTextarea.className = 'chat-template-fix-textarea';
  contentTextarea.rows = 15;
  contentTextarea.placeholder = 'Paste the full template content here...';
  contentTextarea.style.cssText = 'flex:1;padding:6px 8px;font-size:10px;font-family:monospace;line-height:1.5;background:var(--color-bg-primary);border:none;border-radius:0;color:var(--color-text);box-sizing:border-box;resize:none;outline:none;white-space:pre;overflow:auto;';

  const recalculateLineNumbers = () => {
    const wrapped = wrapCheck.checked;
    const lines = contentTextarea.value.split('\n');
    lineNumbers.textContent = '';
    if (!wrapped) {
      const frag = document.createDocumentFragment();
      for (let i = 0; i < lines.length; i++) {
        const div = document.createElement('div');
        div.textContent = (i + 1).toString();
        frag.appendChild(div);
      }
      lineNumbers.appendChild(frag);
      return;
    }
    const textareaWidth = contentTextarea.clientWidth;
    const textareaStyle = getComputedStyle(contentTextarea);
    const paddingX = parseFloat(textareaStyle.paddingLeft) + parseFloat(textareaStyle.paddingRight);
    const effectiveWidth = textareaWidth - paddingX;
    const span = document.createElement('span');
    span.style.cssText = 'position:absolute;visibility:hidden;white-space:pre-wrap;word-wrap:break-word;width:' + effectiveWidth + 'px;padding:0;font-size:10px;font-family:monospace;line-height:1.5;';
    document.body.appendChild(span);
    const frag = document.createDocumentFragment();
    lines.forEach((line, idx) => {
      span.textContent = line || ' ';
      const visualHeight = span.offsetHeight;
      const lineHeight = parseFloat(getComputedStyle(contentTextarea).lineHeight);
      const visualLines = Math.max(1, Math.round(visualHeight / lineHeight));
      for (let v = 0; v < visualLines; v++) {
        const div = document.createElement('div');
        div.textContent = (v === 0) ? (idx + 1).toString() : '';
        frag.appendChild(div);
      }
    });
    document.body.removeChild(span);
    lineNumbers.appendChild(frag);
  };
  contentTextarea.addEventListener('input', recalculateLineNumbers);
  contentTextarea.addEventListener('scroll', () => { lineNumbers.scrollTop = contentTextarea.scrollTop; });
  wrapCheck.addEventListener('change', () => {
    contentTextarea.style.whiteSpace = wrapCheck.checked ? 'pre-wrap' : 'pre';
    contentTextarea.style.wordWrap = wrapCheck.checked ? 'break-word' : 'normal';
    contentTextarea.style.overflowX = wrapCheck.checked ? 'hidden' : 'auto';
    setTimeout(recalculateLineNumbers, 50);
  });
  if (!document.getElementById('chat-template-fix-modal-styles')) {
    const style = document.createElement('style');
    style.id = 'chat-template-fix-modal-styles';
    style.textContent = '.chat-template-fix-textarea { height: 100% !important; }';
    document.head.appendChild(style);
  }
  linesWrap.appendChild(lineNumbers);
  linesWrap.appendChild(contentTextarea);
  editorWrap.appendChild(toolbarRow);
  editorWrap.appendChild(linesWrap);

  const statusDiv = document.createElement('div');
  statusDiv.style.fontSize = '10px';
  statusDiv.style.marginTop = '8px';
  statusDiv.style.minHeight = '14px';

  const btnRow = document.createElement('div');
  btnRow.style.cssText = 'margin-top:10px;display:flex;gap:8px;justify-content:flex-end;';
  const cancelBtn = document.createElement('button');
  cancelBtn.textContent = 'Cancel';
  cancelBtn.className = 'btn-sm btn-preset';
  const submitBtn = document.createElement('button');
  submitBtn.textContent = 'Create & test';
  submitBtn.className = 'btn-sm btn-primary';
  btnRow.appendChild(cancelBtn);
  btnRow.appendChild(submitBtn);

  panel.appendChild(repoInput);
  panel.appendChild(idInput);
  panel.appendChild(titleInput);
  panel.appendChild(editorWrap);
  panel.appendChild(statusDiv);
  panel.appendChild(btnRow);
  modal.appendChild(panel);

  cancelBtn.addEventListener('click', () => container.removeChild(modal));

  submitBtn.addEventListener('click', async () => {
    const repo = repoInput.value.trim();
    const discussionId = idInput.value.trim();
    const dTitle = titleInput.value.trim();
    const content = contentTextarea.value;

    if (!repo || !discussionId || !content) {
      statusDiv.style.color = 'var(--color-danger)';
      statusDiv.textContent = 'Repo, discussion ID, and content are required.';
      return;
    }

    submitBtn.disabled = true;
    submitBtn.textContent = 'Creating release…';
    statusDiv.textContent = '';

    try {
      const installResp = await fetch('/api/chat-template/install-discussion', {
        method: 'POST',
        headers: _jsonHeaders(),
        body: JSON.stringify({
          name: tplName,
          discussion_source: {
            repo,
            discussion_id: parseInt(discussionId, 10),
            title: dTitle || `Fix from discussion #${discussionId}`,
          },
          content,
        }),
      });
      const installResult = installResp.ok ? await installResp.json() : { ok: false };
      if (!installResp.ok || installResult.ok !== true) {
        statusDiv.style.color = 'var(--color-danger)';
        statusDiv.textContent = installResult.error || 'Failed to create release';
        submitBtn.disabled = false;
        submitBtn.textContent = 'Retry';
        return;
      }

      statusDiv.style.color = 'var(--color-success)';
      statusDiv.textContent = 'Release created. Running tool-call smoke test…';

      const smokeResp = await fetch('/api/chat-template/smoke-test', {
        method: 'POST',
        headers: _jsonHeaders(),
        body: JSON.stringify({ name: installResult.release_name, model: '' }),
      });
      const smokeResult = smokeResp.ok ? await smokeResp.json() : { ok: false };

      if (smokeResp.ok && smokeResult.ok === true) {
        statusDiv.textContent = '✓ Smoke test passed. Activating release…';
        const actResult = await activateRelease(installResult.release_name);
        if (actResult.ok) {
          showToast('Fix activated successfully', 'success', 'You can undo this from Version history', 2600);
          container.removeChild(modal);
          if (onInstalled) await onInstalled(installResult);
        } else {
          statusDiv.style.color = 'var(--color-warning)';
          statusDiv.textContent = 'Smoke test passed but activation failed: ' + (actResult.error || 'unknown');
          submitBtn.disabled = false;
          submitBtn.textContent = 'Done';
        }
      } else {
        const failReason = smokeResult.summary || smokeResult.error || 'test failed';
        statusDiv.style.color = 'var(--color-danger)';
        statusDiv.textContent = '✗ Smoke test failed: ' + failReason;
        submitBtn.disabled = false;
        submitBtn.textContent = 'Done';
      }
    } catch (err) {
      statusDiv.style.color = 'var(--color-danger)';
      statusDiv.textContent = 'Error: ' + (err.message || String(err));
      submitBtn.disabled = false;
      submitBtn.textContent = 'Retry';
    }
  });

  container.appendChild(modal);

  if (currentPath) {
    statusDiv.textContent = 'Loading template content…';
    statusDiv.style.color = 'var(--color-text-muted)';
    fetch(`/api/chat-template/read?path=${encodeURIComponent(currentPath)}`, { headers: _headers() })
      .then(r => r.ok ? r.text() : Promise.reject(new Error('Failed to read template')))
      .then(text => {
        contentTextarea.value = text;
        recalculateLineNumbers();
        statusDiv.textContent = '';
      })
      .catch(err => {
        statusDiv.textContent = 'Could not load template content: ' + (err.message || String(err)) + '. Paste fix manually.';
        statusDiv.style.color = 'var(--color-warning)';
      });
  }
}
