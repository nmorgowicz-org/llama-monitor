// Scenario: launch Full config review (Phase 10 / G10).
// Captures the canonical requested + estimator-effective launch review.
import { loadAppDocument } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function (ctx) {
  const { page, baseUrl } = ctx;
  await page.setViewport({ width: 1280, height: 900, deviceScaleFactor: 1 });
  await loadAppDocument(page, baseUrl);
  await page.evaluate(async () => {
    const { openSpawnWizard, wizardState, selectWizardEngine, showStep } = await import('/js/features/spawn-wizard.js');
    openSpawnWizard();
    wizardState.model.source = 'local';
    wizardState.model.path = '/tmp/capture-fixtures/launch-full-config.gguf';
    wizardState.model.paramB = 8;
    wizardState.model.modelBytes = 4 * 1024 * 1024 * 1024;
    selectWizardEngine('llama_cpp', true);
    showStep(2);
  });
  await page.waitForSelector('#wizard-step-2.active', { timeout: 10000 });
  await sleep(500);
  await page.evaluate(() => document.getElementById('spawn-full-config-drawer')?.scrollIntoView({ behavior: 'instant', block: 'start' }));
  await captureShot(page, 'spawn-wizard-launch-full-config.png', {
    fullPage: true,
    runtimeTag: 'llamacpp-local',
    expandSelector: '.wizard-body',
  });
  await page.evaluate(() => {
    const step = document.getElementById('wizard-step-2');
    if (step) step.scrollTop = step.scrollHeight;
  });
  await sleep(250);
  await captureShot(page, 'spawn-wizard-launch-full-config-details.png', {
    fullPage: true,
    runtimeTag: 'llamacpp-local',
    expandSelector: '#wizard-step-2',
  });
}
