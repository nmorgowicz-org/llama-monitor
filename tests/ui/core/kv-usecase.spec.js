import { test, expect } from '@playwright/test';

// The page-1 use-case cards are meant to be the one place a workload changes something
// concrete on llama.cpp: how hard the KV cache is quantized. Tool-calling needs the q8_0
// floor; roleplay would rather spend that VRAM on context.
test.describe('use-case drives KV dtype', () => {
  test('@in-memory-test roleplay drops KV to q4_0, agentic holds q8_0, and a user choice wins', async ({ page }) => {
    await page.goto('/');
    await page.waitForFunction(() => !!document.querySelector('[data-usecase]'), { timeout: 15000 });

    const result = await page.evaluate(async () => {
      const pick = (uc) => document.querySelector(`[data-usecase="${uc}"]`)?.click();
      const kv = () => ({
        k: document.getElementById('spawn-cache-type-k')?.value,
        v: document.getElementById('spawn-cache-type-v')?.value,
      });

      pick('roleplay');
      const afterRoleplay = kv();

      pick('agentic');
      const afterAgentic = kv();

      // Now express a preference of our own, then re-pick a use case.
      const kSel = document.getElementById('spawn-cache-type-k');
      kSel.value = 'f16';
      kSel.dispatchEvent(new Event('change', { bubbles: true }));
      pick('roleplay');
      const afterUserChoice = kv();

      return { afterRoleplay, afterAgentic, afterUserChoice };
    });

    expect(result.afterRoleplay.k).toBe('q4_0');
    expect(result.afterRoleplay.v).toBe('q4_0');
    expect(result.afterAgentic.k).toBe('q8_0');
    expect(result.afterAgentic.v).toBe('q8_0');
    // The seed must not stomp an answer the user already gave.
    expect(result.afterUserChoice.k).toBe('f16');
  });
});
