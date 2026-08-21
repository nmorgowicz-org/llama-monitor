// Scenario: rapid-mlx-runtime
// Extracted from tests/ui/capture.mjs (Phase A3).
import { loadAppDocument } from '../../harness/browser.mjs';
import { DEFAULT_VIEWPORT, sleep } from '../../harness/paths.mjs';
import { captureCloseUp, captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;

    // Load app without attach (uses auth bypass).
    await loadAppDocument(page, baseUrl);

    // Mock Rapid-MLX runtime endpoints.
    await page.evaluate(() => {
        const originalFetch = window.fetch.bind(window);
        window.fetch = (input, init) => {
            const url = new URL(typeof input === 'string' ? input : input.url, window.location.origin);
            const path = url.pathname;

            // Status: active managed runtime
            if (path === '/api/rapid-mlx/runtime/status') {
                return Promise.resolve(new Response(JSON.stringify({
                    runtime: {
                        supported: true,
                        active: {
                            version: '0.10.10',
                            source: 'managed',
                            extras: ['guided', 'vision'],
                            path: '~/.config/llama-monitor/runtimes/rapid-mlx/0.10.10/venv/bin/rapid-mlx',
                        },
                        last_known_good: { version: '0.10.9' },
                        update_available: '0.10.11',
                    },
                }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
            }

            // Releases list
            if (path === '/api/rapid-mlx/runtime/releases') {
                return Promise.resolve(new Response(JSON.stringify([
                    { version: '0.10.11', tag: 'v0.10.11', prerelease: false, published_at: '2026-07-14T12:00:00Z', body: 'Bug fixes and performance improvements for prefix caching.' },
                    { version: '0.10.10', tag: 'v0.10.10', prerelease: false, published_at: '2026-07-10T08:00:00Z', body: 'Stable release with updated capability profile.' },
                    { version: '0.10.9', tag: 'v0.10.9', prerelease: false, published_at: '2026-06-28T10:00:00Z', body: 'Initial verified release baseline.' },
                ]), { status: 200, headers: { 'Content-Type': 'application/json' } }));
            }

            return originalFetch(input, init);
        };
    });

    // Simulate active session WebSocket data for engine indicator.
    await page.evaluate(() => {
        const wsData = {
            backend: 'rapid_mlx',
            active_session_status: 'running',
            active_session_model_identity: 'mlx-community/Qwen3.5-9B-MLX-4bit',
        };
        if (window.__llamaMonitorUpdateCockpit) {
            window.__llamaMonitorUpdateCockpit(wsData);
        }
        // Also inject via a more general hook in case refreshTopCockpit reads a shared object.
        if (!window.__wsData) window.__wsData = {};
        Object.assign(window.__wsData, wsData);
    });
    await sleep(400);

    // ── 1. Settings: Rapid-MLX Runtime card (Loaders pane) ──
    try {
        await page.evaluate(() => { window.openSettingsModal?.(); });
        await page.waitForSelector('#settings-modal.open', { timeout: 5000 });
        await sleep(600);

        // Switch to Loaders tab where the Rapid-MLX Runtime card lives.
        const loadersTab = await page.$('#settings-modal .settings-tab[data-tab="loaders"]');
        if (loadersTab) {
            await loadersTab.click();
            await sleep(500);
        }

        // Look for the Rapid-MLX Runtime card.
        const runtimeCard = await page.$('#rapid-mlx-runtime-summary');
        if (runtimeCard) {
            await captureShot(page, 'settings-rapid-mlx-runtime-card.png', { fullPage: true });
            await captureCloseUp(page, '#settings-modal', 'settings-rapid-mlx-runtime-card.png', options);
        }
    } catch (e) {
        console.log('[CAPTURE] Settings Rapid-MLX card failed, continuing...');
    }

    // ── 2. Rapid-MLX Runtime Manager modal ──
    try {
        const manageBtn = await page.$('#rapid-mlx-manage-btn');
        if (manageBtn) {
            await manageBtn.click();
            await sleep(600);
            await page.waitForSelector('#rapid-mlx-modal.open', { timeout: 5000 });
            await sleep(400);

            // Dark desktop
            await captureShot(page, 'rapid-mlx-runtime-manager-dark.png', { fullPage: true });

            const developmentTab = await page.$('#rapid-mlx-source-development');
            if (developmentTab) {
                await developmentTab.click();
                await sleep(250);
                await captureShot(page, 'rapid-mlx-runtime-manager-development-dark.png', { fullPage: true });
                await page.click('#rapid-mlx-source-official');
                await sleep(150);
            }

            // Light desktop
            await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
            await sleep(250);
            await captureShot(page, 'rapid-mlx-runtime-manager-light.png', { fullPage: true });

            // Narrow layout
            await page.setViewport({ width: 480, height: 900, deviceScaleFactor: 1 });
            await sleep(250);
            await captureShot(page, 'rapid-mlx-runtime-manager-narrow.png', { fullPage: true });

            // Reduced motion
            await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'reduce' }]);
            await sleep(200);
            await captureShot(page, 'rapid-mlx-runtime-manager-reduced.png', { fullPage: true });

            // Changelog expanded state
            await page.evaluate(() => {
                const changelog = document.getElementById('rapid-mlx-changelog');
                const toggle = document.getElementById('rapid-mlx-changelog-toggle');
                const body = document.getElementById('rapid-mlx-changelog-body');
                if (changelog && changelog.style.display !== 'none' && body && body.style.display === 'none') {
                    toggle?.click();
                }
            });
            await sleep(500);
            const changelogExpanded = await page.evaluate(() => {
                const body = document.getElementById('rapid-mlx-changelog-body');
                return body && getComputedStyle(body).display !== 'none';
            });
            if (changelogExpanded) {
                await captureShot(page, 'rapid-mlx-runtime-changelog-expanded.png', { fullPage: true });
                console.log('[CAPTURE] Saved rapid-mlx-runtime-changelog-expanded.png');
            } else {
                console.log('[CAPTURE] Changelog not available or not expanded, skipping.');
            }

            // Reset viewport
            await page.setViewport(DEFAULT_VIEWPORT);
            await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'no-preference' }]);
            await page.evaluate(() => { document.documentElement.dataset.theme = 'dark'; });
        }
    } catch (e) {
        console.log('[CAPTURE] Runtime manager modal failed, continuing...');
    }

    // Close settings modal.
    try {
        await page.keyboard.press('Escape');
        await sleep(200);
    } catch {}

    // ── 3. Engine indicator in nav bar (with simulated generation active) ──
    try {
        // Force refreshTopCockpit to render the engine indicator with our mocked data.
        await page.evaluate(async () => {
            // Ensure backend/model identity are present.
            const wsData = {
                backend: 'rapid_mlx',
                active_session_status: 'running',
                active_session_model_identity: 'mlx-community/Qwen3.5-9B-MLX-4bit',
                local_status: { slot_generation_active: true },
            };
            if (window.__llamaMonitorUpdateCockpit) {
                window.__llamaMonitorUpdateCockpit(wsData);
            }
        });
        await sleep(300);

        // Check engine indicator is visible.
        const indicatorVisible = await page.evaluate(() => {
            const el = document.getElementById('engine-indicator');
            return el && getComputedStyle(el).display !== 'none';
        });

        if (indicatorVisible) {
            // Full nav bar with indicator
            const navBar = await page.$('#top-bar');
            if (navBar) {
                await captureCloseUp(page, '#top-bar', 'nav-engine-indicator.png', options);
            }

            // Check for separate Rapid-MLX engine pill.
            const pillVisible = await page.evaluate(() => {
                const pill = document.getElementById('rapid-mlx-pill');
                return pill && getComputedStyle(pill).display !== 'none';
            });
            if (pillVisible) {
                await captureCloseUp(page, '#rapid-mlx-pill', 'nav-rapid-mlx-pill.png', options);
            }
        }
    } catch (e) {
        console.log('[CAPTURE] Engine indicator capture failed, continuing...');
    }

    console.log('[CAPTURE] Scenario "rapid-mlx-runtime" complete.');
}

// model-discovery captures the HF Download tab with Phase 8B1 discovery controls:
// scope selector (GGUF/MLX/All), sort control, category badges, author roles.
// Uses REAL HF data — hf-token is copied to temp config (see runCli filesToCopy).
// Named model-discovery (not model-library) because this is HF search/discovery,
// not the installed-model Library tab; models-v2 is the Library evidence scenario.
