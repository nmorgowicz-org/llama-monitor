// ── Deferred File Browser Launcher ───────────────────────────────────────────
// Shared lazy entrypoint for the file browser feature.

import { showToast } from './toast.js';

let fileBrowserPromise = null;

async function ensureFileBrowser() {
    if (!fileBrowserPromise) {
        fileBrowserPromise = import('./file-browser.js').then(mod => {
            mod.initFileBrowser();
            return mod;
        });
    }
    return fileBrowserPromise;
}

export async function openDeferredFileBrowser(targetId, filter, defaultPath, context) {
    const mod = await ensureFileBrowser();
    mod.openFileBrowser(targetId, filter, defaultPath, context);
}

async function getChatTemplateLibraryPath() {
    const headers = window.authHeaders ? window.authHeaders() : {};
    const resp = await fetch('/api/chat-template/dir', { headers });
    const data = await resp.json().catch(() => ({}));
    if (!resp.ok || !data?.ok || !data?.path) {
        throw new Error(data?.error || `HTTP ${resp.status}`);
    }
    return data.path;
}

export async function openChatTemplateLibraryBrowser(targetId) {
    const defaultPath = await getChatTemplateLibraryPath();
    return openDeferredFileBrowser(targetId, '', defaultPath);
}

export async function uploadChatTemplateFromBrowser() {
    const picker = document.createElement('input');
    picker.type = 'file';
    picker.accept = '.jinja,.jinja2,.txt,text/plain';

    return new Promise((resolve, reject) => {
        let settled = false;
        const settleResolve = (value) => {
            if (settled) return;
            settled = true;
            resolve(value);
        };
        const settleReject = (err) => {
            if (settled) return;
            settled = true;
            reject(err);
        };
        const handleWindowFocus = () => {
            window.setTimeout(() => {
                if (!picker.files?.length) settleResolve(null);
            }, 0);
        };
        window.addEventListener('focus', handleWindowFocus, { once: true });
        picker.addEventListener('change', async () => {
            const file = picker.files?.[0];
            if (!file) {
                settleResolve(null);
                return;
            }
            try {
                const template = await file.text();
                if (!template.trim()) throw new Error('Template file is empty');
                const headers = window.authHeaders
                    ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                    : { 'Content-Type': 'application/json' };
                const resp = await fetch('/api/chat-template/upload', {
                    method: 'POST',
                    headers,
                    body: JSON.stringify({ template }),
                });
                const data = await resp.json().catch(() => ({}));
                if (!resp.ok || !data?.ok || !data?.path) {
                    throw new Error(data?.error || `HTTP ${resp.status}`);
                }
                settleResolve({
                    path: data.path,
                    templateId: data.template_id || null,
                    filename: file.name,
                });
            } catch (err) {
                showToast('Template upload failed: ' + (err.message || String(err)), 'error');
                settleReject(err);
            }
        }, { once: true });
        picker.click();
    });
}
