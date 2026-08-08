// Scenario: spawn-wizard-gif
// SCENARIO INTENT: Animate the current llama.cpp wizard flow for diagnostic documentation.
// Extracted from tests/ui/capture.mjs (Phase A3).
import fs from 'fs';
import { join } from 'path';
import { execFileSync } from 'child_process';
import { gotoApp } from '../../harness/browser.mjs';
import { currentArtifactsDir, tagFilename, FRAME_DIR, sleep } from '../../harness/paths.mjs';
import { cleanupFrames } from '../../harness/shot.mjs';
import { recordArtifact } from '../../harness/receipt.mjs';

export default async function(ctx, _options) {
    const { page, baseUrl } = ctx;
    const fps = 10;
    let frameIdx = 0;

    await gotoApp(page, baseUrl);
    fs.mkdirSync(FRAME_DIR, { recursive: true });
    console.log('[CAPTURE] Starting spawn-wizard-gif sequential capture...');

    // Capture N milliseconds of the current page state.
    const capture = async (durationMs) => {
        const frameMs = 1000 / fps;
        const n = Math.max(1, Math.round(durationMs / frameMs));
        for (let i = 0; i < n; i++) {
            const path = join(FRAME_DIR, `spawn-wizard-gif_${String(frameIdx).padStart(3, '0')}.png`);
            await page.screenshot({ path });
            frameIdx++;
            if (i < n - 1) await sleep(frameMs);
        }
    };

    // ── Welcome screen ────────────────────────────────────────────────────────
    // Brief hold so viewer registers we are at the app entry point.
    await capture(1500);

    // ── Open wizard ───────────────────────────────────────────────────────────
    await page.evaluate(async () => {
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard();
    });
    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 8000 });
    // Hide the binary prereq banner — it's expected info but clutters the GIF.
    await page.evaluate(() => {
        const banner = document.getElementById('wizard-binary-prereq');
        if (banner) banner.style.display = 'none';
    });
    await sleep(400);
    await capture(800);

    // ── Step 0: Profile ───────────────────────────────────────────────────────
    // Show the initial cards, then make selections with dwell time between.
    await capture(600);

    await page.evaluate(() => {
        (document.querySelector('.profile-card[data-profile="power"]')
            || document.querySelector('.profile-card'))?.click();
    });
    await sleep(200);
    await capture(700);

    await page.evaluate(() => {
        (document.querySelector('.usecase-card[data-usecase="general"]')
            || document.querySelector('.usecase-card'))?.click();
    });
    await sleep(200);
    await capture(1000); // Dwell on selections before advancing.

    // Profile and model selection are on the same step (Option A collapse:
    // 6 steps → 3). No navigation click needed between them.
    await capture(600);

    // ── Step 1: Model source — select HuggingFace ─────────────────────────────
    await page.evaluate(() => {
        document.querySelector('.model-source-card[data-source="hf"]')?.click();
    });
    await sleep(300);
    await capture(700); // HF panel opens.

    // Show the discover pills row (static content, no network needed).
    await capture(600);

    // Show community picks if available (loaded from local community-picks.json).
    const cpToggle = await page.$('#hf-cp-toggle');
    if (cpToggle) {
        await cpToggle.click();
        await page.waitForFunction(
            () => document.getElementById('hf-cp-toggle')?.getAttribute('aria-expanded') === 'true',
            { timeout: 3000 }
        ).catch(() => {});
        await sleep(300);
        // Scroll down to reveal community picks content.
        await page.evaluate(() => {
            const body = document.querySelector('.wizard-body');
            if (body) body.scrollTop = 400;
        });
        await sleep(200);
        await capture(1200);
        // Scroll back to top for next steps.
        await page.evaluate(() => {
            const body = document.querySelector('.wizard-body');
            if (body) body.scrollTop = 0;
        });
        // Collapse picks so the HF repo input is visible again.
        await cpToggle.click();
        await sleep(200);
    }

    // Inject the full model state now so validation passes on Next.
    // Using Qwen3.6-35B-A3B Q4_K_M (MoE, ~20.6 GB file). On a 24 GB GPU this loads fully
    // on GPU with ~3–4 GB headroom — realistic for a high-end laptop (RTX 4080/4090).
    await page.evaluate(async () => {
        const { wizardState } = await import('/js/features/spawn-wizard.js');
        wizardState.model.source   = 'hf';
        wizardState.model.delivery = 'stream_hf';
        wizardState.model.hfRepo   = 'unsloth/Qwen3.6-35B-A3B-GGUF';
        wizardState.model.hfFile   = 'Qwen3.6-35B-A3B-UD-Q4_K_M.gguf';
        wizardState.model.paramB   = 35;
        wizardState.model.modelBytes = 20_600_000_000; // Q4_K_M ~20.6 GB
        // Fallback VRAM if fetchGpuVram fails (matches an RTX 4080/4090 mobile).
        wizardState.vram.available = 24 * 1024 * 1024 * 1024;
    });

    // Show the repo input filled in for visual context.
    await page.$eval('#spawn-hf-repo', el => {
        el.value = 'unsloth/Qwen3.6-35B-A3B-GGUF';
        el.dispatchEvent(new Event('input', { bubbles: true }));
    }).catch(() => {});
    await sleep(200);
    await capture(1000); // Dwell on filled-in HF panel.

    // ── Step 0 → Step 1: Hardware / VRAM ─────────────────────────────────────
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-1')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => console.log('[CAPTURE] Step 1 wait timed out; continuing.'));
    await sleep(400);
    await capture(500);

    // The HF download panel may appear when entering step 2 with an HF file selected.
    // Dismiss it so the VRAM display is unobscured.
    const dlpUseBtn = await page.$('#hf-dlp-use-hf-btn');
    if (dlpUseBtn) {
        await dlpUseBtn.click();
        await sleep(300);
    }

    // Ensure the VRAM display is triggered (scheduleVramUpdate is called by showStep, but
    // vram.available is read from wizardState at display time so it may need a nudge).
    await page.evaluate(async () => {
        const { scheduleVramUpdate } = await import('/js/features/spawn-wizard.js');
        scheduleVramUpdate();
    });

    // Wait for the weights bar to render (confirms VRAM math ran).
    await page.waitForFunction(
        () => parseFloat(document.getElementById('vseg-weights')?.style.width || '0') > 1,
        { timeout: 6000 }
    ).catch(() => console.log('[CAPTURE] VRAM weights bar not populated; continuing.'));
    await sleep(300);

    // ── VRAM panel — initial state (default 8 K ctx) ──────────────────────────
    await capture(2500); // Dwell — this is the centrepiece of the GIF.

    // ── VRAM panel — bump context to 32 K to show KV growing ─────────────────
    await page.evaluate(() => {
        const input = document.getElementById('spawn-context-size');
        if (input) {
            input.value = '32768';
            input.dispatchEvent(new Event('input', { bubbles: true }));
        }
    });
    await sleep(350); // Let debounce fire and bar animate.
    await capture(2000);

    // ── VRAM panel — max GPU layers (all layers on GPU) ───────────────────────
    await page.evaluate(() => {
        const sel = document.getElementById('spawn-gpu-layers');
        if (sel) {
            sel.value = '-1';
            sel.dispatchEvent(new Event('change', { bubbles: true }));
        }
    });
    await sleep(350);
    await capture(1500);

    // ── Same step (Hardware & memory): summary / sampling / network fields ───
    // Option A collapse merged the former Summary step into this one's DOM,
    // further down the page — no navigation needed to reach it.
    // Wait for renderSummary() to populate the list (also applies sampling defaults).
    await sleep(800);
    // Capture the top of the summary step (sampling + network fields).
    await capture(1000);
    // Scroll down to show the config summary list.
    await page.evaluate(() => {
        const list = document.getElementById('spawn-summary-list');
        if (list) list.scrollIntoView({ behavior: 'instant', block: 'start' });
        else {
            const body = document.querySelector('#wizard-step-1 .wizard-main');
            if (body) body.scrollTop = body.scrollHeight;
        }
    });
    await sleep(300);
    await capture(2500); // Hold on the summary list so viewer can read key choices.

    // ── Step 1 → Step 2: Launch (preset settings + spawn) ────────────────────
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-2')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => console.log('[CAPTURE] Step 2 wait timed out; continuing.'));
    // _renderPresetParamsStep() runs synchronously inside showStep; give the DOM a tick.
    await sleep(400);
    // Capture the top of the params page (Model + Hardware sections).
    await capture(1500);
    // Scroll to reveal Sampling and Network sections.
    await page.evaluate(() => {
        const main = document.querySelector('#wizard-step-2 .wizard-main');
        if (main) main.scrollTop = 340;
    });
    await sleep(250);
    await capture(1500);
    // Scroll to the Save Preset row at the bottom.
    await page.evaluate(() => {
        const row = document.getElementById('spawn-save-preset-row');
        if (row) row.scrollIntoView({ behavior: 'instant', block: 'center' });
        else {
            const main = document.querySelector('#wizard-step-2 .wizard-main');
            if (main) main.scrollTop = main.scrollHeight;
        }
    });
    await sleep(250);
    await capture(2000); // Hold so viewer sees the preset name + Save button.

    // Preset settings and Spawn are now on the same step (Option A collapse:
    // 6 steps → 3). No navigation click needed between them.
    await page.evaluate(() => {
        const card = document.getElementById('spawn-config-card');
        if (card) card.scrollIntoView({ behavior: 'instant', block: 'start' });
    });
    await sleep(300);
    await capture(2500); // Hold on the Spawn step — config card + Spawn Server button.

    // ── Convert frames → GIF ──────────────────────────────────────────────────
    // Scale to 900 px wide so the GIF fits GitHub README content width without
    // clipping. Height is computed proportionally (-1).
    console.log(`[CAPTURE] Converting ${frameIdx} frames to GIF at ${fps} fps (scaled to 900 px)...`);
    const output = tagFilename('spawn-wizard-flow.gif');
    execFileSync('ffmpeg', [
        '-y',
        '-framerate', String(fps),
        '-i', join(FRAME_DIR, `spawn-wizard-gif_%03d.png`),
        '-vf', 'scale=900:-1:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5',
        join(currentArtifactsDir(), output),
    ], { stdio: 'inherit' });
    recordArtifact(output, page.viewport());
    cleanupFrames();
    console.log('[CAPTURE] spawn-wizard-flow.gif complete.');
}

// Rapid-MLX variant of the spawn wizard GIF — demonstrates the full wizard flow
// with Rapid-MLX engine selection, Rapid-specific hardware controls (KV dtype,
// retained cache, reasoning mode, parsers, hybrid mode), profile hints, and summary.
// Uses mocked runtime status and an MLX model fixture to keep the path fully deterministic.
