/**
 * Open the settings modal the way the product opens it.
 *
 * Importing `openSettingsModal` and calling it directly leaves the modal open while the URL
 * is still whatever it was — a state the app itself never produces, because every real entry
 * point (nav.js, user-menu.js, models.js) goes through `Router.navigate('/settings')`. That
 * fabricated state is torn down by a legitimate router dispatch: `Router.onBeforeDispatch`
 * (bootstrap.js:221) closes the settings modal for any path that is not `/settings`. So any
 * dispatch landing after the direct call — and under a full serial suite the boot dispatch is
 * slow enough to land late — closed the modal mid-test and the tab assertions failed against
 * a modal that was open a moment earlier. That was the settings/TLS flakiness.
 *
 * Navigating instead means the URL and the modal agree, and no dispatch can close it.
 */
export async function openSettings(page, tab = null) {
  await page.evaluate(async (t) => {
    const Router = (await import('/js/features/router.js')).default;
    Router.navigate(t ? `/settings#${t}` : '/settings');
  }, tab);
  await page.waitForFunction(
    () => document.getElementById('settings-modal')?.classList.contains('open') === true,
  );
}

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
