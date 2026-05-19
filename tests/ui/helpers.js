import { expect } from '@playwright/test';

/**
 * Dismiss the auth shell if it is visible (caused by security-auth.spec.js
 * enabling form auth on the shared server while other tests run concurrently).
 * Uses the auth API directly via fetch inside the page for reliability.
 */
export async function dismissAuthShell(page) {
  const authShell = page.locator('#auth-shell');
  const isVisible = await authShell.isVisible().catch(() => false);
  if (!isVisible) return;

  // Check if auth is actually configured
  const authEnabled = await page.evaluate(async () => {
    try {
      const res = await fetch('/api/auth/status');
      const data = await res.json();
      return data.enabled === true;
    } catch {
      return false;
    }
  });

  if (!authEnabled) {
    // Auth not configured but shell is showing — dismiss via DOM
    await page.evaluate(() => {
      const shell = document.getElementById('auth-shell');
      if (shell) shell.setAttribute('aria-hidden', 'true');
    });
    return;
  }

  const candidates = [
    { username: 'admin', password: 'secret1234' },
    { username: 'admin', password: 'secret123' },
  ];

  for (const cred of candidates) {
    const loginOk = await page.evaluate(
      async (creds) => {
        try {
          const res = await fetch('/api/auth/login', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username: creds.u, password: creds.p }),
          });
          return res.status === 200;
        } catch {
          return false;
        }
      },
      { u: cred.username, p: cred.password },
    );

    if (loginOk) {
      // Login succeeded — wait for the shell to disappear, then reload
      try {
        await authShell.waitFor({ state: 'hidden', timeout: 5000 });
      } catch {
        // Shell may not hide automatically; force reload
      }
      await page.reload();
      await page.waitForSelector('html.modules-ready', { timeout: 15000 });
      return;
    }
  }

  // All credentials failed — try to dismiss via DOM as fallback
  await page.evaluate(() => {
    const shell = document.getElementById('auth-shell');
    if (shell) shell.setAttribute('aria-hidden', 'true');
    const backdrop = document.querySelector('.auth-shell-backdrop');
    if (backdrop) backdrop.remove();
  });
}
