import { test, expect } from '@playwright/test';

async function enterMonitorView(page) {
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
  await expect(page.locator('body')).not.toHaveClass(/setup-active/);
  await expect(page.locator('#view-monitor')).toBeVisible();
  await expect(page.locator('#endpoint-strip-monitor')).toBeVisible();
}

async function openNewSessionForm(page) {
  await page.getByRole('button', { name: /sessions/i }).click();
  await expect(page.locator('#session-modal')).toHaveClass(/open/);
  await expect(page.locator('#session-modal-title')).toHaveText('Sessions');
  await page.locator('#btn-new-session').click();
  await expect(page.locator('#sessions-new-form')).toBeVisible();
}

test.describe('modern UI shell', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
  });

  test('renders setup shell before monitor activation', async ({ page }) => {
    await expect(page.locator('body')).toHaveClass(/setup-active/);
    await expect(page.locator('.top-nav-bar')).toBeVisible();
    await expect(page.locator('.sidebar-nav')).toBeVisible();
    await expect(page.locator('#view-setup')).toBeVisible();
    await expect(page.getByText('Attach to Endpoint')).toBeVisible();
    await expect(page.getByText('Spawn Local Server')).toBeVisible();
  });

  test('top status endpoint is read-only and edit control is in dashboard', async ({ page }) => {
    await enterMonitorView(page);
    await page.evaluate(() => {
      const endpointUrl = document.getElementById('endpoint-url');
      if (endpointUrl) endpointUrl.textContent = 'http://127.0.0.1:8001';
    });
    await expect(page.locator('.endpoint-url')).toBeVisible();
    await expect(page.locator('.endpoint-url')).not.toHaveJSProperty('tagName', 'INPUT');
    await expect(page.locator('#server-endpoint')).toBeEditable();
  });

  test('sidebar page tabs switch server, chat, and logs', async ({ page }) => {
    await enterMonitorView(page);

    await page.getByRole('button', { name: /chat/i }).click();
    await expect(page.locator('#page-chat')).toBeVisible();
    await expect(page.locator('#page-server')).not.toBeVisible();

    await page.getByRole('button', { name: /logs/i }).click();
    await expect(page.locator('#page-logs')).toBeVisible();
    await expect(page.locator('#page-chat')).not.toBeVisible();

    await page.getByRole('button', { name: /server/i }).click();
    await expect(page.locator('#page-server')).toBeVisible();
  });
});

test.describe('modal controls', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
  });

  test('settings opens and secondary tabs switch', async ({ page }) => {
    await page.getByRole('button', { name: /settings/i }).first().click();
    await expect(page.locator('#settings-modal')).toHaveClass(/open/);
    await expect(page.locator('#settings-session')).toBeVisible();

    await page.getByRole('button', { name: 'Advanced' }).click();
    await expect(page.locator('#settings-advanced')).toBeVisible();
    await expect(page.getByRole('button', { name: /open runtime configuration/i })).toBeVisible();
  });

  test('sessions opens without stale id errors', async ({ page }) => {
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));

    await openNewSessionForm(page);
    await page.locator('#modal-session-mode').selectOption('attach');
    await expect(page.locator('#modal-session-port-label')).toHaveText('Endpoint');
    expect(errors).toEqual([]);
  });

  test('models modal opens and lists model discovery state', async ({ page }) => {
    await page.getByRole('button', { name: /models/i }).click();
    await expect(page.locator('#models-modal')).toHaveClass(/open/);
    await expect(page.locator('#models-summary')).toBeVisible();
    await expect(page.locator('#models-list')).toBeVisible();
  });

  test('profile menu remains open after click', async ({ page }) => {
    await page.getByRole('button', { name: /user/i }).click();
    await expect(page.locator('.nav-user-menu')).toHaveClass(/open/);
    await expect(page.getByRole('link', { name: 'Preferences' })).toBeVisible();
  });

  test('profile dropdown actions are wired', async ({ page }) => {
    await page.getByRole('button', { name: /user/i }).click();
    await page.getByRole('link', { name: 'Preferences' }).click();
    await expect(page.locator('#user-preferences-modal')).toHaveClass(/open/);
    await page.locator('#user-preferences-modal .modal-close').click();

    await page.getByRole('button', { name: /user/i }).click();
    await page.waitForSelector('#nav-user-menu-items', { state: 'visible' });
    await page.locator('#user-menu-help').click();
    await expect(page.locator('#keyboard-shortcuts-modal')).toHaveClass(/open/);
    await page.locator('#keyboard-shortcuts-modal .shortcuts-close').click();

    await page.getByRole('button', { name: /user/i }).click();
    await page.getByRole('link', { name: 'Toggle Theme' }).click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', /light|dark/);
  });

  test('remote agent fix opens runtime configuration', async ({ page }) => {
    await enterMonitorView(page);
    await page.evaluate(() => {
      // Force both agent-status and fix button visible for testing
      document.getElementById('agent-status').style.display = '';
      document.querySelector('.btn-agent-fix').style.display = '';
    });
    await page.locator('.btn-agent-fix').click();
    await expect(page.locator('#remote-agent-setup-modal')).toBeVisible();
  });

  test('configuration explains local executable, GPU, and explicit SSH flow', async ({ page }) => {
    await page.getByRole('button', { name: /settings/i }).first().click();
    await page.getByRole('button', { name: 'Advanced' }).click();
    await page.getByRole('button', { name: /open runtime configuration/i }).click();

    await expect(page.locator('#config-modal')).toHaveClass(/open/);
    await expect(page.getByText('Local llama-server executable')).toBeVisible();
    await expect(page.getByText('actual executable file')).toBeVisible();
    await expect(page.getByText('These checks run on this Mac or workstation only.')).toBeVisible();
    await expect(page.getByText('Remote agent actions are opt-in.')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Guided SSH Setup' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Check Host' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Check Release' })).toBeVisible();
  });

  test('guided SSH setup builds a structured target without contacting host', async ({ page }) => {
    let detectCalls = 0;

    await page.route('/api/remote-agent/detect', async route => {
      detectCalls += 1;
      const payload = route.request().postDataJSON();
      expect(payload.ssh_target).toBe('ssh://user@192.0.2.16:2222');
      expect(payload.ssh_connection).toMatchObject({
        host: '192.0.2.16',
        username: 'user',
        port: 2222,
      });
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          ok: true,
          os: 'linux',
          arch: 'x86_64',
          installed: false,
          reachable: false,
          install_path: '~/.config/llama-monitor/bin/llama-monitor',
          matching_asset: { name: 'llama-monitor-linux-x86_64', archive: false },
          latest_release: { tag_name: 'v0.5.1' },
        }),
      });
    });

    await page.getByRole('button', { name: /settings/i }).first().click();
    await page.getByRole('button', { name: 'Advanced' }).click();
    await page.getByRole('button', { name: /open runtime configuration/i }).click();
    await page.getByRole('button', { name: 'Guided SSH Setup' }).click();
    await page.locator('#ssh-guide-host').fill('192.0.2.16');
    await page.locator('#ssh-guide-user').fill('user');
    await page.locator('#ssh-guide-port').fill('2222');
    await page.getByRole('button', { name: 'Preview Plan' }).click();
    await expect(page.locator('#ssh-guide-plan')).toContainText('ssh://user@192.0.2.16:2222');
    await expect(page.getByRole('button', { name: 'Scan Host Key' })).toBeVisible();
    expect(detectCalls).toBe(0);

    await page.getByRole('button', { name: 'Use These Settings' }).click();
    await expect(page.locator('#set-remote-agent-ssh-target')).toHaveValue('ssh://user@192.0.2.16:2222');
    expect(detectCalls).toBe(0);

    await page.getByRole('button', { name: 'Check Host' }).click();
    await expect.poll(() => detectCalls).toBe(1);
  });

  test('typing SSH target does not auto-detect remote host', async ({ page }) => {
    let detectCalls = 0;

    await page.route('/api/remote-agent/detect', async route => {
      detectCalls += 1;
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          ok: true,
          os: 'linux',
          arch: 'x86_64',
          installed: false,
          reachable: false,
          install_path: '~/.config/llama-monitor/bin/llama-monitor',
          matching_asset: { name: 'llama-monitor-linux-x86_64', archive: false },
          latest_release: { tag_name: 'v0.5.1' },
        }),
      });
    });

    await page.getByRole('button', { name: /settings/i }).first().click();
    await page.getByRole('button', { name: 'Advanced' }).click();
    await page.getByRole('button', { name: /open runtime configuration/i }).click();
    await page.getByText('SSH and Agent Details').click();
    await page.locator('#set-remote-agent-ssh-target').fill('user@192.0.2.16');
    await page.waitForTimeout(250);
    expect(detectCalls).toBe(0);

    await page.getByRole('button', { name: 'Check Host' }).click();
    await expect.poll(() => detectCalls).toBe(1);
  });
});

test.describe('responsive shell', () => {
  test('mobile layout keeps navigation and endpoint form usable', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    await expect(page.locator('body')).toHaveClass(/setup-active/);
    await expect(page.locator('.sidebar-nav')).toBeVisible();
    await expect(page.locator('#setup-endpoint-url')).toBeEditable();
    await expect(page.locator('#view-setup .setup-btn-primary')).toBeVisible();
  });
});

test.describe('inference metric rendering', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await enterMonitorView(page);
  });

  test('renders active slot, request, speculative, and sampler state from slot snapshots', async ({ page }) => {
    await page.evaluate(async () => {
      const { renderSlotGrid, renderDecodingConfig, updateRequestActivity, renderActivityRail,
              renderGenerationDetailItems, setChipState } = await import('/js/features/dashboard-render.js');
      const l = {
        slots_processing: 1,
        slots_idle: 0,
        active_task_id: 6686,
        slot_generation_tokens: 690,
        slot_generation_remaining: 10,
        slot_generation_limit: 700,
        slot_generation_available: true,
        slot_generation_active: true,
        context_capacity_tokens: 212992,
        context_high_water_tokens: 121713,
        slots: [{
          id: 0,
          n_ctx: 212992,
          is_processing: true,
          id_task: 6686,
          output_tokens: 690,
          output_remaining: 10,
          output_limit: 700,
          output_active: true,
          output_available: true,
          speculative_enabled: true,
          speculative_type: 'ngram_map_k',
          speculative_config: [
            { label: 'type', value: 'ngram_map_k' },
            { label: 'n_max', value: '48' },
            { label: 'p_min', value: '0.75' },
          ],
          sampler_stack: ['penalties', 'dry', 'top_n_sigma', 'top_k', 'typ_p', 'top_p', 'min_p', 'xtc', 'temperature'],
          sampler_config: [
            { label: 'top_k', value: '20' },
            { label: 'top_p', value: '0.95' },
            { label: 'temp', value: '0.6' },
          ],
        }],
      };

      renderSlotGrid(l, true);
      renderDecodingConfig(l, true, true);
      updateRequestActivity(6686, true, 690, Date.now());
      renderActivityRail(true);
      renderGenerationDetailItems(document.getElementById('m-generation-details'), [
        'task 6686',
        '700 output budget',
        '10 output tokens remaining',
        '1 busy · 0 idle',
      ]);
      setChipState(document.getElementById('m-slots-state'), 'active', 'live');
      setChipState(document.getElementById('m-activity-state'), 'active', 'live');
    });

    await expect(page.locator('#m-slot-grid')).toContainText('task 6686');
    await expect(page.locator('#m-slot-grid')).toContainText('690 output');
    await expect(page.locator('#m-speculative-chip')).toContainText('Speculative · ngram_map_k · n_max 48');
    await expect(page.locator('#m-sampler-params-inline')).toContainText('top_k');
    await expect(page.locator('#m-sampler-params-inline')).toContainText('top_p');
    await expect(page.locator('#m-activity-rail .activity-segment.active')).toBeVisible();
    await expect(page.locator('#m-activity-rail .activity-phase.prompt')).toBeVisible();
    await expect(page.locator('#m-activity-rail .activity-phase.generation')).toBeVisible();
    await expect(page.locator('#m-generation-details .generation-detail-chip')).toHaveCount(4);
  });

  test('request rail leaves completion markers for finished tasks', async ({ page }) => {
    await page.evaluate(async () => {
      const { updateRequestActivity, renderActivityRail } = await import('/js/features/dashboard-render.js');
      const appState = await import('/js/core/app-state.js');
      appState.requestActivity.splice(0);
      const now = Date.now();
      updateRequestActivity(7001, true, 0, now - 4000);
      updateRequestActivity(7001, true, 40, now - 2500);
      updateRequestActivity(7001, false, 80, now - 500);
      renderActivityRail(false);
    });

    await expect(page.locator('#m-activity-rail .activity-segment.complete')).toBeVisible();
    await expect(page.locator('#m-activity-rail .activity-marker')).toBeVisible();
  });

  test('smooths live output estimate across recent polling samples', async ({ page }) => {
    const rate = await page.evaluate(async () => {
      const { updateLiveOutputEstimate } = await import('/js/features/dashboard-render.js');
      const appState = await import('/js/core/app-state.js');
      Object.assign(appState.liveOutputTracker, { taskId: null, previousDecoded: null, previousMs: null, latestRate: 0, rates: [] });
      appState.metricSeries.liveOutput = [];
      const now = Date.now();
      updateLiveOutputEstimate(123, 0, true, now - 3000);
      updateLiveOutputEstimate(123, 100, true, now - 2000);
      return updateLiveOutputEstimate(123, 170, true, now - 1000);
    });

    expect(rate).toBeGreaterThan(80);
    expect(rate).toBeLessThan(90);
  });

  test('capability popover opens by click and reports context honesty', async ({ page }) => {
    await page.evaluate(async () => {
      const { renderCapabilityPopover } = await import('/js/features/dashboard-render.js');
      renderCapabilityPopover({
        capabilities: { inference: true },
        host_metrics_available: false,
        remote_agent_connected: false,
      }, {
        slots_processing: 1,
        slots_idle: 0,
        context_capacity_tokens: 212992,
      }, true, false);
    });

    await page.locator('#endpoint-status').click();
    await expect(page.locator('#capability-popover')).toContainText('Context usage');
    await expect(page.locator('#capability-popover')).toContainText('not exposed');
    await expect(page.locator('#endpoint-status')).toHaveAttribute('aria-expanded', 'true');
  });
});
