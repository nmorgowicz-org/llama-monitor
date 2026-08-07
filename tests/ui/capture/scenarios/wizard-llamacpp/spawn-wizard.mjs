// Scenario: spawn-wizard
// Extracted from tests/ui/capture.mjs (Phase A3).
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);

    // Open setup wizard. Attach is not required; wizard works from the welcome screen.
    await page.evaluate(async () => {
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard();
    });
    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 10000 });
    await sleep(600);

    // Hide the binary prereq banner so it doesn't clutter every shot.
    await page.evaluate(() => {
        const banner = document.getElementById('wizard-binary-prereq');
        if (banner) banner.style.display = 'none';
    });
    await sleep(200);

    // ── Step 0: profile + use-case — capture AFTER selections so state is visible ─
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
    await captureShot(page, 'spawn-wizard-step1-profiles.png', { fullPage: true });

    // Profile/use-case and model selection are now on the same step (Option A
    // collapse: 6 steps → 3). No navigation click needed between them.

    // ── Step 0: model source cards — capture before selecting HF ─────────────
    await captureShot(page, 'spawn-wizard-step2-source-cards.png', { fullPage: true });

    // Select HuggingFace source.
    await page.evaluate(() => {
        document.querySelector('.model-source-card[data-source="hf"]')?.click();
    });
    await sleep(400);

    // Capture the base HF panel: discover pills + quickpick row, no search started.
    await captureShot(page, 'spawn-wizard-step2-hf-base.png', { fullPage: true });

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
    const trendingPill = await page.$('.hf-discover-pill[data-cat-id="trending"]');
    if (trendingPill) {
        await trendingPill.click();
        await waitForResults();
        await sleep(400);
        await scrollToResults();
        await sleep(200);
        await captureShot(page, 'spawn-wizard-step2-discover-trending.png', { fullPage: true });
    }

    // ── Discover pill: Qwen3 ─────────────────────────────────────────────────
    const qwen3Pill = await page.$('.hf-discover-pill[data-cat-id="qwen3"]');
    if (qwen3Pill) {
        await qwen3Pill.click();
        await waitForResults();
        await sleep(400);
        await scrollToResults();
        await sleep(200);
        await captureShot(page, 'spawn-wizard-step2-discover-qwen3.png', { fullPage: true });
    }

    // ── Quantizer quick-pick: bartowski ──────────────────────────────────────
    const bartowskiBtn = await page.$('.hf-qp-btn[data-author="bartowski"]');
    if (bartowskiBtn) {
        await bartowskiBtn.click();
        await waitForResults();
        await sleep(400);
        await scrollToResults();
        await sleep(200);
        await captureShot(page, 'spawn-wizard-step2-quantizer-bartowski.png', { fullPage: true });
    }

    // ── Community picks panel ─────────────────────────────────────────────────
    await page.evaluate(() => {
        const r = document.getElementById('hf-search-results');
        if (r) { r.style.display = 'none'; r.innerHTML = ''; }
        document.querySelectorAll('.hf-discover-pill, .hf-qp-btn')
            .forEach(p => p.classList.remove('active'));
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
        await page.evaluate(() => {
            const picks = document.getElementById('hf-community-picks');
            if (picks) picks.scrollIntoView({ behavior: 'instant', block: 'start' });
        });
        await sleep(300);
        await captureShot(page, 'spawn-wizard-step2-community-picks.png', { fullPage: true });

        // Second tab (MoE / Offload picks) if present.
        await page.evaluate(() => {
            const tabs = document.querySelectorAll('.hf-cp-tab');
            if (tabs.length > 1) tabs[1].click();
        });
        await sleep(300);
        await captureShot(page, 'spawn-wizard-step2-community-picks-moe.png', { fullPage: true });
    }

    // ── Quant advisor: type a known repo so file list populates reliably ──────
    await page.evaluate(() => {
        const cp = document.getElementById('hf-community-picks');
        if (cp) cp.style.display = 'none';
        const body = document.querySelector('.wizard-body');
        if (body) body.scrollTop = 0;
    });
    await sleep(200);

    const repoInput = await page.$('#spawn-hf-repo');
    if (repoInput) {
        await repoInput.click({ clickCount: 3 });
        await repoInput.type('bartowski/Llama-3.2-1B-Instruct-GGUF', { delay: 20 });
        await page.keyboard.press('Enter');
        await page.waitForFunction(() => {
            const fl = document.getElementById('spawn-hf-file-list');
            return fl && fl.classList.contains('visible') && fl.querySelector('.hf-file-item') !== null;
        }, { timeout: 20000 }).catch(() => {});
        await sleep(500);
        await captureShot(page, 'spawn-wizard-step2-quant-advisor.png', { fullPage: true });

        // Select Q4_K_M so validation passes on Next.
        await page.evaluate(() => {
            const q4 = [...document.querySelectorAll('.hf-file-item')]
                .find(el => el.textContent.includes('Q4_K_M') || el.textContent.includes('Q4'));
            (q4 || document.querySelector('.hf-file-item'))?.click();
        });
        await sleep(300);
    }

    // ── Step 2: Hardware / VRAM ───────────────────────────────────────────────
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
    await captureShot(page, 'spawn-wizard-step3-vram.png', { fullPage: true });

    // ── Same step (Hardware & memory): sampling/review config list ───────────
    // Option A collapse merged the former Review/Summary step into this one's
    // DOM, further down the page — no navigation needed to reach it.
    await page.evaluate(() => {
        const list = document.getElementById('spawn-summary-list');
        if (list) list.scrollIntoView({ behavior: 'instant', block: 'start' });
    });
    await sleep(300);
    await captureShot(page, 'spawn-wizard-step4-parameters.png', { fullPage: true });

    // ── Step 2: Launch (preset settings + spawn) ────────────────────────────────
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-2')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => console.log('[CAPTURE] Step 3 (Launch) wait timed out; continuing.'));
    await sleep(600);
    // Scroll to the Save Preset row.
    await page.evaluate(() => {
        const row = document.getElementById('spawn-save-preset-row');
        if (row) row.scrollIntoView({ behavior: 'instant', block: 'center' });
    });
    await sleep(300);
    await captureShot(page, 'spawn-wizard-step5-summary.png', { fullPage: true });

    // Preset settings and Spawn are now on the same step (Option A collapse:
    // 6 steps → 3). No navigation click needed between them.
    await page.evaluate(() => {
        const card = document.getElementById('spawn-config-card');
        if (card) card.scrollIntoView({ behavior: 'instant', block: 'start' });
    });
    await sleep(300);
    await captureShot(page, 'spawn-wizard-step6-spawn.png', { fullPage: true });

    // Close wizard.
    await page.keyboard.press('Escape');
    await sleep(400);
}
