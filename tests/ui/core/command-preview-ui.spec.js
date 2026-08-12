import { test, expect } from '@playwright/test';

// The command-preview endpoint shipped with no caller at all: it demanded an
// `executable_path` that no frontend surface could obtain, and the only spec covering it
// was @runtime-required, so it never ran. These tests exercise the browser side against a
// stubbed endpoint, so they fail if the step-6 card stops calling it.
test.describe('Rapid-MLX launch command preview (step 6)', () => {
  const STUB = {
    argv: ['serve', 'mlx-community/Qwen3-8B-4bit', '--host', '127.0.0.1', '--port', '8001'],
    redacted: true,
    requested_vs_effective: { turboquant_mode: 'requested k8v4, running none' },
    reasons: ['Installed runtime does not support --kv-cache-turboquant.'],
  };

  async function openConfigStep(page, body, status = 200) {
    await page.route('**/api/rapid-mlx/command-preview', (route) =>
      route.fulfill({
        status,
        contentType: 'application/json',
        body: JSON.stringify(body),
      }),
    );

    await page.goto('/');
    await page.waitForLoadState('networkidle');

    await page.evaluate(async () => {
      const { openSpawnWizard, wizardState } = await import('/js/features/spawn-wizard.js');
      openSpawnWizard();
      wizardState.engine.selected = 'rapid_mlx';
      // This is an in-memory command-preview test; bypass the host-platform
      // availability gate so it exercises the preview surface on Linux CI.
      wizardState.engine.rapidMlxLocalAvailable = true;
      wizardState.engine.rapidMlxRuntimeCompatible = true;
      wizardState.model.source = 'local';
      wizardState.model.path = '/tmp/Qwen3-8B-4bit';
      wizardState.access.port = 8001;
    });

    // Walk to the config step rather than calling the private renderer, so the test covers
    // the wiring and not just the function.
    for (let i = 0; i < 5; i += 1) {
      const next = page.locator('#wizard-next-btn');
      if (await next.isVisible() && await next.isEnabled()) await next.click();
    }
    await expect(page.locator('#wizard-step-2')).toHaveClass(/active/);
  }

  test('@in-memory-test renders the argv, the runtime diff, and the reasons', async ({ page }) => {
    await openConfigStep(page, STUB);

    const preview = page.locator('#spawn-command-preview');
    await expect(preview).toBeVisible();
    await expect(preview.locator('.spawn-command-preview-argv')).toContainText(
      'rapid-mlx serve mlx-community/Qwen3-8B-4bit --host 127.0.0.1 --port 8001',
    );
    // Flags the runtime declined are the whole reason this surface is worth having.
    await expect(preview.locator('.spawn-command-preview-diff-key')).toHaveText('turboquant_mode');
    await expect(preview.locator('.spawn-command-preview-reasons li')).toContainText(
      '--kv-cache-turboquant',
    );
    await expect(preview.locator('.spawn-command-preview-note')).toContainText('redacted');
  });

  test('@in-memory-test a clean build shows no diff and no reasons', async ({ page }) => {
    await openConfigStep(page, { argv: ['serve', 'model', '--port', '8001'], redacted: false });

    const preview = page.locator('#spawn-command-preview');
    await expect(preview.locator('.spawn-command-preview-argv')).toContainText('rapid-mlx serve model');
    await expect(preview.locator('.spawn-command-preview-diff')).toHaveCount(0);
    await expect(preview.locator('.spawn-command-preview-reasons')).toHaveCount(0);
    await expect(preview.locator('.spawn-command-preview-note')).toHaveCount(0);
  });

  test('@in-memory-test a preview failure is reported without blocking the launch', async ({ page }) => {
    await openConfigStep(page, { ok: false, error: 'Could not locate the Rapid-MLX executable' }, 400);

    const preview = page.locator('#spawn-command-preview');
    await expect(preview.locator('.spawn-command-preview-error-title')).toContainText(
      'Could not preview the launch command',
    );
    await expect(preview.locator('.spawn-command-preview-error-detail')).toContainText(
      'Could not locate the Rapid-MLX executable',
    );
    // The preview is advisory; failing to build it must not disable spawning.
    await expect(page.locator('#wizard-step-2')).toHaveClass(/active/);
  });
});
