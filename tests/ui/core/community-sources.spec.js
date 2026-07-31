import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

const roles = [
  { id: 'original_author', label: 'Original author', description: 'Created the original weights.' },
  { id: 'gguf_quantizer', label: 'GGUF quantizer', description: 'Produced GGUF weights.' },
  { id: 'mlx_converter', label: 'MLX converter', description: 'Produced native MLX weights.' },
  { id: 'curator', label: 'Curator', description: 'Organizes model collections.' },
];

function initialCatalog() {
  return {
    version: 1,
    entries: [
      {
        username: 'bartowski', displayName: 'bartowski', description: 'Reliable GGUF releases.',
        role: 'gguf_quantizer', alsoKnownFor: [], categories: [], bundled: true,
      },
      {
        username: 'local-curator', displayName: 'Local Curator', description: 'My trusted collection.',
        role: 'curator', alsoKnownFor: [], categories: ['coding'], note: 'Personal choice', bundled: false,
      },
    ],
    preferences: { preferHeretic: false, trustedSources: [] },
  };
}

async function openSources(page, requests) {
  await page.route('**/api/hf/download-dir', route => route.fulfill({ json: { dir: '/models', configured: true } }));
  await page.route('**/api/models', route => route.fulfill({ json: [] }));
  await page.route('**/api/llama-binary/platform-info', route => route.fulfill({
    json: { os: 'linux', arch: 'x86_64', rapid_mlx_local_available: false },
  }));
  await page.route('**/api/hf/community-sources**', async route => {
    const request = route.request();
    const url = new URL(request.url());
    const method = request.method();
    requests.push({ method, pathname: url.pathname, search: url.search, body: request.postDataJSON?.() });
    if (method === 'GET' && url.pathname.endsWith('/community-sources')) {
      return route.fulfill({ json: { ok: true, catalog: initialCatalog(), roles } });
    }
    if (method === 'PUT') return route.fulfill({ json: { ok: true, catalog: request.postDataJSON() } });
    if (method === 'POST' && url.pathname.endsWith('/entry')) return route.fulfill({ json: { ok: true, entry: request.postDataJSON() } });
    if (method === 'DELETE') return route.fulfill({ json: { ok: true, removed: true } });
    if (method === 'POST' && url.pathname.endsWith('/reset')) return route.fulfill({ json: { ok: true, catalog: initialCatalog() } });
    return route.fulfill({ status: 404, json: { ok: false, error: 'not mocked' } });
  });

  await page.goto('/');
  await page.waitForSelector('html.modules-ready');
  await dismissAuthShell(page);
  await page.evaluate(async () => {
    const { openModelsModal } = await import('/js/features/models.js');
    openModelsModal();
  });
  await page.locator('.mm-tab[data-tab="sources"]').click();
  await expect(page.locator('.mm-source-card')).toHaveCount(2);
}

test.describe('Model Manager community sources', () => {
  test('@fake-data-bypass renders server roles and preserves bundled state with full-catalog PUT edits', async ({ page }) => {
    const requests = [];
    await openSources(page, requests);

    const bundled = page.locator('.mm-source-card').filter({ hasText: 'bartowski' });
    await expect(bundled.getByText('GGUF quantizer')).toBeVisible();
    await expect(bundled.getByText('Bundled')).toBeVisible();
    await expect(bundled.getByRole('button', { name: /remove/i })).toHaveCount(0);

    await bundled.getByRole('button', { name: 'Edit bartowski' }).click();
    await expect(page.locator('#mm-source-role option')).toHaveText(roles.map(role => role.label));
    await expect(page.locator('#mm-source-role-help')).toHaveText('Produced GGUF weights.');
    await page.locator('#mm-source-description').fill('Updated reliable GGUF releases.');
    await page.getByRole('button', { name: 'Save source' }).click();
    await expect(page.getByText('Community source updated')).toBeVisible();

    const put = requests.find(request => request.method === 'PUT');
    expect(put.pathname).toBe('/api/hf/community-sources');
    expect(put.body.entries).toHaveLength(2);
    expect(put.body.entries[0]).toMatchObject({
      username: 'bartowski', bundled: true, description: 'Updated reliable GGUF releases.',
    });
    expect(put.body.entries[1]).toMatchObject({ username: 'local-curator', bundled: false });
    expect(put.body.preferences).toEqual({ preferHeretic: false, trustedSources: [] });
  });

  test('@fake-data-bypass supports add/remove and requires explicit confirmation before reset', async ({ page }) => {
    const requests = [];
    await openSources(page, requests);

    await page.getByRole('button', { name: 'Add source' }).click();
    await page.locator('#mm-source-username').fill('mlx-helper');
    await page.locator('#mm-source-display-name').fill('MLX Helper');
    await page.locator('#mm-source-role').selectOption('mlx_converter');
    await expect(page.locator('#mm-source-role-help')).toHaveText('Produced native MLX weights.');
    await page.locator('#mm-source-description').fill('Native conversions.');
    await page.locator('#mm-source-categories').fill('apple-silicon, conversion');
    await page.getByRole('button', { name: 'Save source' }).click();
    const post = requests.find(request => request.method === 'POST' && request.pathname.endsWith('/entry'));
    expect(post.body).toMatchObject({
      username: 'mlx-helper', role: 'mlx_converter', categories: ['apple-silicon', 'conversion'], bundled: false,
    });

    await page.locator('.mm-source-card').filter({ hasText: 'Local Curator' }).getByRole('button', { name: /remove/i }).click();
    await expect(page.getByText('Remove community source?')).toBeVisible();
    await page.getByRole('button', { name: 'Confirm' }).click();
    const deletion = requests.find(request => request.method === 'DELETE');
    expect(deletion.search).toContain('username=local-curator');
    expect(deletion.search).toContain('role=curator');

    await page.getByRole('button', { name: 'Restore bundled sources' }).click();
    await expect(page.getByText('Restore bundled sources?')).toBeVisible();
    expect(requests.filter(request => request.pathname.endsWith('/reset'))).toHaveLength(0);
    await page.getByRole('button', { name: 'Cancel' }).click();
    expect(requests.filter(request => request.pathname.endsWith('/reset'))).toHaveLength(0);
    await page.getByRole('button', { name: 'Restore bundled sources' }).click();
    await page.getByRole('button', { name: 'Confirm' }).click();
    await expect.poll(() => requests.filter(request => request.pathname.endsWith('/reset')).length).toBe(1);
  });

  test('@fake-data-bypass reports HTTP-success application errors instead of claiming a save', async ({ page }) => {
    const requests = [];
    await openSources(page, requests);
    await page.route('**/api/hf/community-sources', async route => {
      if (route.request().method() === 'PUT') {
        return route.fulfill({ status: 200, json: { ok: false, error: 'catalog rejected' } });
      }
      return route.fallback();
    });
    await page.locator('.mm-source-card').filter({ hasText: 'bartowski' }).getByRole('button', { name: /edit/i }).click();
    await page.locator('#mm-source-description').fill('Should fail');
    await page.getByRole('button', { name: 'Save source' }).click();
    await expect(page.locator('#mm-sources-status')).toHaveText('catalog rejected');
    await expect(page.getByText('Community source updated')).toHaveCount(0);
  });
});
