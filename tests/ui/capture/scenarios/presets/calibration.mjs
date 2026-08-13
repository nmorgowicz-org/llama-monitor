import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);

    // Only the benchmark lifecycle is stubbed. The preflight request reaches the
    // real local backend, validating the actual model-library and managed-tool setup.
    await page.setRequestInterception(true);
    let pollCount = 0;
    page.on('request', request => {
        const url = new URL(request.url());
        if (url.pathname === '/api/calibrations' && request.method() === 'POST') {
            request.respond({ status: 202, contentType: 'application/json', body: JSON.stringify({
                ok: true, job: { id: 'capture-job', state: 'queued', planned_trials: 2, completed_trials: 0 },
            }) });
            return;
        }
        if (url.pathname === '/api/calibrations/capture-job' && request.method() === 'GET') {
            pollCount += 1;
            request.respond({ status: 200, contentType: 'application/json', body: JSON.stringify({
                ok: true,
                job: pollCount === 1
                    ? { id: 'capture-job', state: 'running', phase: 'benchmarking', planned_trials: 2, completed_trials: 1 }
                    : { id: 'capture-job', state: 'complete', phase: 'complete', planned_trials: 2, completed_trials: 2 },
            }) });
            return;
        }
        if (url.pathname === '/api/calibrations/capture-job/receipt' && request.method() === 'GET') {
            request.respond({ status: 200, contentType: 'application/json', body: JSON.stringify({
                ok: true,
                receipt: {
                    selected_candidate: 'bounded-batch',
                    candidate_results: [
                        { candidate: { id: 'baseline' }, measurement: { status: 'ok', tg_tps_samples: [12.1] } },
                        { candidate: { id: 'bounded-batch' }, measurement: { status: 'ok', tg_tps_samples: [13.4] } },
                    ],
                },
            }) });
            return;
        }
        if (url.pathname === '/api/db/admin-token') {
            request.respond({ status: 200, contentType: 'application/json', body: JSON.stringify({ token: 'capture-admin-token' }) });
            return;
        }
        if (url.pathname === '/api/calibrations/capture-job/apply') {
            request.respond({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, apply: {
                preset_id: 'derived-capture', derived: true, candidate_id: 'bounded-batch',
                before_fingerprint: 'sha256:before', after_fingerprint: 'sha256:after', validation: 'passed',
            } }) });
            return;
        }
        if (url.pathname === '/api/calibrations/capture-job/rollback') {
            request.respond({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, rollback: {} }) });
            return;
        }
        request.continue();
    });

    await page.waitForSelector('#preset-select option', { timeout: 10000 });
    await page.select('#preset-select', 'calibration-capture-source');
    await sleep(300);
    await page.evaluate(async () => {
        const { openPresetModal } = await import('/js/features/presets.js');
        openPresetModal('edit');
    });
    console.log('[CAPTURE] preset modal state:', await page.evaluate(() => ({
        presetId: document.getElementById('modal-preset-id')?.value,
        buttonDisplay: document.getElementById('preset-modal-calibrate')?.style.display,
        presetModal: document.getElementById('preset-modal')?.className,
    })));
    await page.waitForSelector('#preset-modal-calibrate', { visible: true });
    await page.evaluate(async () => {
        const { initCalibrationUi } = await import('/js/features/calibration.js');
        initCalibrationUi();
        document.getElementById('preset-modal-calibrate')?.click();
    });
    await sleep(500);
    console.log('[CAPTURE] calibration modal state:', await page.$eval('#calibration-modal', el => ({
        className: el.className,
        ariaHidden: el.getAttribute('aria-hidden'),
        status: document.getElementById('calibration-status')?.textContent,
        error: document.getElementById('calibration-error')?.textContent,
    })));
    await page.waitForSelector('#calibration-modal.open', { visible: true });
    await page.waitForFunction(() => document.getElementById('calibration-start')?.disabled === false, { timeout: 15000 });
    await page.evaluate(() => {
        document.querySelectorAll('#toast-container [data-toast-close]').forEach(button => button.click());
    });
    await captureShot(page, 'calibration-preflight.png', { fullPage: true });

    await page.click('#calibration-start');
    await sleep(250);
    await captureShot(page, 'calibration-running.png', { fullPage: true });
    await page.waitForSelector('#calibration-apply:not([disabled])', { timeout: 10000 });
    await captureShot(page, 'calibration-results.png', { fullPage: true });

    await page.click('#calibration-apply');
    await page.waitForSelector('.app-confirm-overlay.active', { visible: true });
    await captureShot(page, 'calibration-apply-confirmation.png', { fullPage: true });
    await page.click('.app-confirm-overlay.active .btn-modal-save');
    await page.waitForSelector('#calibration-rollback:not([hidden])', { visible: true });
    await captureShot(page, 'calibration-applied.png', { fullPage: true });
    await page.click('#calibration-rollback');
    await page.waitForSelector('.app-confirm-overlay.active', { visible: true });
    await captureShot(page, 'calibration-rollback-confirmation.png', { fullPage: true });
    await page.click('.app-confirm-overlay.active .btn-modal-cancel');
}
