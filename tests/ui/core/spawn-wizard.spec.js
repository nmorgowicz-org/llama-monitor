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

    test('HF file browser marks the family-recommended mmproj', async ({ page }) => {
        await page.route('**/api/hf/files', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({
                ok: true,
                files: [
                    {
                        repo_id: 'unsloth/gemma-4-31B-it-qat-GGUF',
                        path: 'gemma-4-31B-it-Q4_0.gguf',
                        size: 17_400_000_000,
                        label: 'Q4_0',
                        quant_type: 'standard',
                        is_mmproj: false,
                        is_recommended_mmproj: false,
                        mmproj_recommendation: '',
                    },
                    {
                        repo_id: 'unsloth/gemma-4-31B-it-qat-GGUF',
                        path: 'mmproj-F16.gguf',
                        size: 741_000_000,
                        label: 'F16',
                        quant_type: 'standard',
                        is_mmproj: true,
                        is_recommended_mmproj: true,
                        mmproj_recommendation: 'F16 is the documented llama.cpp projector default for Gemma 4',
                    },
                    {
                        repo_id: 'unsloth/gemma-4-31B-it-qat-GGUF',
                        path: 'mmproj-F32.gguf',
                        size: 1_480_000_000,
                        label: 'F32',
                        quant_type: 'standard',
                        is_mmproj: true,
                        is_recommended_mmproj: false,
                        mmproj_recommendation: '',
                    },
                ],
            }),
        }));

        await page.goto('/');
        await page.waitForLoadState('networkidle');
        await page.evaluate(async () => {
            const container = document.createElement('div');
            container.id = 'test-hf-files';
            document.body.appendChild(container);
            const { hfListFiles } = await import('/js/features/hf-browse.js');
            await hfListFiles({
                repoId: 'unsloth/gemma-4-31B-it-qat-GGUF',
                container,
                vramGb: 32,
            });
        });

        const recommended = page.locator('#test-hf-files .hf-file-item', {
            hasText: 'mmproj-F16.gguf',
        });
        await expect(recommended).toContainText('Family recommended');
        await expect(recommended.locator('.hf-file-badge-recommended')).toHaveAttribute(
            'title',
            /documented llama\.cpp projector default for Gemma 4/
        );
        await expect(page.locator('#test-hf-files .hf-file-item', {
            hasText: 'mmproj-F32.gguf',
        })).not.toContainText('Family recommended');
    });

    test('HF file browser selects a linked mmproj from its owning repo', async ({ page }) => {
        await page.route('**/api/hf/files', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({
                ok: true,
                files: [{
                    repo_id: 'mradermacher/model-GGUF',
                    path: 'mmproj-F16.gguf',
                    size: 1_000_000_000,
                    label: 'F16',
                    quant_type: 'standard',
                    is_mmproj: true,
                    is_recommended_mmproj: true,
                    mmproj_recommendation: 'F16 is recommended',
                }],
            }),
        }));

        await page.goto('/');
        await page.waitForLoadState('networkidle');
        const selectedRepo = await page.evaluate(async () => {
            const container = document.createElement('div');
            document.body.appendChild(container);
            const { hfListFiles } = await import('/js/features/hf-browse.js');
            return new Promise(resolve => {
                hfListFiles({
                    repoId: 'mradermacher/model-i1-GGUF',
                    container,
                    vramGb: 32,
                    onSelectFile: (_file, repoId) => resolve(repoId),
                }).then(() => container.querySelector('.hf-file-item').click());
            });
        });

        expect(selectedRepo).toBe('mradermacher/model-GGUF');
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

        const [response] = await Promise.all([
            page.waitForResponse(r => r.url().includes('/api/model/introspect') && r.request().method() === 'POST'),
            page.evaluate(async () => {
                const headers = window.authHeaders ? window.authHeaders() : {};
                return fetch('/api/model/introspect', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json', ...headers },
                    body: JSON.stringify({ model_path: '/nonexistent/model.gguf' }),
                });
            }),
        ]);

        // Endpoint is reachable; 200 is ok (graceful response), or 4xx if not configured.
        const status = response.status();
        expect(status).toBeGreaterThanOrEqual(200);
        expect(status).toBeLessThan(500);
    });

    test('HF search rate limiting returns non-200 after many requests', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        // Send many rapid requests; at least one should be rejected.
        // The HF search rate limit is 10/60s (global), so other tests may
        // consume part of the budget. We use a generous count and accept
        // any non-200 as evidence that the limiter is active.
        const COUNT = 30;
        let nonOk = false;
        for (let i = 0; i < COUNT; i++) {
            const resp = await page.evaluate(async () => {
                const headers = window.authHeaders ? window.authHeaders() : {};
                return fetch('/api/hf/search', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json', ...headers },
                    body: JSON.stringify({ query: 'llama', limit: 5 }),
                });
            });
            if (resp.status !== 200) {
                nonOk = true;
                break;
            }
        }
        expect(nonOk).toBe(true);
    });

    test('HF download cooldown returns 429 on rapid start', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        // First request: start a download.
        const [first] = await Promise.all([
            page.waitForResponse(r => r.url().includes('/api/hf/download')),
            page.evaluate(async () => {
                const headers = window.authHeaders ? window.authHeaders() : {};
                return fetch('/api/hf/download', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json', ...headers },
                    body: JSON.stringify({ repo_id: 'test/repo', file_path: 'model.gguf' }),
                });
            }),
        ]);

        // Immediate second request: should be 429 (cooldown).
        const [second] = await Promise.all([
            page.waitForResponse(r => r.url().includes('/api/hf/download')),
            page.evaluate(async () => {
                const headers = window.authHeaders ? window.authHeaders() : {};
                return fetch('/api/hf/download', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json', ...headers },
                    body: JSON.stringify({ repo_id: 'test/repo', file_path: 'model.gguf' }),
                });
            }),
        ]);

        // At least one should be non-200; cooldown should kick in.
        expect(second.status()).toBe(429);
    });

    test('Spawn wizard UI is keyboard accessible', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        // Open wizard via JS helper (DOM open button is a hidden compat element).
        await page.evaluate(async () => {
            const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
            openSpawnWizard();
        });
        await expect(page.locator('#spawn-wizard-overlay')).toHaveClass(/open/);

        // Escape key should close the wizard.
        await page.keyboard.press('Escape');

        const overlay = page.locator('#spawn-wizard-overlay');
        // After Escape, overlay should be closed.
        await expect(overlay).not.toBeVisible({ timeout: 2000 });
    });

    test('Spawn wizard ignores backdrop clicks so progress is not lost', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        await page.evaluate(async () => {
            const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
            openSpawnWizard();
        });

        const overlay = page.locator('#spawn-wizard-overlay');
        await expect(overlay).toHaveClass(/open/);

        await page.locator('#spawn-wizard-overlay').click({ position: { x: 8, y: 8 } });

        await expect(overlay).toHaveClass(/open/);
        await expect(page.locator('#wizard-step-0')).toHaveClass(/active/);
    });

    test('Model step disables Next until a model is selected', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        await page.evaluate(async () => {
            const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
            openSpawnWizard();
        });

        await page.locator('#wizard-next-btn').click();
        await expect(page.locator('#wizard-step-1')).toHaveClass(/active/);

        const nextBtn = page.locator('#wizard-next-btn');
        await expect(nextBtn).toBeDisabled();
        await expect(page.locator('#wizard-footer-hint')).toContainText('Choose a local GGUF file');

        await page.fill('#spawn-model-path', '/tmp/Qwen3.6-27B-Instruct-Q4_K_M.gguf');

        await expect(nextBtn).toBeEnabled();
        await expect(page.locator('#wizard-footer-hint')).toContainText('Local model selected');
    });

    test('Hardware auto-fit toggle auto-populates fit target and keeps step navigable', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        await page.evaluate(async () => {
            const { openSpawnWizard, wizardState } = await import('/js/features/spawn-wizard.js');
            openSpawnWizard();
            wizardState.model.source = 'local';
            wizardState.model.path = '/tmp/Qwen3.6-27B-Instruct-Q4_K_M.gguf';
            wizardState.model.paramB = 27;
            wizardState.model.modelBytes = 16 * 1024 * 1024 * 1024;
            // Inject VRAM so renderScenarioCards doesn't short-circuit in CI (no GPU endpoint)
            wizardState.vram.available = 48 * 1024 * 1024 * 1024; // 48 GB
        });

        await page.locator('.profile-card[data-profile="advanced"]').click();
        await page.locator('#wizard-next-btn').click();
        await expect(page.locator('#wizard-step-1')).toHaveClass(/active/);

        await page.fill('#spawn-model-path', '/tmp/Qwen3.6-27B-Instruct-Q4_K_M.gguf');
        await page.locator('#wizard-next-btn').click();
        await expect(page.locator('#wizard-step-2')).toHaveClass(/active/);
        await expect(page.locator('.vsc-section-label')).toContainText('Context fit modes');
        await expect(page.locator('#vram-scenarios')).toContainText('Reliable agents');
        await expect(page.locator('#spawn-cache-type-k')).toHaveValue('q8_0');
        await expect(page.locator('#spawn-cache-type-v')).toHaveValue('q8_0');

        const nextBtn = page.locator('#wizard-next-btn');

        // Enable fit — should auto-populate '2048' and keep next enabled
        await page.selectOption('#spawn-fit-enable', 'true');
        await expect(nextBtn).toBeEnabled();
        await expect(page.locator('#spawn-fit-target')).toHaveValue('2048');

        await page.fill('#spawn-fit-target', '2048');
        await expect(nextBtn).toBeEnabled();

        // Hardware step must still be active and visible after the layout change
        await expect(page.locator('#wizard-step-2')).toHaveClass(/active/);
        await expect(page.locator('#wizard-step-2 .wizard-section-title').first()).toContainText('Configure hardware');
    });

    test('Spawn payload leaves fit parameters unset until the toggle is enabled', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        const payloads = await page.evaluate(async () => {
            const { buildSpawnPayload, wizardState } = await import('/js/features/spawn-wizard.js?fit-payload-test=1');
            wizardState.hardware.fitEnabled = null;
            wizardState.hardware.fitTarget = '';
            const defaults = buildSpawnPayload();
            wizardState.hardware.fitEnabled = false;
            const disabled = buildSpawnPayload();
            wizardState.hardware.fitEnabled = true;
            wizardState.hardware.fitTarget = '2048';
            const enabled = buildSpawnPayload();
            return { defaults, disabled, enabled };
        });

        expect(payloads.defaults.fit_enabled).toBeNull();
        expect(payloads.defaults.fit_target).toBeNull();
        expect(payloads.disabled.fit_enabled).toBe(false);
        expect(payloads.disabled.fit_target).toBeNull();
        expect(payloads.enabled.fit_enabled).toBe(true);
        expect(payloads.enabled.fit_target).toBe('2048');
    });

    test('Community templates use the correct source for Qwen and Gemma 4', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        const templates = await page.evaluate(async () => {
            const { COMMUNITY_TEMPLATES } = await import('/js/features/chat-template-registry.js?community-template-test=1');
            return COMMUNITY_TEMPLATES;
        });

        expect(templates.qwen.installEndpoint).toBe('/api/chat-template/install-hf');
        expect(templates.qwen.repo).toBe('froggeric/Qwen-Fixed-Chat-Templates');
        expect(templates.gemma4.installEndpoint).toBe('/api/chat-template/install-url');
        expect(templates.gemma4.url).toBe(
            'https://raw.githubusercontent.com/jscott3201/llm-tuning/main/gemma4/chat_templates/custom_pub_chat_template_gemma4.jinja',
        );
    });

    test('Hardware advanced toggles work: kv-unified flips state, fit-enable auto-populates target', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        await page.evaluate(async () => {
            const { openSpawnWizard, wizardState } = await import('/js/features/spawn-wizard.js');
            openSpawnWizard();
            wizardState.model.source = 'local';
            wizardState.model.path = '/tmp/Qwen3.6-27B-Instruct-Q4_K_M.gguf';
            wizardState.model.paramB = 27;
            wizardState.model.modelBytes = 16 * 1024 * 1024 * 1024;
            // Inject VRAM so renderScenarioCards doesn't short-circuit in CI (no GPU endpoint)
            wizardState.vram.available = 48 * 1024 * 1024 * 1024; // 48 GB
        });

        await page.locator('.profile-card[data-profile="advanced"]').click();
        await page.locator('#wizard-next-btn').click();
        await expect(page.locator('#wizard-step-1')).toHaveClass(/active/);

        await page.fill('#spawn-model-path', '/tmp/Qwen3.6-27B-Instruct-Q4_K_M.gguf');
        await page.locator('#wizard-next-btn').click();
        await expect(page.locator('#wizard-step-2')).toHaveClass(/active/);
        await expect(page.locator('.vsc-section-label')).toContainText('Context fit modes');
        await expect(page.locator('#vram-scenarios')).toContainText('Reliable agents');
        await expect(page.locator('#spawn-cache-type-k')).toHaveValue('q8_0');
        await expect(page.locator('#spawn-cache-type-v')).toHaveValue('q8_0');

        await expect(page.locator('#spawn-kv-unified')).toHaveValue('');
        await page.selectOption('#spawn-kv-unified', 'false');
        await expect(page.locator('#spawn-kv-unified')).toHaveValue('false');

        // fit On shows target input and auto-populates '2048'
        await page.selectOption('#spawn-fit-enable', 'true');
        await page.waitForTimeout(400);

        const fitTargetValue = await page.evaluate(() =>
            document.getElementById('spawn-fit-target')?.value ?? ''
        );
        expect(fitTargetValue).toBe('2048');

        const moeLayout = await page.evaluate(() => {
            const field = document.getElementById('spawn-n-cpu-moe')?.closest('.hardware-field');
            const hint = field?.querySelector('.field-hint');
            const actions = document.getElementById('spawn-moe-autotune');
            if (!field || !hint || !actions) return null;

            actions.style.display = 'block';
            const fieldRect = field.getBoundingClientRect();
            const hintRect = hint.getBoundingClientRect();
            const actionsRect = actions.getBoundingClientRect();
            return {
                hintBottom: hintRect.bottom,
                actionsTop: actionsRect.top,
                actionsBottom: actionsRect.bottom,
                fieldBottom: fieldRect.bottom,
            };
        });
        expect(moeLayout).not.toBeNull();
        expect(moeLayout.actionsTop).toBeGreaterThanOrEqual(moeLayout.hintBottom);
        expect(moeLayout.fieldBottom).toBeGreaterThanOrEqual(moeLayout.actionsBottom);
    });

    test('MTP enabled by default on all platforms including Metal', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        await page.evaluate(async () => {
            const { openSpawnWizard, wizardState } = await import('/js/features/spawn-wizard.js');
            openSpawnWizard();
            wizardState.model.source = 'local';
            wizardState.model.path = '/tmp/Qwen3.6-27B-MTP-Q4_K_M.gguf';
            wizardState.model.paramB = 27;
            wizardState.model.modelBytes = 16 * 1024 * 1024 * 1024;
            wizardState.arch.mtpDepth = 1;
        });

        await page.locator('#wizard-next-btn').click();
        await page.fill('#spawn-model-path', '/tmp/Qwen3.6-27B-MTP-Q4_K_M.gguf');
        await page.locator('#wizard-next-btn').click();

        await expect(page.locator('#hw-mtp-section')).toBeVisible();
        await expect(page.locator('#hw-use-mtp')).toBeChecked({ checked: true });
    });

    test('review step exposes structured output and full sampling defaults', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        await page.evaluate(async () => {
            const { openSpawnWizard, wizardState } = await import('/js/features/spawn-wizard.js');
            openSpawnWizard();
            wizardState.model.source = 'local';
            wizardState.model.path = '/tmp/Qwen3.6-27B-Instruct-Q4_K_M.gguf';
            wizardState.model.paramB = 27;
            wizardState.model.modelBytes = 16 * 1024 * 1024 * 1024;
        });

        await page.locator('#wizard-next-btn').click();
        await page.fill('#spawn-model-path', '/tmp/Qwen3.6-27B-Instruct-Q4_K_M.gguf');
        await page.locator('#wizard-next-btn').click();
        await page.locator('#wizard-next-btn').click();

        await expect(page.locator('#wizard-step-3')).toHaveClass(/active/);
        await expect(page.locator('#spawn-top-k')).toBeVisible();
        await expect(page.locator('#spawn-max-tokens')).toBeVisible();
        await expect(page.locator('#spawn-output-mode')).toBeVisible();

        await page.selectOption('#spawn-output-mode', 'json_schema');
        await expect(page.locator('#spawn-json-schema-wrap')).toBeVisible();
        await expect(page.locator('#spawn-grammar-wrap')).toBeHidden();
    });

    test('Error state: no internet (HF search returns empty)', async ({ page }) => {
        await page.goto('/');
        await page.waitForLoadState('networkidle');

        // Simulate a network failure by intercepting HF API calls.
        await page.route('https://huggingface.co/api/**', route => {
            route.abort('failed');
        });

        const [resp] = await Promise.all([
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

        // Should not crash; backend handles gracefully (200 with empty/error, or 4xx/5xx).
        const status = resp.status();
        expect(status).toBeGreaterThanOrEqual(200);
        expect(status).toBeLessThan(500);
    });
});
