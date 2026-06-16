// ── Presets ────────────────────────────────────────────────────────────────────
/* global DOMPurify */
// Preset CRUD: load, save, copy, delete, reset. Modal management.

import { sessionState, lastSystemMetrics } from '../core/app-state.js';
import { escapeHtml } from '../core/format.js';
import { openModelFileBrowser, openChatTemplateLibraryBrowser, uploadChatTemplateFromBrowser } from './file-browser-launcher.js';
import { applySettings, saveSettings } from './settings.js';
import { showToast } from './toast.js';
import { renderSuggestionCards, suggestionPatch, requestNcpuMoeTune } from './tuning-cards.js';
import {
    COMMUNITY_TEMPLATES,
    buildCommunityTemplateInstallRequest,
    detectCommunityTemplateFamily,
} from './chat-template-registry.js';

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

    const [presetsResp, settingsResp, activeResp] = await Promise.all([
        fetch('/api/presets', { headers: auth }),
        selectId === undefined ? fetch('/api/settings', { headers: auth }) : Promise.resolve(null),
        selectId === undefined ? fetch('/api/sessions/active', { headers: auth }) : Promise.resolve(null),
    ]);

    if (presetsResp.status === 401) {
        showToast('Unauthorized: API token missing or invalid', 'error');
        return;
    }

    sessionState.presets = await presetsResp.json();
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
        if (!p.model_path && !p.hf_repo) return;
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
        if (activeData && activeData.preset_id &&
            (String(activeData.status || '').toLowerCase() === 'running')) {
            targetId = activeData.preset_id;
        }
    }
    if (targetId === null) {
        targetId = saved?.preset_id || null;
    }

    if (targetId && sessionState.presets.find(p => p.id === targetId)) {
        sel.value = targetId;
    } else if (sessionState.presets.length > 0) {
        sel.value = sessionState.presets[0].id;
    }

    if (selectId === undefined && saved) {
        applySettings(saved);
    }
    if (selectId === undefined) {
        saveSettings();
    }

    // Sync the visual preset display (short name + chips)
    if (sel && sel.value) {
        syncPresetDisplay(sel);
    }

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
        window.__presetUserSelected = true;

        // Fetch current active session to see if something is running
        try {
            const resp = await fetch('/api/sessions/active', {
                headers: window.authHeaders ? window.authHeaders() : {},
            });
            if (!resp.ok) return;
            const active = await resp.json().catch(() => ({}));

            // If nothing is running, or it's the same preset, nothing special to do
            if (!active || active.status !== 'running' || active.preset_id === chosenId) {
                return;
            }

            // Different preset is running: ask if they want to switch
            const chosenPreset = sessionState.presets.find(p => p.id === chosenId);
            const runningPreset = sessionState.presets.find(p => p.id === active.preset_id);
            const chosenName = chosenPreset?.name || 'selected preset';
            const runningName = runningPreset?.name || 'current preset';

            if (!confirm(
                `A model is already loaded (${runningName}).\n\n` +
                `Do you want to stop it and load ${chosenName}?`
            )) {
                // Revert selection
                sel.value = active.preset_id;
                return;
            }

            // User confirmed: stop running, start new preset
            showToast('Switching preset…', 'info');

            // Stop current
            const tokenResp = await fetch('/api/db/admin-token', {
                headers: window.authHeaders ? window.authHeaders() : {},
            });
            const tokenData = await tokenResp.json().catch(() => ({}));
            const token = tokenData.token;
            if (token) {
                await fetch('/api/kill-llama', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                        'Authorization': `Bearer ${token}`,
                    },
                    body: JSON.stringify({ confirm: 'kill' }),
                }).catch(() => {});
            }

            // Short delay to allow process to shut down
            await new Promise(r => setTimeout(r, 400));

            // Start new preset via existing doStart flow
            // We'll import attach-detach and call doStart directly
            const { doStart } = await import('./attach-detach.js');
            await doStart();

        } catch (e) {
            console.warn('[presets] preset-select change error:', e);
        } finally {
            syncPresetDisplay(sel);
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

    const fullName = preset.name || (preset.model_path || preset.hf_repo || '').split('/').pop() || '';
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
    const base = fullName || p.name || (p.model_path || p.hf_repo || '').split('/').pop() || '';
    if (!base) return '';
    // Normalize underscores to hyphens; CSS text-overflow handles truncation.
    return base.replace(/_/g, '-').replace(/-{2,}/g, '-').trim();
}

function buildPresetChips(p) {
    const chips = [];
    const name = p.name || (p.model_path || p.hf_repo || '').split('/').pop() || '';

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
});

// ── Modal ──────────────────────────────────────────────────────────────────────

// ── Performance advisor (config-time hints) ──────────────────────────────────
let _presetAdvisorTimer = null;
let _presetAdvisorSeq = 0;
let _presetIsUnified = null; // cached platform check
let _presetRamBytes = 0;    // cached RAM total (bytes)
let _presetMetalLimitMb = 0; // cached iogpu.wired_limit_mb (0 = use heuristic)

// ── VRAM live estimate ────────────────────────────────────────────────────────
let _presetVramTimer = null;
let _presetVramSeq = 0;

function _presetAvailBytes() {
    if (!_presetIsUnified || _presetRamBytes === 0) return 0;
    const limitBytes = _presetMetalLimitMb > 0 ? _presetMetalLimitMb * 1024 * 1024 : null;
    const fraction = _presetRamBytes <= 36 * 1024 ** 3 ? 2 / 3 : 3 / 4;
    const cap = limitBytes ?? Math.floor(_presetRamBytes * fraction);
    return Math.max(0, Math.min(cap, _presetRamBytes) - 512 * 1024 * 1024);
}

async function _ensureUnifiedFlag() {
    if (_presetIsUnified !== null) return _presetIsUnified;
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const [platform, sys] = await Promise.all([
            fetch('/api/llama-binary/platform-info', { headers }).then(r => r.ok ? r.json() : null).catch(() => null),
            fetch('/metrics/system', { headers }).then(r => r.ok ? r.json() : null).catch(() => null),
        ]);
        _presetIsUnified = platform?.auto_backend === 'metal';
        _presetRamBytes = (sys?.ram_total_gb || lastSystemMetrics?.ram_total_gb || 0) * 1024 ** 3;
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
    const name = p.model_path || '';
    const repo = p.hf_repo || '';
    const hasMmproj = !!p.mmproj;
    if ((looksLikeQwenName(name) || looksLikeQwenName(repo)) && hasMmproj) {
        return { min_tokens: 1024, max_tokens: 4096 };
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
            available_vram_bytes: _presetAvailBytes(),
            is_unified_memory: !!_presetIsUnified,
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
    const modelVal = document.getElementById('modal-model-path')?.value.trim() || '';
    if (!modelVal) { if (strip) strip.style.display = 'none'; return; }
    if (strip) strip.style.display = '';
    box.innerHTML = '<div class="preset-vram-loading">Estimating VRAM…</div>';
    clearTimeout(_presetVramTimer);
    _presetVramTimer = setTimeout(async () => {
        const isUnified = await _ensureUnifiedFlag();
        const nCtx = parseInt(document.getElementById('modal-context-size')?.value) || 131072;
        const ctk = document.getElementById('modal-ctk')?.value || 'q8_0';
        const ctv = document.getElementById('modal-ctv')?.value || 'f16';
        const parallelSlots = parseInt(document.getElementById('modal-parallel-slots')?.value) || 1;
        const ubatch = parseInt(document.getElementById('modal-ubatch-size')?.value) || 512;
        const nCpuMoe = parseInt(document.getElementById('modal-n-cpu-moe')?.value) || 0;
        const available_vram_bytes = _presetAvailBytes();
        const body = {
            model_path: modelVal,
            n_ctx: nCtx,
            ctk, ctv,
            parallel_slots: parallelSlots,
            ubatch_size: ubatch,
            n_cpu_moe: nCpuMoe,
            available_vram_bytes,
            is_unified_memory: !!isUnified,
        };
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
    const kv      = data.kv_cache_bytes || 0;
    const mmproj  = data.mmproj_bytes || 0;
    const mtp     = data.mtp_bytes || 0;
    const overhead = data.overhead_bytes || 0;

    // Bar 100% = budget so free headroom is visible; fall back to used if no budget
    const barTotal = avail > 0 ? avail : used;
    const free = avail > 0 ? Math.max(0, avail - used) : 0;
    const pct = v => barTotal > 0 ? Math.max(0, Math.min(100, (v / barTotal) * 100)).toFixed(1) + '%' : '0%';

    const rec = data.recommendation || 'fit';
    const recLabel = rec === 'fit' ? 'Fits' : rec === 'tight' ? 'Tight' : 'Risk';
    const recClass = rec === 'fit' ? 'fit' : rec === 'tight' ? 'tight' : 'risk';

    const parts = [];
    if (weights > 0) parts.push(`Weights ${fmt(weights)}`);
    if (kv > 0) parts.push(`KV ${fmt(kv)}`);
    if (mmproj > 0) parts.push(`mmproj ${fmt(mmproj)}`);
    if (mtp > 0) parts.push(`MTP ${fmt(mtp)}`);
    if (overhead > 0) parts.push(`overhead ${fmt(overhead)}`);
    if (avail > 0 && free > 0) parts.push(`${fmt(free)} free of ${fmt(avail)}`);

    // eslint-disable-next-line no-unsanitized/property -- DOMPurify sanitizes the VRAM bar HTML
    el.innerHTML = window.DOMPurify.sanitize(`
        <div class="preset-vram-row">
            <div class="vram-bar">
                <div class="vram-segment seg-weights" style="width:${pct(weights)}" title="Weights"></div>
                <div class="vram-segment seg-kv" style="width:${pct(kv)}" title="KV Cache"></div>
                <div class="vram-segment seg-mmproj" style="width:${pct(mmproj)}" title="Vision Projector"></div>
                <div class="vram-segment seg-mtp" style="width:${pct(mtp)}" title="MTP Heads"></div>
                <div class="vram-segment seg-overhead" style="width:${pct(overhead)}" title="Overhead"></div>
                <div class="vram-segment seg-free" style="width:${pct(free)}" title="Free Headroom"></div>
            </div>
            <span class="launch-card-vram-total">~${fmt(used)}</span>
            <span class="preset-vram-badge preset-vram-badge--${recClass}">${recLabel}</span>
        </div>
        ${parts.length ? `<div class="preset-vram-breakdown">${parts.join(' · ')}</div>` : ''}
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

export function openPresetModal(mode, section) {
    const modal = document.getElementById('preset-modal');
    const title = document.getElementById('modal-title');
    const subtitle = document.getElementById('preset-editor-subtitle');
    const form = document.getElementById('preset-form');
    form.reset();
    clearFieldErrors();

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
        const modelValue = p.model_path || p.hf_repo || '';
        setVal('modal-model-path', modelValue);
        setVal('modal-alias', p.alias || '');
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
        numOrEmpty('modal-n-cpu-moe', p.n_cpu_moe);
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
        setVal('modal-chat-template-file', p.chat_template_file || '');
        // Advanced
        setOpt('modal-bind-host', p.bind_host || '');
        numOrEmpty('modal-port', p.port);
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
    } else {
        title.textContent = 'New Preset';
        if (subtitle) subtitle.textContent = 'New model profile';
        setVal('modal-preset-id', '');
        setVal('modal-context-size', 128000);
        setVal('modal-ctk', 'q8_0');
        setVal('modal-ctv', 'f16');
        setVal('modal-batch-size', 2048);
        setVal('modal-ubatch-size', 2048);
        setVal('modal-parallel-slots', 1);
        _toggleFitTarget(false);
        _toggleSpecFields('');
        setStructuredOutputMode('');
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
        import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard());
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

    const presets = (sessionState.presets || []).filter(
        p => p.model_path || p.hf_repo
    );
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
        if (preset.model_path) metaParts.push(preset.model_path.split(/[/\\]/).pop() || preset.model_path);
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
            const mainSelect = document.getElementById('preset-select');
            if (mainSelect) {
                mainSelect.value = preset.id;
                mainSelect.dispatchEvent(new Event('change', { bubbles: true }));
            }
            closePresetsPanel();
            import('./attach-detach.js').then(({ doStartFromSetup }) => {
                const setupSelect = document.getElementById('setup-preset-select');
                if (setupSelect) setupSelect.value = preset.id;
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
            if (!confirm(`Delete preset "${preset.name}"?`)) return;
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
            if (!confirm(`Delete preset "${preset.name}"? This cannot be undone.`)) return;
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

function _buildFormPreset(existing) {
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
    const existing = id ? (sessionState.presets.find(p => p.id === id) || {}) : {};
    const preset = _buildFormPreset(existing);

    // Inline validation
    let valid = true;
    if (!preset.name) {
        markFieldError('modal-name', 'Preset name is required.');
        valid = false;
    }
    if (!preset.model_path && !preset.hf_repo) {
        markFieldError('modal-model-path', 'Model path or HuggingFace repo is required.');
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
        showToast('Preset saved', 'success');

        // If this is the active preset and the server is running, offer a reload.
        const activePresetId = document.getElementById('preset-select')?.value || '';
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
    const p = sessionState.presets.find(pr => pr.id === id);
    if (!p) { showToast('No preset selected', 'warn'); return; }

    const copy = Object.assign({}, p);
    delete copy.id;

    // Deduplicate copy names: "Foo (copy)", "Foo (copy 2)", "Foo (copy 3)", …
    let baseName = p.name;
    let copyName = baseName + ' (copy)';
    let suffixNum = 2;
    while (sessionState.presets.some(pr => pr.name === copyName)) {
        copyName = baseName + ' (copy ' + suffixNum + ')';
        suffixNum++;
    }
    copy.name = copyName;

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
        await loadPresets(data.preset?.id || null);
        showToast('Preset copied', 'success');
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
        await loadPresets();
        showToast('Preset deleted', 'success');
    } catch (err) {
        showToast('Delete failed: ' + err.message, 'error');
    }
}

export async function resetPresets() {
    if (!confirm('Reset all presets to built-in defaults? Custom presets will be removed.')) return;
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
        const resp = await fetch('/api/model-defaults', {
            method: 'POST',
            headers,
            body: JSON.stringify({ model_name_or_repo: modelName, size_bytes: 0, tags: [] }),
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
    setOpt('modal-reasoning', preset.reasoning ? 'on' : 'off');
    numOrEmpty('modal-reasoning-budget', preset.reasoning_budget);
    setVal('modal-reasoning-budget-message', (preset.reasoning_budget_message || '').replace(/\n/g, '\\n'));
}

// ── Restart after preset save ──────────────────────────────────────────────────

function _offerRestartAfterPresetSave(presetId) {
    if (!presetId) return;

    const modal = document.createElement('div');
    modal.className = 'stop-choice-modal';
    modal.innerHTML = `
        <div class="stop-choice-card">
            <div class="stop-choice-title">Apply changes</div>
            <div class="stop-choice-actions">
                <button class="btn btn-stop-choice-welcome" id="restart-choice-yes">
                    Restart llama-server
                </button>
                <button class="btn btn-stop-choice-stay" id="restart-choice-no">
                    Not now
                </button>
            </div>
        </div>
    `;

    document.body.appendChild(modal);

    const yesBtn = modal.querySelector('#restart-choice-yes');
    const noBtn = modal.querySelector('#restart-choice-no');

    const removeModal = () => {
        if (modal && modal.parentNode) modal.parentNode.removeChild(modal);
    };

    noBtn.addEventListener('click', () => {
        noBtn.disabled = true;
        yesBtn.disabled = true;
        modal.style.transition = 'opacity 200ms ease';
        modal.style.opacity = '0';
        modal.style.pointerEvents = 'none';
        setTimeout(removeModal, 220);
    });

    yesBtn.addEventListener('click', async () => {
        yesBtn.disabled = true;
        noBtn.disabled = true;
        modal.style.opacity = '0.5';
        modal.style.pointerEvents = 'none';
        showToast('Restarting llama-server…', 'info');

        try {
            await _restartServerWithPreset(presetId);
        } catch (e) {
            showToast('Restart failed: ' + (e.message || String(e)), 'error');
        } finally {
            removeModal();
        }
    });
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

        await fetch('/api/kill-llama', {
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

    // Build config from preset
    const config = {
        preset_id: presetId,
        model_path: p.model_path || '',
        hf_repo: p.hf_repo || null,
        context_size: p.context_size || 128000,
        ctk: p.ctk || 'q8_0',
        ctv: p.ctv || 'f16',
        tensor_split: p.tensor_split || '',
        batch_size: p.batch_size || 2048,
        ubatch_size: p.ubatch_size || p.batch_size || 2048,
        no_mmap: !!p.no_mmap,
        port: p.port || 8001,
        ngram_spec: !!p.ngram_spec,
        parallel_slots: p.parallel_slots || 1,
        temperature: p.temperature,
        top_p: p.top_p,
        top_k: p.top_k,
        min_p: p.min_p,
        repeat_penalty: p.repeat_penalty,
        presence_penalty: p.presence_penalty ?? null,
        enable_thinking: p.enable_thinking ?? null,
        preserve_thinking: p.preserve_thinking ?? null,
        reasoning: p.reasoning || null,
        reasoning_budget: p.reasoning_budget ?? null,
        reasoning_budget_message: p.reasoning_budget_message || null,
        n_cpu_moe: p.n_cpu_moe,
        gpu_layers: p.gpu_layers ?? null,
        mlock: !!p.mlock,
        flash_attn: p.flash_attn || '',
        split_mode: p.split_mode || '',
        main_gpu: p.main_gpu ?? null,
        threads: p.threads ?? null,
        threads_batch: p.threads_batch ?? null,
        rope_scaling: p.rope_scaling || '',
        rope_freq_base: p.rope_freq_base ?? null,
        rope_freq_scale: p.rope_freq_scale ?? null,
        spec_type: p.spec_type || null,
        kv_unified: p.kv_unified ?? null,
        cache_ram_mib: p.cache_ram_mib ?? null,
        draft_model: p.draft_model || '',
        draft_min: p.draft_min ?? null,
        draft_max: p.draft_max ?? null,
        spec_ngram_size: p.spec_ngram_size ?? null,
        spec_draft_n_max: p.spec_draft_n_max ?? null,
        seed: p.seed ?? null,
        mmproj: p.mmproj || null,
        chat_template_file: p.chat_template_file || null,
        alias: p.alias || null,
        max_tokens: p.max_tokens ?? null,
        fit_enabled: p.fit_enabled ?? null,
        fit_target: p.fit_target || null,
        system_prompt_file: p.system_prompt_file || '',
        extra_args: p.extra_args || '',
        bind_host: p.bind_host || '127.0.0.1',
        api_key: p.api_key || null,
    };

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
    showToast('Starting llama-server…', 'info', 'Loading model on port ' + config.port, { duration: 12000 });
    try {
        await (await import('./spawn-readiness.js')).waitForSpawnReadiness(config.port);
    } catch (e) {
        throw new Error('Server did not become ready: ' + (e.message || e));
    }

    showToast('llama-server restarted', 'success', '', { duration: 6000 });
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
        const p = sessionState.presets.find(pr => pr.id === id);
        if (!p) {
            showToast('No preset selected', 'warn');
            return;
        }
        try {
            const auth = window.authHeaders ? window.authHeaders() : {};
            const copy = { ...p, name: p.name + ' (copy)' };
            delete copy.id;
            const resp = await fetch('/api/presets', {
                method: 'POST',
                headers: { ...auth, 'Content-Type': 'application/json' },
                body: JSON.stringify(copy),
            });

            if (!resp.ok) {
                const err = await resp.text().catch(() => 'Unknown error');
                showToast('Copy failed: ' + err, 'error');
                return;
            }

            const data = await resp.json();
            await loadPresets(data.preset?.id || null);
            showToast('Preset duplicated', 'success');
        } catch (err) {
            showToast('Copy failed: ' + err.message, 'error');
        }
    });

    // Delete preset from within the modal (only visible in edit mode)
    document.getElementById('preset-modal-delete')?.addEventListener('click', async () => {
        const id = document.getElementById('modal-preset-id').value;
        const p = sessionState.presets.find(pr => pr.id === id);
        if (!p) { showToast('No preset selected', 'warn'); return; }
        if (!confirm(`Delete preset "${p.name}"? This cannot be undone.`)) return;
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
      threadsInput.placeholder = `${pCores} recommended`;
    }
    if (batchThreadsInput && !batchThreadsInput.value) {
      batchThreadsInput.placeholder = `${pCores} recommended`;
    }
    if (hintEl && _presetIsUnified) {
      hintEl.textContent = `Apple Silicon: set both to your P-core count (${pCores}). GPU handles matrix ops; threads affect prefill speed and sampling overhead.`;
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
        pillEl.className = 'preset-context-pill';
        pillEl.textContent = pill.label;
        pillEl.onclick = () => {
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
