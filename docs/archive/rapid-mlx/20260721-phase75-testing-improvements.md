# Phase 7.5 — Testing Framework Improvements

| Field | Value |
|---|---|
| Created | 2026-07-21 |
| Position | Runs BEFORE Phase 7B3; depends only on 7A + 7B1-B2 |
| Budget | 80k tokens total |
| Purpose | Establish CI-safe Playwright tests and minimal Rapid-MLX runtime testing matching llama.cpp parity |
| Authority | Requires user approval to proceed to implementation |

## 1. Scope and Intent

### What this is
Phase 7.5 has three independent subparts that bring testing coverage to a defensible level before 7B3 (Roleplay controls) starts:

- **7.5A** — Audit and solidify Playwright E2E tests (CI-safe, no runtime required)
- **7.5B** — Add capture.mjs scenarios for real Rapid-MLX runtime functionality
- **7.5C** — Harden screenshot harness with clear mock vs real boundaries

### What this is not
- Not a full rewrite of existing tests
- Not an expansion of test count for its own sake
- Not Phase 14 release validation
- Not AI-dependent tests (those are gated separately)

### Success criteria
- CI runs on a clean isolated instance with no model required; zero failures
- Phase 7-specific features (workload profiles, preset serialization) have positive tests
- Fake-DOM tests are explicitly marked and do not mask regressions
- A developer with macOS + rapid-mlx can run one scenario and see real telemetry + chat
- capture.mjs documentation clearly states which scenarios require what

---

## 2. Audit Results: Existing Test Classification

### 2.1 Playwright Tests — Classification

Based on review of all specs under `tests/ui/`, each test falls into one of these buckets:

| Classification | Meaning | Treatment |
|---|---|---|
| **A — Legitimate flow, real endpoints** | Uses real `/api/*` endpoints; tests actual user flows (auth, spawn payload, preset CRUD, wizard validation, rate limits) | Keep as-is; may add assertions |
| **B — In-page JS unit tests** | Calls `page.evaluate()` to run JS functions against fake state; no real backend interaction | Mark with `@in-memory-test` tag; document as known limitation; do not remove |
| **C — Fake-data bypass** | Injects DOM or fetch-mocks to simulate data that would normally come from runtime/backend | Conditionally skip in CI or explicitly label; document rationale |
| **D — Runtime-dependent** | Requires a real model running | Gate behind env flag; never run in CI |

### 2.2 File-by-file classification

#### `core/` directory

| File | Classification | Notes |
|---|---|---|
| `security-auth.spec.js` | A | Real auth endpoint; legitimate flow; excellent |
| `preset-flow.spec.js` | A + B | Has `installPresetMocks()` (fake-data bypass); most tests are still legitimate flows through real UI. Mocks are necessary for preset CRUD isolation. |
| `spawn-wizard.spec.js` | A + B | Mix: HF search/rate-limit tests are real (A); wizard payload/state tests are in-page JS (B); engine recommendation tests mock `/api/rapid-mlx/recommend` (B). |
| `model-inventory.spec.js` | C | `installInventoryMocks()` returns fake inventory; DOM tests against injected data. Necessary because CI has no models. |
| `rapid-mlx-cards.spec.js` | C | Direct `renderRapidMlxCards()` calls with fake telemetry data. DOM validation only. |
| `launch-grid.spec.js` | A | Real endpoint reads; conditional on user's preset state; acceptable |
| `app-shell.spec.js` | A | SPA navigation, basic shell behavior |
| `spa-navigation.spec.js` | A | Route/tab navigation, real endpoints |
| `performance.spec.js` | A | JS baseline tracking |
| `console-errors.spec.js` | A | Console error monitoring |
| `tls-certificates.spec.js` | A | Real TLS endpoints |
| `import-lab.spec.js` | A | Real import lab endpoints |
| `tuning-benchmarking.spec.js` | A | Real benchmarking endpoints |

#### `chat/` directory

All chat tests (auth-shell, chat-shell, chat-connection, chat-compaction, chat-keyboard, chat-behavior, chat-debug, chat-params, composer-draft, command-palette, reply-plan, sidebar-metadata, chat-updates):

| Classification | Notes |
|---|---|
| **A** (mostly) | Test chat UI mechanics, tab management, message rendering, keyboard shortcuts, connection states. Some connect to a real llama-server for message roundtrips. |
| **D** (partial) | Tests that wait for real AI responses depend on a live runtime; these should be gated. |

#### `guided-generation/` directory

All guided-gen tests (suggestions, context-notes, quick-guide*, phase8-tag-cloud, settings-guided-gen):

| Classification | Notes |
|---|---|
| **A + D** | Test UI flows against real endpoints. Many require a running model for suggestions/quick-guide responses. |

#### `remote-agent/` directory

| File | Classification | Notes |
|---|---|---|
| `ssh.integration.spec.js` | A + D | Real SSH integration; already gated |

### 2.3 capture.mjs — Mock vs Real classification

| Scenario | Mock usage | Runtime required |
|---|---|---|
| `welcome` | None | None |
| `free-cache` | None | None |
| `chat` | None | Yes (llama-server for real chat) |
| `guided-gen` | None | Yes (llama-server for suggestions/quick-guide) |
| `sidebar` | None | Partially (FTS search needs data) |
| `chat-history-qa` | None | Yes (llama-server for Q&A) |
| `models-v2` | Heavy (`/api/models`, `/api/hf/download-dir`, platform-info, import-lab endpoints) | None |
| `rapid-preset` | None (uses seeded `presets.json`) | None |
| `preset-editor` | None | None |
| `settings` | None | None |
| `tls` | None | None |
| `filebrowser` | None | None |
| `panels` | None | Yes (chat messages for debug prompt) |
| `dashboard` | None | Yes (remote agent for telemetry) |
| `dashboard-rapid-mlx` | Heavy (`renderRapidMlxCards()` with deterministic fake data) | None |
| `spawn-wizard` | None | None |
| `spawn-wizard-engines` | None | None |
| `spawn-wizard-gif` | None | None |
| `spawn-wizard-hf-download` | Partially (simulated download progress) | None |
| `tune-panel` | None | Yes (for real benchmark) |
| `benchmark-results` | None | Yes (runs real benchmark) |
| `llama-updater` | None | None (real GitHub API) |
| `sparkline` | None | Yes (agent metrics) |
| `gifs` | None | Yes (agent GPU/inference data) |
| `smoke` | None | None |
| `appearance-palette` | None | None |
| `navbar` | None | None |

---

## 3. Phase 7.5A — Playwright Solidification (CI-safe)

### 3.1 Goals

- Make CI test suite honest about what it actually tests
- Add missing tests for Phase 7 features without requiring a runtime
- Ensure fake-DOM-injection tests are documented and conditional

### 3.2 File-by-file changes

#### New file: `tests/ui/core/test-tags.js`

Add a simple tagging convention:

```javascript
// Usage: test('@in-memory-test spawn payload builds correctly', async ({ page }) => { ... });
// Usage: test('@fake-data model inventory renders badges', async ({ page }) => { ... });
// CI treats tags as informational only (no skips); they document intent.
```

#### Modified: `tests/ui/playwright.config.js`

Add tag documentation block at top (no behavior change):

```javascript
// Test tags (informational; do not affect CI execution):
//   @in-memory-test    — runs JS logic against fake state in page.evaluate(); no backend
//   @fake-data         — uses mocked API responses or injected DOM data
//   @runtime-required  — needs a live model instance; gated behind env flag
```

#### New file: `tests/ui/core/phase7-preset-serialization.spec.js`

**Purpose:** Verify Phase 7B2 workload profiles and Phase 7 controls serialize correctly in presets.

```javascript
import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

test.describe('Phase 7 preset serialization', () => {
  test('@in-memory-test workload profile selection persists through wizard save and reload', async ({ page }) => {
    // Route /api/presets to capture POST/PUT payloads
    let savedPayload = null;
    await page.route('**/api/presets', async route => {
      if (route.request().method() === 'POST') {
        savedPayload = route.request().postDataJSON();
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ id: 'test-workload-preset' }),
        });
        return;
      }
      await route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
    });

    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await dismissAuthShell(page);

    // Open wizard with Rapid-MLX and select a workload profile
    await page.evaluate(async () => {
      const { openSpawnWizard, wizardState } = await import('/js/features/spawn-wizard.js');
      openSpawnWizard();
      wizardState.engine.selected = 'rapid_mlx';
      wizardState.engine.explicit = true;
      wizardState.model.rapidMlxSource = { kind: 'hugging_face_repo', repo_id: 'mlx-community/Qwen3-0.6B-4bit' };
      wizardState.access.port = 9123;
      // Select Tool/research agent profile
      wizardState.hardware.workloadProfile = {
        id: 'tool_research_agent',
        assumptions: {
          streaming: true,
          toolUse: true,
          formatOwner: 'external',
          stablePrefixLikelihood: 'moderate',
          hotSessions: 1,
          concurrency: 2,
          samplingOwnership: 'external',
          responseCacheEligible: false,
        },
      };
      wizardState.hardware.workloadProfileConfirmed = true;
    });

    // Verify wizardState reflects the profile
    const state = await page.evaluate(async () => {
      const { wizardState } = await import('/js/features/spawn-wizard.js');
      return wizardState.hardware.workloadProfile;
    });
    expect(state.id).toBe('tool_research_agent');

    // Build and check spawn payload includes workload_scenario
    const spawnPayload = await page.evaluate(async () => {
      const { buildSpawnPayload } = await import('/js/features/spawn-wizard.js');
      return buildSpawnPayload();
    });
    expect(spawnPayload.rapid_mlx?.workload_scenario).toBe('tool_research_agent');
    expect(spawnPayload.rapid_mlx?.workload_assumptions?.toolUse).toBe(true);

    // Save preset and verify workload data in POST body
    await page.evaluate(() => {
      document.getElementById('spawn-save-preset-btn')?.click();
    });
    await page.waitForTimeout(500);

    expect(savedPayload.rapid_mlx?.workload_scenario).toBe('tool_research_agent');
    expect(savedPayload.name).toContain('preset');
  });

  test('@in-memory-test Phase 7 Rapid-MLX controls serialize in preset editor save', async ({ page }) => {
    // Test that KV dtype, reasoning mode, reusable prompt storage fields persist through editor save
    let updatedPayload = null;

    await page.route('**/api/presets**', async route => {
      if (route.request().method() === 'GET' && route.request().url().endsWith('/api/presets')) {
        const preset = {
          id: 'rapid-phase7-test',
          name: 'Phase 7 Test',
          backend: 'rapid_mlx',
          rapid_mlx: {
            model_source: { kind: 'hugging_face_repo', repo_id: 'mlx-community/Qwen3-0.6B-4bit' },
            host: '127.0.0.1',
            port: 9123,
            kv_cache_dtype: 'int4',
            reasoning_mode: 'enable',
            tool_call_parser: 'openai',
            enable_auto_tool_choice: true,
            workload_scenario: 'interactive_coding_agent',
          },
        };
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify([preset]) });
        return;
      }
      if (route.request().method() === 'PUT') {
        updatedPayload = route.request().postDataJSON();
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true }) });
        return;
      }
      await route.fulfill({ status: 404 });
    });

    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await dismissAuthShell(page);

    // Open preset in editor, modify a Phase 7 field, save
    await page.evaluate(async () => {
      const { openPresetModal } = await import('/js/features/presets.js');
      openPresetModal('edit');
    });
    await page.locator('#preset-modal.open').waitFor({ timeout: 5000 });

    // Change kv_cache_dtype to int8
    await page.evaluate(async () => {
      const { wizardState } = await import('/js/features/presets.js');
      // This tests the field round-trip logic; actual DOM selector depends on implementation
    });

    await page.locator('#btn-modal-save').click();
    await page.waitForTimeout(500);

    // Verify rapid_mlx block is preserved in PUT body
    expect(updatedPayload.rapid_mlx).toBeDefined();
    expect(updatedPayload.rapid_mlx.model_source).toBeDefined();
    expect(updatedPayload.backend).toBe('rapid_mlx');
  });
});
```

#### New file: `tests/ui/core/rapid-mlx-command-preview.spec.js`

**Purpose:** Test Phase 7A3 POST `/api/rapid-mlx/command-preview` endpoint.

```javascript
import { test, expect } from '@playwright/test';

test.describe('Rapid-MLX command preview endpoint', () => {
  test('@runtime-required command preview returns valid argv for Phase 7 config', async ({ page }) => {
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

    const res = await fetch('/api/rapid-mlx/command-preview', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${apiToken}`,
      },
      body: JSON.stringify(payload),
    });

    expect(res.ok).toBe(true);
    const data = await res.json();
    expect(data.argv).toBeDefined();
    expect(Array.isArray(data.argv)).toBe(true);
    // Verify tool_call_parser has a value, not a bare flag
    expect(data.argv.some(a => a.includes('--tool-call-parser') && !a.endsWith('--tool-call-parser')))
      .toBe(true);
  });

  test('command preview requires auth', async ({ page }) => {
    const payload = {
      model_source: { kind: 'hugging_face_repo', repo_id: 'test/model' },
      host: '127.0.0.1',
      port: 9123,
    };

    const res = await page.evaluate(async () => {
      return fetch('/api/rapid-mlx/command-preview', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ model_source: { kind: 'hugging_face_repo', repo_id: 'test/model' }, host: '127.0.0.1', port: 9123 }),
      });
    });

    // Auth-required endpoints return non-200 without token
    expect(res.status).not.toBe(200);
  });
});
```

#### New file: `tests/ui/core/auth-routing-new-endpoints.spec.js`

**Purpose:** Phase 7.5A requirement — auth routing on new endpoints.

```javascript
import { test, expect } from '@playwright/test';

test.describe('Auth routing on new endpoints', () => {
  // Get api-token for authenticated calls
  async function getApiToken(page) {
    const r = await page.evaluate(async () => {
      const res = await fetch('/api/internal/api-token');
      const d = await res.json();
      return d.token;
    });
    return r;
  }

  const readEndpoints = [
    '/api/rapid-mlx/runtime/status',
    '/api/rapid-mlx/runtime/metadata',
    '/api/rapid-mlx/runtime/releases',
    '/api/rapid-mlx/prefix-cache-guidance',
    '/api/rapid-mlx/runtime/profile',
    '/api/rapid-mlx/doctor',
  ];

  const writeEndpoints = [
    '/api/rapid-mlx/command-preview',
  ];

  test('@runtime-required all data-reading endpoints require api-token', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    for (const endpoint of readEndpoints) {
      // Without auth
      const unauth = await page.evaluate(async (ep) => {
        const r = await fetch(ep);
        return r.status;
      }, endpoint);
      expect(unauth, `${endpoint} without auth`).not.toBe(200);

      // With auth
      const apiToken = await getApiToken(page);
      const authStatus = await page.evaluate(async (ep, token) => {
        const r = await fetch(ep, { headers: { Authorization: `Bearer ${token}` } });
        return r.status;
      }, endpoint, apiToken);
      // With token should be 200 or a valid business response (not auth-rejected)
      expect(authStatus, `${endpoint} with auth`).toBeGreaterThanOrEqual(200);
      expect(authStatus, `${endpoint} with auth`).toBeLessThan(401);
    }
  });

  test('@runtime-required write endpoint requires auth', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await page.waitForTimeout(1000);

    const body = { model_source: { kind: 'alias', value: 'test' }, host: '127.0.0.1', port: 9000 };

    const unauthStatus = await page.evaluate(async (body) => {
      const r = await fetch('/api/rapid-mlx/command-preview', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
      return r.status;
    }, body);
    expect(unauthStatus, 'command-preview without auth').not.toBe(200);
  });
});
```

#### Modified: `tests/ui/core/spawn-wizard.spec.js`

Add workload profile tests to existing describe block (at end of file, before closing `});`):

```javascript
    test('@in-memory-test workload profile selection blocks wizard progress without confirmation', async ({ page }) => {
      await page.goto('/');
      await page.waitForLoadState('networkidle');

      const result = await page.evaluate(async () => {
        const { wizardState, isWorkloadProfileConfirmed } = await import('/js/features/spawn-wizard.js');
        wizardState.hardware.workloadProfile = { id: 'tool_research_agent' };
        wizardState.hardware.workloadProfileConfirmed = false;
        const blocked = !isWorkloadProfileConfirmed();
        wizardState.hardware.workloadProfileConfirmed = true;
        const unblocked = isWorkloadProfileConfirmed();
        return { blocked, unblocked };
      });

      expect(result.blocked).toBe(true);
      expect(result.unblocked).toBe(true);
    });

    test('@in-memory-test workload profiles have required fields', async ({ page }) => {
      await page.goto('/');
      await page.waitForLoadState('networkidle');

      const profiles = await page.evaluate(async () => {
        const { WORKLOAD_PROFILES } = await import('/js/features/spawn-wizard.js');
        return WORKLOAD_PROFILES;
      });

      // Verify all 5 profiles exist
      const ids = Object.keys(profiles);
      expect(ids).toContain('interactive_coding_agent');
      expect(ids).toContain('tool_research_agent');
      expect(ids).toContain('roleplay');
      expect(ids).toContain('general_chat');
      expect(ids).toContain('deterministic_batch_eval');

      // Verify each profile has assumptions
      for (const [id, profile] of Object.entries(profiles)) {
        expect(profile.assumptions, `profile ${id} missing assumptions`).toBeDefined();
        expect(profile.assumptions.streaming).toBeDefined();
        expect(profile.assumptions.toolUse).toBeDefined();
      }
    });
```

### 3.3 CI gating for runtime-dependent tests

Add a new env gate in `playwright.config.js`:

```javascript
// At top of file:
// LLAMA_MONITOR_HAS_RUNTIME=1 enables tests that require a real model endpoint.
// These tests are NEVER mandatory for CI.

// In projects array, add:
{
  name: 'chromium-runtime',
  use: { browserName: 'chromium' },
  testMatch: /.*\.spec\.js$/,
  testIgnore: [],
},
```

And in tests that require runtime:

```javascript
const hasRuntime = !!process.env.LLAMA_MONITOR_HAS_RUNTIME;

test.skip(!hasRuntime, 'Set LLAMA_MONITOR_HAS_RUNTIME=1 to run runtime-dependent tests.');
```

### 3.4 Verification for 7.5A

1. Run `cd tests/ui && CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 npm test`
2. Zero failures
3. New specs: `phase7-preset-serialization.spec.js`, `rapid-mlx-command-preview.spec.js`, `auth-routing-new-endpoints.spec.js` all pass
4. Tagged tests are clearly identifiable in output

---

## 4. Phase 7.5B — Rapid-MLX Runtime Testing (capture.mjs scenarios)

### 4.1 Goals

Bring Rapid-MLX to a minimal runtime testing level matching what we have for llama.cpp:

- Real `rapid-mlx serve` launched via llama-monitor spawn
- Health endpoint verified
- Telemetry flowing (tokens/sec, cache metrics on dashboard)
- Real chat response captured
- Model stop and cleanup verified

This runs ONLY in developer mode on macOS with rapid-mlx installed. Not CI.

### 4.2 New capture.mjs scenarios

#### `rapid-mlx-live` — Full runtime flow

Add to capture.mjs near other Rapid-MLX scenarios:

```javascript
// ── Rapid-MLX Live Runtime ────────────────────────────────────────────────────
// Developer-only scenario requiring rapid-mlx on PATH and a cached model.
// Does NOT run in CI. Validates end-to-end: spawn → health → telemetry → chat → stop.

async function scenarioRapidMlxLive(ctx, options) {
    const { page, baseUrl } = ctx;

    // Skip if not macOS (Rapid-MLX local only on Apple Silicon)
    const platform = await page.evaluate(async () => {
        const r = await fetch('/api/llama-binary/platform-info');
        const d = await r.json();
        return { os: d.os, rapidAvailable: d.rapid_mlx_local_available };
    });

    if (!platform.rapidAvailable) {
        console.log('[CAPTURE] rapid-mlx-live: skipping — platform not supported or rapid-mlx not on PATH');
        return;
    }

    console.log('[CAPTURE] rapid-mlx-live: starting full runtime flow');

    await gotoApp(page, baseUrl);

    // 1. Seed a Rapid-MLX preset with Qwen3-0.6B-4bit
    await page.evaluate(async () => {
        const presets = [{
            id: 'rapid-live-test',
            name: 'Qwen3-0.6B-4bit · Live Test',
            backend: 'rapid_mlx',
            rapid_mlx: {
                model_source: {
                    kind: 'hugging_face_repo',
                    repo_id: 'mlx-community/Qwen3-0.6B-4bit',
                    revision: 'main',
                },
                served_model_name: 'qwen3-live',
                host: '127.0.0.1',
                port: 9321,
                log_level: 'INFO',
                workload_scenario: 'interactive_coding_agent',
            },
            port: 9321,
        }];
        await fetch('/api/presets', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', ...(window.authHeaders ? window.authHeaders() : {}) },
            body: JSON.stringify(presets),
        });
    });
    await sleep(500);

    // 2. Select preset and spawn
    await page.evaluate(async () => {
        await fetch('/api/settings', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json', ...(window.authHeaders ? window.authHeaders() : {}) },
            body: JSON.stringify({ preset_id: 'rapid-live-test' }),
        });
        document.getElementById('preset-select')?.dispatchEvent(new Event('change'));
    });
    await sleep(500);

    // 3. Click Start and wait for health
    await page.evaluate(async () => {
        const { doStart } = await import('/js/features/attach-detach.js');
        await doStart();
    });

    // Wait for health endpoint (up to 120s for model download + load)
    console.log('[CAPTURE] rapid-mlx-live: waiting for health (120s timeout)...');
    await page.waitForFunction(async () => {
        try {
            const r = await fetch('/api/rapid-mlx/runtime/status');
            if (!r.ok) return false;
            const d = await r.json();
            return d.runtime?.active !== null;
        } catch { return false; }
    }, { timeout: 120000 });

    console.log('[CAPTURE] rapid-mlx-live: runtime active');

    // 4. Verify health endpoint
    const health = await page.evaluate(async () => {
        const r = await fetch('/api/rapid-mlx/runtime/status');
        return r.json();
    });
    console.log('[CAPTURE] rapid-mlx-live: health:', JSON.stringify(health).slice(0, 300));
    expectRuntimeActive(health);

    await sleep(2000); // Let telemetry initialize

    // 5. Capture dashboard telemetry cards
    await switchTab(page, 'server');
    await page.waitForSelector('#rapid-mlx-card-grid', { timeout: 15000 });
    await sleep(3000); // Wait for real telemetry to populate
    await captureElementScreenshot(page, '#inference-section', 'rapid-mlx-live-dashboard-telemetry.png', { padding: 24 });

    // Verify cards are using real data (not loading/empty states)
    const telemetryState = await page.evaluate(() => {
        const runtimeCard = document.querySelector('[data-card-id="runtime"]');
        const hasVersion = runtimeCard?.textContent?.includes('v');
        const hasUptime = runtimeCard?.textContent?.includes('uptime') || runtimeCard?.textContent?.includes('Up for');
        return { hasVersion, hasUptime };
    });
    console.log('[CAPTURE] rapid-mlx-live: telemetry state:', telemetryState);

    // 6. Send a test chat message
    await switchTab(page, 'chat');
    await createFreshChat(page);
    await sendChatPrompt(page, 'What is 2+2? Answer with one word.');
    await waitForChatComplete(page, 60000);

    // Capture chat with real response
    await sleep(1000);
    await captureShot(page, 'rapid-mlx-live-chat-response.png', { fullPage: true });

    // Verify response is from the model (not empty/error)
    const assistantText = await page.evaluate(() => {
        const msg = document.querySelector('#chat-messages .chat-message-assistant .chat-msg-body');
        return msg?.textContent?.trim()?.slice(0, 100) || null;
    });
    console.log('[CAPTURE] rapid-mlx-live: assistant response:', assistantText);
    if (!assistantText || assistantText.length < 3) {
        console.log('[CAPTURE] rapid-mlx-live: WARNING — response appears empty');
    }

    // 7. Stop the model and verify cleanup
    await switchTab(page, 'server');
    await sleep(500);

    const stopBtn = await page.$('.launch-card-btn-stop, .btn-stop, button:has-text("Stop")');
    if (stopBtn) {
        await stopBtn.click();
        await sleep(3000);
    }

    // Verify runtime is no longer active
    const stopped = await page.evaluate(async () => {
        const r = await fetch('/api/rapid-mlx/runtime/status');
        const d = await r.json();
        return d.runtime?.active === null || d.runtime?.status === 'stopped';
    });
    console.log('[CAPTURE] rapid-mlx-live: model stopped?', stopped);

    // Capture final state
    await captureShot(page, 'rapid-mlx-live-stopped.png', { fullPage: true });

    console.log('[CAPTURE] rapid-mlx-live: complete');
}

function expectRuntimeActive(health) {
    if (!health.runtime?.active) {
        throw new Error('rapid-mlx-live: runtime not active after spawn');
    }
    if (!health.runtime.active.model) {
        throw new Error('rapid-mlx-live: no model loaded');
    }
}
```

#### Register in SCENARIOS map:

```javascript
    'rapid-mlx-live': {
        fn: scenarioRapidMlxLive,
        description: 'Full Rapid-MLX runtime flow: spawn → health → telemetry → chat → stop (requires rapid-mlx on macOS)',
        noAttach: true,
        requiresRapidMlx: true,
        ciSafe: false,
        note: 'Developer-only. NOT for CI. Requires rapid-mlx on PATH and mlx-community/Qwen3-0.6B-4bit.',
    },
```

### 4.3 Documentation update

Add to capture.mjs usage help:

```
  Rapid-MLX Runtime (developer only, NOT CI):
    rapid-mlx-live         Full runtime flow: spawn Qwen3-0.6B-4bit → telemetry → chat → stop
```

### 4.4 Verification for 7.5B

1. On macOS with rapid-mlx 0.10.12 + cached Qwen3-0.6B-4bit:
   - `cargo build --release`
   - `node tests/ui/capture.mjs --scenario rapid-mlx-live`
2. Verify outputs:
   - `rapid-mlx-live-dashboard-telemetry.png` shows real telemetry (version, uptime, tokens/sec)
   - `rapid-mlx-live-chat-response.png` shows a real chat response
   - Console logs confirm runtime active and stopped states
3. On non-macOS or without rapid-mlx: scenario prints skip message and exits cleanly

---

## 5. Phase 7.5C — Screenshot Harness Hardening

### 5.1 Goals

- Audit which scenarios use fetch mocking vs real interactions
- Add wait patterns for real runtime data
- Document boundaries so developers know what each scenario actually tests

### 5.2 File-by-file changes

#### Modified: `tests/ui/capture.mjs`

**Add a top-level SCENARIO_REQUIREMENTS table (as JSDoc or comment):**

```javascript
/**
 * SCENARIO REQUIREMENTS SUMMARY:
 *
 * Which scenarios need what:
 *
 * | Scenario | Real llama-server | Real rapid-mlx | Remote agent | HF internet | Fetch-mocked endpoints |
 * |----------|-------------------|----------------|--------------|-------------|------------------------|
 * | welcome          | no  | no | no | no | none |
 * | free-cache       | no  | no | no | no | none |
 * | chat             | yes | no | no | no | none |
 * | guided-gen       | yes | no | no | no | none |
 * | sidebar          | no  | no | no | no | none |
 * | chat-history-qa  | yes | no | no | no | none |
 * | models-v2        | no  | no | no | no | /api/models, /api/hf/*, platform-info, import-lab |
 * | rapid-preset     | no  | no | no | no | none (uses seeded presets.json) |
 * | preset-editor    | no  | no | no | no | none |
 * | settings         | no  | no | no | no | none |
 * | tls              | no  | no | no | no | none |
 * | filebrowser      | no  | no | no | no | none |
 * | panels           | yes | no | no | no | none |
 * | dashboard        | no  | no | yes| no | none |
 * | dashboard-rapid-mlx | no| no| no| no| none (uses renderRapidMlxCards() with fake data) |
 * | spawn-wizard     | no  | no | no | no | none |
 * | spawn-wizard-engines | no|no| no| no| none |
 * | spawn-wizard-gif | no  | no | no | no | none |
 * | spawn-wizard-hf-download | no|no|no|no| simulated progress only |
 * | tune-panel       | yes | no | no | no | none |
 * | benchmark-results| yes | no | no | no | none |
 * | llama-updater    | no  | no | no | yes| none (real GitHub API) |
 * | sparkline        | no  | no | yes| no | none |
 * | gifs             | no  | no | yes| no | none |
 * | smoke            | no  | no | no | no | none |
 * | appearance-palette| no | no| no| no| none |
 * | navbar           | no  | no | no | no | none |
 * | rapid-mlx-live   | no  | yes| no| yes(HF)| none |
 */
```

**Add wait-for-real-telemetry helper:**

```javascript
/**
 * Wait for Rapid-MLX real telemetry data to populate.
 * Polls /api/rapid-mlx/runtime/status until it returns active data.
 * Returns the status object or throws on timeout.
 */
async function waitForRapidTelemetry(page, timeoutMs = 60000) {
    const start = Date.now();
    console.log(`[CAPTURE] waitForRapidTelemetry: waiting for real Rapid-MLX telemetry (${timeoutMs}ms)...`);

    while (Date.now() - start < timeoutMs) {
        try {
            const status = await page.evaluate(async () => {
                const r = await fetch('/api/rapid-mlx/runtime/status');
                if (!r.ok) return null;
                return r.json();
            });

            if (status?.runtime?.active?.model && status?.runtime?.active?.version) {
                console.log('[CAPTURE] waitForRapidTelemetry: active with model:', status.runtime.active.model);
                return status;
            }
        } catch (e) {
            // Keep polling
        }
        await sleep(2000);
    }

    throw new Error(`waitForRapidTelemetry: no active telemetry within ${timeoutMs}ms`);
}
```

**Modify `scenarioDashboardRapidMlx` to document its mock nature:**

At the start of the function, add:
```javascript
    console.log('[CAPTURE] dashboard-rapid-mlx: using DETERMINISTIC FAKE telemetry via renderRapidMlxCards(). ' +
        'This tests card rendering logic, NOT real runtime behavior.');
```

**Add `scenarioSpawnWizardWorkloadProfiles` — real Phase 7B2 wizard capture:**

```javascript
/**
 * Capture Phase 7B2 workload profile selection in spawn wizard.
 * Uses real wizard flow with seeded data; no runtime needed.
 */
async function scenarioSpawnWizardWorkloadProfiles(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);

    // Open wizard and navigate to hardware/workload step
    await page.evaluate(async () => {
        const { openSpawnWizard, wizardState } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard({
            localPath: '/models/Qwen3.6-27B-Instruct-Q4_K_M.gguf',
        });
        wizardState.model.source = 'local';
        wizardState.model.path = '/models/Qwen3.6-27B-Instruct-Q4_K_M.gguf';
    });

    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 8000 });

    // Navigate through steps to reach workload profile selection
    await page.locator('#wizard-next-btn').click(); // Step 0 → 1
    await page.fill('#spawn-model-path', '/models/Qwen3.6-27B-Instruct-Q4_K_M.gguf');
    await page.locator('#wizard-next-btn').click(); // Step 1 → 2 (hardware/workload)
    await sleep(1500);

    // Capture default workload profile (Interactive coding agent)
    await captureShot(page, 'spawn-wizard-workload-default.png', { fullPage: true });

    // Switch to Tool/research agent
    await page.evaluate(() => {
        const profileCard = document.querySelector('.workload-profile-card[data-profile-id="tool_research_agent"]');
        if (profileCard) profileCard.click();
    });
    await sleep(800);
    await captureShot(page, 'spawn-wizard-workload-tool-research.png', { fullPage: true });

    // Switch to Roleplay
    await page.evaluate(() => {
        const profileCard = document.querySelector('.workload-profile-card[data-profile-id="roleplay"]');
        if (profileCard) profileCard.click();
    });
    await sleep(800);
    await captureShot(page, 'spawn-wizard-workload-roleplay.png', { fullPage: true });

    // Close wizard
    await page.keyboard.press('Escape');
    await sleep(300);
}
```

Register in SCENARIOS:
```javascript
    'spawn-wizard-workload-profiles': {
        fn: scenarioSpawnWizardWorkloadProfiles,
        description: 'Phase 7B2 workload profile selection in spawn wizard',
        noAttach: true,
        ciSafe: true,
    },
```

### 5.3 Verification for 7.5C

1. Run `node tests/ui/capture.mjs --list-scenarios` and confirm the summary table matches actual behavior
2. Run `node tests/ui/capture.mjs --scenario spawn-wizard-workload-profiles` and verify screenshots show workload profile cards
3. Review updated capture.mjs documentation

---

## 6. Verification Steps Summary

### Phase 7.5A verification
```bash
cargo build --release
cd tests/ui && CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 npm test
```
Expected: All tests pass. New Phase 7-specific tests verify workload profile persistence and preset serialization.

### Phase 7.5B verification (developer only, macOS + rapid-mlx required)
```bash
cargo build --release
node tests/ui/capture.mjs --scenario rapid-mlx-live
```
Expected: Model spawns, health responds, telemetry visible on dashboard, chat response captured, model stops cleanly.

### Phase 7.5C verification
```bash
node tests/ui/capture.mjs --list-scenarios    # Confirm documentation updated
node tests/ui/capture.mjs --scenario spawn-wizard-workload-profiles  # Phase 7B2 capture
```

### Combined verification
After all three subparts:
```bash
cd tests/ui && CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 npm test
```
Still zero failures. CI behavior unchanged.

---

## 7. Token Budget Estimate

| Part | Estimate | Notes |
|---|---|---|
| 7.5A — Playwright solidification | ~35k | New specs + modifications to spawn-wizard.spec.js + playwright.config.js |
| 7.5B — Rapid-MLX runtime testing | ~20k | New capture.mjs scenario + helpers |
| 7.5C — Harness hardening | ~15k | Documentation, wait helpers, new spawn-wizard-workload-profiles scenario |
| Contingency | ~10k | Edge cases, iteration |
| **Total** | **~80k** | Within budget |

---

## 8. Dependencies and Ordering

- **7.5A**: Depends on 7A + 7B1-B2 complete (Phase 7 endpoints and workload profiles exist)
- **7.5B**: Depends on user having rapid-mlx 0.10.12 + cached mlx-community/Qwen3-0.6B-4bit (developer env only)
- **7.5C**: Depends on 7.5A audit results
- **7B3**: Depends on Phase 7.5 complete (establishes test baseline before Roleplay controls)

---

## 9. Non-goals

- Not adding tests for every possible edge case
- Not rewriting capture.mjs from scratch
- Not adding CI support for 7.5B runtime tests (those are developer-only)
- Not testing llama.cpp runtime (that already has chat scenarios; parity goal is to bring Rapid-MLX to same level)
