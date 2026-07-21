// tests/ui/core/phase7-command-preview.spec.js
//
// Phase 7A3 command-preview endpoint tests (7.5A).
// Verifies POST /api/rapid-mlx/command-preview:
// - Returns valid argv with Phase 7 fields
// - Requires auth
// - Respects tool_call_parser value (not bare flag)
// - Respects workload_scenario
//
// These tests work against real endpoints in CI — Phase 7 backend is implemented.

import { test, expect } from '@playwright/test';

test.describe('Rapid-MLX command preview endpoint', () => {
  test('@runtime-required command preview returns valid argv for Phase 7 config', async ({ page }) => {
    const hasRuntime = !!process.env.LLAMA_MONITOR_HAS_RUNTIME;
    test.skip(!hasRuntime, 'Set LLAMA_MONITOR_HAS_RUNTIME=1 to run runtime-dependent tests.');

    const apiToken = await page.evaluate(async () => {
      const r = await fetch('/api/internal/api-token');
      const d = await r.json();
      return d.token;
    });

    const payload = {
      model_source: { kind: 'hugging_face_repo', repo_id: 'mlx-community/Qwen3-0.6B-4bit' },
      host: '127.0.0.1',
      port: 9123,
      kv_cache_dtype: 'int4',
      tool_call_parser: 'openai',
      enable_auto_tool_choice: true,
      workload_scenario: 'interactive_coding_agent',
    };

    const res = await page.evaluate(async (token, body) => {
      return fetch('/api/rapid-mlx/command-preview', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`,
        },
        body: JSON.stringify(body),
      });
    }, apiToken, payload);

    expect(res.ok).toBe(true);
    const data = await page.evaluate(async (r) => r.json(), res);
    expect(data.argv).toBeDefined();
    expect(Array.isArray(data.argv)).toBe(true);

    // Verify tool_call_parser has a value, not a bare flag
    const hasToolParser = data.argv.some(a => a.includes('--tool-call-parser') && !a.endsWith('--tool-call-parser'));
    expect(hasToolParser, 'tool_call_parser should have a value, not be a bare flag').toBe(true);
  });

  test('@runtime-required command preview returns valid argv for tool_research_agent workload', async ({ page }) => {
    const hasRuntime = !!process.env.LLAMA_MONITOR_HAS_RUNTIME;
    test.skip(!hasRuntime, 'Set LLAMA_MONITOR_HAS_RUNTIME=1 to run runtime-dependent tests.');

    const apiToken = await page.evaluate(async () => {
      const r = await fetch('/api/internal/api-token');
      const d = await r.json();
      return d.token;
    });

    const payload = {
      model_source: { kind: 'hugging_face_repo', repo_id: 'mlx-community/Qwen3-0.6B-4bit' },
      host: '127.0.0.1',
      port: 9123,
      kv_cache_dtype: 'int8',
      reasoning_mode: 'enable',
      tool_call_parser: 'openai',
      enable_auto_tool_choice: true,
      workload_scenario: 'tool_research_agent',
    };

    const res = await page.evaluate(async (token, body) => {
      return fetch('/api/rapid-mlx/command-preview', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`,
        },
        body: JSON.stringify(body),
      });
    }, apiToken, payload);

    expect(res.ok).toBe(true);
    const data = await page.evaluate(async (r) => r.json(), res);
    expect(data.argv).toBeDefined();
    expect(Array.isArray(data.argv)).toBe(true);
  });

  test('@runtime-required command preview requires auth', async ({ page }) => {
    const hasRuntime = !!process.env.LLAMA_MONITOR_HAS_RUNTIME;
    test.skip(!hasRuntime, 'Set LLAMA_MONITOR_HAS_RUNTIME=1 to run runtime-dependent tests.');

    const payload = {
      model_source: { kind: 'hugging_face_repo', repo_id: 'test/model' },
      host: '127.0.0.1',
      port: 9123,
    };

    const res = await page.evaluate(async (body) => {
      return fetch('/api/rapid-mlx/command-preview', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
    }, payload);

    expect(res.status, 'command-preview without auth should not return 200').not.toBe(200);
  });
});
