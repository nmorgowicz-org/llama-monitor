// ── Presets ────────────────────────────────────────────────────────────────────
/* global DOMPurify */
// Preset CRUD: load, save, copy, delete, reset. Modal management.

import { sessionState, lastSystemMetrics } from '../core/app-state.js';
import { getPlatformInfo } from '../core/platform-info.js';
import { escapeHtml } from '../core/format.js';
import { buildArchitectureLabel, isMoEEligible } from './setup-view.js';
import { openModelFileBrowser, openChatTemplateLibraryBrowser, uploadChatTemplateFromBrowser } from './file-browser-launcher.js';
import { applySettings, saveSettings } from './settings.js';
import { showToast, showToastWithActions, showConfirmDialog } from './toast.js';
import { renderSuggestionCards, suggestionPatch, requestNcpuMoeTune } from './tuning-cards.js';
import {
    COMMUNITY_TEMPLATES,
    buildCommunityTemplateInstallRequest,
    detectCommunityTemplateFamily,
} from './chat-template-registry.js';
import { hfSearch, hfCreateFormatToggle } from './hf-browse.js';
import { buildEstimateBody } from './vram-estimate.js';

let newPresetSeed = null;

// ── HF search state for preset editor ─────────────────────────────────────────
let _presetHfSearchTimer = null;
let _presetHfSearchInitialized = false;
let _presetHfSearchFormat = 'gguf';

// ── Helpers ────────────────────────────────────────────────────────────────────

function setVal(id, v) { document.getElementById(id).value = v ?? ''; }
function setChk(id, v) { document.getElementById(id).checked = !!v; }
function setOpt(id, v) { document.getElementById(id).value = v || ''; }
function formatNumberForInput(v) {
    if (v == null || v === '') return '';
    const n = typeof v === 'number' ? v : Number(v);
    if (!Number.isFinite(n)) return String(v);
    if (Number.isInteger(n)) return String(n);
    // toPrecision(6) removes float32 noise (e.g. 0.949999988079 → 0.95)
    return String(parseFloat(n.toPrecision(6)));
}
function numOrEmpty(id, v) { document.getElementById(id).value = formatNumberForInput(v); }
function intOrNull(id) { const v = document.getElementById(id).value; return v !== '' ? parseInt(v) : null; }
function floatOrNull(id) { const v = document.getElementById(id).value; return v !== '' ? parseFloat(v) : null; }
function strVal(id) { return document.getElementById(id).value.trim(); }
function valOrNull(id) { const v = strVal(id); return v === '' ? null : v; }
function nullableBoolOpt(id) {
    const v = document.getElementById(id).value;
    if (v === 'true') return true;
    if (v === 'false') return false;
    return null;
}

function getStructuredOutputMode() {
    return document.getElementById('modal-structured-output-mode')?.value || '';
}

function setStructuredOutputMode(mode) {
    const normalized = mode === 'grammar' || mode === 'json_schema' ? mode : '';
    const modeEl = document.getElementById('modal-structured-output-mode');
    const grammarWrap = document.getElementById('modal-grammar-wrap');
    const schemaWrap = document.getElementById('modal-json-schema-wrap');
    if (modeEl) modeEl.value = normalized;
    if (grammarWrap) grammarWrap.style.display = normalized === 'grammar' ? '' : 'none';
    if (schemaWrap) schemaWrap.style.display = normalized === 'json_schema' ? '' : 'none';
}

function isRunningStatus(status) {
    return String(status || '').toLowerCase() === 'running';
}

export function presetModelSource(preset) {
    const rapidMlx = preset?.rapid_mlx;
    if (preset?.backend === 'rapid_mlx') {
        return rapidMlx?.model_source_view?.canonical_identity
            || rapidMlx?.model_source_view?.display_name
            || rapidMlx?.model_path || '';
    }
    return preset?.model_path || preset?.hf_repo || '';
}

// Same lookup `savePreset`/`_buildFormPreset` use to find the preset currently
// loaded in the modal — needed so VRAM estimates route to the right backend.
function _currentModalPreset() {
    const id = document.getElementById('modal-preset-id')?.value;
    return id ? (sessionState.presets.find(p => p.id === id) || {}) : (newPresetSeed || {});
}

export function syncSelectedPresetSelection(presetId, options = {}) {
    const id = presetId || '';
    const {
        userIntent = false,
        syncSetup = true,
        syncDisplay = true,
        persist = false,
    } = options;

    const mainSelect = document.getElementById('preset-select');
    if (mainSelect && id) {
        const opt = mainSelect.querySelector(`option[value="${CSS.escape(id)}"]`);
        if (opt) mainSelect.value = id;
    } else if (mainSelect && !id) {
        mainSelect.value = '';
    }

    const selectedId = mainSelect?.value || id;
    sessionState.selectedPresetId = selectedId;

    if (syncSetup) {
        const setupSelect = document.getElementById('setup-preset-select');
        if (setupSelect && selectedId) {
            const opt = setupSelect.querySelector(`option[value="${CSS.escape(selectedId)}"]`);
            if (opt) setupSelect.value = selectedId;
        }
    }

    if (syncDisplay && mainSelect) {
        syncPresetDisplay(mainSelect);
    }

    if (userIntent) {
        window.__presetUserSelected = true;
    }

    if (persist) {
        saveSettings();
    }

    import('./setup-view.js').then(m => {
        m.updateRunningCardHighlight?.();
    }).catch(() => {});

    return selectedId;
}

function isWindowsAbsolutePath(value) {
    return /^[A-Za-z]:[\\/]/.test(value);
}

function looksLikeLocalModelSource(value) {
    const v = (value || '').trim();
    if (!v) return false;
    const lower = v.toLowerCase();
    return v.startsWith('/') ||
        v.startsWith('./') ||
        v.startsWith('../') ||
        v.startsWith('~') ||
        v.includes('\\') ||
        isWindowsAbsolutePath(v) ||
        lower.endsWith('.gguf');
}

function normalizeModelSourceInput(value) {
    const input = (value || '').trim();
    if (!input) {
        return { model_path: '', hf_repo: null };
    }
    if (looksLikeLocalModelSource(input)) {
        return { model_path: input, hf_repo: null };
    }
    return { model_path: '', hf_repo: input };
}

async function installRecommendedChatTemplateForPreset() {
    const modelSource = strVal('modal-model-path');
    const family = detectCommunityTemplateFamily(modelSource);
    const template = family ? COMMUNITY_TEMPLATES[family] : null;
    if (!template) {
        showToast('No community template recommendation for this model', 'warn');
        return;
    }

    const button = document.getElementById('preset-recommended-chat-template-btn');
    if (button) button.disabled = true;
    try {
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };
        const install = buildCommunityTemplateInstallRequest(template);
        const resp = await fetch(install.endpoint, {
            method: 'POST',
            headers,
            body: JSON.stringify(install.body),
        });
        const data = await resp.json().catch(() => ({}));
        if (!resp.ok || !data.ok || !data.path) {
            throw new Error(data.error || `HTTP ${resp.status}`);
        }
        setVal('modal-chat-template-file', data.path);
        showToast(
            data.already_existed ? 'Recommended template selected' : 'Recommended template installed',
            'success',
            template.display,
        );
    } catch (err) {
        showToast('Template install failed: ' + (err.message || String(err)), 'error');
    } finally {
        if (button) button.disabled = false;
    }
}

function clearFieldErrors() {
    // Clear field-error class
    document.querySelectorAll('#preset-form .field-error').forEach(el => el.classList.remove('field-error'));
    // Remove any inline error messages we added
    document.querySelectorAll('#preset-form .field-error-msg').forEach(el => el.remove());
}

function markFieldError(fieldId, message) {
    const el = document.getElementById(fieldId);
    if (!el) return;
    el.classList.add('field-error');
    // Insert an inline error message if not already present
    const existing = el.parentElement.querySelector('.field-error-msg');
    if (existing) {
        existing.textContent = message;
    } else {
        const msg = document.createElement('div');
        msg.className = 'field-error-msg';
        msg.textContent = message;
        el.after(msg);
    }
}

function scrollToFirstError() {
    const first = document.querySelector('#preset-form .field-error');
    if (!first) return;
    first.scrollIntoView({ behavior: 'smooth', block: 'center' });
    if (first.focus) first.focus();
}

// ── Load ───────────────────────────────────────────────────────────────────────

export async function loadPresets(selectId) {
    const auth = window.authHeaders ? window.authHeaders() : {};

    const [presetsResp, settingsResp, activeResp, collectionsResp] = await Promise.all([
        fetch('/api/presets', { headers: auth }),
        selectId === undefined ? fetch('/api/settings', { headers: auth }) : Promise.resolve(null),
        selectId === undefined ? fetch('/api/sessions/active', { headers: auth }) : Promise.resolve(null),
        selectId === undefined ? fetch('/api/collections', { headers: auth }) : Promise.resolve(null),
    ]);

    if (presetsResp.status === 401) {
        showToast('Unauthorized: API token missing or invalid', 'error');
        return;
    }

    sessionState.presets = await presetsResp.json();
    if (collectionsResp && collectionsResp.ok) {
        try {
            const collectionsData = await collectionsResp.json();
            sessionState.collections = collectionsData.collections || [];
        } catch {
            sessionState.collections = [];
        }
    } else {
        sessionState.collections = [];
    }
    let saved = null;
    if (settingsResp) {
        if (settingsResp.status === 401) {
            console.warn('[presets] /api/settings returned 401');
        } else {
            saved = await settingsResp.json();
        }
    }

    const sel = document.getElementById('preset-select');
    sel.innerHTML = '';
    sessionState.presets.forEach(p => {
        // Skip built-in/example presets that have no model (they are templates, not usable)
        if (!presetModelSource(p)) return;
        const opt = document.createElement('option');
        opt.value = p.id;
        opt.textContent = p.name;
        sel.appendChild(opt);
    });

    // Determine which preset to select:
    // 1) explicit selectId (used by spawn-wizard / CRUD)
    // 2) running session's preset_id (if available and session is Running)
    // 3) saved UiSettings.preset_id
    // 4) first preset as fallback
    let targetId = selectId ?? null;
    if (targetId === null && activeResp) {
        const activeData = await activeResp.json().catch(() => null);
        if (activeData && activeData.preset_id && isRunningStatus(activeData.status)) {
            sessionState.activeSessionPresetId = activeData.preset_id;
            targetId = activeData.preset_id;
        } else {
            sessionState.activeSessionPresetId = '';
        }
    }
    if (targetId === null) {
        targetId = saved?.preset_id || null;
    }

    if (targetId && sessionState.presets.find(p => p.id === targetId)) {
        syncSelectedPresetSelection(targetId, { syncSetup: false, syncDisplay: false });
    } else if (sel.options.length > 0) {
        syncSelectedPresetSelection(sel.options[0].value, { syncSetup: false, syncDisplay: false });
    }

    if (selectId === undefined && saved) {
        applySettings(saved);
    }
    if (selectId === undefined) {
        saveSettings();
    }

    // Sync the visual preset display (short name + chips)
    if (sel && sel.value) syncSelectedPresetSelection(sel.value, { syncSetup: false });

    // Keep the setup view preset dropdown and launch grid in sync
    import('./setup-view.js').then(m => m.syncSetupPresetSelect?.()).catch(() => {});

    // Main dashboard: handle user changing the preset while on the dashboard
    wirePresetSelectChangeHandler();
}

// Handle user switching presets in the main dashboard dropdown.
// If a model is already loaded (different preset), prompt to stop it and load the new one.
function wirePresetSelectChangeHandler() {
    const sel = document.getElementById('preset-select');
    if (!sel) return;

    // Avoid duplicate wiring (populatePresetSelect can run more than once)
    if (sel.__presetChangeWired) return;
    sel.__presetChangeWired = true;

    sel.addEventListener('change', async () => {
        const chosenId = sel.value;
        if (!chosenId) return;

        // Mark as user-initiated so WS sync won't force it back
        syncSelectedPresetSelection(chosenId, { userIntent: true, persist: true });

        // Fetch current active session to see if something is running
        try {
            const resp = await fetch('/api/sessions/active', {
                headers: window.authHeaders ? window.authHeaders() : {},
            });
            if (!resp.ok) return;
            const active = await resp.json().catch(() => ({}));

            // If nothing is running, or it's the same preset, nothing special to do
            if (!active || !isRunningStatus(active.status) || active.preset_id === chosenId) {
                showToast('Preset selected', 'info');
                return;
            }

            // Different preset is running: show a non-blocking toast to confirm switch
            const chosenPreset = sessionState.presets.find(p => p.id === chosenId);
            const runningPreset = sessionState.presets.find(p => p.id === active.preset_id);
            const chosenName = chosenPreset?.name || 'selected preset';
            const runningName = runningPreset?.name || 'current preset';

            const revert = () => {
                syncSelectedPresetSelection(active.preset_id, { persist: true });
                window.__presetUserSelected = false;
            };

            showToastWithActions(
                `Switch to "${chosenName}"?`,
                'info',
                `Currently running: ${runningName}`,
                [
                    {
                        id: 'restart',
                        label: 'Restart Now',
                        primary: true,
                        handler: async () => {
                            showToast('Switching preset…', 'info');
                            const { doKillLlamaInternal, doStart } = await import('./attach-detach.js');
                            await doKillLlamaInternal();
                            await new Promise(r => setTimeout(r, 400));
                            await doStart(null, { skipRunningConfirm: true });
                        },
                    },
                    {
                        id: 'cancel',
                        label: 'Cancel',
                        primary: false,
                        handler: revert,
                    },
                ],
                { duration: 12000, onDismiss: revert },
            );

        } catch (e) {
            console.warn('[presets] preset-select change error:', e);
        } finally {
            syncSelectedPresetSelection(sel.value);
        }
    });
}

// ── Preset display (short name + chips) ────────────────────────────────────────

function syncPresetDisplay(sel) {
    const labelEl = document.getElementById('preset-display-label');
    const chipsEl = document.getElementById('preset-display-chips');
    if (!labelEl || !chipsEl || !sel || !sel.value) return;

    const preset = (sessionState.presets || []).find(p => p.id === sel.value);
    if (!preset) return;

    const fullName = preset.name || presetModelSource(preset).split('/').pop() || '';
    const displayName = buildShortPresetName(preset, fullName);

    labelEl.textContent = displayName;
    labelEl.title = fullName;

    chipsEl.innerHTML = '';
    const chips = buildPresetChips(preset);
    for (const chip of chips) {
        const span = document.createElement('span');
        span.className = 'preset-display-chip';
        span.textContent = chip.label;
        if (chip.title) span.title = chip.title;
        chipsEl.appendChild(span);
    }
}

function buildShortPresetName(p, fullName) {
    const base = fullName || p.name || presetModelSource(p).split('/').pop() || '';
    if (!base) return '';
    // Normalize underscores to hyphens; CSS text-overflow handles truncation.
    return base.replace(/_/g, '-').replace(/-{2,}/g, '-').trim();
}

function buildPresetChips(p) {
    const chips = [];
    const name = p.name || presetModelSource(p).split('/').pop() || '';

    // Quant chip
    const qMatch = name.match(/(Q\d+[_-]?[A-Z0-9]+)/i);
    if (qMatch) {
        chips.push({ label: qMatch[1].toUpperCase() });
    }

    // Context chip
    const ctx = p.context_size;
    if (ctx != null && ctx > 0) {
        let ctxLabel;
        if (ctx <= 1000) {
            ctxLabel = String(ctx);
        } else if (ctx < 1_000_000) {
            ctxLabel = Math.round(ctx / 1024) + 'k';
        } else {
            ctxLabel = 'large';
        }
        chips.push({ label: ctxLabel, title: `${ctx.toLocaleString()} tokens` });
    }

    // Draft / speculative chip:
    // Only show "DRAFT" when there's an actual draft model or MTP-style spec decoding.
    // Basic ngram or simple lookahead do NOT warrant a "DRAFT" pill.
    const spec = (p.spec_type || '').toLowerCase();
    const draftModel = (p.draft_model_path || p.draft_model || '').trim();
    const hasMtp = spec.includes('mtp');
    const isNgram = spec.includes('ngram');
    const isSimple = spec === 'simple';
    const isEmpty = !spec || spec === 'none';
    if (draftModel || hasMtp) {
        chips.push({ label: 'Draft', title: `Speculative decoding: ${p.spec_type}` });
    } else if (!isNgram && !isSimple && !isEmpty) {
        // Unknown/advanced spec type without ngram → still show Draft
        chips.push({ label: 'Draft', title: `Speculative decoding: ${p.spec_type}` });
    }

    return chips.slice(0, 3);
}

// Click the preset display wrapper to open the underlying <select>
document.addEventListener('DOMContentLoaded', () => {
    const wrapper = document.querySelector('.preset-display-wrapper');
    const sel = document.getElementById('preset-select');
    if (!wrapper || !sel) return;

    wrapper.addEventListener('click', (e) => {
        // Don't interfere with child button clicks
        if (e.target.closest('.preset-inline-actions')) return;
        e.stopPropagation();
        // showPicker is the preferred, least intrusive way to open the native menu
        if (typeof sel.showPicker === 'function') {
            sel.showPicker();
        } else {
            // Fallback: click the select directly so the browser opens its options
            sel.focus();
            sel.click();
        }
    });

    // HF model search for preset editor
    _initPresetHfSearch();
});

function _initPresetHfSearch() {
    if (_presetHfSearchInitialized) return;
    _presetHfSearchInitialized = true;

    const input = document.getElementById('modal-model-path');
    const resultsContainer = document.getElementById('preset-hf-search-results');
    const toggleWrap = document.getElementById('preset-hf-format-toggle-wrap');
    if (!input || !resultsContainer || !toggleWrap) return;

    // Create format toggle (GGUF/MLX)
    hfCreateFormatToggle({
        container: toggleWrap,
        defaultFormat: 'gguf',
        onChange: (fmt) => {
            _presetHfSearchFormat = fmt;
            _triggerPresetHfSearch();
        },
    });

    // Search when user types something that looks like an HF repo id (contains "/")
    input.addEventListener('input', () => {
        clearTimeout(_presetHfSearchTimer);
        _presetHfSearchTimer = setTimeout(_triggerPresetHfSearch, 350);
    });

    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && resultsContainer.style.display !== 'none') {
            e.preventDefault();
            clearTimeout(_presetHfSearchTimer);
            const first = resultsContainer.querySelector('.hf-search-result');
            if (first) first.click();
        }
    });

    // Close results when clicking outside
    document.addEventListener('click', (e) => {
        if (!input.contains(e.target) && !resultsContainer.contains(e.target) && !toggleWrap.contains(e.target)) {
            resultsContainer.style.display = 'none';
        }
    });
}

function _looksLikeHfRepoId(value) {
    if (!value || value.length < 4) return false;
    const v = value.trim();
    // Must contain "/" but not look like a local path
    if (!v.includes('/')) return false;
    if (v.startsWith('/') || v.startsWith('./') || v.startsWith('../')) return false;
    if (v.includes('\\') || /^[A-Za-z]:/.test(v)) return false;
    if (v.toLowerCase().endsWith('.gguf')) return false;
    // Must be owner/repo format (two path segments)
    const parts = v.split('/');
    if (parts.length !== 2 || !parts[0] || !parts[1]) return false;
    return /^[a-zA-Z0-9._-]+$/.test(parts[0]) && /^[a-zA-Z0-9._/-]+$/.test(parts[1]);
}

function _triggerPresetHfSearch() {
    const input = document.getElementById('modal-model-path');
    const resultsContainer = document.getElementById('preset-hf-search-results');
    if (!input || !resultsContainer) return;

    const value = input.value.trim();
    if (!_looksLikeHfRepoId(value)) {
        resultsContainer.style.display = 'none';
        return;
    }

    const parts = value.split('/');
    const author = parts[0];
    const query = parts.slice(1).join('/');

    hfSearch({
        query: query || undefined,
        author,
        sort: 'downloads',
        limit: 8,
        format: _presetHfSearchFormat,
        container: resultsContainer,
        filelistContainer: null,
        quickpicksContainer: null,
        discoverPillsContainerId: null,
        onOpenCardPanel: (repoId) => {
            window.open(`https://huggingface.co/${repoId}`, '_blank', 'noopener');
        },
        onSelectModel: (m) => {
            input.value = m.id;
            resultsContainer.style.display = 'none';
            input.dispatchEvent(new Event('change', { bubbles: true }));
            showToast(`Selected: ${m.id}`, 'info');
        },
    });
}

// ── Modal ──────────────────────────────────────────────────────────────────────

// ── Performance advisor (config-time hints) ──────────────────────────────────
 let _presetAdvisorTimer = null;
let _presetAdvisorSeq = 0;
let _presetIsUnified = null; // cached platform check
let _presetRamUsedBytes = 0;
let _presetVramBytes = 0;
let _presetRamBytes = 0;    // cached RAM total (bytes)
let _presetMetalLimitMb = 0; // cached iogpu.wired_limit_mb (0 = use heuristic)
let _presetSnapshot = null; // MemoryAvailabilitySnapshot — refreshed periodically
let _presetSnapshotAge = 0; // timestamp when snapshot was fetched

// ── VRAM live estimate ────────────────────────────────────────────────────────
let _presetVramTimer = null;
let _presetVramSeq = 0;

/// Phase 5b Part C: Fetch MemoryAvailabilitySnapshot for accurate availability.
/// Cached briefly (30s) to avoid excessive API calls while remaining responsive.
async function _presetRefreshSnapshot() {
    const now = Date.now();
    // Refresh if we have no snapshot or it's older than 30 seconds.
    if (_presetSnapshot && (now - _presetSnapshotAge) < 30000) {
        return _presetSnapshot;
    }
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const resp = await fetch('/api/memory-availability', { headers });
        if (!resp.ok) return null;
        const data = await resp.json();
        if (!data.ok || !data.snapshot) return null;
        _presetSnapshot = data.snapshot;
        _presetSnapshotAge = now;
        return _presetSnapshot;
    } catch {
        return null;
    }
}

function _presetAvailBytes() {
    if (!_presetIsUnified) return _presetVramBytes;
    // Phase 5b Part C: use current_safe_availability_bytes from the snapshot,
    // NOT a stale fraction of total RAM.
    if (_presetSnapshot && _presetSnapshot.current_safe_availability_bytes > 0) {
        return _presetSnapshot.current_safe_availability_bytes;
    }
    // Fallback: stale heuristic (should not normally be used after snapshot is fetched).
    if (_presetRamBytes === 0) return 0;
    const limitBytes = _presetMetalLimitMb > 0 ? _presetMetalLimitMb * 1024 * 1024 : null;
    const fraction = _presetRamBytes <= 36 * 1024 ** 3 ? 2 / 3 : 3 / 4;
    const cap = limitBytes ?? Math.floor(_presetRamBytes * fraction);
    return Math.max(0, Math.min(cap, _presetRamBytes) - 512 * 1024 * 1024);
}

function _presetAvailableRamBytes() {
    return _presetIsUnified ? 0 : Math.max(0, _presetRamBytes - _presetRamUsedBytes);
}

async function _ensureUnifiedFlag() {
    if (_presetIsUnified !== null) return _presetIsUnified;
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const [platform, sys, gpu] = await Promise.all([
            getPlatformInfo().catch(() => null),
            fetch('/metrics/system', { headers }).then(r => r.ok ? r.json() : null).catch(() => null),
            fetch('/metrics/gpu', { headers }).then(r => r.ok ? r.json() : null).catch(() => null),
        ]);
        _presetIsUnified = platform?.auto_backend === 'metal';
        _presetRamBytes = (sys?.ram_total_gb || lastSystemMetrics?.ram_total_gb || 0) * 1024 ** 3;
        _presetRamUsedBytes = (sys?.ram_used_gb || lastSystemMetrics?.ram_used_gb || 0) * 1024 ** 3;
        if (!_presetIsUnified && gpu) {
            const gpus = Array.isArray(gpu) ? gpu : (gpu.gpus ? gpu.gpus : Object.values(gpu));
            _presetVramBytes = gpus.reduce((sum, g) => {
                const totalMb = g.vram_total_mb || g.total_mb || g.total_memory_mb || g.vram_total || 0;
                const usedMb = g.vram_used_mb || g.used_mb || g.vram_used || 0;
                return sum + Math.max(0, totalMb - usedMb) * 1024 * 1024;
            }, 0);
        }
        if (_presetIsUnified) {
            const lim = await fetch('/api/system/metal-gpu-limit', { headers }).then(r => r.ok ? r.json() : null).catch(() => null);
            if (lim?.ok && lim.limit_mb > 0) _presetMetalLimitMb = lim.limit_mb;
        }
    } catch { _presetIsUnified = _presetIsUnified ?? false; }
    return _presetIsUnified;
}

function _parseParamB(name) {
    const m = /(\d+(?:\.\d+)?)\s*b\b/i.exec(name || '');
    return m ? parseFloat(m[1]) : 0;
}

function looksLikeQwenName(name) {
    const n = (name || '').toLowerCase();
    return n.includes('qwen');
}

function qwenVLImageTokens(p) {
    // Return recommended image token budgets for multimodal (mmproj) models.
    // Qwen3.6 vision: 1024 / 4096
    // Gemma4: 280 / 1120 (valid: 70, 140, 280, 560, 1120)
    const name = p.model_path || '';
    const repo = p.hf_repo || '';
    const hasMmproj = !!p.mmproj;
    if (!hasMmproj) return { min_tokens: null, max_tokens: null };

    if (looksLikeQwenName(name) || looksLikeQwenName(repo)) {
        return { min_tokens: 1024, max_tokens: 4096 };
    }
    if ((name || '').toLowerCase().includes('gemma') || (repo || '').toLowerCase().includes('gemma')) {
        return { min_tokens: 280, max_tokens: 1120 };
    }
    return { min_tokens: null, max_tokens: null };
}

function applyPresetSuggestion(suggestion) {
    const patch = suggestionPatch(suggestion);
    const map = { ctk: 'modal-ctk', ctv: 'modal-ctv', context_size: 'modal-context-size', spec_type: 'modal-spec-type' };
    Object.entries(patch).forEach(([k, v]) => {
        const id = map[k];
        const el = id && document.getElementById(id);
        if (!el) return; // spec_draft_n_max has no direct preset field; spec-type drives MTP
        el.value = String(v);
        el.dispatchEvent(new Event('change', { bubbles: true }));
    });
    updatePresetAdvisor();
    showToast('Applied', 'success', suggestion.label);
}

export function updatePresetAdvisor() {
    const box = document.getElementById('preset-advisor');
    const cards = document.getElementById('preset-advisor-cards');
    if (!box || !cards) return;
    clearTimeout(_presetAdvisorTimer);
    _presetAdvisorTimer = setTimeout(async () => {
        const isUnified = await _ensureUnifiedFlag();
        const modelVal = document.getElementById('modal-model-path')?.value.trim() || '';
        const name = modelVal.split(/[/\\]/).pop() || '';
        if (!name) { box.style.display = 'none'; return; }
        const ctk = document.getElementById('modal-ctk')?.value || 'q8_0';
        const ctv = document.getElementById('modal-ctv')?.value || 'q8_0';
        const ctx = parseInt(document.getElementById('modal-context-size')?.value) || 8192;
        const specType = document.getElementById('modal-spec-type')?.value || '';
        const body = {
            name,
            param_b: _parseParamB(name),
            context_size: ctx,
            ctk, ctv,
            is_unified: isUnified,
            spec_type: specType || null,
            has_mtp: /mtp/i.test(name),
        };
        const seq = ++_presetAdvisorSeq;
        try {
            const headers = window.authHeaders
                ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                : { 'Content-Type': 'application/json' };
            const r = await fetch('/api/advise', { method: 'POST', headers, body: JSON.stringify(body) });
            if (seq !== _presetAdvisorSeq) return;
            const data = await r.json();
            const suggestions = (data && data.suggestions) || [];
            const cfgView = { ctk, ctv, context_size: ctx, spec_type: specType };
            renderSuggestionCards(cards, suggestions, { onApply: applyPresetSuggestion, config: cfgView });
            box.style.display = cards.childElementCount ? '' : 'none';
        } catch { box.style.display = 'none'; }
    }, 250);
}

// Render a structured hint (icon + headline + body + optional inline action) into one of
// the `.preset-memory-warning` slots. `severity` picks the colour: info (blue), caution
// (amber, the base), danger (orange-red). Always sets the base class so the box styling
// applies (a prior bug overwrote className to an unstyled class).
function _renderPresetHint(el, { severity = 'caution', icon = '', head = '', body = '', action = null }) {
    if (!el) return;
    el.className =
        severity === 'danger'
            ? 'preset-memory-warning preset-mlock-warning--suggest-off'
            : severity === 'info'
              ? 'preset-memory-warning preset-memory-warning--info'
              : 'preset-memory-warning';
    // Build with DOM APIs (textContent) rather than innerHTML — injection-safe and lint-clean.
    el.replaceChildren();
    const wrap = document.createElement('div');
    wrap.className = 'pe-hint';
    const ic = document.createElement('span');
    ic.className = 'pe-hint-icon';
    ic.textContent = icon;
    const bodyEl = document.createElement('span');
    bodyEl.className = 'pe-hint-body';
    if (head) {
        const h = document.createElement('span');
        h.className = 'pe-hint-head';
        h.textContent = head;
        bodyEl.append(h, document.createTextNode(' '));
    }
    bodyEl.append(document.createTextNode(body));
    if (action) {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'pe-hint-action';
        btn.textContent = action.label;
        btn.addEventListener('click', action.onClick);
        const btnWrap = document.createElement('div');
        btnWrap.append(btn);
        bodyEl.append(btnWrap);
    }
    wrap.append(ic, bodyEl);
    el.append(wrap);
    el.style.display = '';
}

function updatePresetMlockWarning(estimate = null) {
    const el = document.getElementById('preset-mlock-warning');
    if (!el) return;
    const checked = document.getElementById('modal-mlock')?.checked;
    if (!checked) {
        el.style.display = 'none';
        el.innerHTML = '';
        return;
    }

    const rec = estimate?.recommendation || '';
    const total = estimate?.total_bytes || 0;
    const avail = estimate?.available_vram_bytes || 0;
    const ratio = avail > 0 ? total / avail : 0;
    const pressure = rec === 'risk' || rec === 'tight' || ratio >= 0.82;
    const sys = lastSystemMetrics;
    const wiredGb = sys?.memory_wired_gb || 0;
    const modelGib = total / (1024 ** 3);
    const wiredAfter = wiredGb + modelGib;

    // Would mlock push total system RAM above 90%? Past that, macOS can't compress model
    // pages and gets starved of headroom.
    const totalRamGb = sys?.ram_total_gb || 0;
    const usedRamGb = sys?.ram_used_gb || 0;
    const projectedPct = totalRamGb > 0 ? Math.round(((usedRamGb + modelGib) / totalRamGb) * 100) : 0;
    const wiredOverload = _presetIsUnified && totalRamGb > 0 && projectedPct >= 90;
    const wiredNote =
        _presetIsUnified && wiredGb > 0 && modelGib > 0
            ? ` System wired memory: ${wiredGb.toFixed(1)} GB now → ~${wiredAfter.toFixed(1)} GB after loading (wired = non-compressible).`
            : '';

    const turnOff = {
        label: 'Turn off mlock',
        onClick: () => {
            const c = document.getElementById('modal-mlock');
            if (c) {
                c.checked = false;
                // Dispatch change so form listeners (estimate, hints, dirty-tracking) all fire.
                c.dispatchEvent(new Event('change', { bubbles: true }));
            }
        },
    };

    if (wiredOverload) {
        _renderPresetHint(el, {
            severity: 'danger',
            icon: '⚠️',
            head: 'mlock isn’t recommended here.',
            body: `Loading this model pins ~${projectedPct}% of system RAM as non-compressible — macOS can’t relieve pressure and the desktop may stall. On Apple Silicon, Metal already keeps model memory resident while the server runs, so mlock adds risk with no benefit.${wiredNote}`,
            action: turnOff,
        });
    } else if (pressure) {
        _renderPresetHint(el, {
            severity: 'caution',
            icon: '⚠️',
            head: 'Tight fit with mlock.',
            body: `This estimate is already tight — pinned memory can push macOS into compression or swap and make the desktop unresponsive.${wiredNote}`,
            action: _presetIsUnified ? turnOff : null,
        });
    } else {
        _renderPresetHint(el, {
            severity: 'caution',
            icon: '📌',
            head: 'mlock pins model memory.',
            body: `It stops the OS reclaiming model pages. Leave enough free RAM for macOS, browsers, and background tasks.${wiredNote}`,
        });
    }
}

// Apple Silicon recommendation: keep mmap ON (no-mmap OFF). Measured identical throughput
// on M-series, and mmap is zero-copy into Metal — disabling it only slows loads and commits
// the whole model to RAM up front. Shown only on unified memory when no-mmap is enabled.
function updatePresetMmapHint() {
    const el = document.getElementById('preset-mmap-hint');
    if (!el) return;
    const noMmap = document.getElementById('modal-no-mmap')?.checked;
    if (!_presetIsUnified || !noMmap) {
        el.style.display = 'none';
        el.innerHTML = '';
        return;
    }
    _renderPresetHint(el, {
        severity: 'info',
        icon: '🍎',
        head: 'no-mmap isn’t recommended on Apple Silicon.',
        body: 'On unified memory, mmap is zero-copy into Metal — it doesn’t change throughput (measured identical tok/s on M-series) but loads faster and avoids committing the whole model to RAM up front. Leave it off unless the model lives on slow network storage.',
        action: {
            label: 'Turn off no-mmap',
            onClick: () => {
                const c = document.getElementById('modal-no-mmap');
                if (c) {
                    c.checked = false;
                    // Dispatch change so form listeners (estimate, hints, dirty-tracking) all fire.
                    c.dispatchEvent(new Event('change', { bubbles: true }));
                }
            },
        },
    });
}

// ── VRAM live estimate for preset editor ─────────────────────────────────────

async function autoSizePreset() {
    const btn = document.getElementById('preset-vram-auto-size');
    const modelVal = document.getElementById('modal-model-path')?.value.trim() || '';
    if (!modelVal) {
        showToast('Auto-size requires a model', 'warn');
        return;
    }
    if (!btn) return;
    const origText = btn.textContent;
    btn.disabled = true;
    btn.textContent = 'Sizing...';

    try {
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };
        const body = {
            model_path: modelVal,
            n_ctx: parseInt(document.getElementById('modal-context-size')?.value) || 128000,
            ctk: document.getElementById('modal-ctk')?.value || 'q8_0',
            ctv: document.getElementById('modal-ctv')?.value || 'f16',
            parallel_slots: parseInt(document.getElementById('modal-parallel-slots')?.value) || 1,
            ubatch_size: parseInt(document.getElementById('modal-ubatch-size')?.value) || 512,
            n_cpu_moe: parseInt(document.getElementById('modal-n-cpu-moe')?.value) || 0,
            gpu_layers: Number.isFinite(parseInt(document.getElementById('modal-gpu-layers')?.value))
                ? parseInt(document.getElementById('modal-gpu-layers')?.value)
                : -1,
            available_vram_bytes: _presetAvailBytes(),
            available_ram_bytes: _presetAvailableRamBytes(),
            is_unified_memory: !!_presetIsUnified,
            backend: _currentModalPreset()?.backend === 'rapid_mlx' ? 'rapid_mlx' : 'llama_cpp',
        };

        const resp = await fetch('/api/vram/auto-size', { method: 'POST', headers, body: JSON.stringify(body) });
        if (!resp.ok) {
            showToast('Auto-size failed', 'error');
            return;
        }
        const data = await resp.json();
        if (!data.ok || !data.result) {
            showToast('Auto-size: no result', 'warning');
            return;
        }

        const r = data.result;
        setVal('modal-context-size', r.context_size);
        setVal('modal-ctk', r.kv_quant_k);
        setVal('modal-ctv', r.kv_quant_v);
        setVal('modal-ubatch-size', r.ubatch_size);

        // Trigger UI updates
        updatePresetVram();
        updatePresetAdvisor();
        showToast('Auto-sized', 'success', `Optimized to ${r.context_size} tokens`);

    } catch (err) {
        showToast('Auto-size error', 'error', err.message);
    } finally {
        btn.disabled = false;
        btn.textContent = origText;
    }
}

export function updatePresetVram() {
    const box = document.getElementById('preset-vram-display');
    const strip = document.getElementById('preset-vram-strip');
    if (!box) return;
    updatePresetMlockWarning();
    updatePresetMmapHint();
    const modelVal = document.getElementById('modal-model-path')?.value.trim() || '';
    if (!modelVal) { if (strip) strip.style.display = 'none'; return; }
    if (strip) strip.style.display = '';
    box.innerHTML = '<div class="preset-vram-loading">Estimating VRAM…</div>';
    clearTimeout(_presetVramTimer);
    _presetVramTimer = setTimeout(async () => {
        const isUnified = await _ensureUnifiedFlag();
        // Phase 5b Part C: refresh memory state from the snapshot (no infinite cache).
        if (isUnified) {
            await _presetRefreshSnapshot();
        }
        // Platform flag is now resolved — refresh the Apple Silicon mmap hint.
        updatePresetMmapHint();
        const nCtx = parseInt(document.getElementById('modal-context-size')?.value) || 131072;
        const ctk = document.getElementById('modal-ctk')?.value || 'q8_0';
        const ctv = document.getElementById('modal-ctv')?.value || 'f16';
        const parallelSlots = parseInt(document.getElementById('modal-parallel-slots')?.value) || 1;
        const ubatch = parseInt(document.getElementById('modal-ubatch-size')?.value) || 512;
        const nCpuMoe = parseInt(document.getElementById('modal-n-cpu-moe')?.value) || 0;
        const gpuLayers = parseInt(document.getElementById('modal-gpu-layers')?.value);
        const mmprojPath = document.getElementById('modal-mmproj')?.value?.trim() || '';
        const available_vram_bytes = _presetAvailBytes();
        const currentPreset = _currentModalPreset();
        const isRapidMlx = currentPreset?.backend === 'rapid_mlx';
        const backend = isRapidMlx ? 'rapid_mlx' : 'llama_cpp';

        // Builder item 6: use canonical body builder for cross-surface equality.
        const body = buildEstimateBody({
            backend,
            model_path: modelVal,
            n_ctx: nCtx,
            parallel_slots: parallelSlots,
            ubatch_size: ubatch,
            ctk: backend === 'llama_cpp' ? ctk : undefined,
            ctv: backend === 'llama_cpp' ? ctv : undefined,
            n_cpu_moe: nCpuMoe,
            gpu_layers: Number.isFinite(gpuLayers) ? gpuLayers : -1,
            available_vram_bytes,
            available_ram_bytes: _presetAvailableRamBytes(),
            is_unified_memory: !!isUnified,
            mmproj_path: mmprojPath || null,
        });
        const seq = ++_presetVramSeq;
        try {
            const headers = window.authHeaders
                ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                : { 'Content-Type': 'application/json' };
            const r = await fetch('/api/vram-estimate', { method: 'POST', headers, body: JSON.stringify(body) });
            if (seq !== _presetVramSeq) return;
            const hideStrip = () => { if (strip) strip.style.display = 'none'; };
            if (!r.ok) { hideStrip(); return; }
            const data = await r.json();
            if (data.error) { hideStrip(); return; }
            _renderPresetVram(box, data);
            updatePresetMlockWarning(data);
        } catch { if (seq === _presetVramSeq && strip) strip.style.display = 'none'; }
    }, 350);
}

function _renderPresetVram(el, data) {
    const fmt = b => {
        const gib = b / (1024 ** 3);
        return gib >= 1 ? gib.toFixed(1) + ' GiB' : (b / (1024 ** 2)).toFixed(0) + ' MiB';
    };
    const avail   = data.available_bytes || 0;  // budget we sent
    const used    = data.total_bytes || 0;       // total model + context size
    const weights = data.weights_bytes || 0;

    // Builder item 6: Rapid-MLX active/retained KV split — distinct totals.
    // For Rapid-MLX with workload_scenario: show active + retained separately.
    // For llama.cpp or legacy calls: unified kv_cache_bytes.
    const isRapidSplit = (data.active_kv_bytes || 0) > 0 && (data.retained_kv_bytes || 0) > 0;
    const activeKV = data.active_kv_bytes || 0;
    const retainedKV = data.retained_kv_bytes || 0;
    const kv = isRapidSplit ? 0 : (data.kv_cache_bytes || 0); // unified KV when no split
    const mmproj  = data.mmproj_bytes || 0;
    const mtp     = data.mtp_bytes || 0;
    const overhead = data.overhead_bytes || 0;
    const linearState = data.linear_attn_state_bytes || 0;
    const tqTransient = data.turboquant_transient_peak_bytes || 0;
    // Phase 6 Part B: prefix cache budget display (informational, not consumed until active).
    const prefixCacheBudget = data.prefix_cache_budget_bytes || 0;

    // Bar 100% = budget so free headroom is visible; fall back to used if no budget
    const barTotal = avail > 0 ? avail : used;
    const free = avail > 0 ? Math.max(0, avail - used) : 0;
    const pct = v => barTotal > 0 ? Math.max(0, Math.min(100, (v / barTotal) * 100)).toFixed(1) + '%' : '0%';

    const rec = data.recommendation || 'fit';
    const recLabel = rec === 'fit' ? 'Fits' : rec === 'tight' ? 'Tight' : 'Risk';
    const recClass = rec === 'fit' ? 'fit' : rec === 'tight' ? 'tight' : 'risk';
    const ramBytes = data.ram_bytes || 0;
    const ramAvail = data.available_ram_bytes || _presetAvailableRamBytes();
    const ramPct = ramAvail > 0
        ? Math.max(0, Math.min(100, (ramBytes / ramAvail) * 100)).toFixed(1) + '%'
        : '0%';
    const ramLabel = ramAvail > 0 ? `${fmt(ramBytes)} / ${fmt(ramAvail)}` : fmt(ramBytes);
    const ramBar = !_presetIsUnified && ramBytes > 0
        ? `<div class="preset-vram-row preset-vram-row--ram">
            <span class="preset-memory-kind">RAM</span>
            <div class="vram-bar${ramBytes > ramAvail && ramAvail > 0 ? ' over-budget' : ''}">
                <div class="vram-segment seg-ram-moe" style="width:${ramPct}" title="CPU model weights"></div>
            </div>
            <span class="launch-card-vram-total">${ramLabel}</span>
        </div>`
        : '';

    const parts = [];
    if (weights > 0) parts.push(`Weights ${fmt(weights)}`);
    if (isRapidSplit) {
        if (activeKV > 0) parts.push(`Active KV ${fmt(activeKV)}`);
        if (retainedKV > 0) parts.push(`Retained KV ${fmt(retainedKV)}`);
    } else if (kv > 0) {
        parts.push(`KV ${fmt(kv)}`);
    }
    if (linearState > 0) parts.push(`Linear attn ${fmt(linearState)}`);
    if (mmproj > 0) parts.push(`mmproj ${fmt(mmproj)}`);
    if (mtp > 0) parts.push(`MTP ${fmt(mtp)}`);
    if (tqTransient > 0) parts.push(`TQ transient ${fmt(tqTransient)}`);
    if (overhead > 0) parts.push(`overhead ${fmt(overhead)}`);
    if (avail > 0 && free > 0) parts.push(`${fmt(free)} budget headroom`);
    // Phase 6 Part B: show prefix cache budget as informational (not consumed until active).
    if (prefixCacheBudget > 0) parts.push(`Prefix cache budget ${fmt(prefixCacheBudget)}`);

    // Show post-load system RAM projection when we have live metrics
    const sys = lastSystemMetrics;
    let systemLine = '';
    if (sys && sys.ram_total_gb > 0 && sys.ram_used_gb > 0 && used > 0 && _presetIsUnified) {
        const usedGib = used / (1024 ** 3);
        const sysGib = sys.ram_used_gb;
        const totalGib = sys.ram_total_gb;
        const afterGib = sysGib + usedGib;
        const pctAfter = Math.round((afterGib / totalGib) * 100);
        const isTight = pctAfter >= 90;
        const wiredGib = sys.memory_wired_gb || 0;
        const mlockOn = document.getElementById('modal-mlock')?.checked;
        const wiredAfter = wiredGib + (mlockOn ? usedGib : 0);
        const wiredNote = mlockOn && wiredAfter > 0
            ? ` · ${wiredAfter.toFixed(1)} GiB wired (mlock)`
            : '';
        // When mlock is on and the projected load would exceed 90% of RAM, append a
        // direct suggestion to disable it so the user sees it without expanding the warning.
        const mlockHint = mlockOn && isTight && _presetIsUnified
            ? ' — disable mlock to avoid wiring all model memory'
            : '';
        systemLine = `<div class="preset-vram-sysram${isTight ? ' preset-vram-sysram--warn' : ''}">` +
            `System RAM: ${sysGib.toFixed(1)} GiB now → ~${afterGib.toFixed(1)} GiB after loading (${pctAfter}% of ${totalGib.toFixed(0)} GiB${wiredNote})${mlockHint}` +
            `</div>`;
    } else if (!_presetIsUnified && (data.ram_bytes || 0) > 0) {
        const ramNeeded = data.ram_bytes || 0;
        const ramAvail = data.available_ram_bytes || _presetAvailableRamBytes();
        const ramOver = ramAvail > 0 && ramNeeded > ramAvail;
        const ramCapacity = ramAvail > 0 ? ` / ${fmt(ramAvail)} system RAM available` : '';
        systemLine = `<div class="preset-vram-sysram${ramOver ? ' preset-vram-sysram--warn' : ''}">` +
            `CPU weights: ${fmt(ramNeeded)}${ramCapacity}` +
            `</div>`;
    }

    // Builder item 6: distinct active/retained segments for Rapid-MLX when applicable.
    const kvSegments = isRapidSplit
        ? `<div class="vram-segment seg-active-kv" style="width:${pct(activeKV)}" title="Active KV Cache"></div>
           <div class="vram-segment seg-retained-kv" style="width:${pct(retainedKV)}" title="Retained KV Cache"></div>`
        : `<div class="vram-segment seg-kv" style="width:${pct(kv)}" title="KV Cache"></div>`;
    const linearSeg = linearState > 0
        ? `<div class="vram-segment seg-overhead" style="width:${pct(linearState)}" title="Linear Attention State"></div>`
        : '';
    const tqSeg = tqTransient > 0
        ? `<div class="vram-segment seg-overhead" style="width:${pct(tqTransient)}" title="TurboQuant Transient"></div>`
        : '';

    // eslint-disable-next-line no-unsanitized/property -- DOMPurify sanitizes the VRAM bar HTML
    el.innerHTML = window.DOMPurify.sanitize(`
        <div class="preset-vram-row">
            <span class="preset-memory-kind">${_presetIsUnified ? 'MEM' : 'VRAM'}</span>
            <div class="vram-bar">
                <div class="vram-segment seg-weights" style="width:${pct(weights)}" title="Weights"></div>
                ${kvSegments}
                <div class="vram-segment seg-mmproj" style="width:${pct(mmproj)}" title="Vision Projector"></div>
                <div class="vram-segment seg-mtp" style="width:${pct(mtp)}" title="MTP Heads"></div>
                ${linearSeg}
                ${tqSeg}
                <div class="vram-segment seg-overhead" style="width:${pct(overhead)}" title="Overhead"></div>
                <div class="vram-segment seg-free" style="width:${pct(free)}" title="Budget Headroom"></div>
            </div>
            <span class="launch-card-vram-total">~${fmt(used)}</span>
            <span class="preset-vram-badge preset-vram-badge--${recClass}">${recLabel}</span>
        </div>
        ${ramBar}
        ${parts.length ? `<div class="preset-vram-breakdown">${parts.join(' · ')}</div>` : ''}
        ${systemLine}
    `);
    el.style.display = '';
}

// Empirically auto-tune n_cpu_moe for the preset's model via llama-bench.
async function autoTunePreset() {
    const statusEl = document.getElementById('preset-moe-autotune-status');
    const modelVal = document.getElementById('modal-model-path')?.value.trim() || '';
    if (!modelVal.toLowerCase().endsWith('.gguf')) {
        // The sweep launches llama-bench on a local file, not an HF repo id.
        showToast('Sweep needs a local .gguf model file', 'warn');
        return;
    }
    const body = {
        name: modelVal.split(/[/\\]/).pop() || '',
        param_b: _parseParamB(modelVal),
        model_path: modelVal,
        ngl: -1, // -1 → -ngl all
        ctk: document.getElementById('modal-ctk')?.value || 'q8_0',
        ctv: document.getElementById('modal-ctv')?.value || 'q8_0',
        flash_attn: true,
        is_unified_memory: !!(await _ensureUnifiedFlag()),
        verify: true,
    };
    if (statusEl) statusEl.innerHTML = '<span class="moe-autotune-spinner"></span>Running sweep… this can take a few minutes';
    try {
        const data = await requestNcpuMoeTune(body);
        if (data.error) { if (statusEl) statusEl.textContent = data.error; return; }
        const rec = data.recommended_n_cpu_moe;
        const input = document.getElementById('modal-n-cpu-moe');
        if (input) { input.value = String(rec); input.dispatchEvent(new Event('change', { bubbles: true })); }
        if (statusEl) statusEl.textContent = `Best: ${rec} (measured)`;
    } catch {
        if (statusEl) statusEl.textContent = 'Auto-tune failed';
    }
}

export function openPresetModal(mode, section, seedPreset = null) {
    const modal = document.getElementById('preset-modal');
    const title = document.getElementById('modal-title');
    const subtitle = document.getElementById('preset-editor-subtitle');
    const form = document.getElementById('preset-form');
    form.reset();
    clearFieldErrors();
    newPresetSeed = mode === 'new' && seedPreset ? structuredClone(seedPreset) : null;
    _presetRapidMlxProfile = null;

    if (mode === 'edit') {
        const id = document.getElementById('preset-select').value;
        const p = sessionState.presets.find(pr => pr.id === id);
        if (!p) { showToast('No preset selected', 'warn'); return; }
        title.textContent = 'Edit Preset';
        if (subtitle) subtitle.textContent = p.name;
        setVal('modal-preset-id', p.id);
        // Model & Memory
        setVal('modal-name', p.name);
        // Prefill model field:
        // - If model_path present, treat as local file.
        // - Else if hf_repo present, treat as HF repo.
        const modelValue = presetModelSource(p);
        setVal('modal-model-path', modelValue);
        _renderPresetArchInfo(p);
        setVal('modal-alias', p.alias || '');
        // Fetch live Rapid-MLX profile when editing a Rapid-MLX preset
        _schedulePresetRapidMlxProfile();
        numOrEmpty('modal-gpu-layers', p.gpu_layers);
        setChk('modal-no-mmap', p.no_mmap);
        setChk('modal-mlock', p.mlock);
        // Context & KV
        setVal('modal-context-size', p.context_size || 128000);
        setVal('modal-ctk', p.ctk || 'q8_0');
        const pillsContainer = document.getElementById('preset-context-pills'); if (pillsContainer) pillsContainer.style.display = 'flex';
        _renderContextPills(mode, section);
        setVal('modal-ctv', p.ctv || 'f16');
        setOpt('modal-flash-attn', p.flash_attn);
        // Batching
        setVal('modal-batch-size', p.batch_size || 2048);
        setVal('modal-ubatch-size', p.ubatch_size || p.batch_size || 2048);
        setVal('modal-parallel-slots', p.parallel_slots || 1);
        const cacheIdleHint = document.getElementById('cache-idle-slots-hint');
        if (cacheIdleHint) cacheIdleHint.style.display = (p.parallel_slots || 1) > 1 ? '' : 'none';
        setOpt('modal-prio', p.prio != null ? String(p.prio) : '');
        setOpt('modal-prio-batch', p.prio_batch != null ? String(p.prio_batch) : '');
        setOpt('modal-cache-idle-slots', p.cache_idle_slots == null ? '' : p.cache_idle_slots ? 'true' : 'false');
        numOrEmpty('modal-threads', p.threads);
        numOrEmpty('modal-threads-batch', p.threads_batch);
        // Generation
        numOrEmpty('modal-temperature', p.temperature);
        numOrEmpty('modal-top-p', p.top_p);
        numOrEmpty('modal-top-k', p.top_k);
        numOrEmpty('modal-min-p', p.min_p);
        numOrEmpty('modal-repeat-penalty', p.repeat_penalty);
        numOrEmpty('modal-presence-penalty', p.presence_penalty);
        setOpt('modal-enable-thinking', p.enable_thinking == null ? '' : String(!!p.enable_thinking));
        setOpt('modal-preserve-thinking', p.preserve_thinking == null ? '' : String(!!p.preserve_thinking));
        setOpt('modal-tool-call-format', p.tool_call_format || '');
        setOpt('modal-reasoning', p.reasoning || '');
        numOrEmpty('modal-reasoning-budget', p.reasoning_budget);
        setVal('modal-reasoning-budget-message', (p.reasoning_budget_message || '').replace(/\n/g, '\\n'));
        // GPU
        setVal('modal-tensor-split', p.tensor_split);
        setOpt('modal-split-mode', p.split_mode);
        numOrEmpty('modal-main-gpu', p.main_gpu);
        // Threading
        numOrEmpty('modal-threads', p.threads);
        numOrEmpty('modal-threads-batch', p.threads_batch);
        // n_cpu_moe: only for MoE / hybrid-moE with experts
        const moeRow = document.getElementById('modal-n-cpu-moe')?.closest('.pe-field') ||
                       document.getElementById('modal-n-cpu-moe')?.parentElement;
        const moeAutotuneBtn = document.getElementById('preset-moe-autotune-verify');
        if (moeRow) {
            moeRow.style.display = isMoEEligible(p) ? '' : 'none';
        }
        if (moeAutotuneBtn) {
            moeAutotuneBtn.style.display = isMoEEligible(p) ? '' : 'none';
        }
        const moeLayersHint = document.getElementById('modal-n-cpu-moe-layers');
        if (isMoEEligible(p)) {
            numOrEmpty('modal-n-cpu-moe', p.n_cpu_moe);
            const nMoeEl = document.getElementById('modal-n-cpu-moe');
            // Bound the input to the model's layer count (from backend GGUF metadata).
            if (nMoeEl && p.block_count != null) nMoeEl.max = p.block_count;
            else if (nMoeEl) nMoeEl.removeAttribute('max');
            if (moeLayersHint) {
                if (p.block_count != null) {
                    // Real measured routed-expert bytes per MoE layer (VRAM freed per offload).
                    const freed = p.expert_bytes_per_layer
                        ? ` Each offloaded layer frees ~${_formatLayerBytes(p.expert_bytes_per_layer)} of VRAM.`
                        : '';
                    moeLayersHint.textContent = `This model has ${p.block_count} expert layers — values are clamped to 0–${p.block_count}.${freed}`;
                    moeLayersHint.style.display = '';
                } else {
                    moeLayersHint.style.display = 'none';
                }
            }
        } else {
            const el = document.getElementById('modal-n-cpu-moe');
            if (el) { el.value = ''; el.removeAttribute('max'); }
            if (moeLayersHint) moeLayersHint.style.display = 'none';
        }
        // Bound --gpu-layers (-ngl) to the layer count for all models (the primary
        // GPU-offload knob for dense models, where there are no experts to offload).
        const nglEl = document.getElementById('modal-gpu-layers');
        if (nglEl) {
            if (p.block_count != null) nglEl.max = p.block_count;
            else nglEl.removeAttribute('max');
        }
        const nglHint = document.getElementById('modal-gpu-layers-layers');
        if (nglHint) {
            if (p.block_count != null) {
                const off = Math.max(0, p.block_count - 4);
                // Real measured per-layer weight bytes (VRAM each GPU layer occupies).
                const perLayer = p.bytes_per_layer
                    ? ` (~${_formatLayerBytes(p.bytes_per_layer)} of VRAM each)`
                    : '';
                nglHint.textContent = `This model has ${p.block_count} layers${perLayer}. Enter 0–${p.block_count}: layers above your value stay on CPU/RAM (e.g. ${off} keeps 4 layers off the GPU).`;
                nglHint.style.display = '';
            } else {
                nglHint.style.display = 'none';
            }
        }
        // Rope
        setOpt('modal-rope-scaling', p.rope_scaling);
        numOrEmpty('modal-rope-freq-base', p.rope_freq_base);
        numOrEmpty('modal-rope-freq-scale', p.rope_freq_scale);
        // Spec decoding — use spec_type; fallback: ngram_spec bool → ngram-mod
        const specType = p.spec_type || (p.ngram_spec ? 'ngram-mod' : '');
        setOpt('modal-spec-type', specType);
        numOrEmpty('modal-spec-ngram-size', p.spec_ngram_size);
        numOrEmpty('modal-draft-min', p.draft_min);
        numOrEmpty('modal-draft-max', p.draft_max);
        numOrEmpty('modal-spec-draft-n-max', p.spec_draft_n_max);
        numOrEmpty('modal-spec-draft-n-min', p.spec_draft_n_min);
        numOrEmpty('modal-spec-draft-p-min', p.spec_draft_p_min);
        setVal('modal-spec-draft-type-k', p.spec_draft_type_k || '');
        setVal('modal-spec-draft-type-v', p.spec_draft_type_v || '');
        setVal('modal-draft-model', p.draft_model);
        numOrEmpty('modal-spec-draft-ngl', p.spec_draft_ngl);
        setVal('modal-spec-draft-device', p.spec_draft_device ?? '');
        numOrEmpty('modal-spec-draft-n-cpu-moe', p.spec_draft_n_cpu_moe);
        const specDefaultEl = document.getElementById('modal-spec-default');
        if (specDefaultEl) specDefaultEl.checked = !!p.spec_default;
        _toggleSpecFields(specType);
        // Context extras
        setOpt('modal-kv-unified', p.kv_unified == null ? '' : String(p.kv_unified));
        numOrEmpty('modal-cache-ram-mib', p.cache_ram_mib);
        // Model extras
        setVal('modal-mmproj', p.mmproj || '');
        _toggleVisionTokens(!!p.mmproj);
        setVal('modal-chat-template-file', p.chat_template_file || '');
        // Advanced
        setOpt('modal-bind-host', p.bind_host || '');
        numOrEmpty('modal-port', p.backend === 'rapid_mlx' ? p.rapid_mlx?.port : p.port);
        setOpt('modal-rapid-enable-thinking', p.rapid_mlx?.enable_thinking == null ? '' : String(!!p.rapid_mlx.enable_thinking));
        setOpt('modal-rapid-reasoning-effort', p.rapid_mlx?.reasoning_effort || '');
        // Phase 6 Part B: prefix cache enabled checkbox (safe default: false).
        const prefixCacheEnabled = p.rapid_mlx?.prefix_cache_enabled ?? false;
        if (document.getElementById('modal-rapid-prefix-cache-enabled')) {
            document.getElementById('modal-rapid-prefix-cache-enabled').checked = prefixCacheEnabled;
        }
        // Phase 7 Part A: Rapid-MLX advanced controls (kv_cache_dtype, turboquant_mode, workload_scenario, reasoning_mode).
        setOpt('modal-rapid-kv-cache-dtype', p.rapid_mlx?.kv_cache_dtype || '');
        setOpt('modal-rapid-turboquant-mode', p.rapid_mlx?.turboquant_mode || '');
        setOpt('modal-rapid-workload-scenario', p.rapid_mlx?.workload_scenario || '');
        const reasoningModeChecked = !!p.rapid_mlx?.reasoning_mode;
        if (document.getElementById('modal-rapid-reasoning-mode')) {
            document.getElementById('modal-rapid-reasoning-mode').checked = reasoningModeChecked;
        }
        setVal('modal-api-key', p.api_key || '');
        numOrEmpty('modal-max-tokens', p.max_tokens);
        numOrEmpty('modal-seed', p.seed);
        setOpt('modal-fit-enabled', p.fit_enabled == null ? '' : String(p.fit_enabled));
        setVal('modal-fit-target', p.fit_target || '');
        _toggleFitTarget(p.fit_enabled === true);
        setVal('modal-system-prompt-file', p.system_prompt_file);
        setStructuredOutputMode(p.json_schema ? 'json_schema' : p.grammar ? 'grammar' : '');
        setVal('modal-grammar', p.grammar || '');
        setVal('modal-json-schema', p.json_schema || '');
        setVal('modal-extra-args', p.extra_args);
        numOrEmpty('modal-spec-draft-p-split', p.spec_draft_p_split);
        numOrEmpty('modal-image-min-tokens', qwenVLImageTokens(p).min_tokens);
        numOrEmpty('modal-image-max-tokens', p.image_max_tokens);
        _configureBackendPresetEditor(p);
    } else {
        title.textContent = 'New Preset';
        if (subtitle) subtitle.textContent = newPresetSeed?.backend === 'rapid_mlx'
            ? 'Rapid-MLX model profile'
            : 'New model profile';
        setVal('modal-preset-id', '');
        setVal('modal-name', newPresetSeed?.name || '');
        setVal('modal-model-path', presetModelSource(newPresetSeed));
        setVal('modal-context-size', 128000);
        setVal('modal-ctk', 'q8_0');
        setVal('modal-ctv', 'f16');
        setVal('modal-batch-size', 2048);
        setVal('modal-ubatch-size', 2048);
        setVal('modal-parallel-slots', 1);
        numOrEmpty('modal-port', newPresetSeed?.backend === 'rapid_mlx'
            ? newPresetSeed.rapid_mlx?.port
            : newPresetSeed?.port);
        _toggleFitTarget(false);
        _toggleSpecFields('');
        setStructuredOutputMode('');
        _configureBackendPresetEditor(newPresetSeed);
    }

    const presetModel = document.getElementById('modal-model-path')?.value.trim();
    // New preset: fill empty sampling fields + show preset pills.
    // Edit preset: only show preset pills (don't overwrite the user's saved values).
    if (presetModel) _suggestGenerationDefaults(presetModel, mode !== 'edit');
    else _renderGenerationPresetPills([]);

    // Reset change-summary state
    _hideSummary();

    // Show "Delete preset" button only when editing
    const deleteBtn = document.getElementById('preset-modal-delete');
    if (mode === 'edit') {
        if (deleteBtn) deleteBtn.style.display = '';
    } else {
        if (deleteBtn) deleteBtn.style.display = 'none';
    }

    modal.classList.add('open');
    // Navigate to specified section, or reset to first section
    const targetSection = section || 'model';
    document.querySelector(`.preset-nav-item[data-section="${targetSection}"]`)?.click();
    const body = modal.querySelector('.modal-body');
    if (body) body.scrollTop = 0;

    // Apple Silicon-aware hints for Threads fields
    _refreshPresetThreadsHints();
    if (!lastSystemMetrics) _fetchSystemInfoAndRefreshPresetHints();

    // Config-time performance advisor and VRAM estimate
    updatePresetAdvisor();
    updatePresetVram();

    // Focus first interactive element in the modal
    const firstFocusable = modal.querySelector('.preset-nav-item, button, input, select, textarea');
    if (firstFocusable) firstFocusable.focus();

    // Escape key handler to close modal
    function setupEscapeHandler() {
        window.addEventListener('keydown', function escHandler(e) {
            if (e.key === 'Escape') {
                window.removeEventListener('keydown', escHandler);
                closePresetModal();
            }
        });
    }
    setupEscapeHandler();
}

export function closePresetModal() {
    document.getElementById('preset-modal').classList.remove('open');
    newPresetSeed = null;
}

// ── Presets Panel ──────────────────────────────────────────────────────────────

export function openPresetsPanel() {
    const overlay = document.getElementById('presets-panel-overlay');
    if (!overlay) return;
    overlay.style.display = '';
    overlay.classList.add('open');
    _renderPresetsPanel();
    document.getElementById('presets-panel-wizard-btn')?.addEventListener('click', () => {
        closePresetsPanel();
        // Route through the Router so the URL, Back/Forward, and wizard step state
        // stay in sync instead of opening the wizard out-of-band.
        import('./router.js').then(({ default: Router }) => Router.navigate('/spawn'));
    }, { once: true });
}

export function closePresetsPanel() {
    const overlay = document.getElementById('presets-panel-overlay');
    if (!overlay) return;
    overlay.classList.remove('open');
    overlay.style.display = 'none';
}

function _renderPresetsPanel() {
    const body = document.getElementById('presets-panel-body');
    if (!body) return;
    body.innerHTML = '';

    const presets = (sessionState.presets || []).filter(presetModelSource);
    if (!presets.length) {
        const empty = document.createElement('div');
        empty.className = 'presets-panel-empty';
        empty.textContent = 'No presets yet. Use the Setup wizard to create one.';
        body.appendChild(empty);
        return;
    }

    presets.forEach(preset => {
        const card = document.createElement('div');
        card.className = 'preset-panel-card';

        const icon = document.createElement('div');
        icon.className = 'preset-panel-card-icon';
        icon.textContent = '🧠';
        card.appendChild(icon);

        const info = document.createElement('div');
        info.className = 'preset-panel-card-info';

        const name = document.createElement('div');
        name.className = 'preset-panel-card-name';
        name.textContent = preset.name || 'Unnamed preset';
        info.appendChild(name);

        const metaParts = [];
        const rapidMlx = preset.rapid_mlx;
        if (preset.backend === 'rapid_mlx') {
            const modelIdentity = rapidMlx?.model_source_view?.canonical_identity
                || rapidMlx?.model_source_view?.display_name
                || rapidMlx?.model_path;
            if (modelIdentity) {
                metaParts.push(modelIdentity.split(/[/\\]/).pop() || modelIdentity);
                metaParts.push('Rapid-MLX');
            }
        } else if (preset.model_path) metaParts.push(preset.model_path.split(/[/\\]/).pop() || preset.model_path);
        else if (preset.hf_repo) metaParts.push(preset.hf_repo);
        if (preset.bind_host === '0.0.0.0') metaParts.push('LAN');
        if (preset.context_size) metaParts.push(`${Math.round(preset.context_size / 1024)}k context`);
        const ctk = preset.ctk || 'q8_0';
        const ctv = preset.ctv || 'q8_0';
        const kvText = `KV cache: ${ctk}/${ctv}`;
        if (ctk || ctv) metaParts.push(kvText);

        const meta = document.createElement('div');
        meta.className = 'preset-panel-card-meta';
        meta.textContent = metaParts.join(' · ') || 'No details';
        meta.title = metaParts.join(' · ') +
          (ctk || ctv ? ' · KV cache precision (how accurately the model stores past tokens). q8_0 is recommended for most users.' : '');
        info.appendChild(meta);
        card.appendChild(info);

        const actions = document.createElement('div');
        actions.className = 'preset-panel-card-actions';

        const startBtn = document.createElement('button');
        startBtn.type = 'button';
        startBtn.className = 'btn-preset-quick-start';
        startBtn.textContent = '▶ Quick Start';
        startBtn.title = 'Spawn this server configuration now';
        startBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            syncSelectedPresetSelection(preset.id, { userIntent: true, persist: true });
            closePresetsPanel();
            import('./attach-detach.js').then(({ doStartFromSetup }) => {
                doStartFromSetup();
            });
        });
        actions.appendChild(startBtn);

        const delBtn = document.createElement('button');
        delBtn.type = 'button';
        delBtn.className = 'btn-preset-delete';
        delBtn.title = 'Delete preset';
        delBtn.textContent = '✕';
        delBtn.addEventListener('click', async (e) => {
            e.stopPropagation();
            showToastWithActions(
                'Delete preset',
                'warning',
                `Delete "${preset.name}"? This cannot be undone.`,
                [
                    { id: 'cancel', label: 'Cancel', primary: false },
                    {
                        id: 'delete',
                        label: 'Delete',
                        primary: true,
                        handler: async () => {
                            try {
                                const headers = window.authHeaders ? { ...window.authHeaders() } : {};
                                const resp = await fetch(`/api/presets/${preset.id}`, { method: 'DELETE', headers });
                                if (resp.ok) {
                                    await loadPresets();
                                    _renderPresetsPanel();
                                }
                            } catch (err) {
                                console.error('Delete preset failed:', err);
                            }
                        }
                    }
                ]
            );
        });
        actions.appendChild(delBtn);

        card.appendChild(actions);

        // Top-right trash icon (subtle)
        const trashBtn = document.createElement('button');
        trashBtn.type = 'button';
        trashBtn.className = 'preset-panel-card-trash';
        trashBtn.title = 'Delete preset';
        trashBtn.innerHTML =
            '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" ' +
            'stroke-width="2" stroke-linecap="round" stroke-linejoin="round">' +
            '<path d="M3 6h18"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>' +
            '<path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/>' +
            '<line x1="10" y1="11" x2="10" y2="17"/><line x1="14" y1="11" x2="14" y2="17"/>' +
            '</svg>';
        trashBtn.addEventListener('click', async (e) => {
            e.stopPropagation();
            showToastWithActions(
                'Delete preset',
                'warning',
                `Delete "${preset.name}"? This cannot be undone.`,
                [
                    { id: 'cancel', label: 'Cancel', primary: false },
                    {
                        id: 'delete',
                        label: 'Delete',
                        primary: true,
                        handler: async () => {
                            try {
                                const headers = window.authHeaders ? { ...window.authHeaders() } : {};
                                const resp = await fetch(`/api/presets/${preset.id}`, { method: 'DELETE', headers });
                                if (resp.ok) {
                                    await loadPresets();
                                    _renderPresetsPanel();
                                }
                            } catch (err) {
                                console.error('Delete preset failed:', err);
                            }
                        }
                    }
                ]
            );
        });
        card.appendChild(trashBtn);

        body.appendChild(card);
    });
}

// ── Change summary ────────────────────────────────────────────────────────────

function _toggleFitTarget(enabled) {
    const wrap = document.getElementById('modal-fit-target-wrap');
    if (wrap) wrap.style.display = enabled ? '' : 'none';
}

function _toggleVisionTokens(enabled) {
    const wrap = document.getElementById('vision-tokens-wrap');
    if (wrap) wrap.style.display = enabled ? '' : 'none';
}

function _ensureUbatchForImageTokens(imageMaxTokens) {
    // Gemma4-specific: non-causal vision attention requires all image tokens in a single ubatch.
    // If image-max-tokens > ubatch, we auto-raise ubatch to avoid crashes.
    // This constraint is Gemma4-only; other models are unaffected.
    if (!imageMaxTokens || imageMaxTokens <= 0) return;

    const modelVal = (document.getElementById('modal-model-path')?.value || '').toLowerCase();
    const repoVal = (document.getElementById('modal-hf-repo')?.value || '').toLowerCase();
    const isGemma4 = modelVal.includes('gemma') || repoVal.includes('gemma');
    if (!isGemma4) return;

    const ubInput = document.getElementById('modal-ubatch-size');
    const hintEl = document.getElementById('vision-ubatch-hint');
    if (!ubInput) return;
    const currentUbatch = Math.max(1, Number(ubInput.value || 0));
    if (imageMaxTokens <= currentUbatch) {
        if (hintEl) { hintEl.style.display = 'none'; hintEl.textContent = ''; }
        return;
    }
    const prev = currentUbatch;
    ubInput.value = imageMaxTokens;
    if (hintEl) {
        hintEl.textContent = `Micro-batch increased from ${prev} to ${imageMaxTokens} (required for Gemma4: all image tokens must fit in one batch).`;
        hintEl.style.display = '';
    }
}

function _toggleSpecFields(specType) {
    const hasNgram = specType.includes('ngram');
    const hasMtp   = specType.includes('draft-mtp');
    const hasDraft = specType === 'draft-model';
    const hasAny   = !!specType;
    const ngWrap     = document.getElementById('spec-ngram-params-wrap');
    const mtpWrap    = document.getElementById('spec-mtp-wrap');
    const dmWrap     = document.getElementById('spec-draft-model-wrap');
    const hwWrap     = document.getElementById('spec-draft-hw-wrap');
    const defWrap    = document.getElementById('spec-default-wrap');
    const hint       = document.getElementById('spec-type-hint');
    if (ngWrap)  ngWrap.style.display  = hasNgram ? '' : 'none';
    if (mtpWrap) mtpWrap.style.display = hasMtp   ? '' : 'none';
    // Show draft-model input for both draft-model and MTP with external assistant.
    if (dmWrap)  dmWrap.style.display  = (hasDraft || hasMtp) ? '' : 'none';
    // Draft hardware (ngl, device, cpu-moe) only relevant for an external draft file.
    if (hwWrap)  hwWrap.style.display  = hasDraft ? '' : 'none';
    // spec-default checkbox appears whenever any spec type is active.
    if (defWrap) defWrap.style.display = hasAny ? '' : 'none';

    // Auto-populate draft model path from modal-draft-model if available and empty
    const draftInput = document.getElementById('modal-draft-model');
    if (dmWrap && (hasDraft || hasMtp)) {
        if (draftInput && !draftInput.value.trim()) {
            // Try to get from session state preset if available
            const currentPreset = sessionState?.presets?.find(p =>
                document.getElementById('modal-preset-id')?.value === p.id
            );
            if (currentPreset && currentPreset.draft_model) {
                draftInput.value = currentPreset.draft_model;
            }
        }
    }

    const hints = {
        'ngram-mod': 'Best for server deployments with multiple slots. Uses a shared hash pool — requires no extra files or VRAM.',
        'ngram-simple': 'Lightest-weight option. Scans recent history for matching n-grams. Good for single-slot use.',
        'ngram-map-k': 'Hash-map based pattern matching. Works well for repetitive content like code or structured data.',
        'ngram-map-k4v': 'Experimental. Tracks up to 4 candidate tokens per n-gram key. May outperform ngram-map-k on long repetitive content.',
        'draft-mtp,ngram-mod': 'MTP with n-gram fallback. If your model requires an external assistant (e.g. Gemma4-style), set the Draft Model path below. MTP + ngram-mod forces --parallel 1.',
        'draft-mtp': 'Pure MTP with no n-gram fallback. If your model requires an external assistant (e.g. Gemma4-style), set the Draft Model path below. Forces --parallel 1.',
    };
    if (hint) {
        const text = hints[specType] || '';
        hint.textContent = text;
        hint.style.display = text ? '' : 'none';
    }
}

function _hideSummary() {
    const summary = document.getElementById('preset-change-summary');
    const back = document.getElementById('preset-modal-back');
    const cancel = document.getElementById('preset-modal-cancel');
    const saveBtn = document.getElementById('btn-modal-save');
    if (summary) summary.style.display = 'none';
    if (back) back.style.display = 'none';
    if (cancel) cancel.style.display = '';
    if (saveBtn) { saveBtn.textContent = 'Save'; saveBtn.dataset.confirmed = ''; }
}

function _configureBackendPresetEditor(preset) {
    const modal = document.getElementById('preset-modal');
    const isRapid = preset?.backend === 'rapid_mlx';
    modal?.classList.toggle('preset-editor--rapid-mlx', isRapid);

    const modelLabel = document.querySelector('label[for="modal-model-path"]');
    const modelInput = document.getElementById('modal-model-path');
    const portLabel = document.querySelector('label[for="modal-port"]');
    const modelSection = modal?.querySelector('.preset-editor-section[data-section="model"]');
    const modelTitle = modelSection?.querySelector('.pe-section-title');
    const modelDescription = modelSection?.querySelector('.pe-section-desc');
    const advancedSection = modal?.querySelector('.preset-editor-section[data-section="advanced"]');
    const advancedNavLabel = modal?.querySelector('.preset-nav-item[data-section="advanced"] .pni-label');
    const advancedTitle = advancedSection?.querySelector('.pe-section-title');
    const advancedDescription = advancedSection?.querySelector('.pe-section-desc');
    if (modelTitle) modelTitle.textContent = isRapid ? 'Rapid-MLX Model' : 'Model & Memory';
    if (modelDescription) {
        modelDescription.textContent = isRapid
            ? 'MLX model directory or Hugging Face repository'
            : 'Model file path, GPU offloading, memory locking';
    }
    if (advancedTitle) advancedTitle.textContent = isRapid ? 'Rapid-MLX Server' : 'Advanced';
    if (advancedNavLabel) advancedNavLabel.textContent = isRapid ? 'Server' : 'Advanced';
    if (advancedDescription) {
        advancedDescription.textContent = isRapid
            ? 'Local server port'
            : 'Server access, fit-to-VRAM, seed, and extra CLI flags';
    }
    if (modelLabel) {
        modelLabel.firstChild.textContent = isRapid ? 'MLX Model Path ' : 'Model Path ';
        modelLabel.title = isRapid
            ? 'Local MLX model directory or Hugging Face MLX repository id'
            : 'Absolute path to the .gguf model file on disk';
    }
    if (modelInput) {
        modelInput.placeholder = isRapid ? 'mlx-community/model-name or /path/to/model' : '';
    }
    if (portLabel) {
        portLabel.title = isRapid
            ? 'TCP port Rapid-MLX listens on.'
            : 'TCP port llama-server listens on. Default 8001. Change if you run multiple servers simultaneously.';
    }
}

function _buildFormPreset(existing) {
    if (existing.backend === 'rapid_mlx') {
        const rapidPort = intOrNull('modal-port');
        return {
            ...existing,
            name: strVal('modal-name'),
            port: rapidPort,
            rapid_mlx: existing.rapid_mlx ? {
                ...existing.rapid_mlx,
                model_path: strVal('modal-model-path'),
                port: rapidPort,
                ...(function() {
                    const et = nullableBoolOpt('modal-rapid-enable-thinking');
                    const re = strVal('modal-rapid-reasoning-effort');
                    const out = {};
                    if (et != null) out.enable_thinking = et;
                    if (re) out.reasoning_effort = re;
                    // Phase 6 Part B: prefix cache enabled toggle.
                    const pceInput = document.getElementById('modal-rapid-prefix-cache-enabled');
                    if (pceInput) out.prefix_cache_enabled = pceInput.checked;
                    // Phase 7 Part A: Rapid-MLX advanced controls.
                    const kvDtype = strVal('modal-rapid-kv-cache-dtype');
                    const tqMode = strVal('modal-rapid-turboquant-mode');
                    const wlScenario = strVal('modal-rapid-workload-scenario');
                    const rmInput = document.getElementById('modal-rapid-reasoning-mode');
                    if (kvDtype) out.kv_cache_dtype = kvDtype;
                    if (tqMode) out.turboquant_mode = tqMode;
                    if (wlScenario) out.workload_scenario = wlScenario;
                    if (rmInput) out.reasoning_mode = rmInput.checked;
                    return out;
                })(),
            } : null,
        };
    }
    const modelSource = normalizeModelSourceInput(strVal('modal-model-path'));
    const fitEnabled = nullableBoolOpt('modal-fit-enabled');
    return {
        // Spread ALL existing fields first — preserves wizard-set values not shown in the editor
        ...existing,
        // Override only what the editor manages
        name: strVal('modal-name'),
        model_path: modelSource.model_path,
        hf_repo: modelSource.hf_repo,
        alias: strVal('modal-alias') || null,
        mmproj: strVal('modal-mmproj') || null,
        chat_template_file: strVal('modal-chat-template-file') || null,
        gpu_layers: intOrNull('modal-gpu-layers'),
        no_mmap: document.getElementById('modal-no-mmap').checked,
        mlock: document.getElementById('modal-mlock').checked,
        context_size: parseInt(document.getElementById('modal-context-size').value) || 128000,
        ctk: strVal('modal-ctk') || 'q8_0',
        ctv: strVal('modal-ctv') || 'f16',
        flash_attn: strVal('modal-flash-attn'),
        kv_unified: nullableBoolOpt('modal-kv-unified'),
        cache_ram_mib: intOrNull('modal-cache-ram-mib'),
        batch_size: parseInt(document.getElementById('modal-batch-size').value) || 2048,
        ubatch_size: parseInt(document.getElementById('modal-ubatch-size').value) || 2048,
        parallel_slots: parseInt(document.getElementById('modal-parallel-slots').value) || 1,
        prio: intOrNull('modal-prio'),
        prio_batch: intOrNull('modal-prio-batch'),
        cache_idle_slots: nullableBoolOpt('modal-cache-idle-slots'),
        threads: intOrNull('modal-threads'),
        threads_batch: intOrNull('modal-threads-batch'),
        temperature: floatOrNull('modal-temperature'),
        top_p: floatOrNull('modal-top-p'),
        top_k: intOrNull('modal-top-k'),
        min_p: floatOrNull('modal-min-p'),
        repeat_penalty: floatOrNull('modal-repeat-penalty'),
        presence_penalty: floatOrNull('modal-presence-penalty'),
        enable_thinking: nullableBoolOpt('modal-enable-thinking'),
        preserve_thinking: nullableBoolOpt('modal-preserve-thinking'),
        tool_call_format: strVal('modal-tool-call-format') || null,
        reasoning: strVal('modal-reasoning') || null,
        reasoning_budget: intOrNull('modal-reasoning-budget'),
        reasoning_budget_message: (document.getElementById('modal-reasoning-budget-message').value || '').replace(/\\n/g, '\n') || null,
        tensor_split: strVal('modal-tensor-split'),
        split_mode: strVal('modal-split-mode'),
        main_gpu: intOrNull('modal-main-gpu'),
        n_cpu_moe: intOrNull('modal-n-cpu-moe'),
        rope_scaling: strVal('modal-rope-scaling'),
        rope_freq_base: floatOrNull('modal-rope-freq-base'),
        rope_freq_scale: floatOrNull('modal-rope-freq-scale'),
        spec_type: strVal('modal-spec-type') || null,
        spec_default: document.getElementById('modal-spec-default')?.checked || false,
        ngram_spec: false,
        spec_ngram_size: intOrNull('modal-spec-ngram-size'),
        draft_min: intOrNull('modal-draft-min'),
        draft_max: intOrNull('modal-draft-max'),
        spec_draft_n_max: intOrNull('modal-spec-draft-n-max'),
        spec_draft_n_min: intOrNull('modal-spec-draft-n-min'),
        spec_draft_p_min: floatOrNull('modal-spec-draft-p-min'),
        spec_draft_type_k: valOrNull('modal-spec-draft-type-k'),
        spec_draft_type_v: valOrNull('modal-spec-draft-type-v'),
        draft_model: strVal('modal-draft-model'),
        spec_draft_ngl: intOrNull('modal-spec-draft-ngl'),
        spec_draft_device: valOrNull('modal-spec-draft-device'),
        spec_draft_n_cpu_moe: intOrNull('modal-spec-draft-n-cpu-moe'),
        spec_draft_cpu_moe: (intOrNull('modal-spec-draft-n-cpu-moe') ?? 0) > 0,
        bind_host: strVal('modal-bind-host') || null,
        port: intOrNull('modal-port'),
        api_key: strVal('modal-api-key') || null,
        max_tokens: intOrNull('modal-max-tokens'),
        seed: intOrNull('modal-seed'),
        fit_enabled: fitEnabled,
        fit_target: fitEnabled === true ? (strVal('modal-fit-target') || null) : null,
        system_prompt_file: strVal('modal-system-prompt-file'),
        grammar: getStructuredOutputMode() === 'grammar' ? (document.getElementById('modal-grammar').value.trim() || null) : null,
        json_schema: getStructuredOutputMode() === 'json_schema' ? (document.getElementById('modal-json-schema').value.trim() || null) : null,
        extra_args: strVal('modal-extra-args'),
        spec_draft_p_split: floatOrNull('modal-spec-draft-p-split'),
        image_min_tokens: intOrNull('modal-image-min-tokens'),
        image_max_tokens: intOrNull('modal-image-max-tokens'),
    };
}
const CHANGE_LABELS = {
    name: 'Name', model_path: 'Model (local path or HF repo)', hf_repo: 'HuggingFace Repo',
    alias: 'Server Alias', mmproj: 'Multimodal Projector', chat_template_file: 'Chat Template File',
    image_min_tokens: 'Vision Min Tokens', image_max_tokens: 'Vision Max Tokens',
    gpu_layers: 'GPU Layers', no_mmap: 'no-mmap', mlock: 'mlock',
    context_size: 'Context Size', ctk: 'KV Key Type', ctv: 'KV Value Type',
    flash_attn: 'Flash Attn', kv_unified: 'KV Unified', cache_ram_mib: 'Prefix Cache RAM',
    fit_enabled: 'Fit to VRAM', fit_target: 'Fit Target',
    batch_size: 'Batch Size', ubatch_size: 'Micro-batch', parallel_slots: 'Parallel Slots',
    prio: 'Thread Priority', prio_batch: 'Batch Priority', cache_idle_slots: 'Cache Idle Slots',
    threads: 'Threads (-t)', threads_batch: 'Batch Threads (-tb)',
    temperature: 'Temperature', top_p: 'Top-P', top_k: 'Top-K',
    min_p: 'Min-P', repeat_penalty: 'Repeat Penalty', presence_penalty: 'Presence Penalty',
    enable_thinking: 'Thinking Mode', preserve_thinking: 'Preserve Thinking',
    tool_call_format: 'Tool Call Format',
    reasoning: 'Reasoning', reasoning_budget: 'Reasoning Budget',
    reasoning_budget_message: 'Reasoning Budget Message',
    tensor_split: 'Tensor Split', split_mode: 'Split Mode', main_gpu: 'Main GPU',
    n_cpu_moe: 'CPU MoE Threads',
    rope_scaling: 'RoPE Scaling', rope_freq_base: 'RoPE Freq Base', rope_freq_scale: 'RoPE Freq Scale',
    spec_type: 'Speculative Mode', spec_default: 'Spec Defaults',
    spec_ngram_size: 'N-gram Size',
    draft_min: 'Draft Min', draft_max: 'Draft Max', spec_draft_n_max: 'MTP Depth',
    spec_draft_n_min: 'MTP Draft N Min', spec_draft_p_min: 'MTP Draft P Min',
    spec_draft_ngl: 'Draft GPU Layers', spec_draft_device: 'Draft Device',
    spec_draft_n_cpu_moe: 'Draft CPU MoE', draft_model: 'Draft Model',
    bind_host: 'Bind Host', port: 'Port', api_key: 'API Key', max_tokens: 'Max Tokens',
    seed: 'Seed',
    system_prompt_file: 'System Prompt File', grammar: 'Grammar', json_schema: 'JSON Schema', extra_args: 'Extra Args',
};

function _buildChangeSummary(existing, incoming) {
    const changes = [];
    const fmt = v => {
        if (v == null || v === '') return '(none)';
        if (typeof v === 'number') return formatNumberForInput(v);
        return String(v);
    };
    for (const key of Object.keys(CHANGE_LABELS)) {
        const prev = existing[key] ?? null;
        const next = incoming[key] ?? null;
        // Compare formatted representations so float32 noise (0.949999... vs 0.95) doesn't
        // produce a false-positive change that shows as "0.95 → 0.95" in the summary.
        const fPrev = fmt(prev);
        const fNext = fmt(next);
        if (fPrev !== fNext) {
            changes.push({ label: CHANGE_LABELS[key], from: fPrev, to: fNext });
        }
    }
    return changes;
}

// ── CRUD ───────────────────────────────────────────────────────────────────────

export async function savePreset(event) {
    event.preventDefault();
    clearFieldErrors();

    const id = document.getElementById('modal-preset-id').value;
    const saveBtn = document.getElementById('btn-modal-save');
    const existing = id
        ? (sessionState.presets.find(p => p.id === id) || {})
        : (newPresetSeed || {});
    const preset = _buildFormPreset(existing);

    // Inline validation
    let valid = true;
    if (!preset.name) {
        markFieldError('modal-name', 'Preset name is required.');
        valid = false;
    }
    const rapidMlx = preset.rapid_mlx;
    const hasModelSource = preset.backend === 'rapid_mlx'
        ? !!(rapidMlx?.model_source_view || rapidMlx?.model_path)
        : !!(preset.model_path || preset.hf_repo);
    if (!hasModelSource) {
        markFieldError('modal-model-path', 'Model path or HuggingFace repo is required.');
        valid = false;
    }
    if (preset.backend === 'rapid_mlx' && !preset.rapid_mlx?.port) {
        markFieldError('modal-port', 'Rapid-MLX requires a valid server port.');
        valid = false;
    }
    const gpuLayers = parseInt(document.getElementById('modal-gpu-layers').value, 10);
    if (!isNaN(gpuLayers) && gpuLayers < -1) {
        markFieldError('modal-gpu-layers', 'GPU layers must be -1, 0, or a positive number.');
        valid = false;
    }
    const ctxSize = parseInt(document.getElementById('modal-context-size').value, 10);
    if (!isNaN(ctxSize) && ctxSize <= 0) {
        markFieldError('modal-context-size', 'Context size must be a positive number.');
        valid = false;
    }
    const threads = parseInt(document.getElementById('modal-threads').value, 10);
    if (!isNaN(threads) && threads !== -1 && threads < 1) {
        markFieldError('modal-threads', 'Threads must be -1 (auto) or 1 or higher.');
        valid = false;
    }
    const threadsBatch = parseInt(document.getElementById('modal-threads-batch').value, 10);
    if (!isNaN(threadsBatch) && threadsBatch !== -1 && threadsBatch < 1) {
        markFieldError('modal-threads-batch', 'Batch threads must be -1 (auto) or 1 or higher.');
        valid = false;
    }
    if (!valid) {
        scrollToFirstError();
        showToast('Please fix the highlighted error(s)', 'error');
        return;
    }

    // For edits: show change summary and require confirmation
    if (id && saveBtn.dataset.confirmed !== 'yes') {
        const changes = _buildChangeSummary(existing, preset);
        if (changes.length > 0) {
            const summary = document.getElementById('preset-change-summary');
            const list = document.getElementById('preset-change-summary-list');
            const back = document.getElementById('preset-modal-back');
            const cancel = document.getElementById('preset-modal-cancel');
            if (summary && list) {
                list.innerHTML = '';
                changes.forEach(({ label, from, to }) => {
                    const li = document.createElement('li');
                    li.className = 'preset-change-item';
                    li.innerHTML = `<span class="preset-change-field">${escapeHtml(label)}</span> <span class="preset-change-from">${escapeHtml(from)}</span><span class="preset-change-arrow">→</span><span class="preset-change-to">${escapeHtml(to)}</span>`;
                    list.appendChild(li);
                });
                summary.style.display = '';
                if (back) back.style.display = '';
                if (cancel) cancel.style.display = 'none';
                saveBtn.textContent = 'Confirm Save';
                saveBtn.dataset.confirmed = 'yes';

                // If user edits any field after seeing the summary, reset so the
                // next Save click rebuilds the summary with all accumulated changes.
                const form = document.getElementById('preset-form');
                if (form) {
                    const resetOnEdit = () => { _hideSummary(); form.removeEventListener('input', resetOnEdit); form.removeEventListener('change', resetOnEdit); };
                    form.addEventListener('input', resetOnEdit);
                    form.addEventListener('change', resetOnEdit);
                }
            }
            return;
        }
    }

    saveBtn.classList.add('saving');
    saveBtn.textContent = 'Saving...';
    _hideSummary();

    try {
        let resp;
        let savedId;
        if (id) {
            resp = await fetch('/api/presets/' + encodeURIComponent(id), {
                method: 'PUT',
                headers: window.authHeaders
                    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                    : { 'Content-Type': 'application/json' },
                body: JSON.stringify(preset),
            });
            if (!resp.ok) {
                const err = await resp.text().catch(() => 'Unknown error');
                showToast('Save failed: ' + err, 'error');
                return;
            }
            savedId = id;
        } else {
            resp = await fetch('/api/presets', {
                method: 'POST',
                headers: window.authHeaders
                    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                    : { 'Content-Type': 'application/json' },
                body: JSON.stringify(preset),
            });
            if (!resp.ok) {
                const err = await resp.text().catch(() => 'Unknown error');
                showToast('Save failed: ' + err, 'error');
                return;
            }
            const data = await resp.json();
            savedId = data.id || null;
        }
        closePresetModal();
        await loadPresets(savedId);
        if (savedId) syncSelectedPresetSelection(savedId, { userIntent: true, persist: true });
        showToast('Preset saved', 'success');

        // If this is the active preset and the server is running, offer a reload.
        const activePresetId = sessionState.activeSessionPresetId || '';
        if (savedId && activePresetId === savedId && sessionState.serverRunning) {
            _offerRestartAfterPresetSave(savedId);
        }
    } catch (err) {
        showToast('Save failed: ' + err.message, 'error');
    } finally {
        saveBtn.classList.remove('saving');
        saveBtn.textContent = 'Save';
    }
}

export async function copyPreset() {
    const id = document.getElementById('preset-select').value;
    return duplicatePresetById(id, { reopenEditor: false });
}

function buildDuplicatePresetName(baseName) {
    const base = baseName || 'Preset';
    let copyName = base + ' (copy)';
    let suffixNum = 2;
    while (sessionState.presets.some(pr => pr.name === copyName)) {
        copyName = base + ' (copy ' + suffixNum + ')';
        suffixNum++;
    }
    return copyName;
}

async function duplicatePresetById(id, options = {}) {
    const { reopenEditor = false } = options;
    const p = sessionState.presets.find(pr => pr.id === id);
    if (!p) { showToast('No preset selected', 'warn'); return; }

    const copy = Object.assign({}, p);
    delete copy.id;
    copy.name = buildDuplicatePresetName(p.name);

    try {
        const resp = await fetch('/api/presets', {
            method: 'POST',
            headers: window.authHeaders
                ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                : { 'Content-Type': 'application/json' },
            body: JSON.stringify(copy),
        });
        if (!resp.ok) {
            const err = await resp.text().catch(() => 'Unknown error');
            showToast('Copy failed: ' + err, 'error');
            return;
        }
        const data = await resp.json();
        const newId = data.preset?.id || data.id || null;
        await loadPresets(newId);
        if (newId) {
            syncSelectedPresetSelection(newId, { userIntent: true, persist: true });
            if (reopenEditor) openPresetModal('edit');
        }
        showToast(reopenEditor && newId ? 'Preset duplicated - editing copy' : 'Preset copied', 'success');
    } catch (err) {
        showToast('Copy failed: ' + err.message, 'error');
    }
}

export async function deletePreset() {
    const id = document.getElementById('preset-select').value;
    const p = sessionState.presets.find(pr => pr.id === id);
    if (!p) { showToast('No preset selected', 'warn'); return; }

    const confirmed = await _showConfirm('Delete preset', 'Delete preset "' + escapeHtml(p.name) + '"? This cannot be undone.');
    if (!confirmed) return;

    try {
        const resp = await fetch('/api/presets/' + encodeURIComponent(id), {
            method: 'DELETE',
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        if (!resp.ok) {
            const err = await resp.text().catch(() => 'Unknown error');
            showToast('Delete failed: ' + err, 'error');
            return;
        }
        await loadPresets(null);
        showToast('Preset deleted', 'success');
    } catch (err) {
        showToast('Delete failed: ' + err.message, 'error');
    }
}

export async function resetPresets() {
    const ok = await showConfirmDialog(
        'Reset presets',
        'Reset all presets to built-in defaults? Custom presets will be removed.',
        'Reset all'
    );
    if (!ok) return;
    try {
        const resp = await fetch('/api/presets/reset', {
            method: 'POST',
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        if (!resp.ok) {
            const err = await resp.text().catch(() => 'Unknown error');
            showToast('Reset failed: ' + err, 'error');
            return;
        }
        await loadPresets();
        showToast('Presets reset to defaults', 'success');
    } catch (err) {
        showToast('Reset failed: ' + err.message, 'error');
    }
}

// ── Preset Editor Nav ─────────────────────────────────────────────────────────

function initPresetEditorNav() {
    const navItems = document.querySelectorAll('.preset-nav-item');
    const sections = document.querySelectorAll('.preset-editor-section');

    navItems.forEach(btn => {
        btn.addEventListener('click', () => {
            const target = btn.dataset.section;
            // Deactivate all
            navItems.forEach(b => b.classList.remove('active'));
            sections.forEach(s => s.classList.remove('active'));
            // Activate clicked
            btn.classList.add('active');
            const activeSection = document.querySelector('.preset-editor-section[data-section="' + target + '"]');
            if (activeSection) activeSection.classList.add('active');
            const modalBody = document.querySelector('#preset-modal .modal-body');
            if (modalBody) modalBody.scrollTop = 0;
        });
    });
}

// ── Model-family generation defaults ─────────────────────────────────────────

// fillEmpty=true: fill blank sampling fields with model defaults (for new presets).
// fillEmpty=false: only render the preset pill switchers, don't overwrite existing values.
async function _suggestGenerationDefaults(modelPath, fillEmpty = true) {
    const modelName = modelPath.split(/[/\\]/).pop() || modelPath;
    try {
        const headers = window.authHeaders
            ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
            : { 'Content-Type': 'application/json' };

        // Try a quick GGUF metadata read to get arch for finetunes with non-canonical names
        let ggufArch = '';
        if (modelPath.startsWith('/')) {
            try {
                const ir = await fetch('/api/models/gguf-meta', {
                    method: 'POST', headers,
                    body: JSON.stringify({ model_path: modelPath }),
                });
                if (ir.ok) {
                    const id = await ir.json();
                    if (id.ok && id.architecture) ggufArch = id.architecture;
                }
            } catch (_) { /* non-fatal */ }
        }

        const resp = await fetch('/api/model-defaults', {
            method: 'POST',
            headers,
            body: JSON.stringify({ model_name_or_repo: modelName, size_bytes: 0, tags: [], gguf_arch: ggufArch }),
        });
        if (!resp.ok) return;
        const d = await resp.json();
        if (d.error) return;
        const defaults = d.defaults || d;

        if (fillEmpty) {
            // Only fill fields the user hasn't already set
            const fill = (id, val) => {
                const el = document.getElementById(id);
                if (el && el.value === '') numOrEmpty(id, val);
            };
            fill('modal-temperature', defaults.temperature ?? null);
            fill('modal-top-p', defaults.top_p ?? null);
            fill('modal-top-k', defaults.top_k ?? null);
            fill('modal-min-p', defaults.min_p ?? null);
            fill('modal-repeat-penalty', defaults.repeat_penalty ?? null);
            fill('modal-presence-penalty', defaults.presence_penalty ?? null);
            fill('modal-max-tokens', defaults.max_tokens ?? null);
            _fillSelectIfEmpty('modal-enable-thinking', defaults.enable_thinking);
            _fillSelectIfEmpty('modal-preserve-thinking', defaults.preserve_thinking);
            // tool_call_format is intentionally never auto-filled from model-family
            // defaults — it's a template-level opt-in, left blank unless the user
            // explicitly selects "json".
            _fillSelectIfEmpty('modal-reasoning', defaults.reasoning ? 'on' : 'off');
            fill('modal-reasoning-budget', defaults.reasoning_budget ?? null);
            const msgEl = document.getElementById('modal-reasoning-budget-message');
            if (msgEl && msgEl.value === '' && defaults.reasoning_budget_message != null) {
                msgEl.value = defaults.reasoning_budget_message.replace(/\n/g, '\\n');
            }
        }
        _renderGenerationPresetPills(d.presets || []);
    } catch (_) {
        // Silent — best-effort only
    }
}

function _fillSelectIfEmpty(id, value) {
    const el = document.getElementById(id);
    if (!el || el.value !== '' || value == null) return;
    el.value = typeof value === 'boolean' ? String(value) : String(value);
}

function _renderGenerationPresetPills(presets) {
    const container = document.getElementById('modal-generation-presets');
    if (!container) return;
    if (!presets || presets.length <= 1) {
        container.style.display = 'none';
        container.innerHTML = '';
        return;
    }

    container.style.display = 'flex';
    container.style.cssText = 'display:flex;align-items:center;gap:6px;flex-wrap:wrap;margin-bottom:12px;';
    container.innerHTML = '';

    const label = document.createElement('span');
    label.style.cssText = 'font-size:11px;color:var(--color-text-muted);flex-shrink:0;';
    label.textContent = 'Mode:';
    container.appendChild(label);

    presets.forEach((preset, index) => {
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.className = 'sampling-preset-pill' + (index === 0 ? ' active' : '');
        btn.textContent = preset.name;
        if (preset.description) btn.title = preset.description;
        btn.addEventListener('click', () => {
            container.querySelectorAll('.sampling-preset-pill').forEach(p => p.classList.remove('active'));
            btn.classList.add('active');
            _applyGenerationPreset(preset);
        });
        container.appendChild(btn);
    });
}

function _applyGenerationPreset(preset) {
    numOrEmpty('modal-temperature', preset.temperature);
    numOrEmpty('modal-top-p', preset.top_p);
    numOrEmpty('modal-top-k', preset.top_k);
    numOrEmpty('modal-min-p', preset.min_p);
    numOrEmpty('modal-repeat-penalty', preset.repeat_penalty);
    numOrEmpty('modal-presence-penalty', preset.presence_penalty);
    numOrEmpty('modal-max-tokens', preset.max_tokens);
    setOpt('modal-enable-thinking', preset.enable_thinking == null ? '' : String(!!preset.enable_thinking));
    setOpt('modal-preserve-thinking', preset.preserve_thinking == null ? '' : String(!!preset.preserve_thinking));
    setOpt('modal-tool-call-format', preset.tool_call_format || '');
    setOpt('modal-reasoning', preset.reasoning ? 'on' : 'off');
    numOrEmpty('modal-reasoning-budget', preset.reasoning_budget);
    setVal('modal-reasoning-budget-message', (preset.reasoning_budget_message || '').replace(/\n/g, '\\n'));
}

// ── Restart after preset save ──────────────────────────────────────────────────

function _offerRestartAfterPresetSave(presetId) {
    if (!presetId) return;

    showToastWithActions(
        'Apply changes?',
        'info',
        'Restart the local model server to load the updated preset.',
        [
            {
                id: 'restart',
                label: 'Restart Now',
                primary: true,
                handler: async () => {
                    showToast('Restarting local model server…', 'info');
                    try {
                        await _restartServerWithPreset(presetId);
                    } catch (e) {
                        showToast('Restart failed: ' + (e.message || String(e)), 'error');
                    }
                },
            },
            {
                id: 'later',
                label: 'Not now',
                primary: false,
                handler: () => {},
            },
        ],
        { duration: 12000 },
    );
}

async function _restartServerWithPreset(presetId) {
    const p = sessionState.presets.find(pr => pr.id === presetId);
    if (!p) throw new Error('Preset not found');

    // Kill current server
    try {
        const tokenResp = await fetch('/api/db/admin-token', {
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        const tokenData = tokenResp.ok ? await tokenResp.json().catch(() => ({})) : {};
        const token = tokenData.token;
        if (!token) throw new Error('Authentication required');

        await fetch('/api/kill-server', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': 'Bearer ' + token,
            },
            body: JSON.stringify({ confirm: 'kill' }),
        });
    } catch (e) {
        throw new Error('Failed to stop server: ' + (e.message || e));
    }

    // Rust owns backend selection, native config, and the resolved launch port.
    const config = { preset_id: presetId };

    // Spawn new server
    const adminToken = await (async () => {
        const tokenResp = await fetch('/api/db/admin-token', {
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        const tokenData = tokenResp.ok ? await tokenResp.json().catch(() => ({})) : {};
        return tokenData.token || null;
    })();

    if (!adminToken) throw new Error('Authentication required');

    const resp = await fetch('/api/sessions/spawn', {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'Authorization': 'Bearer ' + adminToken,
        },
        body: JSON.stringify(config),
    });

    if (!resp.ok) {
        const text = await resp.text().catch(() => 'Request failed');
        throw new Error('Spawn failed: ' + text);
    }

    const data = await resp.json().catch(() => ({}));
    if (!data.ok) {
        throw new Error(data.error || 'Spawn responded with an error');
    }

    // Wait for server to be ready
    const backendLabel = data.backend === 'rapid_mlx' ? 'Rapid-MLX' : 'llama-server';
    const launchPort = data.port;
    showToast(`Starting ${backendLabel}…`, 'info', 'Loading model on port ' + launchPort, { duration: 12000 });
    try {
        await (await import('./spawn-readiness.js')).waitForSpawnReadiness(launchPort);
    } catch (e) {
        throw new Error('Server did not become ready: ' + (e.message || e));
    }

    showToast(`${backendLabel} restarted`, 'success', '', { duration: 6000 });
}

// ── Model architecture info (preset editor) ───────────────────────────────────

// Format a byte count as GiB/MiB for per-layer VRAM hints.
function _formatLayerBytes(bytes) {
    const gib = bytes / (1024 ** 3);
    if (gib >= 1) return gib.toFixed(gib >= 10 ? 0 : 2) + ' GiB';
    return Math.round(bytes / (1024 ** 2)) + ' MiB';
}

function _renderPresetArchInfo(preset) {
    const container = document.getElementById('pe-arch-info');
    if (!container) return;
    container.innerHTML = '';

    const arch = buildArchitectureLabel(preset, null);
    if (!arch) return;

    // Main line: "Architecture: MoE • 35B (3B active)"
    const main = document.createElement('div');
    main.className = 'pe-arch-main';
    main.textContent = 'Architecture: ' + arch.display;
    main.title = arch.tooltip;

    container.appendChild(main);

    // Expert sub-line (if present)
    if (preset.expert_count != null || preset.expert_used_count != null) {
        const sub = document.createElement('div');
        sub.className = 'pe-arch-sub';
        const parts = [];
        if (preset.expert_count != null) {
            parts.push(preset.expert_count + ' experts');
        }
        if (preset.expert_used_count != null) {
            parts.push(preset.expert_used_count + ' active per token');
        }
        sub.textContent = parts.join(', ');
        container.appendChild(sub);
    }

    // Layer-count sub-line — the value users need to bound the GPU-offload knobs.
    // For MoE/hybrid: --n-cpu-moe offloads expert layers to CPU/RAM.
    // For dense: --gpu-layers (-ngl) is the primary offload knob (no experts).
    if (preset.block_count != null) {
        const layers = document.createElement('div');
        layers.className = 'pe-arch-sub';
        layers.textContent = isMoEEligible(preset)
            ? preset.block_count + ' layers — set --n-cpu-moe between 0 and ' +
                preset.block_count + ' to offload expert layers to CPU/RAM'
            : preset.block_count + ' layers — set --gpu-layers (-ngl) between 0 and ' +
                preset.block_count + ' to offload layers to the GPU';
        container.appendChild(layers);
    }
}

// Clear architecture info + layer hints when model path changes so they don't show
// stale data (block_count is refreshed by the backend only after the preset is saved).
document.getElementById('modal-model-path')?.addEventListener('input', () => {
    const container = document.getElementById('pe-arch-info');
    if (container) container.innerHTML = '';
    ['modal-n-cpu-moe-layers', 'modal-gpu-layers-layers'].forEach(id => {
        const el = document.getElementById(id);
        if (el) { el.textContent = ''; el.style.display = 'none'; }
    });
    ['modal-gpu-layers', 'modal-n-cpu-moe'].forEach(id => {
        document.getElementById(id)?.removeAttribute('max');
    });
    // Fetch live Rapid-MLX profile for this model when backend is rapid_mlx
    _schedulePresetRapidMlxProfile();
});

// ── Rapid-MLX live model profile for preset editor ────────────────────────────

let _presetRapidMlxProfileTimer = null;
let _presetRapidMlxProfile = null;

function _schedulePresetRapidMlxProfile() {
    clearTimeout(_presetRapidMlxProfileTimer);
    _presetRapidMlxProfileTimer = setTimeout(async () => {
        const preset = _currentModalPreset();
        if (!preset || preset.backend !== 'rapid_mlx') {
            _presetRapidMlxProfile = null;
            return;
        }
        const rapidMlx = preset.rapid_mlx;
        const modelId = rapidMlx?.model_source_view?.canonical_identity
            || rapidMlx?.model_source_view?.display_name
            || rapidMlx?.model_path || '';
        if (!modelId || modelId.trim().length < 2) {
            _presetRapidMlxProfile = null;
            return;
        }
        try {
            const headers = window.authHeaders ? window.authHeaders() : {};
            const url = `/api/rapid-mlx/models/${encodeURIComponent(modelId)}/profile`;
            const res = await fetch(url, { headers });
            if (!res.ok) {
                _presetRapidMlxProfile = null;
                return;
            }
            const data = await res.json().catch(() => ({}));
            _presetRapidMlxProfile = data.profile || null;
        } catch {
            _presetRapidMlxProfile = null;
        }
    }, 350);
}

export function getPresetRapidMlxProfile() {
    return _presetRapidMlxProfile;
}

// ── Init ───────────────────────────────────────────────────────────────────────

export function initPresets() {
    // Init preset editor nav
    initPresetEditorNav();

    // Bind preset action buttons (toolbar — minimal)
    document.getElementById('preset-edit-btn')?.addEventListener('click', () => openPresetModal('edit'));
    document.getElementById('preset-new-btn')?.addEventListener('click', () => openPresetModal('new'));

    // Refresh the performance advisor as the preset form changes
    const presetForm = document.getElementById('preset-form');
    if (presetForm) {
        presetForm.addEventListener('input', () => { updatePresetAdvisor(); updatePresetVram(); });
        presetForm.addEventListener('change', () => { updatePresetAdvisor(); updatePresetVram(); });
    }

    // MoE offload auto-tuner (empirical sweep)
    document.getElementById('preset-moe-autotune-verify')?.addEventListener('click', autoTunePreset);

    // Bind preset modal buttons
    document.getElementById('preset-modal-close')?.addEventListener('click', closePresetModal);
    document.getElementById('preset-modal-cancel')?.addEventListener('click', closePresetModal);
    document.getElementById('preset-modal-back')?.addEventListener('click', _hideSummary);
    document.getElementById('preset-vram-auto-size')?.addEventListener('click', autoSizePreset);

    // Duplicate preset from within the modal
    document.getElementById('preset-modal-duplicate')?.addEventListener('click', async () => {
        const id = document.getElementById('modal-preset-id').value;
        await duplicatePresetById(id, { reopenEditor: true });
    });

    // Delete preset from within the modal (only visible in edit mode)
    document.getElementById('preset-modal-delete')?.addEventListener('click', async () => {
        const id = document.getElementById('modal-preset-id').value;
        const p = sessionState.presets.find(pr => pr.id === id);
        if (!p) { showToast('No preset selected', 'warn'); return; }
        const ok = await showConfirmDialog(
            'Delete preset',
            `Delete preset "${p.name}"? This cannot be undone.`,
            'Delete'
        );
        if (!ok) return;
        try {
            const resp = await fetch('/api/presets/' + encodeURIComponent(id), {
                method: 'DELETE',
                headers: window.authHeaders ? window.authHeaders() : {},
            });
            if (!resp.ok) {
                const err = await resp.text().catch(() => 'Unknown error');
                showToast('Delete failed: ' + err, 'error');
                return;
            }
            closePresetModal();
            await loadPresets();
            showToast('Preset deleted', 'success');
        } catch (err) {
            showToast('Delete failed: ' + err.message, 'error');
        }
    });
    document.getElementById('preset-browse-model-btn')?.addEventListener('click', () => openModelFileBrowser('modal-model-path', 'gguf', null, 'model'));
    document.getElementById('preset-browse-mmproj-btn')?.addEventListener('click', () => openModelFileBrowser('modal-mmproj', 'gguf', null, 'mmproj'));
    document.getElementById('modal-mmproj')?.addEventListener('input', (e) => {
        _toggleVisionTokens(!!(e.target.value || '').trim());
    });
    document.getElementById('modal-image-max-tokens')?.addEventListener('input', (e) => {
        _ensureUbatchForImageTokens(Number(e.target.value || 0));
    });
    document.getElementById('preset-browse-chat-template-btn')?.addEventListener('click', async () => {
        try {
            await openChatTemplateLibraryBrowser('modal-chat-template-file');
        } catch (err) {
            showToast('Template library unavailable: ' + (err.message || String(err)), 'error');
        }
    });
    document.getElementById('preset-recommended-chat-template-btn')?.addEventListener(
        'click',
        installRecommendedChatTemplateForPreset,
    );
    document.getElementById('preset-check-chat-template-update-btn')?.addEventListener('click', async () => {
        const path = (document.getElementById('modal-chat-template-file')?.value || '').trim();
        if (!path) {
            showToast('No template selected to check', 'warn');
            return;
        }
        const button = document.getElementById('preset-check-chat-template-update-btn');
        const origText = button.textContent;
        button.disabled = true;
        button.textContent = 'Checking…';
        try {
            const headers = window.authHeaders
                ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                : { 'Content-Type': 'application/json' };
            const resp = await fetch('/api/chat-template/check-update', {
                method: 'POST',
                headers,
                body: JSON.stringify({ path }),
            });
            if (!resp.ok) {
                showToast('Check failed: ' + (resp.statusText || 'Unexpected response'), 'error');
                return;
            }
            const data = await resp.json();
            if (data.changed) {
                showToast(
                    'Upstream template has changed since this install',
                    'warn',
                    'Use Recommended to re-download the latest version',
                );
            } else {
                const hint = data.installed_at
                    ? 'Installed ' + new Date(data.installed_at).toLocaleString()
                    : 'No changes upstream';
                showToast('Template is up to date', 'success', hint);
            }
        } catch (err) {
            showToast('Check failed: ' + (err.message || String(err)), 'error');
        } finally {
            button.disabled = false;
            button.textContent = origText;
        }
    });
    document.getElementById('preset-upload-chat-template-btn')?.addEventListener('click', async () => {
        try {
            const uploaded = await uploadChatTemplateFromBrowser();
            if (!uploaded?.path) return;
            setVal('modal-chat-template-file', uploaded.path);
            showToast('Template uploaded', 'success', uploaded.filename || 'Saved to template library');
        } catch {
            // uploadChatTemplateFromBrowser already surfaced the error
        }
    });
    document.getElementById('preset-clear-chat-template-btn')?.addEventListener('click', () => {
        setVal('modal-chat-template-file', '');
    });
    document.getElementById('preset-browse-draft-model-btn')?.addEventListener('click', () => openModelFileBrowser('modal-draft-model', 'gguf', null, 'draft-model'));

    // Fit-to-VRAM toggle shows/hides fit target
    document.getElementById('modal-fit-enabled')?.addEventListener('change', function() {
        _toggleFitTarget(this.value === 'true');
    });

    // Spec type dropdown shows/hides relevant fields
    document.getElementById('modal-spec-type')?.addEventListener('change', function() {
        _toggleSpecFields(this.value);
    });

    // Show cache-idle-slots hint when parallel slots > 1
    document.getElementById('modal-parallel-slots')?.addEventListener('input', function() {
        const hint = document.getElementById('cache-idle-slots-hint');
        if (hint) hint.style.display = parseInt(this.value) > 1 ? '' : 'none';
    });

    document.getElementById('modal-structured-output-mode')?.addEventListener('change', function() {
        setStructuredOutputMode(this.value);
    });

    // Bind preset form submit
    if (presetForm) presetForm.addEventListener('submit', savePreset);

    // Bind setup view link
    document.getElementById('setup-manage-presets-link')?.addEventListener('click', (e) => {
        e.preventDefault();
        openPresetModal('new');
    });

    // Modal overlay click
    const modal = document.getElementById('preset-modal');
    if (modal) {
        modal.addEventListener('click', e => {
            if (e.target === e.currentTarget) closePresetModal();
        });
    }

    window.closePresetsPanel = closePresetsPanel;

    // Clear field errors on input
    ['modal-name', 'modal-model-path'].forEach(id => {
        const el = document.getElementById(id);
        if (el) {
            el.addEventListener('input', function() {
                this.classList.remove('field-error');
            });
        }
    });

    // When model path changes, suggest model-family generation defaults (only fills empty fields)
    let _modelDefaultsTimer = null;
    document.getElementById('modal-model-path')?.addEventListener('input', function() {
        clearTimeout(_modelDefaultsTimer);
        const path = this.value.trim();
        if (!path) return;
        _modelDefaultsTimer = setTimeout(() => _suggestGenerationDefaults(path), 600);
    });
    // Initial load
    loadPresets();
}

// ── Apple Silicon-aware Threads hints in preset editor ─────────────────────────

function _refreshPresetThreadsHints() {
  const modal = document.getElementById('preset-modal');
  if (!modal || !modal.classList.contains('open')) return;

  const metrics = lastSystemMetrics;
  const pCores = metrics?.p_cores || 0;
  const metricsReady = metrics != null;

  const threadsInput = document.getElementById('modal-threads');
  const batchThreadsInput = document.getElementById('modal-threads-batch');
  if (!threadsInput && !batchThreadsInput) return;

  const hintEl = document.getElementById('preset-threads-hint');
  if (pCores > 0 && metricsReady) {
    if (threadsInput && !threadsInput.value) {
      threadsInput.placeholder = '1 recommended';
    }
    if (batchThreadsInput && !batchThreadsInput.value) {
      batchThreadsInput.placeholder = `${pCores} recommended`;
    }
    if (hintEl && _presetIsUnified) {
      hintEl.textContent = `Apple Silicon: threads = 1 (Metal GPU handles inference), threads-batch = ${pCores} (P-cores for faster prefill).`;
      hintEl.style.display = '';
    }
  } else {
    if (threadsInput && !threadsInput.value) {
      threadsInput.placeholder = 'auto';
    }
    if (batchThreadsInput && !batchThreadsInput.value) {
      batchThreadsInput.placeholder = 'auto';
    }
    if (hintEl) hintEl.style.display = 'none';
  }
}
window.__refreshPresetEditorHints = _refreshPresetThreadsHints;

async function _fetchSystemInfoAndRefreshPresetHints() {
  try {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const [res] = await Promise.all([
      fetch('/api/system/info', { headers }),
      _ensureUnifiedFlag(), // populate _presetIsUnified before hint renders
    ]);
    if (!res.ok) return;
    const data = await res.json();
    if (data.ok && data.p_cores > 0) {
      const { setLastSystemMetrics } = await import('../core/app-state.js');
      setLastSystemMetrics({ p_cores: data.p_cores, e_cores: data.e_cores, cpu_name: data.cpu_name });
      _refreshPresetThreadsHints();
    }
  } catch (err) { console.warn('Failed to fetch system info for preset hints:', err); }
}

export async function _showConfirm(title, message) {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.style.zIndex = '2000';
    overlay.style.display = 'grid';

    const dialog = document.createElement('div');
    dialog.className = 'modal';
    dialog.style.width = '420px';
    dialog.style.padding = '14px 16px';

    const titleEl = document.createElement('div');
    titleEl.style.fontSize = '15px';
    titleEl.style.fontWeight = '600';
    titleEl.style.marginBottom = '8px';
    titleEl.textContent = title;

    const msg = document.createElement('div');
    msg.style.fontSize = '13px';
    msg.style.color = 'var(--color-text-muted)';
    msg.style.marginBottom = '12px';
    msg.textContent = message;

    const actions = document.createElement('div');
    actions.style.display = 'flex';
    actions.style.justifyContent = 'flex-end';
    actions.style.gap = '8px';

    const cancelBtn = document.createElement('button');
    cancelBtn.type = 'button';
    cancelBtn.className = 'btn btn-modal-cancel';
    cancelBtn.textContent = 'Cancel';

    const confirmBtn = document.createElement('button');
    confirmBtn.type = 'button';
    confirmBtn.className = 'btn btn-modal-save';
    confirmBtn.textContent = 'Confirm';

    return new Promise(resolve => {
        let decided = false;

        function cleanup() {
            if (overlay.parentElement) overlay.remove();
        }

        cancelBtn.addEventListener('click', () => {
            if (decided) return;
            decided = true;
            cleanup();
            resolve(false);
        });

        confirmBtn.addEventListener('click', () => {
            if (decided) return;
            decided = true;
            cleanup();
            resolve(true);
        });

        overlay.addEventListener('click', (e) => {
            if (e.target === overlay && !decided) {
                decided = true;
                cleanup();
                resolve(false);
            }
        });

        actions.appendChild(cancelBtn);
        actions.appendChild(confirmBtn);
        dialog.appendChild(titleEl);
        dialog.appendChild(msg);
        dialog.appendChild(actions);
        overlay.appendChild(dialog);
        document.body.appendChild(overlay);
        cancelBtn.focus();
    });
}

function _renderContextPills(mode, section) {
    const pillsContainer = document.getElementById('preset-context-pills');
    if (!pillsContainer) return;
    const pills = [
        { label: '65k', value: 65536 },
        { label: '131k', value: 131072 },
        { label: '160k', value: 163840 },
        { label: '212k', value: 212000 },
        { label: '262k', value: 262144 },
    ];
    pillsContainer.innerHTML = '';
    pills.forEach(pill => {
        const pillEl = document.createElement('button');
        pillEl.type = 'button';
        pillEl.className = 'preset-context-pill';
        pillEl.textContent = pill.label;
        pillEl.onclick = (e) => {
            e.preventDefault();
            const input = document.getElementById('modal-context-size');
            if (input) {
                input.value = pill.value;
                input.dispatchEvent(new Event('input', { bubbles: true }));
                input.dispatchEvent(new Event('change', { bubbles: true }));
            }
        };
        pillsContainer.appendChild(pillEl);
    });
}
