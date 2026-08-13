import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

function llamaPreset() {
    return {
        id: 'calibration-source',
        name: 'Calibration source',
        backend: 'llama_cpp',
        model_path: '/models/calibration.gguf',
        context_size: 8192,
        ctk: 'q8_0',
        ctv: 'q8_0',
        batch_size: 2048,
        ubatch_size: 512,
        parallel_slots: 1,
        port: 8001,
    };
}

async function boot(page) {
    await page.route('**/api/settings', route => route.fulfill({
        status: 200, contentType: 'application/json', body: JSON.stringify({ preset_id: 'calibration-source' }),
    }));
    await page.route('**/api/sessions/active', route => route.fulfill({
        status: 200, contentType: 'application/json', body: JSON.stringify({ status: 'Stopped', preset_id: '' }),
    }));
    await page.route('**/api/sessions/active/readiness', route => route.fulfill({
        status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, ready: true }),
    }));
    await page.route('**/api/presets', route => route.fulfill({
        status: 200, contentType: 'application/json', body: JSON.stringify([llamaPreset()]),
    }));
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await dismissAuthShell(page);
    await expect(page.locator('#preset-select option')).toHaveCount(1);
}

test.describe('Calibration preset flow', () => {
    test('preflight and bounded result review are llama.cpp-only', async ({ page }) => {
        await boot(page);
        await page.route('**/api/calibrations/preflight', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({
                ok: true,
                preflight: {
                    preset_id: 'calibration-source',
                    preset_fingerprint: 'sha256:test',
                    candidate_ids: ['baseline', 'bounded-batch'],
                    planned_trials: 2,
                    confirmation: 'CALIBRATE',
                },
            }),
        }));
        await page.route('**/api/calibrations', async route => {
            if (route.request().method() !== 'POST') { await route.continue(); return; }
            await route.fulfill({
                status: 202,
                contentType: 'application/json',
                body: JSON.stringify({ ok: true, job: { id: 'job-1', state: 'queued', planned_trials: 2, completed_trials: 0 } }),
            });
        });
        await page.route('**/api/calibrations/job-1', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ ok: true, job: { id: 'job-1', state: 'complete', phase: 'complete', planned_trials: 2, completed_trials: 2 } }),
        }));
        await page.route('**/api/calibrations/job-1/receipt', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({
                ok: true,
                receipt: {
                    selected_candidate: 'bounded-batch',
                    candidate_results: [
                        { candidate: { id: 'baseline' }, measurement: { status: 'ok', tg_tps_samples: [12.1] } },
                        { candidate: { id: 'bounded-batch' }, measurement: { status: 'ok', tg_tps_samples: [13.4] } },
                    ],
                },
            }),
        }));

        await page.evaluate(async () => {
            const { openPresetModal } = await import('/js/features/presets.js');
            openPresetModal('edit');
        });
        await expect(page.locator('#preset-modal-calibrate')).toBeVisible();
        await page.locator('#preset-modal-calibrate').click();
        await expect(page.locator('#calibration-modal')).toBeVisible();
        await expect(page.locator('#calibration-start')).toBeEnabled();
        await expect(page.locator('#calibration-candidates')).toContainText('bounded-batch');
        await page.locator('#calibration-start').click();
        await expect(page.locator('#calibration-apply')).toBeEnabled({ timeout: 5000 });
        await expect(page.locator('#calibration-candidates')).toContainText('13.4 tok/s');
    });
});
