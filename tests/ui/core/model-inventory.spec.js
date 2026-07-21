import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

const inventory = [
  {
    model_name: 'Local GGUF', filename: 'local.gguf', path: '/models/gguf/local.gguf',
    format: 'gguf', source: 'local', lifecycle: 'ready', compatibility: 'verified',
    supported_backends: ['llama_cpp'], size_display: '4.2 GiB', quant_type: 'Q4_K_M',
  },
  {
    model_name: 'HF MLX', filename: 'mlx-community/model', path: '/models/cache/hf/model',
    model_source: { kind: 'hugging_face_repo', repo_id: 'mlx-community/model', revision: 'main' },
    format: 'mlx', source: 'hugging_face', lifecycle: 'ready', compatibility: 'provisional',
    supported_backends: ['rapid_mlx'], size_display: '7.1 GiB',
  },
  {
    model_name: 'Official conversion', filename: 'converted-model', path: '/models/mlx/converted/model',
    format: 'mlx', source: 'official_conversion', lifecycle: 'converting', compatibility: 'verified',
    supported_backends: ['rapid_mlx'],
  },
  {
    model_name: 'Recovered FP16 (Experimental)', filename: 'fp16',
    path: '/models/rapid-mlx/imports/cache/fp16', quant_type: 'F16 recovered',
    format: 'mlx', source: 'recovered_gguf', lifecycle: 'ready', compatibility: 'experimental',
    supported_backends: [], provenance: { lineage_kind: 'gguf_recovered_fp16' },
  },
  {
    model_name: 'Re-quantized MLX (Experimental)', filename: 'model',
    path: '/models/rapid-mlx/requantized/cache/model', quant_type: 'affine_8bit_g64',
    format: 'mlx', source: 'requantized_mlx', lifecycle: 'ready', compatibility: 'experimental',
    supported_backends: [], provenance: { lineage_kind: 'mlx_requantized' },
  },
  {
    model_name: 'Legacy staged item', filename: 'legacy.part', path: '/models/.staging/legacy.part',
    format: 'unknown', source: 'legacy', lifecycle: 'incomplete', compatibility: 'unknown',
    supported_backends: [], legacy_location: true,
  },
  {
    model_name: 'Invalid model', filename: 'invalid-model', path: '/models/transformers/invalid',
    format: 'transformers', source: 'local', lifecycle: 'invalid', compatibility: 'unsupported',
    supported_backends: [],
  },
  {
    model_name: 'Vision projector', filename: 'vision.mmproj.gguf', path: '/models/gguf/vision.mmproj.gguf',
    format: 'gguf', source: 'local', lifecycle: 'ready', compatibility: 'verified',
    supported_backends: ['llama_cpp'], companion_kind: 'mmproj',
  },
  {
    model_name: '<img src=x onerror=alert(1)>', filename: 'draft.gguf', path: '/models/gguf/draft.gguf',
    format: 'gguf', source: 'local', lifecycle: 'ready', compatibility: 'verified',
    supported_backends: ['llama_cpp'], companion_kind: 'draft', is_draft_assistant: true,
  },
];

async function installInventoryMocks(page, { rapidMlxAvailable = true, requests = null } = {}) {
  await page.route('**/api/hf/download-dir', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ dir: '/models', configured: true }),
  }));
  await page.route('**/api/models', route => {
    if (requests) requests.models += 1;
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(inventory),
    });
  });
  await page.route('**/api/llama-binary/platform-info', route => {
    if (requests) requests.platform += 1;
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        os: rapidMlxAvailable ? 'macos' : 'linux',
        arch: rapidMlxAvailable ? 'aarch64' : 'x86_64',
        rapid_mlx_local_available: rapidMlxAvailable,
        rapid_mlx_local_requirement: 'Rapid-MLX local execution requires macOS on Apple Silicon',
      }),
    });
  });
}

async function openInventory(page, options) {
  await page.addInitScript(() => {
    localStorage.setItem('llama-monitor-models-prefs', JSON.stringify({
      viewMode: 'cards',
      showMmproj: true,
      showMain: true,
      showSplit: true,
      showDraftModels: true,
    }));
  });
  await installInventoryMocks(page, options);
  await page.goto('/');
  await page.waitForSelector('html.modules-ready');
  await dismissAuthShell(page);
  await page.evaluate(async () => {
    const { openModelsModal } = await import('/js/features/models.js');
    openModelsModal();
  });
  await expect(page.locator('#models-modal')).toHaveClass(/open/);
  await expect(page.locator('.mm-model-card')).toHaveCount(inventory.length);
}

test.describe('typed model inventory', () => {
  test('@fake-data-bypass renders every inventory dimension and companion as accessible first-class badges', async ({ page }) => {
    await openInventory(page);

    for (const label of [
      'Format: GGUF', 'Format: MLX', 'Format: Transformers / safetensors', 'Format: Unknown',
      'Source: Local library', 'Source: Hugging Face', 'Source: Official MLX conversion',
      'Source: Experimental GGUF recovery', 'Source: Experimental MLX re-quantization',
      'Source: Legacy model-library location',
      'Lifecycle: Ready', 'Lifecycle: Incomplete', 'Lifecycle: Converting', 'Lifecycle: Invalid',
      'Compatibility: Verified', 'Compatibility: Experimental and not launchable',
      'Compatibility: Provisional', 'Compatibility: Unsupported',
      'Compatibility: Unknown', 'Supported backend: llama.cpp', 'Supported backend: Rapid-MLX',
      'Supported backend: None', 'Companion type: Multimodal projector',
      'Companion type: Draft / MTP model',
    ]) {
      await expect(page.getByLabel(label).first()).toBeVisible();
    }

    const cards = page.locator('.mm-model-card');
    await expect(cards.filter({ hasText: 'Official conversion' }).getByRole('button', { name: /wizard|configure|load/i })).toHaveCount(0);
    await expect(cards.filter({ hasText: 'Recovered FP16' }).getByRole('button', { name: /wizard|configure|load/i })).toHaveCount(0);
    await expect(cards.filter({ hasText: 'Re-quantized MLX' }).getByRole('button', { name: /wizard|configure|load/i })).toHaveCount(0);
    await expect(cards.filter({ hasText: 'Vision projector' }).getByText(/Vision companion —/)).toBeVisible();
    await expect(cards.filter({ hasText: 'draft.gguf' }).getByText(/Draft companion —/)).toBeVisible();
    await expect(page.locator('.mm-card-name img')).toHaveCount(0);
    await expect(page.getByText('<img src=x onerror=alert(1)>', { exact: true })).toBeVisible();

    await expect(cards.filter({ hasText: 'HF MLX' }).getByRole('button', { name: 'Delete this model from library' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Delete this model from library' })).toHaveCount(3);
  });

  test('@fake-data-bypass search sort filter and view changes reuse inventory and platform state', async ({ page }) => {
    const requests = { models: 0, platform: 0 };
    await openInventory(page, { requests });
    expect(requests).toEqual({ models: 1, platform: 1 });

    await page.getByLabel('Search models').fill('GGUF');
    await page.getByLabel('Search models').press('Enter');
    await page.locator('#mm-lib-sort-select').selectOption('name-desc');
    await page.locator('#mm-lib-view-toggle').click();
    await page.locator('#mm-lib-filters-toggle').click();
    await page.locator('#mm-lib-filters-panel .mm-lib-chip').first().click();

    await expect(page.locator('#models-list')).toBeVisible();
    expect(requests).toEqual({ models: 1, platform: 1 });
  });

  test('@fake-data-bypass creates a typed Rapid-MLX preset from a ready inventory entry', async ({ page }) => {
    let savedPreset = null;
    await page.route('**/api/presets', async route => {
      if (route.request().method() === 'POST') {
        savedPreset = route.request().postDataJSON();
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ id: 'rapid-from-inventory' }),
        });
        return;
      }
      await route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
    });
    await openInventory(page);

    const rapidCard = page.locator('.mm-model-card').filter({ hasText: 'HF MLX' });
    await rapidCard.getByRole('button', { name: 'Configure Rapid-MLX' }).click();

    const editor = page.locator('#preset-modal');
    await expect(editor).toHaveClass(/open/);
    await expect(editor).toHaveClass(/preset-editor--rapid-mlx/);
    await expect(page.locator('#modal-name')).toHaveValue('HF MLX · Rapid-MLX');
    await expect(page.locator('#modal-model-path')).toHaveValue('mlx-community/model');
    await expect(page.locator('#modal-port')).toHaveValue('8000');

    await page.locator('#btn-modal-save').click();
    await expect.poll(() => savedPreset).not.toBeNull();
    expect(savedPreset).toMatchObject({
      backend: 'rapid_mlx',
      name: 'HF MLX · Rapid-MLX',
      port: 8000,
      rapid_mlx: {
        model_path: 'mlx-community/model',
        model_source: {
          kind: 'hugging_face_repo',
          repo_id: 'mlx-community/model',
          revision: 'main',
        },
        host: '127.0.0.1',
        port: 8000,
        log_level: 'INFO',
      },
    });
    expect(savedPreset.model_path).toBe('');
  });

  test('@fake-data-bypass copies the stable repo id for a typed Hugging Face source', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await openInventory(page);

    const rapidCard = page.locator('.mm-model-card').filter({ hasText: 'HF MLX' });
    await rapidCard.getByRole('button', { name: 'Copy Path' }).click();
    await expect.poll(() => page.evaluate(() => navigator.clipboard.readText()))
      .toBe('mlx-community/model');
  });

  test('@fake-data-bypass gates local Rapid-MLX actions off Apple Silicon without hiding model inventory', async ({ page }) => {
    await openInventory(page, { rapidMlxAvailable: false });

    const rapidCard = page.locator('.mm-model-card').filter({ hasText: 'HF MLX' });
    await expect(rapidCard.getByLabel('Rapid-MLX local execution requires macOS on Apple Silicon')).toBeVisible();
    await expect(rapidCard.getByRole('button', { name: 'Configure Rapid-MLX' })).toHaveCount(0);
    await expect(rapidCard.getByText(/You can still manage or copy this model/)).toBeVisible();
    await expect(rapidCard.getByRole('button', { name: 'Copy Path' })).toBeVisible();
  });

  test('@fake-data-bypass remains coherent in light, narrow, and reduced-motion presentation', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.setViewportSize({ width: 390, height: 844 });
    await openInventory(page);
    await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });

    const overflow = await page.locator('#models-list').evaluate(element => {
      const right = element.getBoundingClientRect().right;
      return Math.max(0, ...[...element.children]
        .map(child => child.getBoundingClientRect().right - right));
    });
    expect(overflow).toBeLessThanOrEqual(1);
    const firstCard = page.locator('.mm-model-card').first();
    expect(await firstCard.locator('.mm-inventory-badge').count()).toBeGreaterThanOrEqual(5);
    const transitionSeconds = await firstCard.evaluate(element =>
      parseFloat(getComputedStyle(element).transitionDuration)
    );
    expect(transitionSeconds).toBeLessThanOrEqual(0.00001);
  });
});
