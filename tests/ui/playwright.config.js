// playwright.config.js
//
// Test tags (informational; do not affect CI execution):
//   @in-memory-test     — runs JS logic against fake state in page.evaluate(); no backend
//   @fake-data-bypass   — uses mocked API responses or injected DOM data
//   @runtime-required   — needs a live model instance; gated behind env flag
//
// LLAMA_MONITOR_HAS_RUNTIME=1 enables tests that require a real model endpoint.
// These tests are NEVER mandatory for CI.
//
// IMPORTANT (NEVER KILL PORT 7778):
//
// When running tests while a live llama-monitor (or AI coding session)
// is using port 7778, DO NOT let Playwright manage or kill that instance.
//
// Instead, use a separate port with LLAMA_MONITOR_TEST_PORT:
//
//   LLAMA_MONITOR_TEST_PORT=17778 npm test
//   LLAMA_MONITOR_TEST_PORT=17778 npx playwright test --workers=1 --retries=2
//
// Or point at an explicitly started test instance:
//
//   LLAMA_MONITOR_UI_URL=http://127.0.0.1:17778 npm test
//
// Defaulting to 7778 is only safe when no other instance is running there.

/** @type {import('@playwright/test').PlaywrightTestConfig} */
const config = {
  testDir: '.',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: process.env.LLAMA_MONITOR_UI_URL ||
              (process.env.LLAMA_MONITOR_TEST_PORT
                 ? `http://127.0.0.1:${process.env.LLAMA_MONITOR_TEST_PORT}`
                 : 'http://127.0.0.1:7778'),
    actionTimeout: 15000,
    screenshot: 'only-on-failure',
    trace: 'on-first-retry',
  },
  webServer: process.env.LLAMA_MONITOR_UI_URL
              ? undefined
              : {
                  // Wrapper script creates a fresh temp config dir so tests run with a clean slate,
                  // independent of the developer's local ~/.config/llama-monitor/ data.
                  // LLAMA_MONITOR_TEST_PORT overrides the port (default 7778); run-server.mjs reads it.
                  command: 'node run-server.mjs',
                  cwd: '.',
                  url: `http://127.0.0.1:${process.env.LLAMA_MONITOR_TEST_PORT || '7778'}`,
                  reuseExistingServer: false,
                  stdout: 'pipe',
                  stderr: 'pipe',
                  timeout: 180000,
                },
  projects: [
    {
      name: 'chromium',
      use: { browserName: 'chromium' },
    }
  ],
};

export default config;
