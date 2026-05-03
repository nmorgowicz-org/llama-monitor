// ── Updates ───────────────────────────────────────────────────────────────────
// App version display, update check, release notes, and self-update flow.

import { compareVersions, escapeHtml } from '../core/format.js';
import { renderMd } from './chat-render.js';

// Holds the current pending release object
let _pendingRelease = null;
let initialized = false;
let updateCheckStarted = false;

// ── App Version ───────────────────────────────────────────────────────────────

export function initAppVersion() {
    const el = document.getElementById('app-version');
    if (el && typeof APP_VERSION !== 'undefined') {
        el.textContent = `v${APP_VERSION}`;
    }
}

// ── Update Check ──────────────────────────────────────────────────────────────

export async function checkForUpdate() {
    if (updateCheckStarted) return;
    updateCheckStarted = true;

    try {
        const resp = await fetch('/api/remote-agent/releases/latest');
        if (!resp.ok) return;
        const data = await resp.json();
        const latest = data.release || data;
        if (typeof APP_VERSION === 'undefined') return;

        const current = APP_VERSION.replace(/^v/, '');
        const available = latest.tag_name.replace(/^v/, '');

        if (compareVersions(available, current) > 0) {
            showUpdatePill(latest);
        }
    } catch (e) {
        console.debug('Update check failed:', e.message);
    }
}

function showUpdatePill(release) {
    const dismissed = JSON.parse(localStorage.getItem('update-dismissed') || '{}');
    if (dismissed[release.tag_name] && Date.now() - dismissed[release.tag_name] < 86400000) {
        return;
    }
    _pendingRelease = release;
    const pill = document.getElementById('update-pill');
    const text = document.getElementById('update-pill-text');
    if (pill && text) {
        text.textContent = `${release.tag_name} available`;
        pill.style.display = 'flex';
    }
}

// ── Release Notes ─────────────────────────────────────────────────────────────

export function openReleaseNotes() {
    const release = _pendingRelease;
    if (!release?.tag_name) return;

    const panel    = document.getElementById('release-notes-panel');
    const overlay  = document.getElementById('release-notes-overlay');
    const title    = document.getElementById('release-notes-title');
    const fromEl   = document.getElementById('release-notes-version-from');
    const body     = document.getElementById('release-notes-body');
    const link     = document.getElementById('release-notes-link');
    const updateBtn = document.getElementById('release-notes-update-btn');

    title.textContent = release.tag_name;
    if (fromEl && typeof APP_VERSION !== 'undefined') {
        fromEl.textContent = `from v${APP_VERSION}`;
    }

    link.href = release.html_url || '#';

    // eslint-disable-next-line no-unsanitized/property -- release notes rendered via marked.js (same path as LLM chat output); fallback is hardcoded
    body.innerHTML = release.body
        ? renderMd(release.body)
        : '<p>No release notes available.</p>';

    _resetUpdateBtn(updateBtn);

    panel.style.display = 'flex';
    overlay.style.display = 'block';
    panel.offsetHeight; // trigger reflow for CSS transition
    panel.classList.add('open');
}

function _resetUpdateBtn(btn) {
    if (!btn) return;
    btn.disabled = false;
    btn.dataset.state = '';
    btn.innerHTML = `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="16 16 12 12 8 16"/><line x1="12" y1="12" x2="12" y2="21"/><path d="M20.39 18.39A5 5 0 0018 9h-1.26A8 8 0 103 16.3"/></svg> Update & Restart`;
}

function closeReleaseNotes() {
    const panel   = document.getElementById('release-notes-panel');
    const overlay = document.getElementById('release-notes-overlay');
    panel.classList.remove('open');
    setTimeout(() => {
        panel.style.display = 'none';
        overlay.style.display = 'none';
    }, 300);
}

function dismissUpdate() {
    if (!_pendingRelease?.tag_name) return;
    const dismissed = JSON.parse(localStorage.getItem('update-dismissed') || '{}');
    dismissed[_pendingRelease.tag_name] = Date.now();
    localStorage.setItem('update-dismissed', JSON.stringify(dismissed));
    const pill = document.getElementById('update-pill');
    if (pill) pill.style.display = 'none';
    closeReleaseNotes();
}

// ── Self-Update ───────────────────────────────────────────────────────────────

async function triggerSelfUpdate() {
    const btn = document.getElementById('release-notes-update-btn');
    if (!btn || btn.dataset.state === 'loading') return;

    btn.dataset.state = 'loading';
    btn.disabled = true;
    btn.innerHTML = `<svg class="chat-send-spinner" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/></svg> Downloading…`;

    try {
        const resp = await fetch('/api/self-update', { method: 'POST' });
        const data = await resp.json();

        if (!data.ok) {
            throw new Error(data.error || 'Update failed');
        }

        btn.innerHTML = `<svg class="chat-send-spinner" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/></svg> Restarting…`;
        _pollForReconnect(data.tag_name);

    } catch (e) {
        btn.dataset.state = 'error';
        btn.disabled = false;
        btn.innerHTML = `⚠ ${escapeHtml(e.message)} — retry?`;
    }
}

function _pollForReconnect(newVersion) {
    let attempts = 0;
    const timer = setInterval(async () => {
        attempts++;
        try {
            const r = await fetch('/', { method: 'HEAD', cache: 'no-store' });
            if (r.ok) {
                clearInterval(timer);
                location.reload();
            }
        } catch (_) { /* expected while process is restarting */ }
        if (attempts >= 30) {
            clearInterval(timer);
            const btn = document.getElementById('release-notes-update-btn');
            if (btn) {
                btn.dataset.state = '';
                btn.disabled = false;
                btn.innerHTML = 'Relaunch the app to finish';
            }
        }
    }, 1000);
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initUpdates() {
    if (initialized) return;
    initialized = true;

    // Call setup functions
    initAppVersion();

    // Bind update pill click
    const updatePill = document.getElementById('update-pill');
    if (updatePill) updatePill.addEventListener('click', openReleaseNotes);

    // Bind release notes panel buttons
    document.getElementById('release-notes-close')?.addEventListener('click', closeReleaseNotes);
    document.getElementById('release-notes-overlay')?.addEventListener('click', closeReleaseNotes);
    document.getElementById('release-notes-dismiss')?.addEventListener('click', dismissUpdate);
    document.getElementById('release-notes-update-btn')?.addEventListener('click', triggerSelfUpdate);
}
