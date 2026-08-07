// Scenario: spawn-wizard-hf-download
// Extracted from tests/ui/capture.mjs (Phase A3).
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;

    await gotoApp(page, baseUrl);
    if (!options.noAttach) {
        try { await attachToServer(page); } catch {}
    }

    // Open wizard via JS (safer than DOM click in headless).
    await page.evaluate(async () => {
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard();
    });
    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 8000 });
    await sleep(400);

    // Choose a profile quickly.
    await page.evaluate(() => {
        (document.querySelector('.profile-card[data-profile="power"]')
            || document.querySelector('.profile-card'))?.click();
    });
    await sleep(200);
    await page.evaluate(() => {
        (document.querySelector('.usecase-card[data-usecase="general"]')
            || document.querySelector('.usecase-card'))?.click();
    });
    await sleep(200);

    // Profile/use-case and model selection are on the same step (Option A collapse:
    // 6 steps → 3), so no navigation click is needed before injecting the model.
    // Inject model, then force-jump to the Hardware step below.
    await page.evaluate(async () => {
        const { wizardState } = await import('/js/features/spawn-wizard.js');
        wizardState.model.source   = 'hf';
        wizardState.model.delivery = 'download';
        wizardState.model.hfRepo   = 'bartowski/Meta-Llama-3.1-8B-Instruct-GGUF';
        wizardState.model.hfFile   = 'Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf';
        wizardState.model.paramB   = 8;
        wizardState.model.modelBytes = 4_920_000_000;
        wizardState.vram.available = 64 * 1024 * 1024 * 1024;
    });

    // Move to Hardware step — advance step state + trigger render.
    await page.evaluate(async () => {
        const mod = await import('/js/features/spawn-wizard.js');
        mod.showStep(1);
        mod.scheduleVramUpdate && mod.scheduleVramUpdate();
    });
    await page.waitForFunction(
        () => document.getElementById('wizard-step-1')?.classList.contains('active'),
        { timeout: 8000 }
    ).catch(() => {
        console.log('[CAPTURE] Step 1 (Hardware) wait timed out; continuing.');
    });
    await sleep(600);

    // Ensure the HF download panel is visible in "idle" state.
    await page.evaluate(() => {
        const prereq = document.getElementById('wizard-binary-prereq');
        const panel = document.getElementById('hf-download-panel');
        const idle = document.getElementById('hf-dlp-idle');
        const progress = document.getElementById('hf-dlp-progress');
        const complete = document.getElementById('hf-dlp-complete');
        if (prereq) {
            prereq.style.display = 'none';
        }
        if (panel && idle) {
            panel.style.display = 'block';
            idle.style.display = 'block';
            if (progress) progress.style.display = 'none';
            if (complete) complete.style.display = 'none';
            panel.scrollIntoView({ behavior: 'instant', block: 'center' });
        }
    });
    await sleep(400);

    // 1) Capture idle HF download panel.
    await captureShot(page, 'spawn-wizard-hf-download-idle.png', { fullPage: true, expandSelector: '.wizard-body' });

    // 2) Simulate a progress state for a second shot.
    await page.evaluate(() => {
        const panel = document.getElementById('hf-download-panel');
        const idle = document.getElementById('hf-dlp-idle');
        const progress = document.getElementById('hf-dlp-progress');
        const complete = document.getElementById('hf-dlp-complete');
        const bar = document.getElementById('hf-dlp-bar');
        const pct = document.getElementById('hf-dlp-progress-pct');
        const fileEl = document.getElementById('hf-dlp-progress-file');
        const stats = document.getElementById('hf-dlp-stats');

        if (panel && progress) {
            panel.style.display = 'block';
            if (idle) idle.style.display = 'none';
            progress.style.display = 'block';
            if (complete) complete.style.display = 'none';
            if (bar) bar.style.width = '64%';
            if (pct) pct.textContent = '64%';
            if (fileEl) fileEl.textContent = 'Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf';
            if (stats) stats.textContent = '3.18 GB / 4.92 GB · 98 MiB/s · 17m left';
            panel.scrollIntoView({ behavior: 'instant', block: 'center' });
        }
    });
    await sleep(400);

    // Capture simulated progress.
    await captureShot(page, 'spawn-wizard-hf-download-progress.png', { fullPage: true, expandSelector: '.wizard-body' });

    // Close wizard.
    await page.keyboard.press('Escape');
    await sleep(400);
}

// Chat template Discussions end-to-end: Qwen (froggeric) and Gemma4 (google) workflows.
// Captures: discussions feed dropdown in spawn wizard, Create fix modal with auto-inferred
// HF repo, and the Lifecycle modal Discussions section in the preset editor.
