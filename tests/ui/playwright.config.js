// playwright.config.js

/** @type {import('@playwright/test').PlaywrightTestConfig} */
const config = {
  testDir: '.',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: process.env.LLAMA_MONITOR_UI_URL || 'http://127.0.0.1:7778',
    screenshot: 'only-on-failure',
    trace: 'on-first-retry',
  },
  webServer: process.env.LLAMA_MONITOR_UI_URL ? undefined : {
    // Wrapper script creates a fresh temp config dir so tests run with a clean slate,
    // independent of the developer's local ~/.config/llama-monitor/ data.
    command: './run-server.sh',
    cwd: '.',
    url: 'http://127.0.0.1:7778',
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
