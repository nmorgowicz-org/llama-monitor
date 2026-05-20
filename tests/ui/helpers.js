/**
 * Dismiss the auth shell if it is visible (caused by security-auth.spec.js
 * enabling form auth on the shared server while other tests run concurrently).
 * Uses the auth API directly via fetch inside the page for reliability.
 */
export async function dismissAuthShell(page) {
  const authShell = page.locator('#auth-shell');

  // Check if auth shell element exists and is visible
  const shellExists = await authShell.count();
  if (shellExists === 0) return;

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
    // Auth not configured but shell is showing — dismiss aggressively
    await page.evaluate(() => {
      const shell = document.getElementById('auth-shell');
      if (shell) {
        shell.setAttribute('aria-hidden', 'true');
        shell.style.display = 'none';
        shell.style.pointerEvents = 'none';
      }
      const backdrop = document.querySelector('.auth-shell-backdrop');
      if (backdrop) {
        backdrop.style.display = 'none';
        backdrop.style.pointerEvents = 'none';
      }
    });
    return;
  }

  // Auth is enabled — try to log in
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
        // Shell may not hide automatically
      }
      await page.reload();
      await page.waitForSelector('html.modules-ready', { timeout: 15000 });
      // After reload, check again in case auth shell reappears
      const stillVisible = await authShell.isVisible().catch(() => false);
      if (stillVisible) {
        await page.evaluate(() => {
          const shell = document.getElementById('auth-shell');
          if (shell) {
            shell.style.display = 'none';
            shell.style.pointerEvents = 'none';
          }
          const backdrop = document.querySelector('.auth-shell-backdrop');
          if (backdrop) {
            backdrop.style.display = 'none';
            backdrop.style.pointerEvents = 'none';
          }
        });
      }
      return;
    }
  }

  // All credentials failed — dismiss aggressively via DOM
  await page.evaluate(() => {
    const shell = document.getElementById('auth-shell');
    if (shell) {
      shell.setAttribute('aria-hidden', 'true');
      shell.style.display = 'none';
      shell.style.pointerEvents = 'none';
    }
    const backdrop = document.querySelector('.auth-shell-backdrop');
    if (backdrop) {
      backdrop.style.display = 'none';
      backdrop.style.pointerEvents = 'none';
    }
  });
}
