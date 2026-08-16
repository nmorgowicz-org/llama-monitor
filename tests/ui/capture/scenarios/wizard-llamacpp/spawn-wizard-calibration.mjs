// Phase 6: calibration evidence reuse in the Pro Spawn Wizard review.
// The route is mocked so capture never starts a real benchmark job.
import { loadAppDocument } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

function receiptMatch(matchKind = 'exact') {
  const warning = matchKind === 'exact'
    ? []
    : matchKind === 'compatible'
      ? ['Different llama.cpp runtime build; capability signature matched']
      : ['Different GGUF weight quantization; family and structural shape matched'];
  return {
    receipt: {
      job_id: 'phase6-fixture-job',
      selected_candidate: 'balanced-batch-1024',
      fingerprint: {
        model: { library_relative_id: 'gguf/qwen3.5-9b-fixture.gguf' },
      },
      candidate_results: [{
        candidate: {
          id: 'balanced-batch-1024',
          typed_patch: { context_size: 8192, batch_size: 1024, ubatch_size: 512, ctk: 'q8_0', ctv: 'q8_0' },
        },
      }],
    },
    match_kind: matchKind,
    warnings: warning,
  };
}

export default async function ({ page, baseUrl }) {
  await page.setViewport({ width: 1280, height: 900, deviceScaleFactor: 1 });
  await loadAppDocument(page, baseUrl);
  await page.evaluate(async (matches) => {
    window.__phase6Matches = matches;
    const originalFetch = window.fetch.bind(window);
    window.fetch = (url, options) => {
      if (String(url).includes('/api/calibrations/match')) {
        return Promise.resolve(new Response(JSON.stringify({ ok: true, matches: window.__phase6Matches }), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        }));
      }
      return originalFetch(url, options);
    };
    const { openSpawnWizard, wizardState, selectWizardEngine, showStep } = await import('/js/features/spawn-wizard.js');
    openSpawnWizard({ localPath: '/tmp/capture-fixtures/qwen3.5-9b-fixture.gguf' });
    wizardState.savedPresetId = 'phase6-fixture-preset';
    wizardState.viewMode = 'pro';
    selectWizardEngine('llama_cpp', true);
    const viewMode = document.getElementById('view-mode-select');
    if (viewMode) {
      viewMode.value = 'pro';
      viewMode.dispatchEvent(new Event('change', { bubbles: true }));
    }
    showStep(2);
  }, [receiptMatch('exact'), receiptMatch('compatible')]);
  await page.waitForSelector('#spawn-calibration-card:not([hidden])', { timeout: 10000 });
  await page.locator('#spawn-calibration-check-btn').click();
  await page.waitForSelector('.spawn-calibration-match', { timeout: 10000 });
  await sleep(250);
  // INTENT: Show exact and compatible saved-calibration evidence in the Pro review step.
  await captureShot(page, 'spawn-wizard-calibration-evidence.png', {
    fullPage: true,
    runtimeTag: 'llamacpp-local',
    expandSelector: '#wizard-step-2',
  });
  await page.locator('#spawn-calibration-apply-btn').click();
  await sleep(150);
  // INTENT: Show the calibration candidate applied to the wizard controls.
  await captureShot(page, 'spawn-wizard-calibration-applied.png', {
    fullPage: true,
    runtimeTag: 'llamacpp-local',
    expandSelector: '#wizard-step-2',
  });
  await page.evaluate(() => {
    window.__phase6Matches = [{
      ...window.__phase6Matches[1],
      match_kind: 'related',
      warnings: ['Different GGUF weight quantization; family and structural shape matched'],
    }];
  });
  await page.locator('#spawn-calibration-check-btn').click();
  await page.waitForSelector('.spawn-calibration-match', { timeout: 10000 });
  await sleep(250);
  // INTENT: Show related-model evidence as an explicit review-only choice.
  await captureShot(page, 'spawn-wizard-calibration-related-review.png', {
    fullPage: true,
    runtimeTag: 'llamacpp-local',
    expandSelector: '#wizard-step-2',
  });
}
