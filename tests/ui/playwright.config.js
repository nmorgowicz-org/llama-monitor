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
    command: 'cargo run -- --headless --port 7778',
    url: 'http://127.0.0.1:7778',
    reuseExistingServer: true,
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
