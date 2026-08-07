// Scenario: spawn-wizard-engines
// Extracted from tests/ui/capture.mjs (Phase A3).
import { join } from 'path';
import { loadAppDocument } from '../../harness/browser.mjs';
import { TEMP_APP_CONFIG_DIR, sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';
import { openGroup } from '../../harness/wizard.mjs';

export default async function(ctx) {
    const { page, baseUrl } = ctx;
    // Preserve a full-screen product frame while giving the Wizard enough
    // visual weight for settings text to remain readable in artifacts.
    await page.setViewport({ width: 1280, height: 900, deviceScaleFactor: 1 });
    const rapidFixture = join(TEMP_APP_CONFIG_DIR, 'models', 'capture-nested-mlx');
    await loadAppDocument(page, baseUrl);

    await page.evaluate(() => {
        const originalFetch = window.fetch.bind(window);
        window.fetch = (input, init) => {
            const url = new URL(typeof input === 'string' ? input : input.url, window.location.origin);
            if (url.pathname === '/api/rapid-mlx/runtime/status') {
                return Promise.resolve(new Response(JSON.stringify({
                    runtime: { supported: true, active: { version: '0.10.10' } },
                }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
            }
            if (url.pathname === '/api/rapid-mlx/recommend') {
                return Promise.resolve(new Response(JSON.stringify({
                    recommended_backend: 'rapid_mlx',
                    state: 'ready',
                    reason: 'This source is native to the verified Rapid-MLX resolution path.',
                }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
            }
            if (url.pathname.startsWith('/api/rapid-mlx/models/') && url.pathname.endsWith('/profile')) {
                return Promise.resolve(new Response(JSON.stringify({
                    profile: { extras: { vision: true, has_vision_tower: true } },
                }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
            }
            if (url.pathname === '/api/hf/mtp-preflight') {
                return Promise.resolve(new Response(JSON.stringify({
                    ok: true, repoId: 'unsloth/Qwen3.6-MTP-sidecar', revision: 'a1b2c3d',
                    trustRemoteCodeRequired: true,
                }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
            }
            return originalFetch(input, init);
        };
    });

    await page.evaluate(async (rapidFixturePath) => {
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard({
            localPath: rapidFixturePath,
            localModel: {
                path: rapidFixturePath,
                size_bytes: 400_000_000,
                source_kind: 'mlx_directory',
                model_source: {
                    kind: 'mlx_directory',
                    path: rapidFixturePath,
                },
            },
        });
    }, rapidFixture);
    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 10000 });
    await page.waitForFunction(
        () => document.getElementById('wizard-step-0')?.classList.contains('active'),
        { timeout: 5000 }
    );
    await page.waitForFunction(
        () => document.querySelector('.wizard-engine-card[data-engine="rapid_mlx"]')?.classList.contains('selected'),
        { timeout: 5000 }
    );
    await sleep(350);

    await captureShot(page, 'spawn-wizard-engines-dark.png', { fullPage: true, expandSelector: '.wizard-body' });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
    await sleep(250);
    await captureShot(page, 'spawn-wizard-engines-light.png', { fullPage: true, expandSelector: '.wizard-body' });

    await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'reduce' }]);
    await page.setViewport({ width: 430, height: 900, deviceScaleFactor: 1 });
    await sleep(250);
    await captureShot(page, 'spawn-wizard-engines-reduced-narrow.png', { fullPage: true, expandSelector: '.wizard-body' });

    // Use taller viewport so the rapid-hardware-panel fits without flex compression.
    await page.setViewport({ width: 1440, height: 1200 });
    await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'no-preference' }]);
    await page.evaluate(() => { document.documentElement.dataset.theme = 'dark'; });
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-1')?.classList.contains('active'),
        { timeout: 5000 }
    );
    await sleep(300); // let panel layout settle after step transition
    // Re-select Rapid now that the local fixture is fully loaded, which triggers
    // the live profile fetch used to conditionally render the vision control.
    await page.evaluate(() => document.querySelector('.wizard-engine-card[data-engine="rapid_mlx"]')?.click());
    await page.waitForSelector('#rapid-mlx-profile-hints input[type="checkbox"]', { timeout: 5000 });
    // Ensure rapid-hardware-panel is visible and layout is settled
    await page.waitForFunction(
        () => {
            const panel = document.getElementById('rapid-hardware-panel');
            return panel && !panel.hidden;
        },
        { timeout: 3000 }
    );

    await sleep(200); // allow flex layout to settle
    await page.evaluate(() => document.querySelector('#rapid-mlx-profile-hints input[type="checkbox"]')?.click());
    const textOnlyPayload = await page.evaluate(async () => {
        const { buildSpawnPayload, wizardState } = await import('/js/features/spawn-wizard.js');
        return {
            selectedEngine: wizardState.engine.selected,
            toggleState: wizardState.model.rapidMlxMllm,
            serialized: buildSpawnPayload().rapid_mlx?.mllm_vision,
        };
    });
    if (textOnlyPayload.serialized !== 'off') {
        throw new Error(`Rapid-MLX vision toggle did not serialize text-only mode: ${JSON.stringify(textOnlyPayload)}`);
    }
    // Restore the normal Auto path for the visual workload captures below.
    await page.evaluate(() => document.querySelector('#rapid-mlx-profile-hints input[type="checkbox"]')?.click());
    await sleep(500);

    // Helper: scroll the element's nearest scrollable ancestor to center it in the viewport.
    // Not hardcoded to `.wizard-body` — Phase 0.2 re-parented `#rapid-hardware-panel` inside
    // `.wizard-main`, which scrolls independently, so the right container depends on the target.
    const scrollToElement = (selector, yOffset = 0) =>
        page.evaluate((params) => {
            const el = document.querySelector(params.selector);
            if (!el) return;
            let body = el.closest('.wizard-main') || el.closest('.wizard-body');
            if (body && body.scrollHeight <= body.clientHeight) {
                body = document.querySelector('.wizard-body');
            }
            if (!body) return;
            const rect = el.getBoundingClientRect();
            const elementTop = body.scrollTop + rect.top;
            const viewportCenter = body.clientHeight / 2;
            body.scrollTop = elementTop - viewportCenter + params.offset;
        }, { selector, offset: yOffset });

    // spawn-wizard-reasoning-mode-on.png — Reasoning mode ON with KV dtype locked to int8.
    // Shows: reasoning toggle enabled, int4 option disabled in KV dtype select.
    await page.evaluate(() => {
        const reasoningCheckbox = document.getElementById('spawn-rapid-reasoning-mode');
        if (reasoningCheckbox && !reasoningCheckbox.checked) {
            reasoningCheckbox.click();
        }
    });
    await sleep(400);
    await scrollToElement('#spawn-kv-cache-dtype', 20);
    await sleep(300);
    await captureShot(page, 'spawn-wizard-reasoning-mode-on.png', { runtimeTag: 'rapidmlx-local', expandSelector: '.wizard-body' });

    // Speculative decoding controls live inside the collapsible
    // "Companions & experimental acceleration" group (IA reorg); it must be
    // expanded or the controls stay hidden even when their own inline
    // styles say otherwise.
    await openGroup(page, 'companions');
    await page.evaluate(() => {
        const enabled = document.getElementById('spawn-rapid-speculative-enabled');
        if (enabled && !enabled.checked) enabled.click();
    });
    await scrollToElement('#spawn-rapid-speculative-enabled', 30);
    await sleep(250);
    await captureShot(page, 'spawn-wizard-speculative-enabled-dark.png', { runtimeTag: 'rapidmlx-local', expandSelector: '.wizard-body' });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
    await sleep(150);
    await captureShot(page, 'spawn-wizard-speculative-enabled-light.png', { runtimeTag: 'rapidmlx-local', expandSelector: '.wizard-body' });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'dark'; });
    await sleep(200); // allow theme reflow

    // spawn-wizard-speculative-trust-consent.png — Trust remote code consent warning + checkbox.
    // Trigger: enable speculative, switch to external, type an external model repo.
    // The mocked /api/hf/mtp-preflight returns trustRemoteCodeRequired: true.
    await page.evaluate(async () => {
        const { wizardState } = await import('/js/features/spawn-wizard.js');
        // Switch source to external
        const sourceSelect = document.getElementById('spawn-rapid-speculative-source');
        if (sourceSelect && sourceSelect.value !== 'external') {
            sourceSelect.value = 'external';
            sourceSelect.dispatchEvent(new Event('change', { bubbles: true }));
        }
        // Type model and trigger trust check via the debounced path
        const modelInput = document.getElementById('spawn-rapid-speculative-model');
        if (modelInput) {
            modelInput.value = 'unsloth/Qwen3.6-MTP-sidecar';
            modelInput.dispatchEvent(new Event('input', { bubbles: true }));
        }
        // Wait for the debounced trust check to complete, then show the trust wrap
        await new Promise(r => setTimeout(r, 1200));
        // Directly set state and show the trust wrap (cheat for screenshot capture)
        const h = wizardState.hardware;
        h.speculativeTrustRequired = true;
        h.speculativeTrustConsent = false;
        h.speculativeTrustRepoId = 'unsloth/Qwen3.6-MTP-sidecar';
        h.speculativeTrustRevision = 'a1b2c3d';
        document.getElementById('spawn-rapid-speculative-trust-warning').textContent =
            'This companion model requires trust_remote_code (custom Python code execution).';
        const wrap = document.getElementById('spawn-rapid-speculative-trust-wrap');
        if (wrap) wrap.style.display = '';
    });
    // Confirm the trust consent element is actually VISIBLE (not just in DOM).
    await page.waitForFunction(
        () => {
            const el = document.getElementById('spawn-rapid-speculative-trust-wrap');
            return el && el.style.display !== 'none';
        },
        { timeout: 5000 }
    );
    await scrollToElement('#spawn-rapid-speculative-trust-wrap', 0);
    await sleep(300);
    await captureShot(page, 'spawn-wizard-speculative-trust-consent-dark.png', { runtimeTag: 'rapidmlx-local', expandSelector: '.wizard-body' });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
    await sleep(150);
    await scrollToElement('#spawn-rapid-speculative-trust-wrap', 0);
    await sleep(300);
    await captureShot(page, 'spawn-wizard-speculative-trust-consent-light.png', { runtimeTag: 'rapidmlx-local', expandSelector: '.wizard-body' });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'dark'; });

    // spawn-wizard-parser-detected.png — Parser/hybrid dropdowns with "Detected:" hints.
    await page.evaluate(() => {
        const reasoningCheckbox = document.getElementById('spawn-rapid-reasoning-mode');
        if (reasoningCheckbox && reasoningCheckbox.checked) {
            reasoningCheckbox.click();
        }
    });
    await sleep(300);
    await scrollToElement('#spawn-rapid-tool-call-parser', -50);
    await sleep(400);
    await captureShot(page, 'spawn-wizard-parser-detected.png', { runtimeTag: 'rapidmlx-local', expandSelector: '.wizard-body' });

    // Reset reasoning mode.
    await page.evaluate(() => {
        const reasoningCheckbox = document.getElementById('spawn-rapid-reasoning-mode');
        if (reasoningCheckbox && reasoningCheckbox.checked) {
            reasoningCheckbox.click();
        }
    });
    await sleep(300);

    // spawn-wizard-rapid-mlx-advanced-controls.png — Phase 7 cache-entry
    // recommendation and neighboring backend controls.
    // Use the product-sized full-screen viewport for the final Wizard artifacts:
    // it preserves the backdrop without making the settings illegibly small.
    await page.setViewport({ width: 1280, height: 900, deviceScaleFactor: 1 });
    // Put the measured retained-entry guidance in the readable center of the
    // full modal instead of leaving it below the fold beneath MTP controls.
    await scrollToElement('#spawn-rapid-hybrid-cache-entries', -180);
    await sleep(300);
    // Keep the full-screen state: the Wizard context and backdrop are part of
    // the visual evidence, rather than a separate close-up artifact.
    await captureShot(page, 'spawn-wizard-rapid-mlx-advanced-controls.png', { runtimeTag: 'rapidmlx-local', expandSelector: '.wizard-body' });

    // spawn-wizard-rapid-mlx-escape-hatch.png — Advanced escape-hatch flags expanded.
    try {
        await page.evaluate(() => {
            const section = document.getElementById('rapid-mlx-advanced-section');
            if (section && section.style.display !== 'none' && !section.open) {
                section.open = true;
            }
        });
        await sleep(400);
        const sectionVisible = await page.evaluate(() => {
            const section = document.getElementById('rapid-mlx-advanced-section');
            const flags = document.getElementById('rapid-mlx-advanced-flags');
            return section && section.open && flags && flags.children.length > 0;
        });
        if (sectionVisible) {
            await scrollToElement('#rapid-mlx-advanced-section', -30);
            await sleep(300);
            await captureShot(page, 'spawn-wizard-rapid-mlx-escape-hatch.png', { runtimeTag: 'rapidmlx-local', expandSelector: '.wizard-body' });
        } else {
            console.log('[CAPTURE] Escape-hatch section not visible, skipping.');
        }
    } catch (e) {
        console.log('[CAPTURE] Escape-hatch capture failed:', e.message);
    }

    // Deterministic Rapid end-to-end path: use the nested MLX fixture served by
    // the real estimate endpoint.
    await page.waitForFunction(
        () => document.getElementById('wizard-next-btn')?.disabled === false,
        { timeout: 3000 }
    );
    // This is a fit/estimator artifact, not another advanced-controls view.
    // Reset the scrolled form so its full-screen frame starts at the hardware
    // summary and sticky VRAM estimate rather than a cropped lower section.
    await page.evaluate(() => {
        document.querySelector('.wizard-body')?.scrollTo({ top: 0, behavior: 'instant' });
    });
    await sleep(500);
    await captureShot(page, 'spawn-wizard-rapid-mlx-fit.png', { fullPage: true, runtimeTag: 'rapidmlx-local', expandSelector: '.wizard-body' });

    // Reasoning mode ON so the config preview shows "INT4 → INT8 (reasoning
    // profile)". This lives on the same Hardware step (Option A collapse:
    // 6 steps → 3), so no navigation is needed before toggling it.
    await page.evaluate(() => {
        const cb = document.getElementById('spawn-rapid-reasoning-mode');
        if (cb && !cb.checked) cb.click();
    });
    await sleep(400);
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-2')?.classList.contains('active'),
        { timeout: 8000 }
    );
    await sleep(500);
    await captureShot(page, 'spawn-wizard-rapid-mlx-review.png', { fullPage: true, runtimeTag: 'rapidmlx-local', expandSelector: '.wizard-body' });

    console.log('[CAPTURE] Scenario "spawn-wizard-engines" complete.');
}

// Setup wizard HF download panel: idle options + simulated progress.
