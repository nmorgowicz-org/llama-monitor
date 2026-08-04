// Scenario: guided-gen
// Extracted from tests/ui/capture.mjs (Phase A3).
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp } from '../../harness/browser.mjs';
import { activateScreenshotChat, attachSuggestionsResponseLogger, createFreshChat, describeSuggestionsPanel, sendChatPrompt, waitForChatComplete, waitForChatIdle, waitForSuggestionsSettled } from '../../harness/chat.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureCloseUp, captureElementScreenshot, captureShot, cleanupScreenshotTabs, describePopover, enableGuidedGeneration } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    const detachSuggestionsLogger = attachSuggestionsResponseLogger(page);
    await gotoApp(page, baseUrl);
    await attachToServer(page);
    await enableGuidedGeneration(page);

    await createFreshChat(page);
    await sleep(500);

    // Create multiple chats with distinct, safe content for 06-chat-tabs.png
    console.log('[CAPTURE] Creating test chat tabs with content...');
    await page.evaluate(async () => {
        const { addChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        for (let i = 0; i < 3; i++) {
            addChatTab();
        }
        renderChatTabs();
        renderChatMessages();
    });
    await sleep(500);

    // Seed short messages into each chat so tabs look realistic
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        const tabs = chat.tabs;

        if (tabs[0]) {
            tabs[0].name = 'CI Pipeline';
            tabs[0].messages = [
                { role: 'user', content: 'Outline a simple CI pipeline for a Rust backend.' },
                { role: 'assistant', content: 'Use GitHub Actions with these steps:\n- Run cargo fmt and cargo clippy.\n- Run cargo test.\n- Build a release binary.\n- Optionally upload artifacts.' },
            ];
        }
        if (tabs[1]) {
            tabs[1].name = 'Debugging';
            tabs[1].messages = [
                { role: 'user', content: 'List steps to debug a slow HTTP endpoint.' },
                { role: 'assistant', content: '- Profile request duration.\n- Check database queries and indexes.\n- Inspect external service calls.\n- Review logs for retries or timeouts.' },
            ];
        }
        if (tabs[2]) {
            tabs[2].name = 'GPU Monitoring';
            tabs[2].messages = [
                { role: 'user', content: 'How can I monitor GPU temperature and utilization from the CLI?' },
                { role: 'assistant', content: 'Use tools like nvidia-smi, nvtop, or custom scripts that read from /sys/class/thermal and GPU management APIs.' },
            ];
        }

        for (const t of tabs) t.updated_at = Date.now();
        await switchChatTab(tabs[0].id);
        renderChatTabs();
        renderChatMessages();
    });
    await sleep(800);

    await captureShot(page, 'panels-chat-tabs.png', { fullPage: true });
    await activateScreenshotChat(page);

    // For 08-context-notes-expanded.png:
    // - Use a neo-noir style conversation.
    // - Inject context notes so the sidebar shows real data.
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        const tab = chat.tabs.find(t => t.name?.startsWith('[screenshot]')) || chat.tabs[0];
        if (!tab) return;
        tab.name = '[screenshot] Noir Scene';
        tab.messages = [
            { role: 'user', content: 'Write a short opening scene in a neo-noir detective story.' },
            { role: 'assistant', content: 'The rain fell like needles on the pavement, each drop a tiny hammer against the silence. She stood in the shadow of the alley, her trench coat soaked through, her eyes scanning the street for the man who had promised to deliver the ledger.' },
        ];
        tab.context_notes = [
            { section: 'Character', content: 'Detective in a rain-soaked city, dry humor, haunted by a past case.' },
            { section: 'Setting', content: 'Neo-noir metropolis, neon signs, constant rain, corrupt underworld.' },
            { section: 'Tone', content: 'Tense, cinematic, short punchy lines. No melodrama.' },
        ];
        tab.updated_at = Date.now();
        await switchChatTab(tab.id);
        renderChatTabs();
        renderChatMessages();
    });
    await sleep(600);

    // Open context sidebar with real notes visible
    await page.evaluate(async () => {
        const { toggleContextSidebar } = await import('/js/features/chat-notes.js');
        toggleContextSidebar();
    });
    await sleep(1200);
    await captureShot(page, 'guided-gen-context-notes-expanded.png', { fullPage: true });
    // Close context sidebar
    await page.evaluate(async () => {
        const { toggleContextSidebar } = await import('/js/features/chat-notes.js');
        toggleContextSidebar();
    });
    await sleep(500);

    // Suggestions dropdown (09-suggestions-dropdown.png)
    // Fresh chat to reduce context buildup
    await createFreshChat(page);
    await sleep(500);
    // Use a short prompt to set context (best-effort; continue if server is slow)
    await sendChatPrompt(page, 'Brainstorm 3 product names for a CLI tool that monitors GPUs.');
    try {
        await waitForChatComplete(page);
    } catch (e) {
        console.log(`[CAPTURE] guided-gen: chat complete timed out, continuing with UI captures... ${e.message}`);
    }
    await sleep(2000);

    await page.evaluate(async () => {
        const { toggleSuggestionsDropdown } = await import('/js/features/chat-suggestions.js');
        toggleSuggestionsDropdown();
    });
    await sleep(1000);
    console.log('[CAPTURE] Suggestions pre-generate state:', JSON.stringify(
        {
            ...(await describePopover(page, '#suggestions-toggle', '#suggestions-dropdown')),
            ...(await describeSuggestionsPanel(page)),
        }
    ));
    await captureShot(page, 'guided-gen-suggestions-dropdown.png', { fullPage: true });
        await captureCloseUp(page, '#suggestions-dropdown', 'guided-gen-suggestions-dropdown.png', options);
    await page.click('#suggestions-generate-btn');
    await waitForSuggestionsSettled(page);
    await sleep(1200);
    console.log('[CAPTURE] Suggestions generated state:', JSON.stringify(
        await describeSuggestionsPanel(page)
    ));
    await captureShot(page, 'guided-gen-suggestions-results.png', { fullPage: true });
        await captureCloseUp(page, '#suggestions-dropdown', 'guided-gen-suggestions-results.png', options);
    await page.evaluate(async () => {
        const { toggleSuggestionsDropdown } = await import('/js/features/chat-suggestions.js');
        toggleSuggestionsDropdown();
    });
    await sleep(500);

    // Quick guide dropdown with real conversation
    await page.evaluate(async () => {
        const { toggleQuickGuide } = await import('/js/features/chat-quick-guide.js');
        toggleQuickGuide();
    });
    await sleep(1000);
    console.log('[CAPTURE] Quick guide state:', JSON.stringify(
        await describePopover(page, '#quick-guide-toggle', '#quick-guide-container')
    ));
    await captureShot(page, 'guided-gen-quick-guide-dropdown.png', { fullPage: true });
        await captureCloseUp(page, '#quick-guide-container', 'guided-gen-quick-guide-dropdown.png', options);
    await page.evaluate(async () => {
        const { toggleQuickGuide } = await import('/js/features/chat-quick-guide.js');
        toggleQuickGuide();
    });
    await sleep(500);

    // Quick guide response: apply a quick guide instruction and capture resulting reply
    // Fresh chat with seeded context for quick guide demo
    await createFreshChat(page);
    await sleep(500);
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        // Use the active tab (last one after createFreshChat)
        const tab = chat.tabs[chat.tabs.length - 1];
        if (tab) {
            tab.messages = [
                { role: 'user', content: 'I need help optimizing database queries for a web application.' },
                { role: 'assistant', content: 'I can help with database query optimization. What database are you using, and what kind of queries are slow?' },
            ];
            await switchChatTab(tab.id);
            renderChatTabs();
            renderChatMessages();
        }
    });
    await sleep(600);
    await page.evaluate(async () => {
        const { toggleQuickGuide } = await import('/js/features/chat-quick-guide.js');
        toggleQuickGuide();
    });
    await sleep(400);
    await page.type('#quick-guide-input', 'Keep the next reply concise and technical, 3 bullets max.');
    await sleep(300);
    await page.keyboard.press('Enter');
    // Wait for quick guide response to complete before sending next message
    await waitForChatIdle(page);
    // Now send a user message that will use the guide
    await sendChatPrompt(page, 'Explain how connection pooling improves performance.');
    await waitForChatComplete(page);
    await sleep(1500);
    await captureShot(page, 'guided-gen-quick-guide-response.png', { fullPage: true });

  // Director mode: switch to director mode and generate ideas
    // Fresh chat with seeded noir scene for director demo
    await createFreshChat(page);
    await sleep(500);
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab, renderChatTabs, renderChatMessages } = await import('/js/features/chat-state.js');
        const { renderChatTabs: renderTabs2, renderChatMessages: renderMsgs2 } = await import('/js/features/chat-render.js');
        // Find the active tab (last one after createFreshChat)
        const tab = chat.tabs[chat.tabs.length - 1];
        if (tab) {
            tab.name = '[director] Noir Scene';
            tab.messages = [
                { role: 'user', content: 'Write a short opening scene in a neo-noir detective story.' },
                { role: 'assistant', content: 'The rain fell like needles on the pavement, each drop a tiny hammer against the silence. She stood in the shadow of the alley, her trench coat soaked through, her eyes scanning the street for the man who had promised to deliver the ledger.' },
            ];
            await switchChatTab(tab.id);
            renderTabs2();
            renderMsgs2();
        }
    });
    await sleep(600);
    await page.evaluate(async () => {
        const { toggleQuickGuide } = await import('/js/features/chat-quick-guide.js');
        toggleQuickGuide();
    });
    await sleep(400);
    await page.evaluate(() => {
        const btn = Array.from(document.querySelectorAll('.quick-guide-mode-btn'))
            .find(b => b.dataset.guideMode === 'director');
        if (btn) btn.click();
    });
    await sleep(400);
    const directorInput = await page.$('#quick-guide-director-input');
    if (directorInput) {
        await directorInput.type('Raise tension and introduce a suspect who may be lying.', { delay: 20 });
        const generateBtn = await page.$('#quick-guide-director-generate-btn');
        if (generateBtn) {
            await generateBtn.click();
            await sleep(2000);
            // Wait for director results
            await page.waitForSelector('.quick-guide-director-item', { timeout: 120000 }).catch(() => {});
            await sleep(800);
            await captureShot(page, 'guided-gen-director-options.png', { fullPage: true });

            // 10d-guide-ai-director-results.png: apply one idea and capture resulting reply
            const applyBtn = await page.$('.quick-guide-director-apply-btn');
            if (applyBtn) {
                await applyBtn.click();
                // Wait for director apply response to complete before sending next message
                await waitForChatIdle(page);
                await sendChatPrompt(page, 'Continue the scene with higher tension.');
await waitForChatComplete(page);
                await sleep(1500);
                await captureShot(page, 'guided-gen-director-applied.png', { fullPage: true });
            }
        }
    }

    // Surprise mode: switch to surprise mode and arm a surprise
    // Fresh chat with content for chat-related screenshot
    await createFreshChat(page);
    await sleep(500);
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        // Use the active tab (last one after createFreshChat)
        const tab = chat.tabs[chat.tabs.length - 1];
        if (tab) {
            tab.messages = [
                { role: 'user', content: 'Write a scene where a detective discovers a hidden clue.' },
                { role: 'assistant', content: 'The safe was empty, but behind the false back she found a single photograph—her partner, standing next to the victim, both of them smiling.' },
            ];
            await switchChatTab(tab.id);
            renderChatTabs();
            renderChatMessages();
        }
    });
    await sleep(600);
    await page.evaluate(async () => {
        const { toggleQuickGuide } = await import('/js/features/chat-quick-guide.js');
        toggleQuickGuide();
    });
    await sleep(400);
    await page.evaluate(() => {
        const btn = Array.from(document.querySelectorAll('.quick-guide-mode-btn'))
            .find(b => b.dataset.guideMode === 'surprise');
        if (btn) btn.click();
    });
    await sleep(400);
    const surpriseInput = await page.$('#quick-guide-surprise-input');
    if (surpriseInput) {
        await surpriseInput.type('A contact leaks a key piece of evidence that changes everything.', { delay: 20 });
        const armBtn = await page.$('#quick-guide-surprise-arm-btn');
        if (armBtn) {
            await armBtn.click();
            await sleep(800);
            await captureShot(page, 'guided-gen-surprise-armed.png', { fullPage: false });
        }
    }

    // Close quick guide if open
    await page.evaluate(async () => {
        const { closeQuickGuide } = await import('/js/features/chat-quick-guide.js');
        closeQuickGuide();
    });
    await sleep(300);

    // 11-chat-input-buttons.png with conversation present
    await captureShot(page, 'panels-chat-input-buttons.png', { fullPage: true });

    // Explicit mode toggles (12a/12b/12c) with real content
    await page.evaluate(() => document.getElementById('chat-explicit-toggle-footer')?.click());
    await sleep(800);
    await captureShot(page, 'guided-gen-explicit-unlocked.png', { fullPage: false });
    await page.evaluate(() => document.getElementById('chat-explicit-toggle-footer')?.click());
    await sleep(800);
    await captureShot(page, 'guided-gen-explicit-unrestricted.png', { fullPage: false });
    await page.evaluate(() => document.getElementById('chat-explicit-toggle-footer')?.click());
    await sleep(800);
    await captureShot(page, 'guided-gen-explicit-locked.png', { fullPage: false });

    // Re-open suggestions and ensure setup area is expanded for tag cloud shot
    await page.evaluate(async () => {
        const { toggleSuggestionsDropdown } = await import('/js/features/chat-suggestions.js');
        toggleSuggestionsDropdown();
    });
    await sleep(600);
    // Expand setup if collapsed
    await page.evaluate(() => {
        const toggle = document.getElementById('suggestions-view-toggle');
        if (toggle && toggle.textContent?.trim() === 'Show Setup') {
            toggle.click();
        }
    });
    await sleep(800);
    await captureShot(page, 'guided-gen-suggestions-tag-cloud.png', { fullPage: false });

    // Type into search input and wait for filter to apply
    await page.click('#suggestion-search-input');
    await page.evaluate(() => {
        const input = document.getElementById('suggestion-search-input');
        if (input) input.value = '';
    });
    await page.type('#suggestion-search-input', 'horror', { delay: 50 });
    await sleep(800);
    await captureShot(page, 'guided-gen-suggestions-search-filter.png', { fullPage: false });

    // Open manage categories modal to validate rendering
    await page.evaluate(() => document.getElementById('suggestions-manage-btn')?.click());
    await sleep(800);
    await captureShot(page, 'guided-gen-manage-categories.png', { fullPage: false });
    if (options.closeUp) {
        await captureElementScreenshot(page, '#categories-builtin-list', 'guided-gen-categories-builtin-list.png', { padding: 12 });
    }
    await page.keyboard.press('Escape');
    await sleep(300);

    await cleanupScreenshotTabs(page);
    detachSuggestionsLogger();
}

// ── Settings & Modals ───────────────────────────────────────────────────────────
// Settings modal, preferences, persona, models, keyboard shortcuts.
