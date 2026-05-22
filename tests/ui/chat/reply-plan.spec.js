// ── Reply Plan Summary Tests ──────────────────────────────────────────────────

import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

async function switchToMonitor(page) {
  await dismissAuthShell(page);
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
}

test.describe('reply plan summary', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
    await expect(page.locator('#page-chat')).toBeVisible();
  });

  test('reply plan is hidden when no active steering', async ({ page }) => {
    // Clear all steering including auto_compact (default true)
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { updateReplyPlanSummary } = await import('/js/features/chat-reply-plan.js');
      const tab = activeChatTab();
      if (tab) {
        tab.active_template_id = '';
        tab.explicit_level = 0;
        tab.context_notes = [];
        tab.quick_guide_active = '';
        tab.quick_guide_draft = '';
        tab.armed_story_beats = [];
        tab.auto_compact = false;
        updateReplyPlanSummary();
      }
    });
    const container = page.locator('#reply-plan-summary');
    await expect(container).not.toHaveClass(/visible/);
  });

  test('shows persona chip when persona is active', async ({ page }) => {
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { updateReplyPlanSummary } = await import('/js/features/chat-reply-plan.js');
      const tab = activeChatTab();
      if (tab) {
        tab.active_template_id = 'test-persona';
        updateReplyPlanSummary();
      }
    });

    const container = page.locator('#reply-plan-summary');
    await expect(container).toHaveClass(/visible/);
    const personaChip = container.locator('.chip-persona');
    await expect(personaChip).toBeVisible();
  });

  test('shows explicit chip when explicit mode is active', async ({ page }) => {
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { updateReplyPlanSummary } = await import('/js/features/chat-reply-plan.js');
      const tab = activeChatTab();
      if (tab) {
        tab.explicit_level = 2;
        updateReplyPlanSummary();
      }
    });

    const container = page.locator('#reply-plan-summary');
    await expect(container).toHaveClass(/visible/);
    const explicitChip = container.locator('.chip-explicit');
    await expect(explicitChip).toBeVisible();
    await expect(explicitChip).toContainText('Explicit: 2');
  });

  test('shows multiple chips for combined steering', async ({ page }) => {
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { updateReplyPlanSummary } = await import('/js/features/chat-reply-plan.js');
      const tab = activeChatTab();
      if (tab) {
        tab.active_template_id = 'writer';
        tab.explicit_level = 1;
        tab.context_notes = [{ name: 'Setting', content: 'A dark alley' }];
        tab.auto_compact = false;
        updateReplyPlanSummary();
      }
    });

    const container = page.locator('#reply-plan-summary');
    await expect(container).toHaveClass(/visible/);

    await expect(container.locator('.chip-persona')).toBeVisible();
    await expect(container.locator('.chip-explicit')).toBeVisible();
    await expect(container.locator('.chip-notes')).toBeVisible();
  });

  test('hides reply plan when all steering is cleared', async ({ page }) => {
    // First set up some steering
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { updateReplyPlanSummary } = await import('/js/features/chat-reply-plan.js');
      const tab = activeChatTab();
      if (tab) {
        tab.active_template_id = 'test-persona';
        tab.explicit_level = 1;
        tab.auto_compact = false;
        updateReplyPlanSummary();
      }
    });

    await expect(page.locator('#reply-plan-summary')).toHaveClass(/visible/);

    // Now clear all steering
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { updateReplyPlanSummary } = await import('/js/features/chat-reply-plan.js');
      const tab = activeChatTab();
      if (tab) {
        tab.active_template_id = '';
        tab.explicit_level = 0;
        tab.context_notes = [];
        tab.quick_guide_active = '';
        tab.quick_guide_draft = '';
        tab.armed_story_beats = [];
        tab.auto_compact = false;
        updateReplyPlanSummary();
      }
    });

    await expect(page.locator('#reply-plan-summary')).not.toHaveClass(/visible/);
  });
});
