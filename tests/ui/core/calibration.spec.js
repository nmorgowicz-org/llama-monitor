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

function rapidPreset() {
    return {
        id: 'rapid-source', name: 'Rapid source', schema_version: 3, backend: 'rapid_mlx', model_path: '', context_size: 8192,
        rapid_mlx: {
            model_path: 'mlx-community/Qwen3-30B-A3B-4bit', served_model_name: 'rapid-source',
            host: '127.0.0.1', port: 9123, log_level: 'INFO', kv_cache_dtype: 'int4',
            reasoning_mode: 'on', enable_thinking: true, prefix_cache_enabled: true,
            retained_cache_mib: 8192, prefill_step_size: 512,
        },
    };
}

async function boot(page, presets = [llamaPreset()]) {
    await page.route('**/api/settings', route => route.fulfill({
        status: 200, contentType: 'application/json', body: JSON.stringify({ preset_id: presets[0].id }),
    }));
    await page.route('**/api/sessions/active', route => route.fulfill({
        status: 200, contentType: 'application/json', body: JSON.stringify({ status: 'Stopped', preset_id: '' }),
    }));
    await page.route('**/api/sessions/active/readiness', route => route.fulfill({
        status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, ready: true }),
    }));
    await page.route('**/api/presets', route => route.fulfill({
        status: 200, contentType: 'application/json', body: JSON.stringify(presets),
    }));
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await dismissAuthShell(page);
    await expect(page.locator('#preset-select option')).toHaveCount(1);
}

async function openEditor(page) {
    await page.evaluate(async () => {
        const { openPresetModal } = await import('/js/features/presets.js');
        openPresetModal('edit');
    });
}

async function installHappyCalibration(page) {
    await page.route('**/api/calibrations/preflight', route => route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
            ok: true,
            preflight: {
                preset_id: 'calibration-source', preset_fingerprint: 'sha256:test',
                candidate_ids: ['baseline', 'bounded-batch'], planned_trials: 2, confirmation: 'CALIBRATE',
            },
        }),
    }));
    await page.route('**/api/calibrations', async route => {
        if (route.request().method() !== 'POST') { await route.continue(); return; }
        await route.fulfill({
            status: 202, contentType: 'application/json',
            body: JSON.stringify({ ok: true, job: { id: 'job-1', state: 'queued', planned_trials: 2, completed_trials: 0 } }),
        });
    });
    await page.route('**/api/calibrations/job-1', route => route.fulfill({
        status: 200, contentType: 'application/json',
        body: JSON.stringify({ ok: true, job: { id: 'job-1', state: 'complete', phase: 'complete', planned_trials: 2, completed_trials: 2 } }),
    }));
    await page.route('**/api/calibrations/job-1/receipt', route => route.fulfill({
        status: 200, contentType: 'application/json',
        body: JSON.stringify({ ok: true, receipt: {
            selected_candidate: 'bounded-batch',
            baseline: {
                effective: {
                    batch_size: { value: '2048', source: 'calibration_policy' },
                    ubatch_size: { value: '512', source: 'calibration_policy' },
                },
                llama_server_help_defaults: { batch_size: '2048', ubatch_size: '512' },
                llama_server_help_sha256: 'help-hash',
            },
            candidate_results: [
                { candidate: { id: 'baseline' }, measurement: { status: 'ok', tg_tps_samples: [12.1] } },
                { candidate: { id: 'bounded-batch' }, measurement: { status: 'ok', tg_tps_samples: [13.4] } },
            ],
        } }),
    }));
}

test.describe('Calibration preset flow', () => {
    test('preflight and bounded result review are llama.cpp-only', async ({ page }) => {
        await boot(page);
        await installHappyCalibration(page);
        await openEditor(page);
        await expect(page.locator('#preset-modal-calibrate')).toBeVisible();
        await page.locator('#preset-modal-calibrate').click();
        await expect(page.locator('#calibration-modal')).toBeVisible();
        await expect(page.locator('#calibration-start')).toBeEnabled();
        await expect(page.locator('#calibration-candidates')).toContainText('bounded-batch');
        await page.locator('#calibration-start').click();
        await expect(page.locator('#calibration-apply')).toBeEnabled({ timeout: 5000 });
        await expect(page.locator('#calibration-candidates')).toContainText('13.4 tok/s');
        await expect(page.locator('#calibration-baseline')).toContainText('batch size');
        await expect(page.locator('#calibration-baseline')).toContainText('2048 (calibration policy)');
        await expect(page.locator('#calibration-baseline-help-note')).toContainText('help-hash');
    });

    test('stale preflight is shown without opening a run', async ({ page }) => {
        await boot(page);
        await page.route('**/api/calibrations/preflight', route => route.fulfill({
            status: 400, contentType: 'application/json', body: JSON.stringify({ ok: false, error: 'Preset changed since preflight' }),
        }));
        await openEditor(page);
        await page.locator('#preset-modal-calibrate').click();
        await expect(page.locator('#calibration-modal')).toBeVisible();
        await expect(page.locator('#calibration-error')).toContainText('Preset changed since preflight');
        await expect(page.locator('#calibration-start')).toBeDisabled();
    });

    test('cancel requests cleanup and closes the modal', async ({ page }) => {
        await boot(page);
        let cancelled = false;
        await page.route('**/api/calibrations/preflight', route => route.fulfill({
            status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, preflight: {
                preset_id: 'calibration-source', preset_fingerprint: 'sha256:test', candidate_ids: ['baseline'], planned_trials: 1, confirmation: 'CALIBRATE',
            } }),
        }));
        await page.route('**/api/calibrations', route => route.fulfill({
            status: 202, contentType: 'application/json', body: JSON.stringify({ ok: true, job: { id: 'job-1', state: 'queued', planned_trials: 1, completed_trials: 0 } }),
        }));
        await page.route('**/api/calibrations/job-1/cancel', async route => {
            cancelled = true;
            await route.fulfill({ status: 202, contentType: 'application/json', body: JSON.stringify({ ok: true }) });
        });
        await openEditor(page);
        await page.locator('#preset-modal-calibrate').click();
        await page.locator('#calibration-start').click();
        await expect(page.locator('#calibration-cancel')).toBeEnabled();
        await page.locator('#calibration-cancel').click();
        await expect(page.locator('#calibration-modal')).toBeHidden();
        expect(cancelled).toBe(true);
    });

    test('derived apply waits for confirmation and reports update conflicts', async ({ page }) => {
        await boot(page);
        await installHappyCalibration(page);
        let applyBody = null;
        await page.route('**/api/db/admin-token', route => route.fulfill({
            status: 200, contentType: 'application/json', body: JSON.stringify({ token: 'db-admin' }),
        }));
        await page.route('**/api/calibrations/job-1/apply', async route => {
            applyBody = JSON.parse(route.request().postData() || '{}');
            await route.fulfill({
                status: 400, contentType: 'application/json', body: JSON.stringify({ ok: false, error: 'Preset changed since Calibration; refresh before applying' }),
            });
        });
        await openEditor(page);
        await page.locator('#preset-modal-calibrate').click();
        await expect(page.locator('#calibration-start')).toBeEnabled();
        await page.locator('#calibration-start').click();
        await expect(page.locator('#calibration-apply')).toBeEnabled({ timeout: 5000 });
        await page.locator('#calibration-apply').click();
        await expect(page.locator('.app-confirm-overlay.active')).toBeVisible();
        expect(applyBody).toBe(null);
        await page.locator('.app-confirm-overlay.active .btn-modal-save').click();
        await expect(page.locator('#calibration-error')).toContainText('Preset changed since Calibration');
        expect(applyBody.create_derived).toBe(true);
    });

    test('successful apply exposes an explicit rollback action', async ({ page }) => {
        await boot(page);
        await installHappyCalibration(page);
        await page.route('**/api/db/admin-token', route => route.fulfill({
            status: 200, contentType: 'application/json', body: JSON.stringify({ token: 'db-admin' }),
        }));
        await page.route('**/api/calibrations/job-1/apply', route => route.fulfill({
            status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, apply: {
                preset_id: 'derived-1', derived: true, candidate_id: 'bounded-batch',
                before_fingerprint: 'sha256:before', after_fingerprint: 'sha256:after', validation: 'passed',
            } }),
        }));
        let rollbackBody = null;
        await page.route('**/api/calibrations/job-1/rollback', async route => {
            rollbackBody = JSON.parse(route.request().postData() || '{}');
            await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, rollback: {} }) });
        });
        await openEditor(page);
        await page.locator('#preset-modal-calibrate').click();
        await page.locator('#calibration-start').click();
        await expect(page.locator('#calibration-apply')).toBeEnabled({ timeout: 5000 });
        await page.locator('#calibration-apply').click();
        await page.locator('.app-confirm-overlay.active .btn-modal-save').click();
        await expect(page.locator('#calibration-rollback')).toBeVisible();
        await page.locator('#calibration-rollback').click();
        await page.locator('.app-confirm-overlay.active .btn-modal-save').click();
        await expect(page.locator('#calibration-rollback')).toBeDisabled();
        expect(rollbackBody).toEqual({ expected_target_fingerprint: 'sha256:after', exact_confirmation: 'ROLLBACK_CALIBRATION' });
    });

    test('Rapid-MLX presets never expose the llama.cpp Calibration action', async ({ page }) => {
        await boot(page);
        await page.evaluate(async (preset) => {
            const { sessionState } = await import('/js/core/app-state.js');
            sessionState.presets[0] = { ...sessionState.presets[0], ...preset };
        }, rapidPreset());
        await openEditor(page);
        await expect(page.locator('#preset-modal-calibrate')).toBeHidden();
    });
});
