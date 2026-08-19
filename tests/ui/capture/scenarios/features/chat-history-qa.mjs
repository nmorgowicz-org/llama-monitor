// Scenario: chat-history-qa
// Extracted from tests/ui/capture.mjs (Phase A3).
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp, switchTab } from '../../harness/browser.mjs';
import { createFreshChat } from '../../harness/chat.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureCloseUp, captureShot, cleanupScreenshotTabs } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    await switchTab(page, 'chat');
    await sleep(500);

    // Create a chat with some content so the panel has context
    await createFreshChat(page);
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        const tab = chat.tabs[chat.tabs.length - 1];
        if (tab) {
            tab.messages = [
                { role: 'user', content: 'Start a detective story set in 1940s Chicago.' },
                { role: 'assistant', content: 'The rain drummed against the window as Detective Malone stared at the case file. Another missing person, another dead end.' },
                { role: 'user', content: 'Introduce a suspect with a hidden motive.' },
                { role: 'assistant', content: 'Victor Crane stepped into the office, his trench coat dripping wet. He claimed he was looking for his sister, but Malone noticed the fear in his eyes.' },
            ];
            await switchChatTab(tab.id);
            renderChatTabs();
            renderChatMessages();
        }
    });
    await sleep(600);

    // Open the History Q&A panel by clicking its button
    await page.evaluate(() => {
        const btn = document.getElementById('chat-history-qa-btn');
        if (btn) btn.click();
    });
    await page.waitForSelector('#chat-history-qa-panel.slide-panel-open', { timeout: 5000 }).catch(() => {
        // Fallback: wait for panel to be visible via display/transform
        page.waitForFunction(() => {
            const panel = document.getElementById('chat-history-qa-panel');
            if (!panel) return false;
            const style = getComputedStyle(panel);
            return style.display !== 'none' && (panel.classList.contains('slide-panel-open') || style.transform !== 'translateX(100%)');
        }, { timeout: 5000 });
    }).catch(() => {
        console.log('[CAPTURE] History Q&A panel did not become visible; capturing anyway.');
    });
    await sleep(800);
    await captureShot(page, 'chat-history-qa-panel.png', { fullPage: true });
    await captureCloseUp(page, '#chat-history-qa-panel', 'chat-history-qa-panel.png', options);

    // Close panel
    await page.evaluate(() => {
        const btn = document.getElementById('chqa-close-btn');
        if (btn) btn.click();
    });
    await sleep(300);

    await cleanupScreenshotTabs(page);
}

// ── Navbar: theme toggle and low-power pill ───────────────────────────────────
// Captures the top nav bar in its key visual states:
//   1. Connected / idle  — theme toggle visible, sleep pill with ambient amber
//   2. Low-power active  — sleep pill fully lit
//   3. Light theme       — theme toggle shows moon icon; pill is warm amber on light bg
