// Scenario: spawn-wizard
// SCENARIO INTENT: Capture the current llama.cpp three-step wizard state without treating it as future Guided or Pro acceptance.
// Extracted from tests/ui/capture.mjs (Phase A3).
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);

    // Open setup wizard. Attach is not required; wizard works from the welcome screen.
    console.log('[CAPTURE] Opening wizard...');
    const result = await page.evaluate(async () => {
        // Clear sessionStorage
        sessionStorage.clear();
        console.log('[CAPTURE] About to import spawn-wizard.js');
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        console.log('[CAPTURE] openSpawnWizard:', typeof openSpawnWizard);
        openSpawnWizard();
        console.log('[CAPTURE] openSpawnWizard called');
        return 'done';
    });
    console.log('[CAPTURE] evaluate result:', result);
    console.log('[CAPTURE] Waiting for wizard to open...');
    try {
        await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 10000 });
        console.log('[CAPTURE] Wizard opened');
        // Check if view-mode-select exists
        const hasSelect = await page.$('#view-mode-select') !== null;
        console.log('[CAPTURE] #view-mode-select exists:', hasSelect);
        // Check if wizardState is defined
        const hasWizardState = await page.evaluate(() => typeof wizardState !== 'undefined');
        console.log('[CAPTURE] wizardState defined:', hasWizardState);
    } catch (e) {
        console.log('[CAPTURE] Wizard open timeout:', e.message);
    }
    await sleep(600);

    // Hide the binary prereq banner so it doesn't clutter every shot.
    await page.evaluate(() => {
        const banner = document.getElementById('wizard-binary-prereq');
        if (banner) banner.style.display = 'none';
    });
    await sleep(200);

    // ── Step 1 (Model): profile + use-case — capture AFTER selections so state is visible ─
    await page.evaluate(() => {
        (document.querySelector('.profile-card[data-profile="power"]')
            || document.querySelector('.profile-card'))?.click();
    });
    await sleep(200);
    await page.evaluate(() => {
        (document.querySelector('.usecase-card[data-usecase="general"]')
            || document.querySelector('.usecase-card'))?.click();
    });
    await sleep(300);
    await captureShot(page, 'spawn-wizard-model-profiles.png', { fullPage: true, expandSelector: '.wizard-body' });

    // Profile/use-case and model selection are now on the same step (Option A
    // collapse: 6 steps → 3). No navigation click needed between them.

    // ── Step 1 (Model): model source cards — capture before selecting HF ─────
    await captureShot(page, 'spawn-wizard-model-source-cards.png', { fullPage: true, expandSelector: '.wizard-body' });

    // Select HuggingFace source.
    await page.evaluate(() => {
        const hfCard = document.querySelector('.model-source-card[data-source="hf"]');
        if (hfCard) {
            hfCard.click();
        } else {
            console.log('[CAPTURE] ERROR: HF card not found');
        }
    });
    // Wait for HF panel to become visible
    await page.waitForFunction(
        () => document.getElementById('model-input-hf')?.classList.contains('visible'),
        { timeout: 3000 }
    ).catch(() => console.log('[CAPTURE] HF panel wait timed out'));
    await sleep(300);
    // Scroll to show HF source card + panel
    await page.evaluate(() => {
        const body = document.querySelector('.wizard-body');
        const hfCard = document.querySelector('.model-source-card[data-source="hf"]');
        if (body && hfCard) {
            hfCard.scrollIntoView({ behavior: 'instant', block: 'start' });
        }
    });
    await sleep(200);
    await captureShot(page, 'spawn-wizard-model-hf-base.png', { fullPage: true });

    // Collapse community picks so discover pill captures have clean space.
    await page.evaluate(() => {
        const cp = document.getElementById('hf-community-picks');
        const toggle = document.getElementById('hf-cp-toggle');
        const body = document.getElementById('hf-cp-body');
        if (cp) cp.style.display = 'none';
        if (toggle) toggle.setAttribute('aria-expanded', 'false');
        if (body) body.style.display = 'none';
    });
    await sleep(200);

    // Helper: wait up to 20 s for real result cards; continues silently if none arrive.
    const waitForResults = () => page.waitForFunction(() => {
        const r = document.getElementById('hf-search-results');
        return r && r.style.display !== 'none'
            && !r.querySelector('.hf-search-loading')
            && r.querySelector('.hf-search-result') !== null;
    }, { timeout: 20000 }).catch(() => {});

    // Helper: scroll wizard body so the search results area is visible.
    const scrollToResults = () => page.evaluate(() => {
        const results = document.getElementById('hf-search-results');
        if (results && results.style.display !== 'none') {
            results.scrollIntoView({ behavior: 'instant', block: 'start' });
        } else {
            const body = document.querySelector('.wizard-body');
            if (body) body.scrollTop = 240;
        }
    });

    // ── Discover pill: Trending ───────────────────────────────────────────────
    // Reset scroll and clear any leftover search from hf-base step.
    await page.evaluate(() => {
        document.getElementById('spawn-hf-repo').value = '';
        document.getElementById('hf-search-results')?.classList.remove('visible');
        const body = document.querySelector('.wizard-body');
        if (body) body.scrollTop = 0;
    });
    await sleep(200);
    const trendingPill = await page.$('.hf-discover-pill[data-cat-id="trending"]');
    if (trendingPill) {
        await trendingPill.click();
        await waitForResults();
        await sleep(400);
        await captureShot(page, 'spawn-wizard-model-discover-trending.png', { fullPage: true, expandSelector: '.wizard-body' });
    }

    // ── Discover pill: Qwen3 ─────────────────────────────────────────────────
    const qwen3Pill = await page.$('.hf-discover-pill[data-cat-id="qwen3"]');
    if (qwen3Pill) {
        await qwen3Pill.click();
        await waitForResults();
        await sleep(400);
        await captureShot(page, 'spawn-wizard-model-discover-qwen3.png', { fullPage: true, expandSelector: '.wizard-body' });
    }

    // ── Quantizer quick-pick: bartowski ──────────────────────────────────────
    const bartowskiBtn = await page.$('.hf-qp-btn[data-author="bartowski"]');
    if (bartowskiBtn) {
        await bartowskiBtn.click();
        await waitForResults();
        await sleep(400);
        await captureShot(page, 'spawn-wizard-model-quantizer-bartowski.png', { fullPage: true, expandSelector: '.wizard-body' });
    }

    // ── Community picks panel ─────────────────────────────────────────────────
    await page.evaluate(() => {
        const r = document.getElementById('hf-search-results');
        if (r) { r.style.display = 'none'; r.innerHTML = ''; }
        document.querySelectorAll('.hf-discover-pill, .hf-qp-btn')
            .forEach(p => p.classList.remove('active'));
        // Show community picks panel (we hid it earlier for discover captures)
        const cp = document.getElementById('hf-community-picks');
        if (cp) cp.style.display = '';
        const body = document.querySelector('.wizard-body');
        if (body) body.scrollTop = 0;
    });
    await sleep(300);

    const cpToggle = await page.$('#hf-cp-toggle');
    if (cpToggle) {
        await cpToggle.click();
        await page.waitForFunction(
            () => document.getElementById('hf-cp-toggle')?.getAttribute('aria-expanded') === 'true',
            { timeout: 3000 }
        ).catch(() => {});
         await sleep(500);
          // Scroll to show all community picks models
          await page.evaluate(() => {
              const body = document.querySelector('.wizard-body');
              const picksBody = document.getElementById('hf-cp-body');
              if (body && picksBody && picksBody.children.length > 0) {
                  const lastModel = picksBody.lastElementChild;
                  if (lastModel) {
                      lastModel.scrollIntoView({ behavior: 'instant', block: 'end' });
                  }
              }
          });
          await sleep(300);
          await captureShot(page, 'spawn-wizard-model-community-picks.png', { fullPage: true });

        // Second tab (MoE / Offload picks) if present.
        await page.evaluate(() => {
            const tabs = document.querySelectorAll('.hf-cp-tab');
            if (tabs.length > 1) tabs[1].click();
        });
        await sleep(300);
        await captureShot(page, 'spawn-wizard-model-community-picks-moe.png', { fullPage: true });
    }

    // ── Quant advisor: type a known repo so file list populates reliably ──────
    await page.evaluate(() => {
        // Hide non-essential panels so only HF browse + quant advisor are visible
        document.getElementById('hf-community-picks')?.style.setProperty('display', 'none', 'important');
        document.querySelectorAll('.hf-quantizer-tabs-wrap, #hf-chat-template-panel, #model-input-local, .hf-discover-pills').forEach(el => {
            if (el) el.style.display = 'none';
        });
        const body = document.querySelector('.wizard-body');
        if (body) body.scrollTop = 0;
    });
    await sleep(300);

    const repoInput = await page.$('#spawn-hf-repo');
    if (repoInput) {
        await repoInput.click({ clickCount: 3 });
        await repoInput.type('bartowski/Llama-3.2-1B-Instruct-GGUF', { delay: 20 });
        await page.keyboard.press('Enter');
        await sleep(500);

        // Scroll down to show file list area
        await page.evaluate(() => {
            const body = document.querySelector('.wizard-body');
            const fileSection = document.getElementById('spawn-hf-file-list') || document.getElementById('hf-search-results');
            if (body && fileSection) {
                fileSection.scrollIntoView({ behavior: 'instant', block: 'start' });
            }
        });
        await sleep(300);

        // Wait for file list to load (up to 20s)
        let fileListLoaded = await page.waitForFunction(() => {
            const fl = document.getElementById('spawn-hf-file-list');
            return fl && fl.classList.contains('visible') && fl.querySelector('.hf-file-item') !== null;
        }, { timeout: 20000 }).catch(() => false);

        if (fileListLoaded) {
            await sleep(300);
            // Debug: check state before selecting file
            const beforeSelect = await page.evaluate(() => ({
                paramB: wizardState?.model?.paramB || 0,
                modelBytes: wizardState?.model?.modelBytes || 0,
            }));
            console.log('[CAPTURE] Before file select:', JSON.stringify(beforeSelect));

            // Find and click Q4_K_M file using Puppeteer (fires real DOM events)
            const fileItem = await page.$('.hf-file-item');
            console.log('[CAPTURE] Found file item:', !!fileItem);

            if (fileItem) {
                await fileItem.click();
                console.log('[CAPTURE] Clicked file item');
            }

            await sleep(500);
            // Check state immediately after select
            const afterSelect = await page.evaluate(() => ({
                paramB: wizardState?.model?.paramB || 0,
                modelBytes: wizardState?.model?.modelBytes || 0,
                quantAdvisorVisible: document.getElementById('quant-advisor')?.style.display !== 'none',
            }));
            console.log('[CAPTURE] After file select:', JSON.stringify(afterSelect));

            // Wait for quant advisor to populate (poll for VRAM data)
            await page.waitForFunction(() => {
                const advisor = document.getElementById('quant-advisor');
                if (!advisor || advisor.style.display === 'none') return false;
                // Check if quant advisor has content (not just loading state)
                return advisor.textContent.includes('GB') ||
                       advisor.textContent.includes('fits') ||
                       advisor.querySelector('.quant-row');
            }, { timeout: 10000 }).catch(() => {
                // If quant advisor didn't fully populate, inject state as fallback
                console.log('[CAPTURE] Quant advisor timeout, injecting fallback state');
                page.evaluate(() => {
                    if (!wizardState.model.paramB || wizardState.model.paramB === 0) {
                        wizardState.model.paramB = 1.24;
                    }
                    if (typeof triggerQuantAdvisor === 'function') {
                        triggerQuantAdvisor();
                    }
                });
            });

            await sleep(1500);
            // Check final state
            const finalState = await page.evaluate(() => ({
                paramB: wizardState?.model?.paramB || 0,
                quantAdvisorVisible: document.getElementById('quant-advisor')?.style.display !== 'none',
                quantAdvisorContent: document.getElementById('quant-advisor')?.textContent?.substring(0, 150) || '',
            }));
            console.log('[CAPTURE] Final state:', JSON.stringify(finalState));
        } else {
            // File list didn't load, inject model state so quant advisor renders
            await page.evaluate(() => {
                if (!wizardState.model.paramB || wizardState.model.paramB === 0) {
                    wizardState.model.paramB = 1.24;
                }
                if (!wizardState.model.modelBytes || wizardState.model.modelBytes === 0) {
                    wizardState.model.modelBytes = 800_000_000;
                }
                if (!wizardState.vram.available || wizardState.vram.available === 0) {
                    wizardState.vram.available = 64 * 1024 * 1024 * 1024;
                }
                if (typeof triggerQuantAdvisor === 'function') {
                    triggerQuantAdvisor();
                }
            });
            await page.waitForFunction(() => {
                const advisor = document.getElementById('hf-quant-advisor');
                return advisor && advisor.style.display !== 'none' &&
                       (advisor.textContent.includes('GB') || advisor.textContent.includes('fits'));
            }, { timeout: 8000 }).catch(() => {});
        }

        await sleep(800);
        // Debug: check state
        const debug = await page.evaluate(() => ({
            paramB: wizardState?.model?.paramB || 0,
            modelBytes: wizardState?.model?.modelBytes || 0,
            vramAvail: wizardState?.vram?.available || 0,
            quantAdvisorVisible: document.getElementById('quant-advisor')?.style.display !== 'none',
            quantAdvisorContent: document.getElementById('quant-advisor')?.textContent?.substring(0, 100) || '',
            sidebarHasTips: document.querySelector('.wizard-sidebar')?.textContent?.includes('tips'),
        }));
        console.log('[CAPTURE] Quant advisor debug:', JSON.stringify(debug));

        // Scroll to show file list + quant advisor
        await page.evaluate(() => {
            const body = document.querySelector('.wizard-body');
            const fileSection = document.getElementById('spawn-hf-file-list') || document.getElementById('hf-search-results');
            if (body && fileSection) {
                fileSection.scrollIntoView({ behavior: 'instant', block: 'start' });
            }
        });
        await sleep(300);
        await captureShot(page, 'spawn-wizard-model-quant-advisor.png', { fullPage: true });
    }

    // ── Step 2 (Hardware): VRAM ────────────────────────────────────────────────
    // Inject missing model metadata so the VRAM bar renders correctly.
    // Only fill in values that may not have been set by the UI interaction.
    await page.evaluate(async () => {
        const { wizardState } = await import('/js/features/spawn-wizard.js');
        // Set VRAM fallback if API call failed
        if (!wizardState.vram.available || wizardState.vram.available === 0) {
            wizardState.vram.available = 64 * 1024 * 1024 * 1024;
        }
        // Set paramB from filename if inference failed
        if (!wizardState.model.paramB || wizardState.model.paramB === 0) {
            wizardState.model.paramB = 8; // default for 8B models
        }
        // Set modelBytes if not already set
        if (!wizardState.model.modelBytes || wizardState.model.modelBytes === 0) {
            wizardState.model.modelBytes = 4_920_000_000;
        }
    });

    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-1')?.classList.contains('active'),
        { timeout: 8000 }
    ).catch(() => console.log('[CAPTURE] Step 2 (Hardware) wait timed out; continuing.'));
    await sleep(600);

    // Dismiss the HF download panel so the VRAM display is unobscured.
    await page.evaluate(() => {
        document.getElementById('hf-dlp-use-hf-btn')?.click();
        const panel = document.getElementById('hf-download-panel');
        if (panel) panel.style.display = 'none';
    });

    // Force a VRAM refresh so the bar renders with the injected state.
    await page.evaluate(async () => {
        const { scheduleVramUpdate } = await import('/js/features/spawn-wizard.js');
        scheduleVramUpdate();
    });
    await page.waitForFunction(
        () => parseFloat(document.getElementById('vseg-weights')?.style.width || '0') > 1,
        { timeout: 6000 }
    ).catch(() => {});
    await sleep(500);
     await captureShot(page, 'spawn-wizard-hardware-vram.png', { fullPage: true, expandSelector: '.wizard-body' });

    // ── Same step (Hardware & memory): decision cards 3-4 + all settings drawer ─
    await page.evaluate(() => {
        const body = document.querySelector('.wizard-body');
        const card3 = document.getElementById('hw-decision-vision');
        if (body && card3) {
            body.scrollTop = card3.offsetTop - 20;
        }
    });
    await sleep(300);
    await captureShot(page, 'spawn-wizard-hardware-page2.png', { fullPage: false });

    // ── Same step (Hardware & memory): sampling/review config list ───────────
    // Expand All settings drawer to show parameters
    await page.evaluate(() => {
        const btn = document.getElementById('all-settings-btn');
        if (btn) btn.click();
        const list = document.getElementById('spawn-summary-list');
        if (list) list.scrollIntoView({ behavior: 'instant', block: 'start' });
    });
    await sleep(400);
    await captureShot(page, 'spawn-wizard-hardware-parameters.png', { fullPage: true, expandSelector: '.wizard-body' });

    // ── Step 3 (Launch): preset settings + spawn ──────────────────────────────
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-2')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => console.log('[CAPTURE] Step 3 (Launch) wait timed out; continuing.'));
     await sleep(600);
     // Scroll to top of summary step
     await page.evaluate(() => {
         const body = document.querySelector('.wizard-body');
         if (body) body.scrollTop = 0;
     });
     await sleep(300);
     await captureShot(page, 'spawn-wizard-launch-summary.png', { fullPage: true, expandSelector: '.wizard-body' });

    // Preset settings and Spawn are now on the same step (Option A collapse:
    // 6 steps → 3). No navigation click needed between them.
    await page.evaluate(() => {
        const card = document.getElementById('spawn-config-card');
        if (card) card.scrollIntoView({ behavior: 'instant', block: 'start' });
    });
    await sleep(300);
    await captureShot(page, 'spawn-wizard-launch-spawn.png', { fullPage: true, expandSelector: '.wizard-body' });

    // ── M5-A: Pro mode capture (switch view mode, capture left rail + dense layout) ──
    // Close wizard and reopen in Pro mode for clean capture
    await page.keyboard.press('Escape');
    await sleep(300);
    await page.evaluate(async () => {
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard();
    });
    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 10000 });
    await sleep(500);
    await page.evaluate(() => {
        const banner = document.getElementById('wizard-binary-prereq');
        if (banner) banner.style.display = 'none';
    });
    // Select profile + use-case (same as main scenario)
    await page.evaluate(() => {
        (document.querySelector('.profile-card[data-profile="power"]')
            || document.querySelector('.profile-card'))?.click();
    });
    await sleep(200);
    await page.evaluate(() => {
        (document.querySelector('.usecase-card[data-usecase="general"]')
            || document.querySelector('.usecase-card'))?.click();
    });
    await sleep(300);
    // Select HF source
    await page.evaluate(() => {
        document.querySelector('.model-source-card[data-source="hf"]')?.click();
    });
    await sleep(400);
    // Set repo ID via evaluate (avoids select-all/typing issues)
    await page.evaluate(() => {
        const input = document.getElementById('spawn-hf-repo');
        if (input) {
            input.value = 'bartowski/Llama-3.2-1B-Instruct-GGUF';
            input.dispatchEvent(new Event('input', { bubbles: true }));
            input.dispatchEvent(new Event('change', { bubbles: true }));
        }
    });
    await page.keyboard.press('Enter');
    await page.waitForFunction(() => {
        const fl = document.getElementById('spawn-hf-file-list');
        return fl && fl.classList.contains('visible') && fl.querySelector('.hf-file-item') !== null;
    }, { timeout: 20000 }).catch(() => {});
    await sleep(500);
    // Select Q4_K_M so validation passes on Next
    await page.evaluate(() => {
        const q4 = [...document.querySelectorAll('.hf-file-item')]
            .find(el => el.textContent.includes('Q4_K_M') || el.textContent.includes('Q4'));
        (q4 || document.querySelector('.hf-file-item'))?.click();
    });
    await sleep(300);
    // Click Next to go to hardware step
    await page.evaluate(() => {
        const nextBtn = document.getElementById('wizard-next-btn');
        if (nextBtn) nextBtn.click();
    });
    await page.waitForFunction(
        () => document.getElementById('wizard-step-1')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => {});
     await sleep(400);
     // TODO: Pro mode implementation (Phase 10b) — wireframe in docs/archive/rapid-mlx/20260806-spawn_wizard_uiux_redesign.md §4
     // Pro mode: left rail + dense multi-column panes + ⌘K filter + "Modified only" toggle
     // For now, Pro toggle is present but does nothing until Option B is implemented

     // Close wizard.
    await page.keyboard.press('Escape');
    await sleep(400);
}
