import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

function preset(id, name, modelPath = `/models/${id}.gguf`) {
  return {
    id,
    name,
    model_path: modelPath,
    hf_repo: null,
    context_size: 8192,
    ctk: 'q8_0',
    ctv: 'q8_0',
    batch_size: 512,
    ubatch_size: 512,
    parallel_slots: 1,
    port: 8001,
    bind_host: '127.0.0.1',
  };
}

async function installPresetMocks(page, options = {}) {
  const state = {
    presets: [...(options.presets || [preset('original', 'Original'), preset('other', 'Other')])],
    active: options.active || { status: 'Stopped', preset_id: '' },
    postCount: 0,
    putCount: 0,
    spawnPayloads: [],
  };

  await page.route('**/api/settings', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ preset_id: options.savedPresetId || state.presets[0]?.id || '' }),
  }));

  await page.route('**/api/sessions/active/readiness', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: true, ready: true }),
  }));

  await page.route('**/api/sessions/active', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify(state.active),
  }));

  await page.route('**/api/db/admin-token', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ token: 'admin-token' }),
  }));

  await page.route('**/api/kill-llama', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: true }),
  }));

  await page.route('**/api/sessions/spawn', async route => {
    state.spawnPayloads.push(route.request().postDataJSON());
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ ok: true, session_id: 'spawned-session' }),
    });
  });

  await page.route('**/api/presets**', async route => {
    const request = route.request();
    const url = new URL(request.url());
    const method = request.method();
    const parts = url.pathname.split('/').filter(Boolean);
    const id = parts.length === 3 ? decodeURIComponent(parts[2]) : '';

    if (url.pathname === '/api/presets' && method === 'GET') {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(state.presets),
      });
      return;
    }

    if (url.pathname === '/api/presets' && method === 'POST') {
      state.postCount += 1;
      const body = request.postDataJSON();
      const created = { ...body, id: body.id || (body.name === 'Wizard preset' ? 'wizard-id' : `copy-${state.postCount}`) };
      state.presets.push(created);
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ ok: true, preset: created }),
      });
      return;
    }

    if (id && method === 'GET') {
      const found = state.presets.find(p => p.id === id);
      await route.fulfill({
        status: found ? 200 : 404,
        contentType: 'application/json',
        body: JSON.stringify(found ? { ok: true, preset: found } : { ok: false, error: 'preset not found' }),
      });
      return;
    }

    if (id && method === 'PUT') {
      state.putCount += 1;
      const body = { ...request.postDataJSON(), id };
      const index = state.presets.findIndex(p => p.id === id);
      if (index >= 0) state.presets[index] = body;
      else state.presets.push(body);
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ ok: true, preset: body }),
      });
      return;
    }

    await route.continue();
  });

  return state;
}

async function boot(page) {
  await page.goto('/');
  await page.waitForSelector('html.modules-ready');
  await dismissAuthShell(page);
  await expect(page.locator('#preset-select option')).not.toHaveCount(0);
}

test.describe('preset flow', () => {
  test('duplicating from the preset editor selects and reopens the copy', async ({ page }) => {
    await installPresetMocks(page, {
      presets: [preset('original', 'Original'), preset('other', 'Other')],
      savedPresetId: 'original',
    });
    await boot(page);

    await page.evaluate(async () => {
      const { openPresetModal } = await import('/js/features/presets.js');
      openPresetModal('edit');
    });
    await expect(page.locator('#modal-name')).toHaveValue('Original');

    await page.locator('#preset-modal-duplicate').click();
    await expect(page.locator('#modal-name')).toHaveValue('Original (copy)');
    await expect(page.locator('#preset-select')).toHaveValue('copy-1');
    await expect(page.locator('#setup-preset-select')).toHaveValue('copy-1');

    await page.locator('#preset-modal-close').click();
    await page.evaluate(async () => {
      const { openPresetModal } = await import('/js/features/presets.js');
      openPresetModal('edit');
    });
    await expect(page.locator('#modal-name')).toHaveValue('Original (copy)');
  });

  test('selecting a different preset while running prompts and spawns the selected preset', async ({ page }) => {
    const state = await installPresetMocks(page, {
      presets: [preset('original', 'Original'), preset('other', 'Other')],
      active: { status: 'Running', preset_id: 'original' },
      savedPresetId: 'original',
    });
    page.on('dialog', dialog => dialog.accept());
    await boot(page);

    await page.locator('#preset-select').selectOption('other');

    await expect.poll(() => state.spawnPayloads.length).toBe(1);
    expect(state.spawnPayloads[0].preset_id).toBe('other');
  });

  test('spawn wizard save records the created preset id and updates on the second save', async ({ page }) => {
    const state = await installPresetMocks(page, {
      presets: [preset('original', 'Original')],
      savedPresetId: 'original',
    });
    await boot(page);

    await page.evaluate(async () => {
      const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
      openSpawnWizard({ localPath: '/models/wizard.gguf' });
      const name = document.getElementById('spawn-preset-name-input');
      name.value = 'Wizard preset';
      name.dispatchEvent(new Event('input', { bubbles: true }));
      document.getElementById('spawn-save-preset-btn').dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    });
    await expect.poll(() => state.postCount).toBe(1);
    await expect(page.locator('#spawn-save-preset-btn')).toHaveText('Save as Preset');

    await page.evaluate(() => {
      document.getElementById('spawn-save-preset-btn').dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    });
    await expect.poll(() => state.putCount).toBe(1);
    expect(state.presets.filter(p => p.name === 'Wizard preset')).toHaveLength(1);
    expect(state.presets.find(p => p.name === 'Wizard preset')?.id).toBe('wizard-id');
  });
});
