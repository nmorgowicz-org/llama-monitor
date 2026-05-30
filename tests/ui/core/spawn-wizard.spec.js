// ── Spawn Wizard E2E Tests (Phase 3 + Phase 4) ───────────────────────────────
// Tests for:
// - HF model search integration
// - Third-party model import
// - Introspection-driven config
// - Error states (no GPU, no models, no internet)
// - Rate limiting (HF search / download)
// These tests assume the app is running and the spawn wizard is accessible.

import { test, expect } from '@playwright/test';

test.describe('Spawn Wizard - Phase 3 + Phase 4', () => {
    test('HF model search returns results', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        // Open spawn wizard if button exists.
        const openBtn = page.locator('#spawn-wizard-btn, button:has-text("Spawn")').first();
        if (await openBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
            await openBtn.click();
        }

        // Check that the HF search endpoint is reachable via a simple network request.
        const [response] = await Promise.all([
            page.waitForResponse(r => r.url().includes('/api/hf/search') && r.request().method() === 'POST'),
            page.evaluate(async () => {
                const headers = window.authHeaders ? window.authHeaders() : {};
                return fetch('/api/hf/search', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json', ...headers },
                    body: JSON.stringify({ query: 'llama', limit: 5 }),
                });
            }),
        ]);

        expect(response.ok()).toBeTruthy();
        const data = await response.json();
        expect(data.ok).toBe(true);
        expect(Array.isArray(data.models)).toBe(true);
    });

    test('Third-party models endpoint responds', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        const [response] = await Promise.all([
            page.waitForResponse(r => r.url().includes('/api/third-party-models') && r.request().method() === 'POST'),
            page.evaluate(async () => {
                const headers = window.authHeaders ? window.authHeaders() : {};
                return fetch('/api/third-party-models', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json', ...headers },
                    body: JSON.stringify({ include_subdirs: true }),
                });
            }),
        ]);

        expect(response.ok()).toBeTruthy();
        const data = await response.json();
        expect(data.ok).toBe(true);
        expect(Array.isArray(data.models)).toBe(true);
    });

    test('Model introspect endpoint responds with 400 when not configured', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        const response = await page.evaluate(async () => {
            const headers = window.authHeaders ? window.authHeaders() : {};
            return fetch('/api/model/introspect', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json', ...headers },
                body: JSON.stringify({ model_path: '/nonexistent/model.gguf' }),
            });
        });

        // Either 400 (not configured or model not found) or 401 (no token) is acceptable.
        expect([400, 401, 403]).toContain(response.status);
    });

    test('HF search rate limiting returns 429 after many requests', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        // Send 12 rapid requests; at least one should be 429.
        let rateLimited = false;
        for (let i = 0; i < 12; i++) {
            const resp = await page.evaluate(async () => {
                const headers = window.authHeaders ? window.authHeaders() : {};
                return fetch('/api/hf/search', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json', ...headers },
                    body: JSON.stringify({ query: 'llama', limit: 5 }),
                });
            });
            if (resp.status === 429) {
                rateLimited = true;
                break;
            }
        }
        // If rate limiting is enforced, we should see at least one 429.
        // In local-first mode, this may not trigger; treat as best-effort.
        expect(rateLimited).toBe(true);
    });

    test('HF download cooldown returns 429 on rapid start', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        // First request: start a download.
        const first = await page.evaluate(async () => {
            const headers = window.authHeaders ? window.authHeaders() : {};
            return fetch('/api/hf/download', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json', ...headers },
                body: JSON.stringify({ repo_id: 'test/repo', file_path: 'model.gguf' }),
            });
        });

        // Immediate second request: should be 429 (cooldown).
        const second = await page.evaluate(async () => {
            const headers = window.authHeaders ? window.authHeaders() : {};
            return fetch('/api/hf/download', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json', ...headers },
                body: JSON.stringify({ repo_id: 'test/repo', file_path: 'model.gguf' }),
            });
        });

        // At least one should be non-200; cooldown should kick in.
        expect(second.status).toBe(429);
    });

    test('Spawn wizard UI is keyboard accessible', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        const openBtn = page.locator('#spawn-wizard-btn, button:has-text("Spawn")').first();
        if (!(await openBtn.isVisible({ timeout: 3000 }).catch(() => false))) {
            test.skip();
        }

        // Open wizard.
        await openBtn.click();

        // Escape key should close the wizard.
        await page.keyboard.press('Escape');

        const overlay = page.locator('#spawn-wizard-overlay');
        // After Escape, overlay should be closed.
        await expect(overlay).not.toBeVisible({ timeout: 2000 });
    });

    test('Error state: no internet (HF search returns empty)', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        // Simulate a network failure by intercepting HF API calls.
        await page.route('https://huggingface.co/api/**', route => {
            route.abort('failed');
        });

        const resp = await page.evaluate(async () => {
            const headers = window.authHeaders ? window.authHeaders() : {};
            return fetch('/api/hf/search', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json', ...headers },
                body: JSON.stringify({ query: 'llama', limit: 5 }),
            });
        });

        // Should not crash; should return 500 or similar with error.
        expect([400, 401, 403, 500, 502, 503]).toContain(resp.status);
    });
});
