// ── Spawn Wizard E2E Tests (Phases 3, 4, and Rapid-MLX Phase 6) ──────────────
// Tests for:
// - HF model search integration
// - Third-party model import
// - Introspection-driven config
// - Error states (no GPU, no models, no internet)
// - Rate limiting (HF search / download)
// These tests assume the app is running and the spawn wizard is accessible.

import { test, expect } from '@playwright/test';

test.describe('Spawn Wizard - Phases 3, 4, and Rapid-MLX Phase 6', () => {
    test('engine classifier leaves bare HF repos ambiguous and recognizes GGUF inventory', async ({ page }) => {
        await page.goto('/');
        const result = await page.evaluate(async () => {
            const { classifyWizardArtifact } = await import('/js/features/spawn-wizard.js');
            return {
                bareRepo: classifyWizardArtifact({ hfRepo: 'owner/model', hfFile: '', quantFiles: [] }),
                ggufRepo: classifyWizardArtifact({
                    hfRepo: 'owner/model',
                    hfFile: '',
                    quantFiles: [{ path: 'model-Q4_K_M.gguf' }],
                }),
                mlx: classifyWizardArtifact({
                    path: '/models/Qwen-MLX',
                    localMeta: { source_kind: 'mlx_directory' },
                }),
                arbitraryFile: classifyWizardArtifact({ path: '/models/README.txt' }),
            };
        });

        expect(result).toEqual({
            bareRepo: 'unknown',
            ggufRepo: 'gguf',
            mlx: 'mlx_directory',
            arbitraryFile: 'unknown',
        });
    });

    test('external Rapid-MLX runtime can be recommended and an explicit llama.cpp choice is preserved', async ({ page }) => {
        await page.route('**/api/llama-binary/platform-info', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ rapid_mlx_local_available: true, auto_backend: 'metal' }),
        }));
        await page.route('**/api/rapid-mlx/runtime/status', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ runtime: { supported: true, active: null } }),
        }));
        await page.route('**/api/rapid-mlx/recommend', async route => {
            await new Promise(resolve => setTimeout(resolve, 100));
            await route.fulfill({
                status: 200,
                contentType: 'application/json',
                body: JSON.stringify({
                    recommended_backend: 'rapid_mlx',
                    state: 'ready',
                    reason: 'A compatible external Rapid-MLX runtime is ready.',
                }),
            });
        });

        await page.goto('/');
        await page.evaluate(async () => {
            const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
            openSpawnWizard({
                localPath: '/models/Qwen-MLX',
                localModel: { source_kind: 'mlx_directory', path: '/models/Qwen-MLX' },
            });
        });
        await page.locator('.wizard-engine-card[data-engine="llama_cpp"]').click();

        await expect(page.locator('.wizard-engine-card[data-engine="llama_cpp"]')).toHaveClass(/selected/);
        await expect(page.locator('#wizard-engine-reason')).toContainText('manual llama.cpp choice is preserved');
    });

    test('Rapid-MLX payload is backend-exclusive while its preset preserves shared sampling', async ({ page }) => {
        await page.goto('/');
        const payloads = await page.evaluate(async () => {
            const {
                buildSpawnPayload,
                buildPresetPayload,
                launchPortForPayload,
                supportsTunePanelForPayload,
                wizardState,
            } = await import('/js/features/spawn-wizard.js');
            wizardState.engine.selected = 'rapid_mlx';
            wizardState.model.source = 'local';
            wizardState.model.path = '/models/Qwen-MLX';
            wizardState.access.port = 9123;
            wizardState.access.bindHost = '127.0.0.1';
            wizardState.access.apiKey = 'rapid-secret';
            wizardState.hardware.gpuLayers = 'all';
            wizardState.hardware.contextSize = 131072;
            wizardState.hardware.cacheTypeK = 'q4_0';
            wizardState.hardware.temperature = 0.42;
            wizardState.hardware.topP = 0.88;
            wizardState.hardware.topK = 32;
            wizardState.hardware.minP = 0.06;
            wizardState.hardware.repeatPenalty = 1.07;
            wizardState.hardware.presencePenalty = 0.15;
            wizardState.hardware.maxTokens = 2048;
            wizardState.hardware.seed = 7;
            const spawn = buildSpawnPayload();
            return {
                spawn,
                preset: buildPresetPayload(),
                launchPort: launchPortForPayload(spawn),
                supportsTune: supportsTunePanelForPayload(spawn),
            };
        });

        expect(payloads.spawn).toEqual({
            backend: 'rapid_mlx',
            rapid_mlx: {
                model_source: { kind: 'mlx_directory', path: '/models/Qwen-MLX' },
                served_model_name: null,
                host: '127.0.0.1',
                port: 9123,
                api_key: 'rapid-secret',
            },
        });
        expect(payloads.spawn).not.toHaveProperty('gpu_layers');
        expect(payloads.spawn).not.toHaveProperty('context_size');
        expect(payloads.spawn).not.toHaveProperty('ctk');
        expect(payloads.launchPort).toBe(9123);
        expect(payloads.supportsTune).toBe(false);
        expect(payloads.preset).toMatchObject({
            backend: 'rapid_mlx',
            temperature: 0.42,
            top_p: 0.88,
            top_k: 32,
            min_p: 0.06,
            repeat_penalty: 1.07,
            presence_penalty: 0.15,
            max_tokens: 2048,
            seed: 7,
            api_key: 'rapid-secret',
        });
        expect(payloads.preset.rapid_mlx).not.toHaveProperty('api_key');
        const saveResult = await page.evaluate(async preset => {
            const headers = window.authHeaders ? window.authHeaders() : {};
            const response = await fetch('/api/presets', {
                method: 'POST',
                headers: { ...headers, 'Content-Type': 'application/json' },
                body: JSON.stringify({ ...preset, name: 'Rapid protected key test' }),
            });
            return { ok: response.ok, status: response.status, body: await response.text() };
        }, payloads.preset);
        expect(saveResult, saveResult.body).toMatchObject({ ok: true });
    });

    test('Rapid-MLX template restores typed source and reopening clears stale engine state', async ({ page }) => {
        await page.goto('/');
        const state = await page.evaluate(async () => {
            const { openSpawnWizard, closeSpawnWizard, buildSpawnPayload, wizardState } = await import('/js/features/spawn-wizard.js');
            openSpawnWizard({
                templatePreset: {
                    backend: 'rapid_mlx',
                    temperature: 0.3,
                    top_p: 0.91,
                    top_k: 44,
                    min_p: 0.08,
                    repeat_penalty: 1.09,
                    presence_penalty: 0.2,
                    max_tokens: 3072,
                    seed: 17,
                    rapid_mlx: {
                        model_source: { kind: 'hugging_face_repo', repo_id: 'mlx-community/Qwen', revision: 'v2' },
                        host: '127.0.0.1',
                        port: 9000,
                    },
                },
            });
            const restored = buildSpawnPayload();
            const sampling = {
                temperature: wizardState.hardware.temperature,
                topP: wizardState.hardware.topP,
                topK: wizardState.hardware.topK,
                minP: wizardState.hardware.minP,
                repeatPenalty: wizardState.hardware.repeatPenalty,
                presencePenalty: wizardState.hardware.presencePenalty,
                maxTokens: wizardState.hardware.maxTokens,
                seed: wizardState.hardware.seed,
            };
            closeSpawnWizard();
            openSpawnWizard();
            return {
                restored,
                sampling,
                reset: { selected: wizardState.engine.selected, explicit: wizardState.engine.explicit },
            };
        });

        expect(state.restored.rapid_mlx.model_source).toEqual({
            kind: 'hugging_face_repo',
            repo_id: 'mlx-community/Qwen',
            revision: 'v2',
        });
        expect(state.restored.rapid_mlx.port).toBe(9000);
        expect(state.sampling).toEqual({
            temperature: 0.3,
            topP: 0.91,
            topK: 44,
            minP: 0.08,
            repeatPenalty: 1.09,
            presencePenalty: 0.2,
            maxTokens: 3072,
            seed: 17,
        });
        expect(state.reset).toEqual({ selected: 'llama_cpp', explicit: false });
    });

    test('alias and authoritative Hugging Face sources restore into navigable Rapid-MLX templates', async ({ page }) => {
        await page.route('**/api/llama-binary/platform-info', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ rapid_mlx_local_available: true, auto_backend: 'metal' }),
        }));
        await page.route('**/api/rapid-mlx/runtime/status', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ runtime: { supported: true, active: { version: '0.10.10' } } }),
        }));
        await page.route('**/api/rapid-mlx/recommend', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({
                recommended_backend: 'rapid_mlx',
                state: 'ready',
                reason: 'This source is native to the verified Rapid-MLX resolution path.',
            }),
        }));

        await page.goto('/');
        await page.evaluate(async () => {
            const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
            openSpawnWizard({
                templatePreset: {
                    backend: 'rapid_mlx',
                    rapid_mlx: {
                        model_source: { kind: 'alias', value: 'team/production-model' },
                        host: '127.0.0.1',
                        port: 8001,
                    },
                },
            });
        });
        await expect(page.locator('#spawn-model-path')).toHaveValue('team/production-model');
        await expect(page.locator('#wizard-next-btn')).toBeEnabled();
        await page.locator('#wizard-next-btn').click();
        await expect(page.locator('#wizard-step-2')).toHaveClass(/active/);

        await page.evaluate(async () => {
            const { closeSpawnWizard, openSpawnWizard } = await import('/js/features/spawn-wizard.js');
            closeSpawnWizard();
            openSpawnWizard({
                templatePreset: {
                    backend: 'rapid_mlx',
                    rapid_mlx: {
                        model_source: {
                            kind: 'authoritative_safetensors',
                            source: {
                                kind: 'hugging_face_repo',
                                repo_id: 'owner/source-model',
                                revision: 'release-2',
                            },
                            revision_or_hash: 'release-2',
                            recipe: 'q6',
                        },
                        host: '127.0.0.1',
                        port: 8001,
                    },
                },
            });
        });
        await expect(page.locator('#spawn-hf-repo')).toHaveValue('owner/source-model');
        await expect(page.locator('#wizard-next-btn')).toBeEnabled();
        await page.locator('#wizard-next-btn').click();
        await expect(page.locator('#wizard-step-2')).toHaveClass(/active/);
    });

    test('recommendations select llama.cpp for GGUF and Rapid-MLX for a typed MLX directory', async ({ page }) => {
        await page.route('**/api/llama-binary/platform-info', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ rapid_mlx_local_available: true, auto_backend: 'metal' }),
        }));
        await page.route('**/api/rapid-mlx/runtime/status', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ runtime: { supported: true, active: { version: '0.10.10' } } }),
        }));
        await page.route('**/api/rapid-mlx/recommend', async route => {
            const { artifact_kind: kind } = route.request().postDataJSON();
            await route.fulfill({
                status: 200,
                contentType: 'application/json',
                body: JSON.stringify(kind === 'gguf'
                    ? { recommended_backend: 'llama_cpp', state: 'ready', reason: 'GGUF runs natively with llama.cpp.' }
                    : { recommended_backend: 'rapid_mlx', state: 'ready', reason: 'This source is native to Rapid-MLX.' }),
            });
        });

        await page.goto('/');
        await page.evaluate(async () => {
            const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
            openSpawnWizard({ localPath: '/models/model.gguf' });
        });
        await expect(page.locator('.wizard-engine-card[data-engine="llama_cpp"]')).toHaveClass(/selected/);
        await page.locator('.wizard-engine-card[data-engine="rapid_mlx"]').click();
        await expect(page.locator('#wizard-next-btn')).toBeDisabled();
        await expect(page.locator('#wizard-footer-hint')).toContainText('GGUF runs with llama.cpp');
        await page.locator('.wizard-engine-card[data-engine="llama_cpp"]').click();

        await page.evaluate(async () => {
            const { closeSpawnWizard, openSpawnWizard } = await import('/js/features/spawn-wizard.js');
            closeSpawnWizard();
            openSpawnWizard({
                localPath: '/models/model-mlx',
                localModel: {
                    path: '/models/model-mlx',
                    size_bytes: 6_450_000_000,
                    source_kind: 'mlx_directory',
                    model_source: { kind: 'mlx_directory', path: '/models/model-mlx' },
                },
            });
        });
        await expect(page.locator('.wizard-engine-card[data-engine="rapid_mlx"]')).toHaveClass(/selected/);
        await expect(page.locator('#wizard-engine-reason')).toContainText('native to Rapid-MLX');
        await expect(page.locator('#wizard-binary-prereq')).toBeHidden();
        await expect(page.locator('.model-source-card[data-source="import"]')).toBeHidden();

        await page.locator('.wizard-engine-card[data-engine="llama_cpp"]').click();
        await expect(page.locator('#wizard-next-btn')).toBeDisabled();
        await expect(page.locator('#wizard-footer-hint')).toContainText('requires Rapid-MLX');
        await page.locator('.wizard-engine-card[data-engine="rapid_mlx"]').click();

        await page.locator('#wizard-next-btn').click();
        await expect(page.locator('#rapid-hardware-panel')).toBeVisible();
        await expect(page.locator('#wizard-step-2 > .hw-vram-sidebar')).toBeHidden();
    });

    test('typed Rapid-MLX sources round-trip unchanged and llama.cpp payloads exclude Rapid-MLX config', async ({ page }) => {
        await page.goto('/');
        const result = await page.evaluate(async () => {
            const { buildSpawnPayload, wizardState } = await import('/js/features/spawn-wizard.js');
            const sources = [
                { kind: 'mlx_directory', path: '/models/mlx' },
                { kind: 'hugging_face_repo', repo_id: 'mlx-community/model', revision: 'release-1' },
                { kind: 'alias', value: 'team-model' },
                {
                    kind: 'authoritative_safetensors',
                    source: { kind: 'hugging_face_repo', repo_id: 'owner/source', revision: 'abc123' },
                    revision_or_hash: 'abc123',
                    recipe: 'q6',
                },
            ];
            wizardState.engine.selected = 'rapid_mlx';
            const roundTrips = sources.map(source => {
                wizardState.model.rapidMlxSource = source;
                return buildSpawnPayload().rapid_mlx.model_source;
            });
            wizardState.engine.selected = 'llama_cpp';
            wizardState.model.source = 'local';
            wizardState.model.path = '/models/model.gguf';
            const llama = buildSpawnPayload();
            return { sources, roundTrips, llama };
        });

        expect(result.roundTrips).toEqual(result.sources);
        expect(result.llama.backend).toBe('llama_cpp');
        expect(result.llama).not.toHaveProperty('rapid_mlx');
    });

    test('unsupported Rapid-MLX local launch is visibly blocked with actionable guidance', async ({ page }) => {
        await page.route('**/api/llama-binary/platform-info', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ rapid_mlx_local_available: false, auto_backend: 'cuda' }),
        }));
        await page.route('**/api/rapid-mlx/runtime/status', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ runtime: { supported: false, active: null } }),
        }));
        await page.route('**/api/rapid-mlx/recommend', route => route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({
                recommended_backend: null,
                state: 'platform_unavailable',
                reason: 'Local Rapid-MLX requires Apple Silicon macOS; remote attachment remains available.',
            }),
        }));

        await page.goto('/');
        await page.evaluate(async () => {
            const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
            openSpawnWizard({
                localPath: '/models/model-mlx',
                localModel: { source_kind: 'mlx_directory', path: '/models/model-mlx' },
            });
        });
        await page.locator('.wizard-engine-card[data-engine="rapid_mlx"]').click();

        await expect(page.locator('.wizard-engine-card[data-engine="rapid_mlx"]')).toHaveClass(/is-unavailable/);
        await expect(page.locator('[data-engine-badge="rapid_mlx"]')).toContainText('Apple Silicon only');
        await expect(page.locator('#wizard-engine-reason')).toContainText('remote attachment remains available');
        await expect(page.locator('#wizard-next-btn')).toBeDisabled();
        await expect(page.locator('#wizard-footer-hint')).toContainText('Apple Silicon macOS');
    });
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
        // Scenario cards are rendered via async backend calls; wait for them.
        await expect(page.locator('#vram-scenarios')).toContainText('Reliable agents', { timeout: 8000 });
        await expect(page.locator('#spawn-cache-type-k')).toHaveValue('q8_0');
        await expect(page.locator('#spawn-cache-type-v')).toHaveValue('q8_0');

        const nextBtn = page.locator('#wizard-next-btn');

        // Enable fit — should auto-populate '2048' and keep next enabled
        await page.selectOption('#spawn-fit-enable', 'true');
        await expect(page.locator('#vram-scenarios')).toContainText('Reliable agents', { timeout: 8000 });
        await expect(nextBtn).toBeEnabled();
        await expect(page.locator('#spawn-fit-target')).toHaveValue('2048');

        await page.fill('#spawn-fit-target', '2048');
        await expect(nextBtn).toBeEnabled();

        // Hardware step must still be active and visible after the layout change
        await expect(page.locator('#wizard-step-2')).toHaveClass(/active/);
        await expect(page.locator('#wizard-step-2 > .wizard-main .wizard-section-title', { hasText: 'Configure hardware' })).toBeVisible();
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
        // Scenario cards are rendered via async backend calls; wait for them.
        await expect(page.locator('#vram-scenarios')).toContainText('Reliable agents', { timeout: 8000 });
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
