// Scenario: llama-updater
// Extracted from tests/ui/capture.mjs (Phase A3).
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    // Simulate an available llama.cpp binary update pill
    await page.evaluate(() => {
        const pill = document.getElementById('llama-pill');
        const verSpan = document.getElementById('llama-pill-version');
        if (pill && verSpan) {
            verSpan.textContent = 'llama.cpp · ↑ b4620';
            pill.classList.remove('llama-pill-idle');
            pill.classList.add('llama-pill-update');
            pill.style.display = 'flex';
            pill.title = 'Update available: b4500 → b4620. Click to update.';
        }
    });
    await sleep(600);
    await captureShot(page, 'llama-updater-pill.png', { fullPage: true });

    // Open the llama.cpp version modal — shows release list + notes panel
    await page.evaluate(async () => {
        const pill = document.getElementById('llama-pill');
        if (pill) pill.click();
    });
    await page.waitForSelector('#llama-version-modal.open', { timeout: 8000 }).catch(() => {
        console.warn('[CAPTURE] llama-version-modal did not open; may not have binary installed.');
    });
    // Wait for release list to populate (real GitHub API call)
    await sleep(2000);
    const latestRelease = await page.evaluate(async () => {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const response = await fetch('/api/llama-binary/latest', { headers });
        return response.json();
    });
    if (!/^b\d+$/.test(latestRelease.tag || '') || !Number.isInteger(latestRelease.build)) {
        throw new Error(`Latest installable release was not a nightly build: ${latestRelease.tag || 'unknown'}`);
    }

    // Versioned releases publish bare commit lines; keep a regression capture
    // proving those lines render as a readable list rather than one paragraph.
    // The modal pins the latest versioned (stable) release even when it has
    // aged out of the newest-8 window, so this check stays active as
    // nightlies publish on top of it.
    const stableTag = await page.evaluate(() => {
        const stableRow = [...document.querySelectorAll('.llama-version-row')].find(row =>
            /^v\d+\.\d+\.\d+$/.test(row.dataset.tag || '')
        );
        if (stableRow) stableRow.click();
        return stableRow?.dataset.tag || '';
    });
    if (!stableTag) {
        throw new Error('Latest versioned (stable) release was not present in the version modal');
    }
    await sleep(800);
    const stableNotes = await page.evaluate(() => ({
        tag: document.getElementById('llama-version-notes-tag')?.textContent || '',
        changelogItems: document.querySelectorAll('#llama-version-notes-body li').length,
    }));
    const stableRowState = await page.evaluate(async (tag) => {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const response = await fetch('/api/llama-binary/releases', { headers });
        const data = await response.json();
        const item = (data.releases ?? []).find(r => r.tag === tag);
        const row = [...document.querySelectorAll('.llama-version-row')].find(item => item.dataset.tag === tag);
        return {
            installable: item?.installable ?? null,
            hasInstallButton: Boolean(row?.querySelector('.llama-version-install-btn')),
            hasUnavailableBadge:
                row?.textContent.includes('Notes only') ||
                row?.textContent.includes('No build for your platform') ||
                false,
        };
    }, stableTag);
    if (stableNotes.tag !== stableTag || stableNotes.changelogItems === 0) {
        throw new Error(`Versioned release notes were not rendered as a list: ${stableTag}`);
    }
    if (stableRowState.installable) {
        if (!stableRowState.hasInstallButton) {
            throw new Error(`Installable stable release is missing its install button: ${stableTag}`);
        }
    } else if (stableRowState.hasInstallButton || !stableRowState.hasUnavailableBadge) {
        throw new Error(`Stable release row affordances are inconsistent: ${stableTag}`);
    }
    await captureShot(page, 'llama-updater-stable-release-notes.png', { fullPage: true });

    // Keep the beta/nightly notes capture independent of release ordering.
    await page.evaluate(() => {
        const betaRow = [...document.querySelectorAll('.llama-version-row')].find(row =>
            /^b\d+$/.test(row.dataset.tag || '')
        );
        if (betaRow) betaRow.click();
    });
    await sleep(800);
    await captureShot(page, 'llama-updater-version-modal.png', { fullPage: true });

    // Close the modal so later captures aren't obscured
    await page.evaluate(() => {
        const closeBtn = document.getElementById('llama-version-modal-close');
        if (closeBtn) closeBtn.click();
    });
    await sleep(300);
}

// ── Chat History Q&A Panel ────────────────────────────────────────────────────
// Slide-in panel for asking questions about chat history.
