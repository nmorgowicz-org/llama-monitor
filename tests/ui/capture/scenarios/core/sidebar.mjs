// Scenario: sidebar
// Extracted from tests/ui/capture.mjs (Phase A3).
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp, switchTab, waitForMonitor } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureCloseUp, captureElementScreenshot, captureShot, cleanupScreenshotTabs } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    if (!options.noAttach) {
        await attachToServer(page);
    } else {
        // attachToServer normally exits the setup view into #view-monitor
        // (which hosts #page-chat / #chat-sessions-panel). With --no-attach
        // that transition never fires, so force it directly.
        await page.evaluate(async () => {
            const { ensureMonitorView } = await import('/js/features/setup-view.js');
            ensureMonitorView();
        });
        await waitForMonitor(page);
    }

    // Create multiple chats with different names for grouping
    // Directly show the chat page and session panel (nav state may not be initialized)
    await page.evaluate(async () => {
        // Show chat page
        const pages = document.querySelectorAll('.page');
        pages.forEach(p => p.classList.remove('active'));
        const chatPage = document.getElementById('page-chat');
        if (chatPage) chatPage.classList.add('active');
        // Show session panel
        const panel = document.getElementById('chat-sessions-panel');
        if (panel) panel.classList.add('visible');
        // Render sidebar items (normally done by switchTab)
        const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');
        renderChatSessionsSidebar();
    });
    await page.waitForSelector('#chat-sessions-panel', { visible: true });
    await sleep(500);

    // Create multiple chats with distinct content for sidebar and FTS search
    await page.evaluate(async () => {
        const { addChatTab } = await import('/js/features/chat-state.js');
        for (let i = 0; i < 8; i++) {
            addChatTab();
        }
    });
    await sleep(500);

    // Rename chats and seed them with content that will be searchable
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');
        const tabs = chat.tabs;

        const repeated = [
            'The ledger vanished into the rain before dawn.',
            'Rain washed the alley clean, but the ledger stayed hidden.',
            'She traced the ledger route through rain-soaked streets.',
        ];

        if (tabs[0]) {
            tabs[0].name = 'Noir Scene';
            tabs[0].messages = [
                { role: 'user', content: 'Write a noir scene in progress.' },
                { role: 'assistant', content: 'The rain fell like needles on the pavement, each drop a tiny hammer against the silence. She stood in the shadow of the alley, her trench coat soaked through, her eyes scanning the street for the man who had promised to deliver the ledger.' },
                { role: 'user', content: repeated[0] },
                { role: 'assistant', content: repeated[1] },
            ];
            tabs[0].pinned = true;
        }
        if (tabs[1]) {
            tabs[1].name = 'Debug Session';
            tabs[1].messages = [
                { role: 'user', content: 'Help me debug a slow HTTP endpoint.' },
                { role: 'assistant', content: 'Start by profiling the request duration, then inspect database queries, external service calls, and any retries or timeouts in the logs.' },
                { role: 'user', content: repeated[1] },
                { role: 'assistant', content: repeated[2] },
            ];
        }
        if (tabs[2]) {
            tabs[2].name = 'CI Pipeline';
            tabs[2].messages = [
                { role: 'user', content: 'Outline a simple CI pipeline for a Rust backend.' },
                { role: 'assistant', content: 'Use GitHub Actions: run cargo fmt, cargo clippy, cargo test, then build a release binary and upload artifacts.' },
                { role: 'assistant', content: repeated[0] },
            ];
        }
        if (tabs[3]) {
            tabs[3].name = 'Recipe Ideas';
            tabs[3].messages = [
                { role: 'user', content: 'Suggest 3 quick dinner recipes using chicken and rice.' },
                { role: 'assistant', content: '- Chicken and rice skillet with vegetables.\n- One-pot lemon herb chicken rice.\n- Stir-fried chicken with soy-ginger rice.' },
                { role: 'user', content: repeated[2] },
            ];
        }
        if (tabs[4]) {
            tabs[4].name = 'GPU Monitoring';
            tabs[4].messages = [
                { role: 'user', content: 'How can I monitor GPU temperature and utilization from the CLI?' },
                { role: 'assistant', content: 'Use tools like nvidia-smi, nvtop, or custom scripts reading from /sys/class/thermal and GPU management APIs.' },
                { role: 'assistant', content: repeated[0] },
            ];
        }
        if (tabs[5]) {
            tabs[5].name = 'Rain Ledger Notes';
            tabs[5].messages = [
                { role: 'user', content: repeated[0] },
                { role: 'assistant', content: repeated[1] },
                { role: 'user', content: repeated[2] },
            ];
        }
        if (tabs[6]) {
            tabs[6].name = 'Shadow Draft';
            tabs[6].messages = [
                { role: 'assistant', content: repeated[1] },
                { role: 'assistant', content: repeated[0] },
            ];
        }
        if (tabs[7]) {
            tabs[7].name = 'Archive Search';
            tabs[7].messages = [
                { role: 'user', content: repeated[2] },
                { role: 'assistant', content: repeated[0] },
                { role: 'assistant', content: repeated[1] },
            ];
        }

        for (const t of tabs) t.updated_at = Date.now();
        renderChatTabs();
        renderChatMessages();
        renderChatSessionsSidebar();
    });
    await sleep(800);

    // Persist all tabs to database so FTS search can find them
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        for (const tab of chat.tabs) {
            const tabPayload = {
                id: tab.id,
                name: tab.name || '',
                system_prompt: tab.system_prompt || '',
                ai_name: tab.ai_name || null,
                user_name: tab.user_name || null,
                explicit_level: tab.explicit_level || 0,
                active_template_id: tab.active_template_id || null,
                auto_compact: tab.auto_compact !== false,
                auto_compact_summarize: tab.auto_compact_summarize !== false,
                compact_mode: tab.compact_mode || 'summarize',
                compact_threshold: tab.compact_threshold || 0.8,
                model_params: tab.model_params || {},
                context_notes: tab.context_notes || [],
                sidebar_width: tab.sidebar_width || 320,
                tab_order: tab.tab_order || 0,
                pinned: tab.pinned || false,
                last_ctx_pct: tab.last_ctx_pct || null,
                total_input_tokens: tab.total_input_tokens || 0,
                total_output_tokens: tab.total_output_tokens || 0,
                created_at: tab.created_at || Date.now(),
                updated_at: Date.now(),
                messages: [],
            };
            const _auth = window.__API_TOKEN
                    ? { 'Authorization': `Bearer ${window.__API_TOKEN}` }
                    : {};
                await fetch('/api/chat/tabs', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json', ..._auth },
                    body: JSON.stringify(tabPayload),
                });
                if (tab.messages && tab.messages.length > 0) {
                    const msgPayload = {
                        messages: tab.messages.map((m, idx) => ({
                            tab_id: tab.id,
                            role: m.role || 'user',
                            content: m.content || '',
                            timestamp_ms: m.timestamp_ms || Date.now(),
                            seq: idx,
                        })),
                    };
                    await fetch(`/api/chat/tabs/${tab.id}/messages`, {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json', ..._auth },
                    body: JSON.stringify(msgPayload),
                });
            }
        }
    });
    await sleep(1500);

    // Ensure sidebar is rendered with all chats
    await page.evaluate(async () => {
        const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');
        renderChatSessionsSidebar();
    });
    await sleep(300);

    // Capture expanded sidebar with multiple chats
await captureShot(page, 'sidebar-sidebar-expanded.png', { fullPage: true });

   // Capture sidebar element detail (close-up only)

    if (options.closeUp) {
        await captureElementScreenshot(page, '#chat-sessions-panel', 'sidebar-sidebar-panel-detail.png', { padding: 16 });
    }

    // Collapse the sidebar (directly set class for reliability)
    await page.evaluate(() => {
        const panel = document.getElementById('chat-sessions-panel');
        if (panel) panel.classList.add('collapsed');
        localStorage.setItem('csp-collapsed', 'true');
    });
    await sleep(500);

    // Capture collapsed strip
    await captureShot(page, 'sidebar-sidebar-collapsed.png', { fullPage: true });

    // Capture collapsed strip detail (close-up only)
    if (options.closeUp) {
        await captureElementScreenshot(page, '#csp-collapsed-strip', 'sidebar-sidebar-collapsed-detail.png', { padding: 12 });
    }

    // Expand again
    await page.evaluate(() => {
        const panel = document.getElementById('chat-sessions-panel');
        if (panel) panel.classList.remove('collapsed');
        localStorage.setItem('csp-collapsed', 'false');
    });
    await sleep(300);

    // Re-render sidebar after expanding (items may not be visible when collapsed)
    await page.evaluate(async () => {
        const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');
        renderChatSessionsSidebar();
    });
    await sleep(1000);

    // Test FTS search: open search mode, query across multiple chats, capture results
    const searchBtn = await page.$('#csp-message-search-btn');
    if (searchBtn) {
        console.log('[CAPTURE] FTS search button found, opening search mode...');
        await page.evaluate(async () => {
            const { openSearch } = await import('/js/features/chat-search.js');
            openSearch();
        });
        await sleep(600);

        const searchInput = await page.$('#csp-search-input');
        if (searchInput) {
            console.log('[CAPTURE] Search input found, typing query that matches multiple chats...');
            await page.type('#csp-search-input', 'ledger');
            await sleep(1200);

            // Ensure results area is visible
            const resultsVisible = await page.evaluate(() => {
                const results = document.querySelector('.csp-search-results');
                return results ? (results.style.display !== 'none') : false;
            });
            console.log('[CAPTURE] Search results visible:', resultsVisible);

            // Capture full-page with search mode and results
            await captureShot(page, 'sidebar-fts-search-active.png', { fullPage: true });

            // Capture close-up of search results (close-up only)
            if (options.closeUp) {
                const searchResults = await page.$('.csp-search-panel');
                if (searchResults) {
                    await captureElementScreenshot(page, '.csp-search-panel', 'sidebar-fts-search-results.png', { padding: 12 });
                    await captureCloseUp(page, '.csp-search-panel', 'sidebar-fts-search-results.png', options);
                }
            }

            // Close search
            await page.keyboard.press('Escape');
            await sleep(300);
        } else {
            console.log('[CAPTURE] Search input not found after opening search mode');
        }
    } else {
        console.log('[CAPTURE] FTS search button not found; skipping FTS search captures');
    }

    // Test context menu: hover over a chat item and click the "..." button
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab } = await import('/js/features/chat-state.js');
        const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');
        // Switch to first tab to make it active
        const tab = chat.tabs[0];
        if (tab) {
            await switchChatTab(tab.id);
            renderChatSessionsSidebar();
        }
    });
    await sleep(500);

    // Hover over first item to reveal action buttons
    const hoverDebug = await page.evaluate(() => {
        const item = document.querySelector('.csp-item');
        const panel = document.getElementById('chat-sessions-panel');
        return {
            itemExists: !!item,
            itemVisible: item ? getComputedStyle(item).display !== 'none' : false,
            panelVisible: panel ? panel.classList.contains('visible') : false,
            panelCollapsed: panel ? panel.classList.contains('collapsed') : false,
        };
    });
    console.log('[CAPTURE] Hover debug:', JSON.stringify(hoverDebug));
    // Actually, the hover is handled by CSS :hover pseudo-class, not a class
    // Need to use puppeteer's hover instead
    const firstItem = await page.$('.csp-item');
    if (firstItem) {
        // Try scrolling the sidebar to ensure the item is in the viewport
        await firstItem.evaluate(el => el.scrollIntoView({ behavior: 'instant', block: 'center' }));
        await sleep(200);
        // Try hovering using mouse.move
        const box = await firstItem.boundingBox();
        if (box) {
            await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
            await sleep(500);
        }
    }

    // Click the "..." more button to open context menu (using JS to bypass CSS hover)
    await page.evaluate(() => {
        const item = document.querySelector('.csp-item');
        if (item) {
            const moreBtn = item.querySelector('button[data-action="more"]');
            if (moreBtn) {
                // Show the actions container (normally shown on hover)
                const actions = item.querySelector('.csp-item-actions');
                if (actions) actions.style.display = 'flex';
                // Click the more button
                moreBtn.click();
            }
        }
    });
    await sleep(500);
    await captureShot(page, 'chat-context-menu.png', { fullPage: true });
    // Capture context menu detail (close-up only)
    if (options.closeUp) {
        const menu = await page.$('.csp-context-menu');
        if (menu) {
            await captureElementScreenshot(page, '.csp-context-menu', 'chat-context-menu-detail.png', { padding: 12 });
        }
    }
    // Close menu
    await page.keyboard.press('Escape');
    await sleep(300);

    // Sidebar resize handle — capture with hover state (close-up only)
    if (options.closeUp) {
        try {
            const resizeHandle = await page.$('#sidebar-resize-handle');
            if (resizeHandle) {
                const box = await resizeHandle.boundingBox();
                if (box) {
                    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
                    await sleep(300);
                    await captureElementScreenshot(page, '.sidebar-nav', 'sidebar-resize-handle.png', { padding: 0 });
                }
            }
        } catch {
            console.log('[CAPTURE] Sidebar resize handle capture failed, skipping...');
        }
    }

    // Test search filter (title filter, not FTS)
    await page.type('#csp-search', 'Noir');
    await sleep(500);
    await captureShot(page, 'sidebar-sidebar-title-filter.png', { fullPage: true });

    // Clear filter
    await page.evaluate(() => {
        document.getElementById('csp-search').value = '';
        document.getElementById('csp-search').dispatchEvent(new Event('input'));
    });
    await sleep(300);

    await cleanupScreenshotTabs(page);
}
