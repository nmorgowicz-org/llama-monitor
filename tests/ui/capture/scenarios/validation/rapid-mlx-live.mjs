// Scenario: rapid-mlx-live
// Extracted from tests/ui/capture.mjs (Phase A3).
import { gotoApp, switchTab } from '../../harness/browser.mjs';
import { createFreshChat, sendChatPrompt, waitForChatComplete } from '../../harness/chat.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureElementScreenshot, captureShot, deleteRapidLiveTestPreset, waitForRapidTelemetry } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    const presetId = 'rapid-live-test';
    const liveModelRepo = process.env.RAPID_MLX_LIVE_MODEL || 'mlx-community/Qwen3-0.6B-4bit';
    const liveModelPath = process.env.RAPID_MLX_LIVE_MODEL_PATH || '';
    // Derive a unique port from the capture harness port to avoid conflicts with fixed 9321.
    const harnessPort = parseInt(new URL(baseUrl).port || '8892', 10);
    const rapidPort = 9321 + (harnessPort - 8892);

    // Kill any stale rapid-mlx process that might hold the port.
    try {
        const { execSync } = await import('child_process');
        execSync(`lsof -i :${rapidPort} -t 2>/dev/null | xargs kill -9 2>/dev/null || true`, { stdio: 'ignore' });
    } catch {
        // Non-fatal
    }

    // Navigate first so relative fetches inside evaluate resolve correctly.
    await gotoApp(page, baseUrl);

    // Enable Rapid-MLX availability flag (required for mock platform-info endpoint).
    await page.evaluate(() => { window.__captureRapidMlxAvailable = true; });

    // Skip if not macOS (Rapid-MLX local only on Apple Silicon).
    const platform = await page.evaluate(async () => {
        const headers = { ...(window.authHeaders ? window.authHeaders() : {}) };
        const r = await fetch('/api/llama-binary/platform-info', { headers });
        if (!r.ok) {
            throw new Error('platform-info ' + r.status);
        }
        const d = await r.json();
        return { os: d.os, arch: d.arch, rapidAvailable: d.rapid_mlx_local_available };
    });

    if (!platform.rapidAvailable) {
        console.log('[CAPTURE] rapid-mlx-live: skipping — platform not supported or rapid-mlx not on PATH (os=' + platform.os + ', arch=' + platform.arch + ')');
        return;
    }

    console.log('[CAPTURE] rapid-mlx-live: starting full runtime flow (developer-only)');
    let presetCreated = false;
    try {

    // 1. Seed a Rapid-MLX preset with a cached HuggingFaceRepo typed source.
    await page.evaluate(async ({ id, port, modelRepo, modelPath }) => {
        const preset = {
            id,
            name: `${(modelPath || modelRepo).split('/').pop()} · Live Test`,
            backend: 'rapid_mlx',
            rapid_mlx: {
                model_source: modelPath
                    ? { kind: 'mlx_directory', path: modelPath }
                    : { kind: 'hugging_face_repo', repo_id: modelRepo, revision: 'main' },
                served_model_name: 'qwen3-live',
                host: '127.0.0.1',
                port,
                log_level: 'INFO',
                workload_scenario: 'interactive_coding_agent',
                enable_thinking: true,
                default_temperature: 1.0,
                default_top_p: 0.95,
                default_top_k: 20,
                default_min_p: 0.0,
                default_presence_penalty: 0.0,
                default_repetition_penalty: 1.0,
                max_tokens: 2048,
            },
            port,
        };
        const resp = await fetch('/api/presets', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', ...(window.authHeaders ? window.authHeaders() : {}) },
            body: JSON.stringify(preset),
        });
        const result = await resp.json();
        if (!result.ok) throw new Error('Failed to create preset: ' + (result.error || resp.statusText));
        console.log('[CAPTURE] rapid-mlx-live: preset created:', result.preset?.id);
    }, { id: presetId, port: rapidPort, modelRepo: liveModelRepo, modelPath: liveModelPath });
    presetCreated = true;
    await sleep(500);

    // 2. Reload presets and set as active preset.
    await page.evaluate(async (id) => {
        // Refresh sessionState.presets so doStart() finds the new preset.
        const { loadPresets } = await import('/js/features/presets.js');
        await loadPresets(id);
    }, presetId);
    await sleep(500);

    // Verify preset-select is set correctly.
    const selectedPreset = await page.evaluate(() => {
        return document.getElementById('preset-select')?.value;
    });
    console.log('[CAPTURE] rapid-mlx-live: preset-select value:', selectedPreset);

    // 3. Close wizard if open, then spawn via doStart().
    await page.evaluate(async () => {
        const wizard = document.getElementById('spawn-wizard-overlay');
        if (wizard && wizard.classList.contains('open')) {
            const { closeSpawnWizard } = await import('/js/features/spawn-wizard.js');
            closeSpawnWizard();
        }
    });
    await sleep(500);

    // Spawn via doStart().
    await page.evaluate(async () => {
        const { doStart } = await import('/js/features/attach-detach.js');
        await doStart();
    });

    // 4. Check active session status after spawn.
    await sleep(3000); // Give spawn time to initiate.
    const activeSession = await page.evaluate(async () => {
        try {
            const r = await fetch('/api/sessions/active', {
                headers: window.authHeaders ? window.authHeaders() : {}
            });
            return r.ok ? await r.json() : { error: r.status };
        } catch (e) {
            return { error: e.message };
        }
    });
    console.log('[CAPTURE] rapid-mlx-live: active session:', JSON.stringify(activeSession).slice(0, 500));

    // Check for any spawn errors in the UI.
    const spawnError = await page.evaluate(() => {
        const toast = document.querySelector('.toast-body')?.textContent || '';
        const errorEl = document.querySelector('.spawn-error')?.textContent || '';
        return toast || errorEl;
    });
    if (spawnError) {
        console.log('[CAPTURE] rapid-mlx-live: UI error detected:', spawnError);
    }

    // Check spawn wizard state for errors.
    const wizardState = await page.evaluate(() => {
        return document.getElementById('spawn-wizard-overlay') ? {
            visible: true,
            status: document.querySelector('#spawn-status-text')?.textContent || '',
            error: document.querySelector('.spawn-error')?.textContent || '',
        } : { visible: false };
    });
    console.log('[CAPTURE] rapid-mlx-live: wizard state:', wizardState);

    // 4. Wait for health endpoint (up to 120s for model download + load).
    console.log('[CAPTURE] rapid-mlx-live: waiting for health (120s timeout)...');
    const health = await waitForRapidTelemetry(page, 120000).catch((err) => {
        console.log('[CAPTURE] rapid-mlx-live: health check failed:', err.message);
        console.log('[CAPTURE] rapid-mlx-live: NOTE — this scenario requires rapid-mlx on PATH + cached mlx-community/Qwen3-0.6B-4bit model');
        throw err;
    });
    console.log('[CAPTURE] rapid-mlx-live: runtime active —', JSON.stringify(health).slice(0, 400));

    await sleep(2000); // Let telemetry initialize.

    // 5. Capture dashboard telemetry cards right after spawn. Totals are legitimately zero
    // here — no request has been sent yet — so this frame documents the pre-chat/idle state,
    // not "the" dashboard. The post-chat capture below is the one with meaningful totals.
    await switchTab(page, 'server');
    await page.waitForSelector('#rapid-mlx-card-grid', { timeout: 15000 });
    await sleep(3000); // Wait for real telemetry to populate.
    await captureElementScreenshot(page, '#inference-section', 'rapid-mlx-live-dashboard-idle.png', { padding: 24 });

    // Verify cards show real data (not loading/empty states).
    const telemetryState = await page.evaluate(() => {
        const runtimeCard = document.querySelector('[data-card-id="runtime"]');
        const hasVersion = runtimeCard?.textContent?.includes('v');
        const hasUptime = runtimeCard?.textContent?.includes('uptime') || runtimeCard?.textContent?.includes('Up for');
        return { hasVersion, hasUptime };
    });
    console.log('[CAPTURE] rapid-mlx-live: telemetry state:', telemetryState);

    // 6. Real chat: send prompt and wait for response.
    await switchTab(page, 'chat');
    await createFreshChat(page);
    await sendChatPrompt(page, 'Say hello and stop.');
    await waitForChatComplete(page, 180000);

    await sleep(1000);
    await captureShot(page, 'rapid-mlx-live-chat-response.png', { fullPage: true });

    // Verify response is from the model (not empty/error).
    const assistantText = await page.evaluate(() => {
        const msg = document.querySelector('#chat-messages .chat-message-assistant .chat-msg-body');
        return msg?.textContent?.trim()?.slice(0, 100) || null;
    });
    console.log('[CAPTURE] rapid-mlx-live: assistant response:', assistantText);
    if (!assistantText || assistantText.length < 3) {
        console.log('[CAPTURE] rapid-mlx-live: WARNING — response appears empty');
    }

    // 6b. Capture dashboard telemetry cards after the request completes, while the session is
    // still live, so this frame (unlike the idle one above) shows meaningful request/token
    // totals rather than zeros. Keep this separate from the post-stop historic frame below,
    // which reflects the runtime's final/cumulative state after teardown.
    await switchTab(page, 'server');
    await page.waitForSelector('#rapid-mlx-card-grid', { timeout: 15000 });
    await sleep(2000); // Let the post-request telemetry poll land.
    await captureElementScreenshot(page, '#inference-section', 'rapid-mlx-live-dashboard-active.png', { padding: 24 });

    // 7. Stop the model and verify cleanup.
    await switchTab(page, 'server');
    await sleep(500);

    // Call doStop() via JS for reliability.
    await page.evaluate(async () => {
        const { doStop } = await import('/js/features/attach-detach.js');
        if (typeof doStop === 'function') await doStop();
    });
    await sleep(3000);

    // Verify the session is no longer active. `/api/rapid-mlx/runtime/status` reflects the
    // managed-runtime *installation*, not the spawned session, so it can stay "active" after
    // doStop() tears down the process — check the actual session/process contract instead,
    // the same one waitForRapidTelemetry() uses to detect a live session.
    const stopped = await page.evaluate(async () => {
        try {
            const r = await fetch('/api/sessions/active', {
                headers: window.authHeaders ? window.authHeaders() : {}
            });
            if (!r.ok) return true;
            const d = await r.json();
            return d.error === 'No active session' || d.backend !== 'rapid_mlx' || d.status !== 'Running';
        } catch {
            return false;
        }
    });
    console.log('[CAPTURE] rapid-mlx-live: model stopped?', stopped);

    await captureShot(page, 'rapid-mlx-live-stopped.png', { fullPage: true });

    console.log('[CAPTURE] rapid-mlx-live: complete');
    } finally {
        // A failed health/chat/screenshot step must not strand the managed
        // Rapid child or leave the temporary preset behind.
        try {
            await page.evaluate(async () => {
                const { doStop } = await import('/js/features/attach-detach.js');
                if (typeof doStop === 'function') await doStop();
            });
            await sleep(1000);
        } catch (error) {
            console.log('[CAPTURE] rapid-mlx-live: stop cleanup non-fatal:', error.message);
        }
        if (presetCreated) await deleteRapidLiveTestPreset(page, presetId);
    }
}

// ── Rapid-MLX Runtime Manager, Engine Indicator, and Settings Card ──
// Captures the Rapid-MLX runtime management UI in Settings and the nav bar engine indicator.
