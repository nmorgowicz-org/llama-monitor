// ── File Browser ───────────────────────────────────────────────────────────────
// File/directory browser modal. Used by preset modal, session modal, settings.

import { escapeHtml } from '../core/format.js';

let fbTargetId = '';
let fbFilter = '';
let fbCurrentPath = '';
let initialized = false;

export function openFileBrowser(targetId, filter) {
    fbTargetId = targetId;
    fbFilter = filter === 'dir' ? '' : (filter || '');
    const modal = document.getElementById('file-browser-modal');

    const current = document.getElementById(targetId).value;
    let startPath = '';
    if (current) {
        const parts = current.split('/');
        parts.pop();
        startPath = parts.join('/') || '/';
    }

    const selectBtn = modal.querySelector('.btn-modal-save');
    selectBtn.style.display = filter === 'dir' ? '' : 'none';
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
        const resp = await fetch('/api/browse?' + params);
        const data = await resp.json();
        if (data.error) {
            entriesEl.innerHTML = '<div class="fb-empty">' + escapeHtml(data.error) + '</div>';
            return;
        }

        fbCurrentPath = data.path;
        document.getElementById('fb-path-input').value = data.path;
        if (data.entries.length === 0) {
            entriesEl.innerHTML = '<div class="fb-empty">Empty directory</div>';
            return;
        }

        // eslint-disable-next-line no-unsanitized/property -- all server strings (name, path, size_display) wrapped in escapeHtml()
        entriesEl.innerHTML = data.entries.map(e => {
            const name = escapeHtml(e.name);
            const size = escapeHtml(e.size_display || '');
            if (e.is_dir) {
                return `<div class="fb-entry fb-entry-dir" data-path="${escapeHtml(e.path)}">` +
                    '<span class="fb-entry-icon">\u{1F4C1}</span>' +
                    '<span class="fb-entry-name">' + name + '</span></div>';
            } else {
                return `<div class="fb-entry fb-entry-file fb-match" data-path="${escapeHtml(e.path)}">` +
                    '<span class="fb-entry-icon">\u{1F4C4}</span>' +
                    '<span class="fb-entry-name">' + name + '</span>' +
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
    document.getElementById(fbTargetId).value = path || fbCurrentPath;
    document.getElementById(fbTargetId).dispatchEvent(new Event('input', { bubbles: true }));
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
