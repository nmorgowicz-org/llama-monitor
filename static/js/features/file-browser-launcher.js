// ── Deferred File Browser Launcher ───────────────────────────────────────────
// Shared lazy entrypoint for the file browser feature.

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

export async function openDeferredFileBrowser(targetId, filter) {
    const mod = await ensureFileBrowser();
    mod.openFileBrowser(targetId, filter);
}
