// Scenario: preset-editor
// Extracted from tests/ui/capture.mjs (Phase A3).
import { join } from 'path';
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp } from '../../harness/browser.mjs';
import { ARTIFACTS_DIR, sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    if (!options.noAttach) {
        try { await attachToServer(page); } catch {}
    }

    // 1. Open preset editor via "New"
    const newBtn = await page.$('#preset-new-btn');
    if (!newBtn) {
        console.log('[CAPTURE] #preset-new-btn not found; skipping preset-editor scenario');
        return;
    }
    // The button can exist while an ancestor is still transitioning, which
    // makes Puppeteer's clickable-point calculation fail intermittently.
    await page.evaluate(() => document.getElementById('preset-new-btn')?.click());
    await page.waitForSelector('#preset-modal.open', { visible: true, timeout: 6000 });
    await sleep(800);

    // Capture Model+Context section (default active)
    await captureShot(page, 'preset-editor-model-tab.png', { fullPage: true });

    // 2. Capture Context section at the host-cache recommendation.
    await page.evaluate(() => {
        const contextNav = document.querySelector('#preset-modal .preset-editor-nav [data-section="context"]');
        if (contextNav) contextNav.click();
    });
    await sleep(300);
    await page.evaluate(() => {
        document.getElementById('modal-cache-ram-mib')?.scrollIntoView({ block: 'center' });
    });
    await sleep(300);
    await captureShot(page, 'preset-editor-context-tab.png', { fullPage: true });

    // 3. Capture GPU section
    await page.evaluate(() => {
        const gpuNav = document.querySelector('#preset-modal .preset-editor-nav [data-section="gpu"]');
        if (gpuNav) gpuNav.click();
    });
    await sleep(500);
    await captureShot(page, 'preset-editor-gpu-tab.png', { fullPage: true });

    // 4. Capture Advanced section
    await page.evaluate(() => {
        const advNav = document.querySelector('#preset-modal .preset-editor-nav [data-section="advanced"]');
        if (advNav) advNav.click();
    });
    await sleep(500);
    await captureShot(page, 'preset-editor-advanced-tab.png', { fullPage: true });

    // 5. Switch back to Model tab for Chat Template row captures
    await page.evaluate(() => {
        const modelNav = document.querySelector('#preset-modal .preset-editor-nav [data-section="model"]');
        if (modelNav) modelNav.click();
    });
    await sleep(400);

    // 5a. Set realistic model path + template path for a real-looking screenshot with VRAM bar
    const realisticModelPath = '/Users/nick/.config/llama-monitor/models/gguf/Qwen3-Coder-Next-Opus-Distilled-Q4_K_M.gguf';
    const realTemplatePath = '/Users/nick/.config/llama-monitor/chat-templates/qwen-froggeric-fixed.jinja';
    await page.evaluate((modelPath, templatePath) => {
        const modelInput = document.getElementById('modal-model-path');
        if (modelInput) {
            modelInput.value = modelPath;
            modelInput.dispatchEvent(new Event('input', { bubbles: true }));
        }
        const templateInput = document.getElementById('modal-chat-template-file');
        if (templateInput) templateInput.value = templatePath;
    }, realisticModelPath, realTemplatePath);
    // Wait for VRAM estimation to complete
    await page.waitForFunction(() => {
        const display = document.getElementById('preset-vram-display');
        return display && !display.textContent.includes('Estimating');
    }, { timeout: 8000 }).catch(() => {});
    await sleep(600);

    // 5b. Switch to light theme, scroll to Chat Template row
    await page.evaluate(() => {
        document.documentElement.setAttribute('data-theme', 'light');
    });
    await sleep(300);
    await page.evaluate(() => {
        const row = document.getElementById('modal-chat-template-file');
        if (row) {
            row.scrollIntoView({ block: 'center' });
        }
        // Ensure Lifecycle modal is closed before this capture
        const lifecycleModal = document.getElementById('chat-template-lifecycle-modal');
        if (lifecycleModal) lifecycleModal.classList.remove('open');
    });
    await sleep(400);
    await captureShot(page, 'preset-editor-chat-template-row-light.png', { fullPage: true });

    // 5c. Capture Discussions feed
    await page.evaluate(() => {
        // Ensure Lifecycle modal is closed before this capture
        const lifecycleModal = document.getElementById('chat-template-lifecycle-modal');
        if (lifecycleModal) lifecycleModal.classList.remove('open');
        document.getElementById('preset-chat-template-discussions-btn')?.click();
    });
    // Wait for feed to render (API call takes time)
    await page.waitForFunction(() => {
        const list = document.getElementById('preset-chat-template-discussions-list');
        return list && list.style.display !== 'none' && list.textContent.length > 20;
    }, { timeout: 5000 }).catch(() => {
        console.log('[CAPTURE] Discussions feed did not load within timeout');
    });
    await sleep(400);
    await captureShot(page, 'preset-editor-discussions-feed.png', { fullPage: true });
    // Collapse discussions feed
    await page.evaluate(() => {
        document.getElementById('preset-chat-template-discussions-btn')?.click();
    });
    await sleep(300);

    // 5c. Capture Chat Template Lifecycle Modal (light theme)
    await page.evaluate(() => {
        document.getElementById('preset-chat-template-manage-btn')?.click();
    });
    // Wait for lifecycle modal to be visible and populated
    await page.waitForFunction(() => {
        const modal = document.getElementById('chat-template-lifecycle-modal');
        if (!modal || !modal.classList.contains('open')) return false;
        const versionEl = document.getElementById('chat-template-lifecycle-version');
        const discussionsEl = document.getElementById('chat-template-lifecycle-discussions');
        // Consider loaded if not showing "Loading..." and has some content
        return versionEl && !versionEl.textContent.includes('Loading') &&
               (versionEl.textContent.length > 20 || discussionsEl?.textContent.length > 10);
    }, { timeout: 5000 }).catch(() => {
        console.log('[CAPTURE] Lifecycle modal did not fully load within timeout');
    });
    await sleep(500);
    await captureShot(page, 'preset-editor-lifecycle-modal-light.png', { fullPage: true });
    // Close lifecycle modal
    await page.evaluate(() => {
        const modal = document.getElementById('chat-template-lifecycle-modal');
        if (modal) modal.classList.remove('open');
    });
    await sleep(300);

    // 5d. Capture Create fix modal in light theme
    await page.evaluate(() => {
        document.getElementById('preset-chat-template-create-fix-btn')?.click();
    });
    // Wait for modal textarea to be populated with template content
    await page.waitForFunction(() => {
        const textarea = document.querySelector('.chat-template-fix-textarea');
        return textarea && textarea.value.length > 500;
    }, { timeout: 5000 }).catch(() => {});
    await sleep(400);
    await captureShot(page, 'preset-editor-create-fix-modal-light.png', { fullPage: true });

    // Close light Create fix modal (it's dynamically inserted inside preset-modal .modal)
    await page.evaluate(() => {
        const textarea = document.querySelector('.chat-template-fix-textarea');
        if (textarea) {
            let el = textarea;
            while (el && el.parentElement) {
                const parent = el.parentElement;
                if (parent.style.overflow === 'auto' || parent.style.overflowY === 'auto') {
                    parent.remove();
                    break;
                }
                el = parent;
            }
        }
    });
    await sleep(200);

    // 5d. Switch back to dark theme and capture Create fix modal
    await page.evaluate(() => {
        document.documentElement.removeAttribute('data-theme');
    });
    await sleep(300);

    // 5d. Capture Chat Template Lifecycle Modal (dark theme)
    await page.evaluate(() => {
        document.getElementById('preset-chat-template-manage-btn')?.click();
    });
    // Wait for lifecycle modal to be visible and populated
    await page.waitForFunction(() => {
        const modal = document.getElementById('chat-template-lifecycle-modal');
        if (!modal || !modal.classList.contains('open')) return false;
        const versionEl = document.getElementById('chat-template-lifecycle-version');
        const discussionsEl = document.getElementById('chat-template-lifecycle-discussions');
        return versionEl && !versionEl.textContent.includes('Loading') &&
               (versionEl.textContent.length > 20 || discussionsEl?.textContent.length > 10);
    }, { timeout: 5000 }).catch(() => {
        console.log('[CAPTURE] Lifecycle modal dark theme did not fully load within timeout');
    });
    await sleep(500);
    await captureShot(page, 'preset-editor-lifecycle-modal-dark.png', { fullPage: true });
    // Close lifecycle modal
    await page.evaluate(() => {
        const modal = document.getElementById('chat-template-lifecycle-modal');
        if (modal) modal.classList.remove('open');
    });
    await sleep(300);

    await page.evaluate(() => {
        document.getElementById('preset-chat-template-create-fix-btn')?.click();
    });
    // Wait for modal textarea to be populated with template content
    await page.waitForFunction(() => {
        const textarea = document.querySelector('.chat-template-fix-textarea');
        return textarea && textarea.value.length > 500;
    }, { timeout: 5000 }).catch(() => {});
    await sleep(400);
    await captureShot(page, 'preset-editor-create-fix-modal-dark.png', { fullPage: true });

    // 5e. Capture with wrap enabled — verify line numbers align correctly with wrapped lines
    await page.evaluate(() => {
        const textarea = document.querySelector('.chat-template-fix-textarea');
        if (!textarea) return;
        // Walk up to find panel with wrap checkbox
        let parent = textarea.parentElement;
        let panel = null;
        for (let i = 0; i < 10 && parent; i++) {
            const cbs = Array.from(parent.querySelectorAll('input[type="checkbox"]'));
            if (cbs.length > 0) { panel = parent; break; }
            parent = parent.parentElement;
        }
        if (panel) {
            const cb = panel.querySelector('input[type="checkbox"]');
            if (cb && !cb.checked) {
                cb.checked = true;
                cb.dispatchEvent(new Event('change', { bubbles: true }));
            }
        }
    });
    // Wait for wrap + line recalc to settle
    await sleep(800);
    const wrapInfo = await page.evaluate(() => {
        const textarea = document.querySelector('.chat-template-fix-textarea');
        if (!textarea) return {};
        return {
            ws: textarea.style.whiteSpace,
            ww: textarea.style.wordWrap,
            scrollTop: textarea.scrollTop,
            scrollHeight: textarea.scrollHeight,
            clientHeight: textarea.clientHeight,
            clientWidth: textarea.clientWidth,
            lineHeight: parseFloat(getComputedStyle(textarea).lineHeight),
            overflowX: textarea.style.overflowX,
        };
    });
    console.log('[CAPTURE] wrap info:', wrapInfo);
    // Force scroll to show wrapped content in middle of file
    await page.evaluate(() => {
        const textarea = document.querySelector('.chat-template-fix-textarea');
        if (!textarea) return;
        let parent = textarea.parentElement;
        let panel = null;
        for (let i = 0; i < 10 && parent; i++) {
            const cbs = Array.from(parent.querySelectorAll('input[type="checkbox"]'));
            if (cbs.length > 0) { panel = parent; break; }
            parent = parent.parentElement;
        }
        if (panel) {
            const cb = panel.querySelector('input[type="checkbox"]');
            if (cb && !cb.checked) {
                cb.click();
            }
        }
    });
    await sleep(800);
    // Capture just the modal area to avoid Puppeteer fullPage issues
    const box = await page.evaluate(() => {
        const textarea = document.querySelector('.chat-template-fix-textarea');
        if (!textarea) return null;
        const modal = textarea.closest('div[style*="position:absolute"]') || textarea.closest('div');
        if (!modal) return null;
        const rect = modal.getBoundingClientRect();
        return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    });
    if (box) {
        await page.screenshot({ path: join(ARTIFACTS_DIR, 'preset-editor-create-fix-modal-wrap.png'), clip: box });
    } else {
        await page.screenshot({ path: join(ARTIFACTS_DIR, 'preset-editor-create-fix-modal-wrap.png'), fullPage: false });
    }
    console.log(`[CAPTURE] Saved preset-editor-create-fix-modal-wrap.png`);

    await page.evaluate(() => {
        const modal = document.querySelector('#preset-modal .modal > div[style*="position:absolute"]');
        if (modal) modal.remove();
    });
    await sleep(300);

    // 5. Close preset modal
    await page.evaluate(() => {
        const close = document.getElementById('preset-modal-close');
        if (close) close.click();
    });
    await sleep(300);
}
