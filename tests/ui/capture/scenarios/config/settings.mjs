// Scenario: settings
// Extracted from tests/ui/capture.mjs (Phase A3).
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp, switchTab } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureCloseUp, captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    await switchTab(page, 'chat');
    await sleep(500);

    // Settings modal via button click
    try {
        await page.evaluate(() => { window.openSettingsModal?.(); });
        await page.waitForSelector('#settings-modal.open', { timeout: 5000 });
        await sleep(800);
        await captureShot(page, 'settings-settings-modal.png', { fullPage: true });
        await captureCloseUp(page, '#settings-modal', 'settings-settings-modal.png', options);

        const perfTab = await page.$('#settings-modal .settings-tab[data-tab="performance"]');
        if (perfTab) {
            await perfTab.click();
            await sleep(500);
            await page.evaluate(() => {
                const hidden = document.getElementById('settings-sleep-mode-when-hidden');
                const idle = document.getElementById('settings-sleep-mode-idle');
                if (hidden) hidden.checked = true;
                if (idle) idle.value = '600';
            });
            await sleep(100);
            await captureShot(page, 'settings-settings-performance.png', { fullPage: true });
            await captureCloseUp(page, '#settings-modal', 'settings-settings-performance.png', options);
        }

        const loadersTab = await page.$('#settings-modal .settings-tab[data-tab="loaders"]');
        if (loadersTab) {
            await loadersTab.click();
            await sleep(500);
            await captureShot(page, 'settings-settings-loaders.png', { fullPage: true });
            await captureCloseUp(page, '#settings-modal', 'settings-settings-loaders.png', options);
        }

        await page.keyboard.press('Escape');
        await sleep(300);
    } catch (e) {
        console.log('[CAPTURE] Settings modal failed, skipping...');
    }

    // Persona modal
    const personaBtn = await page.$('#chat-persona-btn');
    if (personaBtn) {
        try {
            await personaBtn.click();
            let personaMenuOpened = true;
            try {
                await page.waitForSelector('#chat-persona-menu:not(.hidden)', { timeout: 1200 });
            } catch {
                personaMenuOpened = false;
            }
            if (personaMenuOpened) {
                await page.waitForSelector('#chat-persona-menu .chat-persona-menu-item', { timeout: 5000 });
                await page.click('#chat-persona-menu .chat-persona-menu-item');
                await sleep(250);
                await personaBtn.click();
                await page.waitForSelector('#chat-persona-menu:not(.hidden)', { timeout: 5000 });
                const manageTemplatesBtn = await page.$('#chat-persona-edit-prompt');
                if (manageTemplatesBtn) {
                    await manageTemplatesBtn.click();
                }
            } else {
                await page.evaluate(() => {
                    document.getElementById('chat-persona-edit-prompt')?.click();
                });
            }
            await page.waitForSelector('#template-manager-modal.active', { timeout: 5000 });
            await page.waitForSelector('#template-manager-modal .template-list-item', { timeout: 5000 });
            await sleep(500);
            await page.evaluate(() => {
                document.querySelector('#template-manager-modal .template-list-item')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
                const details = document.querySelector('#persona-explicit-policies details');
                if (details) details.open = true;
            });
            await sleep(600);
            await captureShot(page, 'guided-gen-persona-modal.png', { fullPage: true });
            await captureCloseUp(page, '#template-manager-modal', 'guided-gen-persona-modal.png', options);
            await page.click('#template-manager-close');
            await page.waitForSelector('#template-manager-modal.active', { hidden: true, timeout: 5000 });
            await sleep(300);
        } catch (e) {
            console.log('[CAPTURE] Persona modal failed, skipping...');
        }
    }

    // Keyboard shortcuts via Ctrl+/
    try {
        await page.keyboard.down('Control');
        await page.keyboard.press('/');
        await page.keyboard.up('Control');
        await page.waitForSelector('#keyboard-shortcuts-modal.open', { timeout: 5000 });
        await sleep(500);
        await captureShot(page, 'panels-keyboard-shortcuts.png', { fullPage: true });
        await captureCloseUp(page, '#keyboard-shortcuts-modal', 'panels-keyboard-shortcuts.png', options);
        await page.click('#keyboard-shortcuts-modal .shortcuts-close');
        await page.waitForSelector('#keyboard-shortcuts-modal.open', { hidden: true, timeout: 5000 });
        await sleep(300);
    } catch (e) {
        console.log('[CAPTURE] Keyboard shortcuts modal failed, skipping...');
    }
}

// ── Chat Panels ─────────────────────────────────────────────────────────────────
// Behavior, model params, style panels, debug prompt.

// ── Models modal ─────────────────────────────────────────────────────────────
// Seeds fake GGUF files so the models modal has real cards to show.

// scenarioModels is called with a pre-seeded models dir passed via --models-dir
// CLI flag. The runCli() path seeds fake .gguf files and passes the dir when
// spawning the server — see the 'models' entry in SCENARIOS for the wrapper.
