// Rapid-MLX Pro parity contract (plan §16 / G9): backend-native categories,
// effective-state evidence, access controls, modified filtering, and narrow
// presentation. The scenario uses the same canonical controls as Guided.
import { loadAppDocument } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

async function openRapidPro(page, baseUrl) {
  await page.setViewport({ width: 1280, height: 900, deviceScaleFactor: 1 });
  await loadAppDocument(page, baseUrl);
  await page.evaluate(async () => {
    sessionStorage.clear();
    const {
      openSpawnWizard,
      wizardState,
      selectWizardEngine,
      showStep,
    } = await import('/js/features/spawn-wizard.js');
    openSpawnWizard();
    wizardState.model.source = 'local';
    wizardState.model.path = '/tmp/capture-fixtures/rapid-pro-model';
    wizardState.model.paramB = 7;
    wizardState.model.modelBytes = 4 * 1024 * 1024 * 1024;
    selectWizardEngine('rapid_mlx', true);
    showStep(1);
    const select = document.getElementById('view-mode-select');
    select.value = 'pro';
    select.dispatchEvent(new Event('change', { bubbles: true }));
  });
  await page.waitForSelector('#spawn-wizard-overlay.open #pro-layout', { timeout: 10000 });
  await page.waitForFunction(
    () => document.querySelectorAll('#pro-rail-nav .pro-rail-item').length === 7,
    { timeout: 5000 },
  );
  await sleep(250);
}

export default async function ({ page, baseUrl }) {
  await openRapidPro(page, baseUrl);

 // INTENT: Rapid Pro shell shows backend-native categories and active pane.
 await captureShot(page, 'spawn-wizard-rapid-pro-shell.png', {
    fullPage: true,
    runtimeTag: 'rapidmlx-local',
    expandSelector: '.wizard-body',
  });

  const categoryShots = [
    [0, 'spawn-wizard-rapid-pro-rail-model.png'],
    [1, 'spawn-wizard-rapid-pro-rail-memory.png'],
    [2, 'spawn-wizard-rapid-pro-rail-performance.png'],
    [3, 'spawn-wizard-rapid-pro-rail-generation.png'],
    [4, 'spawn-wizard-rapid-pro-rail-tools.png'],
    [5, 'spawn-wizard-rapid-pro-rail-network.png'],
    [6, 'spawn-wizard-rapid-pro-rail-advanced.png'],
  ];
  for (const [index, name] of categoryShots) {
    await page.evaluate((i) => {
      document.querySelectorAll('#pro-rail-nav .pro-rail-item')[i]?.click();
      const category = document.querySelectorAll('#pro-rail-nav .pro-rail-item')[i]?.dataset.category;
      const host = document.getElementById('pro-controls-host');
      const target = category && host?.querySelector(`[data-pro-category="${CSS.escape(category)}"]`);
      if (host && target) host.scrollTop = Math.max(0, target.offsetTop - 8);
    }, index);
    await sleep(250);
    await captureShot(page, name, {
      fullPage: true,
      runtimeTag: 'rapidmlx-local',
      expandSelector: '.wizard-body',
    });
  }

  await page.evaluate(() => {
    const retained = document.getElementById('spawn-retained-cache-mib');
    if (retained) {
      retained.value = '16384';
      retained.dispatchEvent(new Event('change', { bubbles: true }));
    }
  });
  await sleep(150);
 // INTENT: Retained-cache edit remains visible in Rapid Pro.
 await captureShot(page, 'spawn-wizard-rapid-pro-retained-cache-edit.png', {
    fullPage: true,
    runtimeTag: 'rapidmlx-local',
    expandSelector: '.wizard-body',
  });

  await page.evaluate(() => {
    const filter = document.getElementById('pro-filter-input');
    if (filter) {
      filter.value = 'cache';
      filter.dispatchEvent(new Event('input', { bubbles: true }));
    }
  });
  await sleep(150);
 // INTENT: Rapid Pro search filters the retained-cache setting.
 await captureShot(page, 'spawn-wizard-rapid-pro-search-cache.png', {
    fullPage: true,
    runtimeTag: 'rapidmlx-local',
    expandSelector: '.wizard-body',
  });

  await page.evaluate(() => {
    const filter = document.getElementById('pro-filter-input');
    if (filter) {
      filter.value = '';
      filter.dispatchEvent(new Event('input', { bubbles: true }));
    }
    document.querySelector('.pro-modified-only')?.click();
  });
  await sleep(150);
 // INTENT: Rapid modified-only view shows the edited canonical setting.
 await captureShot(page, 'spawn-wizard-rapid-pro-modified-only.png', {
    fullPage: true,
    runtimeTag: 'rapidmlx-local',
    expandSelector: '.wizard-body',
  });

  await page.evaluate(() => {
    document.getElementById('pro-modified-only')?.click();
    document.getElementById('spawn-rapid-speculative-enabled')?.click();
  });
  await sleep(200);
 // INTENT: Rapid companion gating is explicit for unavailable capability.
 await captureShot(page, 'spawn-wizard-rapid-pro-companion-gated.png', {
    fullPage: true,
    runtimeTag: 'rapidmlx-local',
    expandSelector: '.wizard-body',
  });

  await page.setViewport({ width: 430, height: 900, deviceScaleFactor: 1 });
  await page.evaluate(() => document.getElementById('pro-layout')?.scrollIntoView({ behavior: 'instant', block: 'start' }));
  await sleep(200);
 // INTENT: Narrow Rapid Pro layout remains usable without clipping.
 await captureShot(page, 'spawn-wizard-rapid-pro-narrow.png', {
    fullPage: true,
    runtimeTag: 'rapidmlx-local',
    expandSelector: '.wizard-body',
  });
}
