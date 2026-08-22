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
    const stableTag = await page.evaluate(() => {
        const stableRow = [...document.querySelectorAll('.llama-version-row')].find(row =>
            /^v\d+\.\d+\.\d+$/.test(row.dataset.tag || '')
        );
        if (stableRow) stableRow.click();
        return stableRow?.dataset.tag || '';
    });
    if (stableTag) {
        await sleep(800);
        const stableNotes = await page.evaluate(() => ({
            tag: document.getElementById('llama-version-notes-tag')?.textContent || '',
            changelogItems: document.querySelectorAll('#llama-version-notes-body li').length,
        }));
        const stableRowState = await page.evaluate((tag) => {
            const row = [...document.querySelectorAll('.llama-version-row')].find(item => item.dataset.tag === tag);
            return {
                hasInstallButton: Boolean(row?.querySelector('.llama-version-install-btn')),
                hasNotesOnlyBadge: row?.textContent.includes('Notes only') || false,
            };
        }, stableTag);
        if (
            stableNotes.tag !== stableTag ||
            stableNotes.changelogItems === 0 ||
            stableRowState.hasInstallButton ||
            !stableRowState.hasNotesOnlyBadge
        ) {
            throw new Error(`Versioned release notes were not rendered as a list: ${stableTag}`);
        }
        await captureShot(page, 'llama-updater-stable-release-notes.png', { fullPage: true });
    }

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
