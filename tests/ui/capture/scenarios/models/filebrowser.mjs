// Scenario: filebrowser
// Extracted from tests/ui/capture.mjs (Phase A3).
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;

    try {
        await gotoApp(page, baseUrl);

        // Open Settings modal (use evaluate to avoid Puppeteer click failure)
        const settingsBtnExists = await page.evaluate(() => {
            const btn = document.getElementById('sidebar-btn-settings');
            return !!btn;
        });
        if (!settingsBtnExists) {
            console.log('[CAPTURE] Settings button not found');
            return;
        }
        await page.evaluate(() => {
            const btn = document.getElementById('sidebar-btn-settings');
            if (btn) btn.click();
        });
        await sleep(600);

        // Ensure Settings modal is visible
        const settingsModal = await page.$('#settings-modal');
        if (!settingsModal) {
            console.log('[CAPTURE] Settings modal not found');
            return;
        }

        // Switch to Advanced tab
        await page.evaluate(() => {
            const tab = document.querySelector('.settings-tab[data-tab="advanced"]');
            if (tab) tab.click();
        });
        await sleep(600);

        // Open Config modal from Advanced tab
        const configBtnExists = await page.evaluate(() =>
            !!document.getElementById('settings-open-config-btn')
        );
        if (!configBtnExists) {
            console.log('[CAPTURE] Open Config button not found');
            return;
        }
        await page.evaluate(() => {
            const btn = document.getElementById('settings-open-config-btn');
            if (btn) btn.click();
        });
        await sleep(800);

        // Ensure Config modal is visible
        const configModal = await page.$('#config-modal');
        if (!configModal) {
            console.log('[CAPTURE] Config modal not found');
            return;
        }

        // Scroll to llama-server executable section
        await page.evaluate(() => {
            const section = document.querySelector('#config-modal .modal-section');
            if (section) {
                section.scrollIntoView({ behavior: 'instant', block: 'start' });
            }
        });
        await sleep(400);

        // Capture the llama-server executable section with Browse button visible
        await captureShot(page, 'filebrowser-config-browse-btn.png', { fullPage: true });

        // Ensure Browse button is visible and scroll into view if needed
        const browseBtn = await page.$('#config-browse-server-path');
        if (!browseBtn) {
            console.log('[CAPTURE] Browse button for server path not found');
            return;
        }

        // Scroll the button into view within the modal
        await page.evaluate(() => {
            const btn = document.getElementById('config-browse-server-path');
            if (btn) {
                btn.scrollIntoView({ behavior: 'instant', block: 'center' });
            }
        });
        await sleep(300);

        // Use evaluate to trigger click directly (more robust)
        await page.evaluate(() => {
            const btn = document.getElementById('config-browse-server-path');
            if (btn) {
                btn.click();
            }
        });
        await sleep(1000);

        // Ensure file browser modal is visible
        const fileBrowserModal = await page.$('#file-browser-modal');
        if (!fileBrowserModal) {
            console.log('[CAPTURE] File browser modal not found');
            return;
        }

        // Wait for entries to load so hint and list are visible
        await page.waitForSelector('#fb-entries .fb-entry', { timeout: 3000 }).catch(() => {});
        await sleep(400);

        // Capture the file browser modal open
        await captureShot(page, 'filebrowser-modal-open.png', { fullPage: true });

        // Close file browser modal
        await page.keyboard.press('Escape');
        await sleep(300);

        // Close Config modal
        await page.keyboard.press('Escape');
        await sleep(300);

        // Close Settings modal
        await page.keyboard.press('Escape');
        await sleep(300);
    } catch (e) {
        console.log('[CAPTURE] Filebrowser scenario failed:', e.message);
    }
}

// ── Tune Panel ────────────────────────────────────────────────────────────────
// Performance benchmark panel on the server tab.
