// ── File Browser ───────────────────────────────────────────────────────────────
// File/directory browser modal. Used by preset modal, session modal, settings.

import { escapeHtml } from '../core/format.js';

let fbTargetId = '';
let fbFilter = '';
let fbCurrentPath = '';
let fbContext = '';   // 'model' → hide mmproj files, badge-match base models
let initialized = false;

// Strip quant suffix to get model stem for mmproj matching.
function _fbModelStem(filename) {
    return filename
        .replace(/\.gguf$/i, '')
        .replace(/-?(Q\d[^.]*|IQ\d[^.]*|F16|BF16)$/i, '');
}

// Returns true if mmprojName looks like a companion for modelName.
function _mmprojMatchesModel(mmprojName, modelName) {
    const stem = _fbModelStem(modelName).toLowerCase();
    const mp   = mmprojName.toLowerCase();
    // Stem-in-mmproj: "Qwen2.5-VL-7B-Q4…" → stem "Qwen2.5-VL-7B" ∈ "qwen2.5-vl-7b-mmproj-f16"
    if (stem.length > 4 && mp.includes(stem)) return true;
    return false;
}

// Returns true if draftName looks like an MTP draft companion for modelName.
function _draftMatchesModel(draftName, modelName) {
    const stem = _fbModelStem(modelName).toLowerCase();
    // Strip quant suffix then strip MTP keyword + tail to get draft's base prefix
    const dr = draftName.toLowerCase()
        .replace(/\.gguf$/i, '')
        .replace(/[-_](?:mtp[-_]draft|draft[-_]model|mtp_small|mtp[-_]heads|mtp[-_]\d+|assistant)\b.*$/i, '');
    if (stem.length <= 4) return false;
    // QAT parity: a QAT-trained draft only works with QAT bases and vice versa
    const drQat = /-qat[-_]/.test(dr) || dr.endsWith('-qat');
    const stemQat = /-qat[-_]/.test(stem) || stem.endsWith('-qat');
    if (drQat !== stemQat) return false;
    // Direct containment covers the generic assistant case
    if (stem.includes(dr) || dr.includes(stem)) return true;
    // QAT/variant drafts diverge from the base name after a shared prefix
    // (e.g. "...-qat-q4_0-unquantized" vs "...-qat-UD"). Match when >= 5
    // leading dash-segments are identical so "gemma-4-31b-it-qat" qualifies
    // but a plain "gemma-4-31b-it" draft doesn't over-match a different family.
    const drSegs = dr.split('-');
    const stemSegs = stem.split('-');
    let shared = 0;
    for (let i = 0; i < Math.min(drSegs.length, stemSegs.length); i++) {
        if (drSegs[i] === stemSegs[i]) shared++;
        else break;
    }
    return shared >= 5;
}

export function openFileBrowser(targetId, filter, defaultPath, context) {
    fbTargetId = targetId;
    fbFilter = filter === 'dir' ? '' : (filter || '');
    fbContext = context || '';
    const modal = document.getElementById('file-browser-modal');
    const title = document.getElementById('fb-title');
    const selectBtn = modal.querySelector('.btn-modal-save');

    // Set title based on filter
    if (filter === 'gguf') {
        title.textContent = 'Browse Model Files';
    } else if (filter === 'executable') {
        title.textContent = 'Browse Executable';
    } else if (filter === 'dir') {
        title.textContent = 'Browse Directory';
    } else {
        title.textContent = 'Browse Files';
    }

    // Show/hide Select button based on mode
    if (filter === 'dir') {
        selectBtn.style.display = '';
        selectBtn.textContent = 'Select This Folder';
    } else {
        selectBtn.style.display = '';
        selectBtn.textContent = 'Select';
    }

    // Determine starting path: current input value → defaultPath → ''
    const current = document.getElementById(targetId).value;
    let startPath = defaultPath || '';
    if (current) {
        // Try to extract parent directory from current path
        const sep = current.includes('\\') ? '\\' : '/';
        const parts = current.split(sep);
        parts.pop();
        startPath = parts.join(sep) || (current.includes('\\') ? 'C:\\' : '/');
    }

    modal.classList.add('open');

    fileBrowserGo(startPath);
}

export function closeFileBrowser() {
    document.getElementById('file-browser-modal').classList.remove('open');
}

export async function fileBrowserGo(path) {
    const entriesEl = document.getElementById('fb-entries');
    entriesEl.innerHTML = '<div class="fb-empty">Loading...</div>';

    const params = new URLSearchParams();
    if (path) params.set('path', path);
    if (fbFilter) params.set('filter', fbFilter);

    try {
        const resp = await fetch('/api/browse?' + params, {
            headers: window.authHeaders ? window.authHeaders() : {},
        });
        const data = await resp.json();
        if (data.error) {
            // Handle "Path not allowed" with a clear message
            if (data.error === 'Path not allowed') {
                entriesEl.innerHTML =
                    '<div class="fb-empty">' +
                    'This path is not allowed. Only certain directories are accessible for security.' +
                    '</div>';
            } else {
                entriesEl.innerHTML = '<div class="fb-empty">' + escapeHtml(data.error) + '</div>';
            }
            return;
        }

        fbCurrentPath = data.path;
        document.getElementById('fb-path-input').value = data.path;
        if (data.entries.length === 0) {
            entriesEl.innerHTML = '<div class="fb-empty">Empty directory</div>';
            return;
        }

        // In model-selection context: hide mmproj and draft assistant files;
        // badge base models that have a companion.
        let entries = data.entries;
        let mmprojNames = [];
        let draftNames = [];
        if (fbContext === 'model') {
            // Mirror src/models/mod.rs is_draft_assistant_filename: unambiguous keywords
            // are safe at any size; broad keywords require size <= 3 GB to avoid
            // mis-tagging instruct-tuned main models.
            const isAssist = (e) => {
                const n = e.name.toLowerCase();
                const sz = e.size || 0;
                const isUnambiguous = n.includes('mtp-draft')
                    || n.includes('mtp_small')
                    || n.includes('mtp-heads')
                    || (n.startsWith('mtp-') && sz <= 3_000_000_000);
                if (isUnambiguous) return true;
                const isBroad = n.includes('assistant') || n.includes('draft-model');
                return isBroad && sz > 0 && sz <= 3_000_000_000;
            };
            mmprojNames = entries
                .filter(e => !e.is_dir && e.name.toLowerCase().includes('mmproj'))
                .map(e => e.name);
            draftNames = entries
                .filter(e => !e.is_dir && isAssist(e))
                .map(e => e.name);
            entries = entries.filter(e => {
                if (e.is_dir) return true;
                const n = e.name.toLowerCase();
                if (n.includes('mmproj')) return false;
                if (isAssist(e)) return false;
                return true;
            });
        }

        // eslint-disable-next-line no-unsanitized/property -- all server strings (name, path, size_display) wrapped in escapeHtml()
        entriesEl.innerHTML = entries.map(e => {
            const name = escapeHtml(e.name);
            const size = escapeHtml(e.size_display || '');
            if (e.is_dir) {
                return `<div class="fb-entry fb-entry-dir" data-path="${escapeHtml(e.path)}" title="${name}">` +
                    '<span class="fb-entry-icon">\u{1F4C1}</span>' +
                    '<span class="fb-entry-name">' + name + '</span></div>';
            } else {
                let badge = '';
                if (fbContext === 'model') {
                    if (mmprojNames.length > 0) {
                        const matched = mmprojNames.some(mp => _mmprojMatchesModel(mp, e.name));
                        if (matched) {
                            badge += '<span class="fb-mmproj-badge fb-mmproj-matched" title="A companion mmproj file was found in this folder">mmproj ✓</span>';
                        } else {
                            // Dir has unmatched mmproj files — softer hint
                            badge += '<span class="fb-mmproj-badge" title="mmproj file(s) present in this folder">mmproj</span>';
                        }
                    }
                    if (draftNames.length > 0) {
                        const matched = draftNames.some(d => _draftMatchesModel(d, e.name));
                        if (matched) {
                            badge += '<span class="fb-mtp-badge fb-mtp-matched" title="A companion MTP draft file was found in this folder">mtp ✓</span>';
                        } else {
                            badge += '<span class="fb-mtp-badge" title="MTP draft file(s) present in this folder">mtp</span>';
                        }
                    }
                }
                return `<div class="fb-entry fb-entry-file fb-match" data-path="${escapeHtml(e.path)}" title="${name}">` +
                    '<span class="fb-entry-icon">\u{1F4C4}</span>' +
                    '<span class="fb-entry-name">' + name + '</span>' +
                    badge +
                    '<span class="fb-entry-size">' + size + '</span></div>';
            }
        }).join('');
    } catch (err) {
        entriesEl.innerHTML = '<div class="fb-empty">Error: ' + escapeHtml(err.message) + '</div>';
    }
}

export function fileBrowserUp() {
    if (fbCurrentPath && fbCurrentPath !== '/') {
        const parts = fbCurrentPath.split('/');
        parts.pop();
        fileBrowserGo(parts.join('/') || '/');
    }
}

export function fileBrowserSelect(path) {
    const value = path || fbCurrentPath;
    const el = document.getElementById(fbTargetId);
    if (!el) { closeFileBrowser(); return; }

    if (el.tagName === 'SELECT') {
        // For <select> targets: add or update an option for this value
        let opt = el.querySelector(`option[value="${value.replace(/"/g, '\\"')}"]`);
        if (!opt) {
            const file = value.split(/[\\/]/).pop() || value;
            opt = document.createElement('option');
            opt.value = value;
            opt.textContent = file;
            el.appendChild(opt);
        }
        el.value = value;
        el.dispatchEvent(new Event('change', { bubbles: true }));
    } else {
        el.value = value;
        el.dispatchEvent(new Event('input', { bubbles: true }));
    }
    closeFileBrowser();
}

// ── Init ───────────────────────────────────────────────────────────────────────

export function initFileBrowser() {
    if (initialized) return;
    initialized = true;

    // Bind file browser buttons
    const fbClose = document.getElementById('filebrowser-close');
    if (fbClose) fbClose.addEventListener('click', closeFileBrowser);

    const fbCancel = document.getElementById('filebrowser-cancel');
    if (fbCancel) fbCancel.addEventListener('click', closeFileBrowser);

    const fbSelect = document.getElementById('filebrowser-select');
    if (fbSelect) fbSelect.addEventListener('click', () => fileBrowserSelect());

    const fbUp = document.getElementById('filebrowser-up');
    if (fbUp) fbUp.addEventListener('click', fileBrowserUp);

    // Bind path input Enter key
    const fbPathInput = document.getElementById('fb-path-input');
    if (fbPathInput) {
        fbPathInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') fileBrowserGo(fbPathInput.value);
        });
    }

    // Event delegation for dynamically generated entries
    const entriesEl = document.getElementById('fb-entries');
    if (entriesEl) {
        entriesEl.addEventListener('click', (e) => {
            const entry = e.target.closest('.fb-entry');
            if (!entry) return;
            const path = entry.dataset.path;
            if (entry.classList.contains('fb-entry-dir')) {
                fileBrowserGo(path);
            } else {
                fileBrowserSelect(path);
            }
        });
    }

    // Modal overlay click
    const modal = document.getElementById('file-browser-modal');
    if (modal) {
        modal.addEventListener('click', e => {
            if (e.target === e.currentTarget) closeFileBrowser();
        });
    }

    // Escape key
    document.addEventListener('keydown', e => {
        if (e.key === 'Escape' && modal && modal.classList.contains('open')) {
            closeFileBrowser();
            e.stopImmediatePropagation();
        }
    }, true);
}
