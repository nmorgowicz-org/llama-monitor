// ── Spawn Wizard E2E Tests (Phase 3) ─────────────────────────────────────────
// Tests for:
// - HF model search integration
// - Third-party model import
// - Introspection-driven config
// These tests assume the app is running and the spawn wizard is accessible.

import { test, expect } from '@playwright/test';

test.describe('Spawn Wizard - Phase 3', () => {
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
});
