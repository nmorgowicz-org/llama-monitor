// ── Console Error Detection ───────────────────────────────────────────────────
// Catch JavaScript errors in the browser console before they reach users.
// Run with: npx playwright test console-errors.spec.js

import { test, expect } from '@playwright/test';

test.describe('console error detection', () => {
  let consoleErrors = [];

  test.beforeEach(async ({ page }) => {
    consoleErrors = [];
    
    // Capture all console errors and warnings
    page.on('console', msg => {
      const type = msg.type();
      const text = msg.text();
      
      // Track errors and warnings
      if (type === 'error' || type === 'warning') {
        consoleErrors.push({ type, text, url: msg.location() });
      }
    });
    
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
  });

  test('no fatal JavaScript errors', async () => {
    // Filter out expected warnings (e.g., service worker registration failures, CSP for external resources)
    const fatalErrors = consoleErrors.filter(e => {
      // Ignore these common non-fatal messages:
      const ignored = [
        'Unsupported site:', // content.js check
        'Fetch API', // Service worker external requests (expected)
        'content-security-policy', // External CDN CSP (expected for external resources)
      ];
      
      return !ignored.some(pattern => e.text.includes(pattern)) &&
             e.type === 'error' &&
             !e.text.includes('Failed to load resource');
    });

    if (fatalErrors.length > 0) {
      console.log('=== Console Errors Detected ===');
      fatalErrors.forEach(e => {
        console.log(`[${e.type}] ${e.text}`);
        if (e.url) {
          console.log(`  at ${e.url.url || 'unknown'}`);
        }
      });
    }

    expect(fatalErrors.length).toBe(0);
  });

  test('no assignment to constant errors', async () => {
    const assignmentErrors = consoleErrors.filter(e => 
      e.text.includes('Assignment to constant variable') ||
      e.text.includes('Cannot assign to read only property')
    );

    expect(assignmentErrors.length).toBe(0);
  });

  test('no undefined function errors', async () => {
    const undefinedErrors = consoleErrors.filter(e => 
      e.text.includes('is not a function') ||
      e.text.includes('Cannot read property')
    );

    expect(undefinedErrors.length).toBe(0);
  });
});
