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

    // Click on the second release row to show its notes (latest is auto-selected)
    await page.evaluate(() => {
        const rows = document.querySelectorAll('.llama-version-row');
        if (rows.length > 1) {
            rows[1].click();
        }
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
