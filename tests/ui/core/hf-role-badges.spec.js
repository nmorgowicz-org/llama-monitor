import { test, expect } from '@playwright/test';

// Role badges used to come from KNOWN_CONVERTER_PATTERNS, a hardcoded regex list in
// hf-browse.js with three roles. It classified `Qwen/` as a converter -- Qwen is the original
// author of Qwen -- and the user had no way to correct it, while the seven-role, editable
// CommunitySourceCatalog sat on the server with no route to reach it. These tests pin the
// badges to the catalog instead.
test.describe('HF role badges come from the community source catalog', () => {
  // Mirrors the server's wire format: snake_case role ids, camelCase field names.
  const CATALOG = {
    ok: true,
    catalog: {
      entries: [
        {
          username: 'bartowski', displayName: 'bartowski', description: 'Standard GGUF quants.',
          role: 'gguf_quantizer', bundled: true,
        },
        {
          username: 'mlx-community', displayName: 'MLX Community', description: 'MLX conversions.',
          role: 'mlx_converter', bundled: true,
        },
        {
          username: 'unsloth', displayName: 'Unsloth', description: 'UD quants and finetunes.',
          role: 'original_author', alsoKnownFor: ['gguf_quantizer'], bundled: true,
        },
      ],
      preferences: {},
      version: 1,
    },
    roles: [
      { id: 'original_author', label: 'Original author', description: 'Created the original model weights or first fine-tune.' },
      { id: 'gguf_quantizer', label: 'GGUF quantizer', description: 'Produced GGUF quantized weights from this model.' },
      { id: 'mlx_converter', label: 'MLX converter', description: 'Converted or produced native MLX weights from this model.' },
    ],
  };

  const MODELS = [
    { id: 'Qwen/Qwen3-8B', tags: ['text-generation'], param_b: 8, downloads: 100 },
    { id: 'bartowski/Qwen3-8B-GGUF', tags: ['gguf'], param_b: 8, downloads: 90 },
    { id: 'mlx-community/Qwen3-8B-4bit', tags: ['mlx'], param_b: 8, downloads: 80 },
    { id: 'unsloth/Qwen3-8B-GGUF', tags: ['gguf'], param_b: 8, downloads: 70 },
  ];

  async function search(page, { catalog = CATALOG, models = MODELS } = {}) {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    await page.route('**/api/hf/community-sources', async (route) => {
      if (catalog === null) {
        await route.fulfill({ status: 500, contentType: 'application/json', body: '{}' });
        return;
      }
      await route.fulfill({
        status: 200, contentType: 'application/json', body: JSON.stringify(catalog),
      });
    });
    await page.route('**/api/hf/search', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ models, next_cursor: null }),
      });
    });

    return page.evaluate(async () => {
      const mod = await import('/js/features/hf-browse.js');
      // Each test gets a fresh catalog fetch; the module caches it for the page lifetime.
      mod._resetCommunitySourceCatalog();
      const container = document.createElement('div');
      container.id = 'role-badge-probe';
      document.body.appendChild(container);
      await mod.hfSearch({ query: 'qwen3', container, allActive: true });
      return [...container.querySelectorAll('.hf-sg-variant')].map(v => ({
        id: v.querySelector('.hf-sg-variant-name')?.textContent,
        role: v.querySelector('.hf-sg-role-badge')?.className || '',
        label: v.querySelector('.hf-sg-role-badge')?.textContent || '',
        title: v.querySelector('.hf-sg-role-badge')?.getAttribute('title') || '',
      }));
    });
  }

  test('@in-memory-test Qwen is the original author, not a converter', async ({ page }) => {
    const rows = await search(page);
    const qwen = rows.find(r => r.id === 'Qwen/Qwen3-8B');
    expect(qwen, 'Qwen/Qwen3-8B was not rendered').toBeTruthy();
    expect(qwen.label).toBe('Original author');
    expect(qwen.role).toContain('hf-sg-role-badge--original-author');
  });

  test('@in-memory-test catalog roles drive the badge, with the role description as its title', async ({ page }) => {
    const rows = await search(page);
    const byId = Object.fromEntries(rows.map(r => [r.id, r]));

    expect(byId['bartowski/Qwen3-8B-GGUF'].label).toBe('GGUF quantizer');
    expect(byId['mlx-community/Qwen3-8B-4bit'].label).toBe('MLX converter');
    // The tooltip is the role's own description from the Rust enum, not a string retyped in JS.
    expect(byId['mlx-community/Qwen3-8B-4bit'].title)
      .toBe('Converted or produced native MLX weights from this model.');
  });

  // Unsloth authors finetunes and quantizes them. A single-role lookup would label every
  // Unsloth repo the same way; the repo's own format tags pick which of their roles applies.
  test('@in-memory-test a multi-role owner is resolved by the repo tags', async ({ page }) => {
    const rows = await search(page);
    const unsloth = rows.find(r => r.id === 'unsloth/Qwen3-8B-GGUF');
    expect(unsloth.label).toBe('GGUF quantizer');
  });

  // The catalog is an enhancement to the badges, not a prerequisite for search working.
  test('@in-memory-test search still renders when the catalog cannot be loaded', async ({ page }) => {
    const rows = await search(page, { catalog: null });
    expect(rows.length).toBe(4);
    // Unknown owners fall back to the repo-name heuristic rather than to a wrong role.
    expect(rows.find(r => r.id === 'Qwen/Qwen3-8B').label).toBe('Original author');
    expect(rows.find(r => r.id === 'bartowski/Qwen3-8B-GGUF').label).toBe('');
  });
});
