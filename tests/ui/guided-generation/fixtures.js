// ── Guided Generation Test Utilities ──────────────────────────────────────────

import { test, expect } from '@playwright/test';

export async function switchToChat(page) {
  await page.goto('/');
  await page.waitForSelector('html.modules-ready');
  
  // Switch to monitor view
  await page.evaluate(() => {
    return import('/js/features/setup-view.js').then(({ switchView }) => {
      switchView('monitor');
    });
  });
  
  await page.getByRole('button', { name: /chat/i }).click();
  
  // Initialize chat tabs
  await page.evaluate(() => {
    return import('/js/features/chat-state.js').then(({ initChatTabs }) => {
      return initChatTabs();
    });
  });
}

export async function enableGuidedGenFeatures(page, features = {}) {
  await page.evaluate((features) => {
    const defaults = {
      enabled_context_notes: true,
      enabled_suggestions: true,
      enabled_quick_guide: true,
      context_depth: 10,
      suggestion_count: 5,
      default_sidebar_width: 280,
      suggestion_prompts: {
        general: 'You are a creative brainstorming partner. Generate {count} suggestions based on the conversation context. Format each as: [EMOJI] Title\nDescription',
        'plot-twist': 'You are a plot twist specialist. Generate {count} unexpected but logical plot developments. Format: [EMOJI] Title\nDescription',
        'new-character': 'You are a character introduction specialist. Generate {count} compelling new character concepts. Format: [EMOJI] Title\nDescription'
      }
    };
    localStorage.setItem('llama_monitor_settings', JSON.stringify({ ...defaults, ...features }));
  }, features);
}

export async function seedChatMessages(page, messages) {
  await page.evaluate((msgs) => {
    return Promise.all([
      import('/js/features/chat-state.js'),
      import('/js/features/chat-render.js'),
    ]).then(([{ activeChatTab }, { renderChatMessages }]) => {
      const tab = activeChatTab();
      tab.messages = msgs;
      renderChatMessages();
    });
  }, messages);
}

export async function mockSuggestionsAPI(page, response) {
  await page.route('/api/chat/suggestions', route => {
    route.fulfill({
      status: 200,
      json: response || {
        suggestions: [
          '[🎭] Character Development\nDeepen the protagonist\'s backstory',
          '[⚡] Plot Acceleration\nIntroduce a time limit to increase tension'
        ],
        category: 'general',
        count: 2
      }
    });
  });
}

export async function waitForSelector(page, selector, options = {}) {
  await page.waitForSelector(selector, { ...options, timeout: 5000 });
}
