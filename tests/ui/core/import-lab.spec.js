import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

const exactSource = 'gguf/SmolLM2-135M-Instruct-Q8_0.gguf';

async function openImportLab(page, { initialJobs = [], requests = null, diskSufficient = true } = {}) {
  let jobs = [...initialJobs];
  page.on('dialog', dialog => {
    throw new Error(`Native dialog opened: ${dialog.type()} ${dialog.message()}`);
  });
  await page.route('**/api/hf/download-dir', route => route.fulfill({
    status: 200, contentType: 'application/json', body: JSON.stringify({ dir: '/models', configured: true }),
  }));
  await page.route('**/api/models', route => {
    if (requests) requests.models += 1;
    return route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
  });
  await page.route('**/api/llama-binary/platform-info', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ os: 'macos', arch: 'aarch64', rapid_mlx_local_available: true }),
  }));
  await page.route('**/api/models/import-lab/availability', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      local_execution_available: true,
      platform_requirement: 'Apple Silicon macOS',
      supported_profile: 'smollm2-135m-instruct-llama-v1',
      compatibility: 'experimental',
      launchable: false,
      fallback_engine: 'llama.cpp',
    }),
  }));
  await page.route('**/api/models/gguf/import/compatibility/preview', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      architecture: 'llama', tensor_count: 272, compatibility: 'experimental',
      missing_profile_fields: [], missing_assets: [], warnings: ['Experimental profile'],
      unsupported_reasons: [],
    }),
  }));
  await page.route('**/api/models/import-lab/resource-estimate', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      source_bytes: 146000000, estimated_fp16_bytes: 272000000,
      required_disk_bytes: 1080000000, available_disk_bytes: 5000000000,
      available_ram_bytes: 16000000000, disk_sufficient: diskSufficient, ram_guidance: 'comfortable',
    }),
  }));
  await page.route('**/api/models/import-lab/jobs**', async route => {
    const request = route.request();
    const url = new URL(request.url());
    if (request.method() === 'POST' && url.pathname.endsWith('/cancel')) {
      jobs = jobs.map(job => ({
        ...job, state: 'cancelled', phase: 'cancelled', progress_percent: 35,
        message: 'Recovery cancelled; staging files were removed', can_cancel: false,
      }));
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(jobs[0]) });
      return;
    }
    if (request.method() === 'POST') {
      jobs = [{
        id: 'job-1', state: 'recovering', phase: 'recovering_fp16', progress_percent: 35,
        message: 'Recovering tensors into an isolated non-launchable staging cache',
        can_cancel: true, diagnostics: ['Original GGUF will not be modified'],
      }];
      await route.fulfill({ status: 202, contentType: 'application/json', body: JSON.stringify(jobs[0]) });
      return;
    }
    if (request.method() === 'DELETE') {
      jobs = [];
      await route.fulfill({ status: 200, contentType: 'application/json', body: '{"ok":true}' });
      return;
    }
    await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(jobs) });
  });

  await page.goto('/');
  await page.waitForSelector('html.modules-ready');
  await dismissAuthShell(page);
  await page.evaluate(async () => {
    const { openModelsModal } = await import('/js/features/models.js');
    openModelsModal();
  });
  await page.getByRole('button', { name: /Import Lab/ }).click();
  await expect(page.locator('#mm-import-platform')).toContainText('Apple Silicon ready');
}

test.describe('experimental GGUF Import Lab', () => {
  test('analyzes, queues, cancels, and cleans a recovery without native dialogs', async ({ page }) => {
    await openImportLab(page);
    await page.locator('#mm-import-source').fill(exactSource);
    await page.locator('#mm-import-analyze').click();

    await expect(page.locator('.mm-import-verdict-badge')).toHaveText('experimental');
    await expect(page.locator('#mm-import-report')).toContainText('llama · 272 tensors');
    await expect(page.locator('#mm-import-report')).toContainText('Engine fallback');
    const recover = page.getByRole('button', { name: 'Recover experimental FP16' });
    await expect(recover).toBeEnabled();
    await recover.click();

    const progress = page.getByRole('progressbar', { name: /Recovery progress/ });
    await expect(progress).toHaveAttribute('aria-valuenow', '35');
    await expect(page.locator('.mm-import-job')).toContainText('Original GGUF will not be modified');
    await page.getByRole('button', { name: 'Cancel and clean staging' }).click();
    await expect(page.locator('.mm-import-job')).toContainText('staging files were removed');
    await page.getByRole('button', { name: 'Clear job' }).click();
    await expect(page.locator('#mm-import-jobs-list')).toContainText('No recovery jobs yet');
  });

  test('keeps recovery disabled when the report is unsupported', async ({ page }) => {
    await openImportLab(page);
    await page.route('**/api/models/gguf/import/compatibility/preview', route => route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        architecture: 'qwen3', tensor_count: 100, compatibility: 'unsupported',
        missing_profile_fields: [], missing_assets: [], warnings: [],
        unsupported_reasons: ['No validated recovery profile'],
      }),
    }));
    await page.locator('#mm-import-source').fill('gguf/finetune.gguf');
    await page.locator('#mm-import-analyze').click();
    await expect(page.locator('.mm-import-verdict-badge')).toHaveText('unsupported');
    await expect(page.getByRole('button', { name: 'Recover experimental FP16' })).toBeDisabled();
    await expect(page.locator('#mm-import-engine-note')).toContainText('Recommended engine: llama.cpp');
  });

  test('treats unknown disk headroom as unknown and keeps recovery disabled', async ({ page }) => {
    await openImportLab(page, { diskSufficient: null });
    await page.locator('#mm-import-source').fill(exactSource);
    await page.locator('#mm-import-analyze').click();
    await expect(page.locator('#mm-import-report')).toContainText('Unknown');
    await expect(page.getByRole('button', { name: 'Recover experimental FP16' })).toBeDisabled();
  });

  test('refetches inventory when a completed recovery returns to Library', async ({ page }) => {
    const requests = { models: 0 };
    await openImportLab(page, {
      requests,
      initialJobs: [{
        id: 'complete-job', state: 'complete', phase: 'complete', progress_percent: 100,
        message: 'Experimental recovery completed; runtime launch remains disabled',
        can_cancel: false, diagnostics: [],
      }],
    });
    expect(requests.models).toBe(1);
    await page.getByRole('button', { name: /^Library/ }).click();
    await expect.poll(() => requests.models).toBe(2);
  });
});
