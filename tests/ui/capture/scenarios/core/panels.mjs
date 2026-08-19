// Scenario: panels
// Extracted from tests/ui/capture.mjs (Phase A3).
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp, switchTab } from '../../harness/browser.mjs';
import { createFreshChat, sendChatPrompt, waitForChatComplete } from '../../harness/chat.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureCloseUp, captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    await switchTab(page, 'chat');
    await sleep(500);

    // Create a chat with a short, safe conversation so panels have real content behind them
    await createFreshChat(page);
    await sendChatPrompt(page, 'Compare REST and gRPC in 4 short bullets.');
    await waitForChatComplete(page);
    await sleep(1500);

    const styleBtn = await page.$('#btn-chat-style');
    if (styleBtn) {
        await styleBtn.click();
        await sleep(500);
        await captureShot(page, 'panels-chat-style.png', { fullPage: true });
        await captureCloseUp(page, '#chat-style-sidebar', 'panels-chat-style.png', options);
        await styleBtn.click();
        await sleep(300);
    }

    const behaviorBtn = await page.$('#btn-behavior');
    if (behaviorBtn) {
        await behaviorBtn.click();
        await sleep(500);
        await captureShot(page, 'panels-behavior-settings.png', { fullPage: true });
        await captureCloseUp(page, '#behavior-sidebar', 'panels-behavior-settings.png', options);
        await behaviorBtn.click();
        await sleep(300);
    }

    const responseBtn = await page.$('#btn-model-params');
    if (responseBtn) {
        await responseBtn.click();
        await sleep(500);
        await captureShot(page, 'panels-model-settings.png', { fullPage: true });
        await captureCloseUp(page, '#model-params-sidebar', 'panels-model-settings.png', options);
        await responseBtn.click();
        await sleep(300);
    }

    // Send a real message so the prompt debug modal has actual content
    await sendChatPrompt(page, 'Explain the difference between TCP and UDP in 3 bullet points.');
    await waitForChatComplete(page);
    await sleep(1500);

    const debugDropdownExists = await page.evaluate(() =>
        !!document.getElementById('btn-debug-dropdown')
    );
    if (debugDropdownExists) {
        try {
            // Use evaluate to avoid Puppeteer click issues on dropdown button
            await page.evaluate(() => {
                const btn = document.getElementById('btn-debug-dropdown');
                if (btn) btn.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
            });
            await page.waitForFunction(() => {
                const menu = document.getElementById('debug-dropdown-menu');
                return !!menu && menu.classList.contains('open');
            }, { timeout: 5000 });

            // Click the Prompt Debug menu item via evaluate
            await page.evaluate(() => {
                const btn = document.getElementById('btn-debug-prompt');
                if (btn) btn.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
            });
            await sleep(400);

            // Wait for modal to open; accept either content or empty-state
            await page.waitForFunction(() => {
                const modal = document.getElementById('debug-prompt-modal');
                return !!modal && modal.classList.contains('active');
            }, { timeout: 5000 });

            // Accept either: real debug content visible, or empty state visible
            const hasContent = await page.evaluate(() => {
                const content = document.getElementById('debug-content');
                const empty = document.getElementById('debug-empty-state');
                const hasReal = !!content && !content.classList.contains('hidden');
                const hasEmpty = !!empty && !empty.classList.contains('hidden');
                return hasReal || hasEmpty;
            });
            if (!hasContent) {
                console.log('[CAPTURE] Debug modal open but no content/empty-state; treating as success anyway');
            }

            await sleep(500);
            await captureShot(page, 'panels-prompt-debug.png', { fullPage: true });
            await captureCloseUp(page, '#debug-prompt-modal', 'panels-prompt-debug.png', options);
            await page.keyboard.press('Escape');
            await sleep(300);
        } catch (e) {
            console.log(`[CAPTURE] Debug prompt modal failed, skipping... ${e.message}`);
        }
    }
}

// ── Dashboard ───────────────────────────────────────────────────────────────────
// Server tab, GPU section.
