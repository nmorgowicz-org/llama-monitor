// Chat template auto-install / model-family detection for the spawn wizard.
import { showToast } from './toast.js';
import { openChatTemplateLibraryBrowser, uploadChatTemplateFromBrowser } from './file-browser-launcher.js';
import {
  buildCommunityTemplateInstallRequest,
  detectCommunityTemplateFamily,
  getDefaultTemplateForFamily,
  getTemplateFamilies,
  getTemplatesForFamily,
} from './chat-template-registry.js';
import { wizardState } from './spawn-wizard.js';
import { awaitOriginResolve } from './spawn-wizard-hf-origin.js';

// Cache of installed community templates keyed by template name.
// Avoids re-downloading the same template for each model of the same family.
const _installedTemplateCache = {};

export function _chatTemplateDisplayName(path) {
  if (!path) return 'Embedded (from model file)';
  return path.split(/[\\/]/).pop() || path;
}

export function _applyCustomChatTemplate(path) {
  wizardState.model.chatTemplatePath = path || null;
  wizardState.model.chatTemplateMode = path ? 'custom' : 'embedded';
  const hiddenInput = document.getElementById('spawn-chat-template-path');
  if (hiddenInput) hiddenInput.value = path || '';
  const identityName = wizardState.model.source === 'hf' ? wizardState.model.hfRepo : wizardState.model.path;
  const family = detectModelFamily(identityName);
  const tpl = getDefaultTemplateForFamily(family);
  _renderChatTemplateStatus(path ? 'custom' : 'embedded', family, tpl, { path });
}

export function detectModelFamily(name) {
  const lower = (name || '').toLowerCase();
  const communityFamily = detectCommunityTemplateFamily(lower);
  if (communityFamily) return communityFamily;
  if (lower.includes('llama-3') || lower.includes('llama3') || lower.match(/llama.?3/)) return 'llama3';
  if (lower.includes('mistral') || lower.includes('mixtral')) return 'mistral';
  return null;
}

// Map GGUF general.architecture values to community template family keys
// (e.g. "qwen3_6" → "qwen", "llama" → "llama3" if LLaMA 3+)
function _ggufArchToFamily(arch) {
  const a = arch.toLowerCase();
  if (a.includes('qwen')) return 'qwen';
  if (a.includes('gemma4') || a.includes('gemma_4')) return 'gemma4';
  if (a.includes('mistral') || a.includes('mixtral')) return 'mistral';
  if (a.includes('llama')) return 'llama3';
  return null;
}

// Async family detection that tries multiple sources:
// 1) Persisted family tag from model-tags.json
// 2) GGUF metadata general.architecture (for local models — reads file header, instant)
// 3) HF model card base_model tag (via /api/hf/meta)
// 4) Filename heuristics (as fallback)
export async function detectModelFamilyAsync(identityName, localPath, timeoutMs) {
  const timeout = timeoutMs || 5000;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeout);
  const headers = window.authHeaders ? window.authHeaders() : {};

  // 1) Check persisted family tag
  if (localPath) {
    try {
      const resp = await fetch('/api/models/tags', { headers, signal: controller.signal });
      if (resp.ok) {
        const td = await resp.json().catch(() => ({}));
        const tags = td.tags?.[localPath] || [];
        const familyTag = tags.find(t => t.startsWith('family:'));
        if (familyTag) return familyTag.slice('family:'.length);
      }
    } catch (e) { if (e.name !== 'AbortError') { /* non-fatal */ } }
  }

  // 2) Read GGUF metadata for local models — architecture field is authoritative
  if (localPath) {
    try {
      const metaResp = await fetch('/api/models/gguf-meta', {
        method: 'POST',
        headers: { ...headers, 'Content-Type': 'application/json' },
        signal: controller.signal,
        body: JSON.stringify({ model_path: localPath }),
      });
      if (metaResp.ok) {
        const meta = await metaResp.json().catch(() => ({}));
        if (meta.ok && meta.architecture) {
          const arch = meta.architecture.toLowerCase();
          const family = _ggufArchToFamily(arch);
          if (family) return family;
        }
      }
    } catch (e) { if (e.name !== 'AbortError') { /* non-fatal */ } }
  }

  // 2) Check HF model card base_model tag (for locally resolved origins or HF repos)
  const repoId = identityName || wizardState.model.originRepo;
  // Only query HF API if repoId looks like an HF repo (not a local file path).
  // HF repos are "owner/name" — no dots before the slash, no backslashes.
  const looksLikeHfRepo = repoId && repoId.includes('/') &&
    !repoId.includes('\\') &&
    !repoId.split('/')[0].includes('.');
  if (looksLikeHfRepo) {
    try {
      const metaResp = await fetch(`/api/hf/meta?repo=${encodeURIComponent(repoId)}`, { headers, signal: controller.signal });
      if (metaResp.ok) {
        const meta = await metaResp.json().catch(() => ({}));
        const tags = meta.tags || [];
        const baseModelTag = tags.find(t => {
          if (!t.startsWith('base_model:')) return false;
          const rest = t.slice('base_model:'.length);
          return rest.includes('/');
        });
        if (baseModelTag) {
          const baseRepo = baseModelTag.slice('base_model:'.length);
          const detected = detectModelFamily(baseRepo);
          if (detected) return detected;
        }
      }
    } catch (e) { if (e.name !== 'AbortError') { /* non-fatal */ } }
  }

  clearTimeout(timer);

  // 3) Filename heuristics
  return detectModelFamily(identityName || localPath || '');
}

export async function autoInstallChatTemplate(force = false) {
  const { source, path, hfRepo } = wizardState.model;
  const identityName = source === 'hf' ? hfRepo : path;

  // Fast path: family already known (from wizard state or filename)
  let family = wizardState.model.family || detectModelFamily(identityName);
  const tpl = getDefaultTemplateForFamily(family);

  // If no family from fast path, we need to detect it.
  // For local/import models, await the origin resolver first (it fires from
  // the model path input handler and includes family detection in the same pass).
  if (!family && (source === 'local' || source === 'import')) {
    _renderChatTemplateStatus('detecting', null, null, null);
    // Await the origin resolver promise (created from model path input handler).
    // The resolver is idempotent — the one from loadLocalModel will be a no-op.
    // 1.5s timeout is generous: the HF search takes ~500ms.
    await awaitOriginResolve(1500);
    // After the resolver, check again (family may now be set by the resolver).
    family = wizardState.model.family || detectModelFamily(identityName);
  }
  // If still no family, query HF directly (for models not covered by origin resolver)
  if (!family) {
    try {
      family = await detectModelFamilyAsync(identityName, path, 8000);
    } catch { /* non-fatal */ }
  }
  // Update wizard state for future use
  if (family) wizardState.model.family = family;

  // Use explicitly chosen candidate (from force-family dropdown) or fall back to default for family
  const candidates = getTemplatesForFamily(family);
  const tplForFamily = wizardState.model.chatTemplateCandidate
    ? candidates.find(c => c.name === wizardState.model.chatTemplateCandidate) || getDefaultTemplateForFamily(family)
    : getDefaultTemplateForFamily(family);

  if (wizardState.model.chatTemplateMode === 'custom' && wizardState.model.chatTemplatePath) {
    _renderChatTemplateStatus('custom', family, tplForFamily, { path: wizardState.model.chatTemplatePath });
    return;
  }

  if (wizardState.model.chatTemplateMode === 'embedded') {
    wizardState.model.chatTemplatePath = null;
    _renderChatTemplateStatus('embedded', family, tplForFamily, null);
    return;
  }

  if (!tplForFamily) {
    wizardState.model.chatTemplatePath = null;
    wizardState.model.chatTemplateMode = 'auto';
    _renderChatTemplateStatus('embedded', family, null, null);
    return;
  }

  // Cache hit: template already installed for this family (skip when forcing a re-fetch)
  const cached = !force && _installedTemplateCache[tplForFamily.name];
  if (cached) {
    wizardState.model.chatTemplatePath = (tplForFamily.transformed && cached.transformed_path) ? cached.transformed_path : cached.path;
    wizardState.model.chatTemplateMode = 'auto';
    _renderChatTemplateStatus('installed', family, tplForFamily, cached);
    return;
  }

  _renderChatTemplateStatus('installing', family, tplForFamily, null);

  try {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const install = buildCommunityTemplateInstallRequest(tplForFamily, force);
    const resp = await fetch(install.endpoint, {
      method: 'POST', headers,
      body: JSON.stringify(install.body),
    });
    const data = resp.ok ? await resp.json() : { ok: false, error: `HTTP ${resp.status}` };
    if (data.ok && data.path) {
      wizardState.model.chatTemplatePath = (tplForFamily.transformed && data.transformed_path) ? data.transformed_path : data.path;
      wizardState.model.chatTemplateMode = 'auto';
      // Cache the template metadata for this family (avoids re-downloading for
      // other models of the same family in the same session)
      _installedTemplateCache[tplForFamily.name] = data;

      // Mark a force-refresh so the UI can show "Updated!" briefly
      const displayData = force
        ? { ...data, _forceRefresh: true }
        : data;

      _renderChatTemplateStatus('installed', family, tplForFamily, displayData);

       if (force) {
         showToast(
           _templateDisplayName(tplForFamily, family) + ' re-downloaded',
           'success',
           'Template has been refreshed from its upstream source',
           3000
         );
      }
    } else {
      _renderChatTemplateStatus('error', family, tplForFamily, data);
    }
  } catch (err) {
    _renderChatTemplateStatus('error', family, tplForFamily, { error: err.message || String(err) });
  }
}

// Formats display name with provenance label when multiple candidates exist for the family
function _templateDisplayName(tpl, family) {
  if (!tpl) return '';
  const candidates = family ? getTemplatesForFamily(family) : [];
  const provLabel = candidates.length > 1 && tpl.provenance
    ? ` (${tpl.provenance === 'official' ? 'Official' : 'Community'})`
    : '';
  return tpl.display + provLabel;
}

function _renderChatTemplateStatus(state, family, tpl, data) {
  const section = document.getElementById('chat-template-section');
  const statusEl = document.getElementById('ct-status');
  const bodyEl = document.getElementById('ct-body');
  const actionsEl = document.getElementById('ct-actions');
  if (!section) return;

  section.style.display = '';

  if (actionsEl) {
    actionsEl.innerHTML = '';

    if (tpl) {
      const recommendedBtn = document.createElement('button');
      recommendedBtn.type = 'button';
      recommendedBtn.className = 'btn-wizard-tertiary ct-action-btn';
       const isUsing = wizardState.model.chatTemplateMode === 'auto';
       const familyLabel = family ? ` (${family} family)` : '';
       const displayName = _templateDisplayName(tpl, family);
       recommendedBtn.textContent = isUsing
         ? `Re-fetch Recommended${familyLabel}`
         : `Use ${displayName}${familyLabel}`;
      recommendedBtn.title = isUsing
        ? 'Force re-download this template from source, even if already installed'
        : '';
      recommendedBtn.addEventListener('click', async () => {
        // If already on this template, treat the click as an explicit force
        // re-fetch (bypasses both the session cache and the on-disk shortcut) —
        // this is the only way to pick up upstream changes when "Check for
        // updates" isn't available or reports a false negative.
        const wasUsing = wizardState.model.chatTemplateMode === 'auto';
        wizardState.model.chatTemplateMode = 'auto';
        await autoInstallChatTemplate(wasUsing);
      });
      actionsEl.appendChild(recommendedBtn);
    }

    const embeddedBtn = document.createElement('button');
    embeddedBtn.type = 'button';
    embeddedBtn.className = 'btn-wizard-tertiary ct-action-btn';
    embeddedBtn.textContent = 'Use Embedded';
    embeddedBtn.disabled = wizardState.model.chatTemplateMode === 'embedded' && !wizardState.model.chatTemplatePath;
    embeddedBtn.addEventListener('click', () => {
      _applyCustomChatTemplate(null);
    });
    actionsEl.appendChild(embeddedBtn);

    const libraryBtn = document.createElement('button');
    libraryBtn.type = 'button';
    libraryBtn.className = 'btn-wizard-tertiary ct-action-btn';
    libraryBtn.textContent = 'Choose Existing';
    libraryBtn.addEventListener('click', async () => {
      try {
        await openChatTemplateLibraryBrowser('spawn-chat-template-path');
      } catch (err) {
        showToast('Template library unavailable: ' + (err.message || String(err)), 'error');
      }
    });
    actionsEl.appendChild(libraryBtn);

    const uploadBtn = document.createElement('button');
    uploadBtn.type = 'button';
    uploadBtn.className = 'btn-wizard-tertiary ct-action-btn';
    uploadBtn.textContent = 'Upload .jinja';
    uploadBtn.addEventListener('click', async () => {
      try {
        const uploaded = await uploadChatTemplateFromBrowser();
        if (!uploaded?.path) return;
        _applyCustomChatTemplate(uploaded.path);
        showToast('Template uploaded', 'success', uploaded.filename || 'Saved to template library');
      } catch {
        // uploadChatTemplateFromBrowser already surfaced the error
      }
                 });
    actionsEl.appendChild(uploadBtn);

    // Force family override — lets user manually pick a family when auto-detection fails
    const forceFamilyWrap = document.createElement('div');
    forceFamilyWrap.className = 'ct-force-family-wrap';
    forceFamilyWrap.style.display = 'flex';
    forceFamilyWrap.style.alignItems = 'center';
    forceFamilyWrap.style.gap = '6px';
    forceFamilyWrap.style.marginTop = '6px';

    const forceFamilyLabel = document.createElement('span');
    forceFamilyLabel.style.fontSize = '10px';
    forceFamilyLabel.style.fontWeight = '600';
    forceFamilyLabel.style.color = 'var(--color-text-muted)';
    forceFamilyLabel.style.textTransform = 'uppercase';
    forceFamilyLabel.style.letterSpacing = '0.06em';
    forceFamilyLabel.textContent = 'Force family';

    const forceFamilySelect = document.createElement('select');
    forceFamilySelect.className = 'ct-force-family-select';
    forceFamilySelect.style.fontSize = '11px';
    forceFamilySelect.style.padding = '3px 6px';
    forceFamilySelect.style.borderRadius = '4px';
    forceFamilySelect.style.border = '1px solid rgba(99,102,241,0.2)';
    forceFamilySelect.style.background = 'var(--color-surface-elevated)';
    forceFamilySelect.style.color = 'var(--color-text)';
    forceFamilySelect.title = 'Override the auto-detected model family to force a specific chat template';

    const currentFamily = wizardState.model.family || '';
    const currentCandidate = wizardState.model.chatTemplateCandidate || '';
    const families = getTemplateFamilies();

    const autoOpt = document.createElement('option');
    autoOpt.value = '';
    autoOpt.textContent = 'auto-detect';
    if (!currentFamily) autoOpt.selected = true;
    forceFamilySelect.appendChild(autoOpt);

    families.forEach(fam => {
      const candidates = getTemplatesForFamily(fam);
      if (candidates.length === 1) {
        const tpl = candidates[0];
        const opt = document.createElement('option');
        opt.value = fam;
        const provLabel = tpl.provenance ? ` (${tpl.provenance === 'official' ? 'Official' : 'Community'})` : '';
        opt.textContent = `${fam} — ${tpl.display}${provLabel}`;
        if (currentFamily === fam && !currentCandidate) opt.selected = true;
        forceFamilySelect.appendChild(opt);
      } else {
        const group = document.createElement('optgroup');
        const provLabels = {};
        const famName = fam.charAt(0).toUpperCase() + fam.slice(1);
        group.label = famName;
        candidates.forEach(tpl => {
          provLabels[tpl.name] = tpl.provenance ? ` (${tpl.provenance === 'official' ? 'Official' : 'Community'})` : '';
          const opt = document.createElement('option');
          opt.value = `${fam}:${tpl.name}`;
          opt.textContent = `${tpl.display}${provLabels[tpl.name]}`;
          if (currentFamily === fam && currentCandidate === tpl.name) opt.selected = true;
          group.appendChild(opt);
        });
        forceFamilySelect.appendChild(group);
      }
    });

    forceFamilySelect.addEventListener('change', () => {
      const chosen = forceFamilySelect.value;
      if (!chosen) {
        wizardState.model.family = null;
        wizardState.model.chatTemplateCandidate = null;
        return;
      }
      const [fam, candidateName] = chosen.split(':');
      wizardState.model.family = fam;
      wizardState.model.chatTemplateCandidate = candidateName || null;
      wizardState.model.chatTemplateMode = 'auto';
      autoInstallChatTemplate();
    });

    forceFamilyWrap.appendChild(forceFamilyLabel);
    forceFamilyWrap.appendChild(forceFamilySelect);
    actionsEl.appendChild(forceFamilyWrap);
  }

  if (state === 'detecting') {
    const modelName = (wizardState.model.path || '').split(/[\\/]/).pop() || '';
    if (statusEl) { statusEl.textContent = 'Detecting…'; statusEl.className = 'ct-status ct-installing'; }
    if (bodyEl) {
      bodyEl.textContent = modelName
        ? `Detecting family for ${modelName}…`
        : 'Checking HuggingFace for model family and recommended template…';
    }
    return;
  }

  if (state === 'installing') {
    if (statusEl) { statusEl.textContent = 'Downloading…'; statusEl.className = 'ct-status ct-installing'; }
    if (bodyEl) {
      bodyEl.textContent = '';
      const nameEl = document.createElement('span');
      nameEl.className = 'ct-name';
       nameEl.textContent = _templateDisplayName(tpl, family);
       bodyEl.appendChild(nameEl);
       bodyEl.appendChild(document.createTextNode(' — downloading…'));
    }
    return;
  }

  if (state === 'embedded') {
    if (statusEl) { statusEl.textContent = 'Embedded'; statusEl.className = 'ct-status ct-neutral'; }
    if (bodyEl) {
      bodyEl.textContent = family && tpl
        ? 'Using the template embedded in the model file instead of the recommended community override.'
        : 'Using template embedded in model file. You can choose an existing Jinja or upload a new one here.';
    }
    return;
  }

  if (state === 'installed') {
    const isForceRefresh = !!data?._forceRefresh;
    const installedDate = data?.installed_at
      ? new Date(data.installed_at).toLocaleString()
      : null;

    if (statusEl) {
      if (isForceRefresh) {
        statusEl.textContent = 'Updated!';
        statusEl.className = 'ct-status ct-ok';
        // Briefly highlight the update, then fall back to standard text
        setTimeout(() => {
          if (!statusEl.textContent || statusEl.textContent === 'Updated!') {
            statusEl.textContent = '✓ Installed';
          }
        }, 2200);
      } else {
        statusEl.textContent = data?.already_existed ? '✓ Cached' : '✓ Installed';
        statusEl.className = 'ct-status ct-ok';
      }
    }
    if (bodyEl) {
      bodyEl.textContent = '';
      const nameEl = document.createElement('strong');
       nameEl.textContent = _templateDisplayName(tpl, family);
       const descEl = document.createElement('span');
      descEl.textContent = ` — ${tpl.description}`;
      const sourceUrl = data?.source_url || tpl?.sourceUrl;
      const link = sourceUrl
        ? (() => { const a = document.createElement('a'); a.href = sourceUrl; a.target = '_blank'; a.rel = 'noopener noreferrer'; a.textContent = ' ↗'; a.className = 'ct-hf-link'; return a; })()
        : null;
      bodyEl.appendChild(nameEl);
      bodyEl.appendChild(descEl);
      if (link) bodyEl.appendChild(link);

      // Helpers to sync with template-autoupdater lastStatus
      const STORAGE_KEY = 'template_autoupdater_lastStatus';
      function _readAutoStatus() {
        try {
          const v = localStorage.getItem(STORAGE_KEY);
          if (!v) return { templates_with_updates: [] };
          const obj = JSON.parse(v);
          if (!obj || !Array.isArray(obj.templates_with_updates)) {
            return { templates_with_updates: [] };
          }
          return obj;
        } catch {
          return { templates_with_updates: [] };
        }
      }
      function _writeAutoStatus(status) {
        try { localStorage.setItem(STORAGE_KEY, JSON.stringify(status)); } catch { /* ignore */ }
      }
      function _markTemplateChanged(path, tplData) {
        if (!path) return;
        const s = _readAutoStatus();
        if (!s.templates_with_updates.some(t => t.path === path)) {
          s.templates_with_updates.push({
            name: tplData?.name || tpl?.display || '',
            path,
            source_url: tplData?.source_url || tpl?.sourceUrl || '',
          });
          _writeAutoStatus(s);
        }
      }
      function _clearTemplateChanged(path) {
        if (!path) return;
        const s = _readAutoStatus();
        s.templates_with_updates = s.templates_with_updates.filter(t => t.path !== path);
        _writeAutoStatus(s);
      }

      // Staleness / update hint (no misleading "may have changed" by default)
      const hint = document.createElement('div');
      hint.style.fontSize = '10px';
      hint.style.color = 'var(--color-text-muted)';
      hint.style.marginTop = '4px';
      const hintSpan = document.createElement('span');
      const tplPath = data?.path;
      const autoStatus = _readAutoStatus();
      const hasUpstreamChange = tplPath && autoStatus.templates_with_updates.some(t => t.path === tplPath);

      if (hasUpstreamChange) {
        // Auto-checker or previous "Check for updates" detected change.
        hintSpan.textContent = installedDate
          ? `Installed ${installedDate}. Upstream has changed since install.`
          : 'Template installed. Upstream has changed since install.';
      } else {
        // No known upstream change.
        hintSpan.textContent = installedDate
          ? `Installed ${installedDate}.`
          : 'Template installed.';
      }
      hint.appendChild(hintSpan);

      // "Check for updates" button
      const checkBtn = document.createElement('button');
      checkBtn.type = 'button';
      checkBtn.className = 'btn-wizard-tertiary';
      checkBtn.style.fontSize = '11px';
      checkBtn.style.fontWeight = '700';
      checkBtn.style.marginLeft = '6px';
      checkBtn.style.padding = '2px 8px';
      checkBtn.style.color = 'var(--color-accent)';
      checkBtn.style.textDecoration = 'underline';
      checkBtn.textContent = 'Check for updates';
      checkBtn.addEventListener('click', async () => {
        const path = tplPath;
        if (!path) return;
        const origText = checkBtn.textContent;
        checkBtn.disabled = true;
        checkBtn.textContent = 'Checking…';
        try {
          // Fallback fetch/source URLs for legacy installs that predate update-tracking
          // metadata (no meta.json on disk yet) — lets the backend still diff against
          // upstream instead of just erroring.
          const fallbackFetchUrl = tpl?.url
            || (tpl?.repo && tpl?.file ? `https://huggingface.co/${tpl.repo}/raw/main/${tpl.file}` : undefined);
          const resp = await fetch('/api/chat-template/check-update', {
            method: 'POST',
            headers: {
              ...(window.authHeaders ? window.authHeaders() : {}),
              'Content-Type': 'application/json',
            },
            body: JSON.stringify({
              path,
              fetch_url: fallbackFetchUrl,
              source_url: tpl?.sourceUrl,
            }),
          });
          const result = resp.ok ? await resp.json() : { ok: false };
          const now = new Date().toLocaleString();

          if (resp.ok && result.ok === true) {
            if (result.changed) {
              // Upstream changed: update hint text and mark in auto-status.
              hintSpan.textContent = `Checked on ${now} · upstream has changed.`;
              _markTemplateChanged(path, tpl);
              showToast(
                'Upstream template has changed since this install',
                'warn',
                'Use Recommended to re-download the latest version',
                6000
              );
              // Button still available so they can re-check after updating
              checkBtn.textContent = 'Check again';
            } else {
              // Up to date: update hint text and clear auto-status for this template.
              hintSpan.textContent = `Checked on ${now} · up to date.`;
              _clearTemplateChanged(path);
              showToast('Template is up to date', 'success', null, 2400);
              checkBtn.textContent = 'Check again';
            }
          } else {
            showToast(result.error || 'Failed to check for updates', 'error');
            // Restore original text on failure
            hintSpan.textContent = hasUpstreamChange
              ? (installedDate
                  ? `Installed ${installedDate}. Upstream has changed since install.`
                  : 'Template installed. Upstream has changed since install.')
              : (installedDate
                  ? `Installed ${installedDate}.`
                  : 'Template installed.');
            checkBtn.textContent = origText;
          }
        } catch (err) {
          showToast('Check failed: ' + (err.message || String(err)), 'error');
          hintSpan.textContent = hasUpstreamChange
            ? (installedDate
                ? `Installed ${installedDate}. Upstream has changed since install.`
                : 'Template installed. Upstream has changed since install.')
            : (installedDate
                ? `Installed ${installedDate}.`
                : 'Template installed.');
          checkBtn.textContent = origText;
        } finally {
          checkBtn.disabled = false;
        }
      });

      hint.appendChild(checkBtn);

      // "History" button — lists retained releases for this template name and lets the
      // user roll back (or forward) to any previously installed revision.
      const historyBtn = document.createElement('button');
      historyBtn.type = 'button';
      historyBtn.className = 'btn-wizard-tertiary';
      historyBtn.style.fontSize = '11px';
      historyBtn.style.fontWeight = '700';
      historyBtn.style.marginLeft = '6px';
      historyBtn.style.padding = '2px 8px';
      historyBtn.style.color = 'var(--color-accent)';
      historyBtn.style.textDecoration = 'underline';
      historyBtn.textContent = 'History';

      const historyList = document.createElement('div');
      historyList.style.marginTop = '6px';
      historyList.style.display = 'none';

      const tplName = data?.name || tpl?.name;
      historyBtn.addEventListener('click', async () => {
        if (historyList.style.display !== 'none') {
          historyList.style.display = 'none';
          return;
        }
        if (!tplName) return;
        historyList.textContent = 'Loading…';
        historyList.style.display = 'block';
        try {
          const resp = await fetch(`/api/chat-template/releases?name=${encodeURIComponent(tplName)}`, {
            headers: { ...(window.authHeaders ? window.authHeaders() : {}) },
          });
          const result = resp.ok ? await resp.json() : { ok: false };
          historyList.textContent = '';
          if (!resp.ok || result.ok !== true) {
            historyList.textContent = result.error || 'Failed to load history';
            return;
          }
          const releases = result.releases || [];
          if (releases.length === 0) {
            historyList.textContent = 'No retained releases yet.';
            return;
          }
          releases.forEach((rel) => {
            const row = document.createElement('div');
            row.style.fontSize = '10px';
            row.style.color = 'var(--color-text-muted)';
            row.style.marginTop = '2px';
            const isActive = rel.sha256 === result.active_sha256;
            const label = document.createElement('span');
            const when = rel.installed_at ? new Date(rel.installed_at).toLocaleString() : 'unknown date';
            const rev = rel.revision ? ` (${rel.revision.slice(0, 8)})` : '';
            label.textContent = `${when}${rev} — ${rel.sha256.slice(0, 10)}${isActive ? ' · active' : ''}`;
            row.appendChild(label);
            if (!isActive) {
              const activateBtn = document.createElement('button');
              activateBtn.type = 'button';
              activateBtn.className = 'btn-wizard-tertiary';
              activateBtn.style.fontSize = '10px';
              activateBtn.style.marginLeft = '6px';
              activateBtn.style.padding = '0 6px';
              activateBtn.style.color = 'var(--color-accent)';
              activateBtn.style.textDecoration = 'underline';
              activateBtn.textContent = 'Activate';
              activateBtn.addEventListener('click', async () => {
                activateBtn.disabled = true;
                try {
                  const actResp = await fetch('/api/chat-template/activate', {
                    method: 'POST',
                    headers: {
                      ...(window.authHeaders ? window.authHeaders() : {}),
                      'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({ name: tplName, sha256: rel.sha256 }),
                  });
                  const actResult = actResp.ok ? await actResp.json() : { ok: false };
                  if (actResp.ok && actResult.ok === true) {
                    showToast('Activated release ' + rel.sha256.slice(0, 10), 'success', null, 2400);
                    _renderChatTemplateStatus('installed', family, tpl, { ...data, _forceRefresh: true });
                  } else {
                    showToast(actResult.error || 'Failed to activate release', 'error');
                    activateBtn.disabled = false;
                  }
                } catch (err) {
                  showToast('Activate failed: ' + (err.message || String(err)), 'error');
                  activateBtn.disabled = false;
                }
              });
              row.appendChild(activateBtn);
            }
            historyList.appendChild(row);
          });
        } catch (err) {
          historyList.textContent = 'Failed to load history: ' + (err.message || String(err));
        }
      });

      hint.appendChild(historyBtn);
      bodyEl.appendChild(hint);
      bodyEl.appendChild(historyList);

      // "Discussions" button — shows HF discussions for this template's source repo
      const discussionsBtn = document.createElement('button');
      discussionsBtn.type = 'button';
      discussionsBtn.className = 'btn-wizard-tertiary';
      discussionsBtn.style.fontSize = '11px';
      discussionsBtn.style.fontWeight = '700';
      discussionsBtn.style.marginLeft = '6px';
      discussionsBtn.style.padding = '2px 8px';
      discussionsBtn.style.color = 'var(--color-accent)';
      discussionsBtn.style.textDecoration = 'underline';
      discussionsBtn.textContent = 'Discussions';

      const discussionsList = document.createElement('div');
      discussionsList.style.marginTop = '6px';
      discussionsList.style.display = 'none';
      discussionsList.style.maxWidth = '360px';

      discussionsBtn.addEventListener('click', async () => {
        if (discussionsList.style.display !== 'none') {
          discussionsList.style.display = 'none';
          return;
        }
        if (!tplName) return;
        discussionsList.textContent = 'Loading…';
        discussionsList.style.display = 'block';
        try {
          const resp = await fetch(`/api/chat-template/discussions?name=${encodeURIComponent(tplName)}`, {
            headers: { ...(window.authHeaders ? window.authHeaders() : {}) },
          });
          const result = resp.ok ? await resp.json() : { ok: false };
          discussionsList.textContent = '';
          if (!resp.ok || result.ok !== true) {
            discussionsList.textContent = result.error || 'Failed to load discussions';
            return;
          }
          const discussions = result.discussions || [];
          if (discussions.length === 0) {
            discussionsList.textContent = 'No discussions found for this template.';
            return;
          }
          const header = document.createElement('div');
          header.style.fontSize = '10px';
          header.style.color = 'var(--color-text-muted)';
          header.style.marginBottom = '4px';
          header.textContent = `${discussions.length} active discussion${discussions.length === 1 ? '' : 's'} on ${result.source_repo || 'source repo'}`;
          discussionsList.appendChild(header);
          discussions.forEach((d) => {
            const row = document.createElement('div');
            row.style.fontSize = '10px';
            row.style.color = 'var(--color-text-muted)';
            row.style.marginTop = '3px';
            const statusBadge = d.status === 'open' ? '●' : '○';
            const prLabel = d.is_pull_request ? 'PR' : '';
            const label = document.createElement('span');
            label.textContent = `${statusBadge} ${prLabel ? prLabel + ' ' : ''}${d.title} (${d.num_comments})`;
            row.appendChild(label);
            const link = document.createElement('a');
            link.href = `https://huggingface.co/${result.source_repo}/discussions/${d.number}`;
            link.target = '_blank';
            link.rel = 'noopener noreferrer';
            link.textContent = ' ↗';
            link.style.color = 'var(--color-accent)';
            link.style.marginLeft = '4px';
            row.appendChild(link);
            discussionsList.appendChild(row);
          });
        } catch (err) {
          discussionsList.textContent = 'Failed to load: ' + (err.message || String(err));
        }
      });

      hint.appendChild(discussionsBtn);
      bodyEl.appendChild(hint);
      bodyEl.appendChild(discussionsList);

      // "Create fix from discussion" button — opens modal to create a testable fix release
      const createFixBtn = document.createElement('button');
      createFixBtn.type = 'button';
      createFixBtn.className = 'btn-wizard-tertiary';
      createFixBtn.style.fontSize = '11px';
      createFixBtn.style.fontWeight = '700';
      createFixBtn.style.marginLeft = '6px';
      createFixBtn.style.padding = '2px 8px';
      createFixBtn.style.color = 'var(--color-accent)';
      createFixBtn.style.textDecoration = 'underline';
      createFixBtn.textContent = 'Create fix';

      const tplRepo = data?.source_url
        ? data.source_url.replace('https://huggingface.co/', '').split('/')[0] + '/' + data.source_url.replace('https://huggingface.co/', '').split('/')[1]
        : (tpl?.repo || '');
      createFixBtn.addEventListener('click', () => {
        const modal = document.createElement('div');
        modal.style.cssText = 'position:absolute;inset:0;background:rgba(0,0,0,0.4);display:flex;align-items:center;justify-content:center;z-index:2000;width:100%;height:100%;backdrop-filter:blur(6px);';
        const panel = document.createElement('div');
        panel.style.cssText = 'background:var(--pe-panel-bg);border:1px solid var(--pe-panel-border);border-radius:8px;padding:16px;min-width:550px;max-width:600px;width:90%;max-height:80vh;overflow-y:auto;box-shadow:0 12px 48px rgba(0,0,0,0.5),0 2px 12px rgba(0,0,0,0.35);flex-shrink:0;';
        const title = document.createElement('strong');
        title.textContent = 'Create fix from discussion';
        title.style.fontSize = '13px';
        panel.appendChild(title);
        const desc = document.createElement('div');
        desc.style.fontSize = '10px';
        desc.style.color = 'var(--color-text-muted)';
        desc.style.marginTop = '4px';
        desc.textContent = 'Paste a proposed template fix from a discussion. It will be stored as a separate release and tested before activation.';
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

        const contentTextarea = document.createElement('textarea');
        contentTextarea.className = 'chat-template-fix-textarea';
        contentTextarea.rows = 15;
        contentTextarea.placeholder = 'Paste the full template content here...';
        contentTextarea.style.cssText = 'width:100%;margin-top:8px;padding:6px 8px;font-size:11px;font-family:monospace;background:var(--color-bg-primary);border:1px solid var(--color-border);border-radius:4px;color:var(--color-text);box-sizing:border-box;resize:vertical;';
        if (!document.getElementById('chat-template-fix-modal-styles')) {
          const style = document.createElement('style');
          style.id = 'chat-template-fix-modal-styles';
          style.textContent = '.chat-template-fix-textarea { height: 250px !important; min-height: 250px !important; }';
          document.head.appendChild(style);
        }

        const statusDiv = document.createElement('div');
        statusDiv.style.fontSize = '10px';
        statusDiv.style.marginTop = '8px';
        statusDiv.style.minHeight = '14px';

        const btnRow = document.createElement('div');
        btnRow.style.cssText = 'margin-top:10px;display:flex;gap:8px;justify-content:flex-end;';
        const cancelBtn = document.createElement('button');
        cancelBtn.textContent = 'Cancel';
        cancelBtn.className = 'btn-wizard-tertiary';
        cancelBtn.style.fontSize = '11px';
        const submitBtn = document.createElement('button');
        submitBtn.textContent = 'Create & test';
        submitBtn.className = 'btn-wizard-primary';
        submitBtn.style.fontSize = '11px';
        btnRow.appendChild(cancelBtn);
        btnRow.appendChild(submitBtn);

        panel.appendChild(repoInput);
        panel.appendChild(idInput);
        panel.appendChild(titleInput);
        panel.appendChild(contentTextarea);
        panel.appendChild(statusDiv);
        panel.appendChild(btnRow);
        modal.appendChild(panel);

        const container = document.querySelector('.spawn-wizard-modal') || document.body;
        cancelBtn.addEventListener('click', () => {
          container.removeChild(modal);
        });

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
              headers: {
                ...(window.authHeaders ? window.authHeaders() : {}),
                'Content-Type': 'application/json',
              },
              body: JSON.stringify({
                name: tplName,
                discussion_source: {
                  repo: repo,
                  discussion_id: parseInt(discussionId, 10),
                  title: dTitle || `Fix from discussion #${discussionId}`,
                },
                content: content,
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
              headers: {
                ...(window.authHeaders ? window.authHeaders() : {}),
                'Content-Type': 'application/json',
              },
              body: JSON.stringify({
                name: installResult.release_name,
                model: wizardState.model.repoId || wizardState.model.modelPath || '',
              }),
            });
            const smokeResult = smokeResp.ok ? await smokeResp.json() : { ok: false };

            if (smokeResp.ok && smokeResult.ok === true) {
              statusDiv.textContent = '✓ Smoke test passed. Activating release…';
              const actResp = await fetch('/api/chat-template/activate', {
                method: 'POST',
                headers: {
                  ...(window.authHeaders ? window.authHeaders() : {}),
                  'Content-Type': 'application/json',
                },
                body: JSON.stringify({ name: installResult.release_name }),
              });
              const actResult = actResp.ok ? await actResp.json() : { ok: false };
              if (actResp.ok && actResult.ok === true) {
                showToast('Fix activated successfully', 'success', null, 2400);
                container.removeChild(modal);
                _renderChatTemplateStatus('installed', family, tpl, { ...data, _forceRefresh: true });
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

        // Pre-populate textarea with current template content
        if (tplPath) {
          statusDiv.textContent = 'Loading template content…';
          statusDiv.style.color = 'var(--color-text-muted)';
          fetch(`/api/chat-template/read?path=${encodeURIComponent(tplPath)}`, {
            headers: { ...(window.authHeaders ? window.authHeaders() : {}) }
          })
            .then(r => r.ok ? r.text() : Promise.reject(new Error('Failed to read template')))
            .then(text => {
              contentTextarea.value = text;
              statusDiv.textContent = '';
            })
            .catch(err => {
              statusDiv.textContent = 'Could not load template content: ' + (err.message || String(err)) + '. Paste fix manually.';
              statusDiv.style.color = 'var(--color-warning)';
            });
        }
      });

      hint.appendChild(createFixBtn);
      bodyEl.appendChild(hint);
    }
    return;
  }

  if (state === 'custom') {
    if (statusEl) {
      statusEl.textContent = 'Custom';
      statusEl.className = 'ct-status ct-ok';
    }
    if (bodyEl) {
      bodyEl.textContent = '';
      const nameEl = document.createElement('strong');
      nameEl.textContent = _chatTemplateDisplayName(data?.path || wizardState.model.chatTemplatePath);
      const descEl = document.createElement('span');
      descEl.textContent = ' — using your selected template from the local template library.';
      bodyEl.appendChild(nameEl);
      bodyEl.appendChild(descEl);
    }
    return;
  }

  if (state === 'error') {
    if (statusEl) { statusEl.textContent = '⚠ Failed'; statusEl.className = 'ct-status ct-error'; }
    if (bodyEl) {
      bodyEl.textContent = '';
      const msg = document.createElement('span');
      msg.className = 'ct-error-msg';
      msg.textContent = `${data?.error || 'Download failed'} — server will use embedded template.`;
      const retryBtn = document.createElement('button');
      retryBtn.type = 'button'; retryBtn.className = 'ct-retry-btn btn-wizard-tertiary';
      retryBtn.textContent = 'Retry';
      retryBtn.addEventListener('click', autoInstallChatTemplate);
      bodyEl.appendChild(msg);
      bodyEl.appendChild(document.createTextNode(' '));
      bodyEl.appendChild(retryBtn);
    }
  }
}
