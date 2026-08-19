// Scenario: chat
// Extracted from tests/ui/capture.mjs (Phase A3).
import { connectSource } from '../../harness/source.mjs';
import { gotoApp, switchTab } from '../../harness/browser.mjs';
import { sendChatPrompt, waitForChatComplete } from '../../harness/chat.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureCloseUp, captureShot, cleanupScreenshotTabs } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    await gotoApp(ctx.page, ctx.baseUrl);
    const source = await connectSource(ctx.page, options);
    try {
        await runChat(ctx, options);
    } finally {
        await source.teardown();
    }
}

async function runChat(ctx, options) {
    const { page, baseUrl } = ctx;

    await switchTab(page, 'chat');
    await sleep(500);

    // Create a fresh chat with a short, safe conversation
    await cleanupScreenshotTabs(page);
    await page.evaluate(async () => {
        const { addChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        addChatTab();
        renderChatTabs();
        renderChatMessages();
    });
    await sleep(300);

    await sendChatPrompt(page, 'Explain how a database index speeds up queries in 3 bullet points.');
    await waitForChatComplete(page);
    await sleep(2000);

    await captureShot(page, 'chat-chat.png', { fullPage: true });

    const telemetryToggle = await page.$('#chat-telemetry-btn');
    if (telemetryToggle) {
        await telemetryToggle.click();
        await sleep(500);
        await captureShot(page, 'chat-chat-telemetry.png', { fullPage: true });
        const telemetryPin = await page.$('#chat-telemetry-pin-btn');
        if (telemetryPin) {
            await telemetryPin.click();
            await sleep(500);
            await captureShot(page, 'chat-chat-telemetry-pinned.png', { fullPage: true });
        }
    }
    // File menu
    const fileBtn = await page.$('#chat-file-btn');
    if (fileBtn) {
        try {
            await fileBtn.click();
            await page.waitForSelector('#chat-file-menu:not(.hidden)', { timeout: 2000 });
            await captureCloseUp(page, '#chat-file-menu', 'chat-file-menu.png', options);
            await page.keyboard.press('Escape');
            await sleep(300);
        } catch {
            console.log('[CAPTURE] File menu open failed, skipping...');
        }
    }

    // Focus mode
    try {
        await page.locator('#chat-focus-mode-btn').click();
        await page.waitForFunction(() => document.body.classList.contains('chat-focus-mode'), { timeout: 2000 });
        await sleep(400);
        await captureShot(page, 'chat-focus-mode.png', { fullPage: true });
        await captureCloseUp(page, '#focus-mode-exit-pill', 'chat-focus-mode-pill.png', options);
        await page.locator('#focus-mode-exit-beacon').click();
        await page.waitForFunction(() => !document.body.classList.contains('chat-focus-mode'), { timeout: 2000 });
        await sleep(300);
    } catch {
        console.log('[CAPTURE] Focus mode capture failed, skipping...');
    }

    await cleanupScreenshotTabs(page);

    await switchTab(page, 'logs');
    await page.waitForSelector('#logs-empty-state.visible', { timeout: 10000 });
    await captureShot(page, 'chat-logs.png', { fullPage: true });
}

// ── Guided Generation ───────────────────────────────────────────────────────────
// Suggestions, quick guide, director, surprise, explicit mode, context notes.
