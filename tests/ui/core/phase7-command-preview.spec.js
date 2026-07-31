// tests/ui/core/phase7-command-preview.spec.js
//
// Phase 7A3 command-preview endpoint tests (7.5A).
// Verifies POST /api/rapid-mlx/command-preview:
// - Returns valid argv with Phase 7 fields
// - Requires auth
// - Respects tool_call_parser value (not bare flag)
// - Emits concrete Phase 7 argv and excludes disabled controls
//
// These tests work against real endpoints in CI — Phase 7 backend is implemented.
//
// NOTE: every test here is @runtime-required and skipped unless LLAMA_MONITOR_HAS_RUNTIME=1,
// so this file has never actually run. It was also written before the endpoint could resolve
// its own binary: these payloads carry no `executable_path`, which the handler used to
// require, so they would have failed with BAD_REQUEST. Browser-side coverage that does run
// on every invocation lives in command-preview-ui.spec.js.

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
      auto_tool_choice: true,
      reasoning_mode: 'off',
      prefill_step_size: 1536,
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

    const flagIndex = (flag) => data.argv.indexOf(flag);
    expect(flagIndex('--tool-call-parser')).toBeGreaterThanOrEqual(0);
    expect(data.argv[flagIndex('--tool-call-parser') + 1]).toBe('openai');
    expect(data.argv).toContain('--enable-auto-tool-choice');
    expect(data.argv).toContain('--reasoning');
    expect(data.argv).toContain('--no-thinking');
    expect(data.argv[flagIndex('--prefill-step-size') + 1]).toBe('1536');
  });

  test('@runtime-required command preview omits disabled optional Phase 7 flags', async ({ page }) => {
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
      reasoning_mode: 'on',
      auto_tool_choice: false,
      prefill_step_size: 512,
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
    expect(data.argv).toContain('--reasoning');
    expect(data.argv).not.toContain('--no-thinking');
    expect(data.argv).not.toContain('--enable-auto-tool-choice');
    expect(data.argv).not.toContain('--tool-call-parser');
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
