// Scenario: spawn-wizard-tier-matrix (plan §5 Phase 4b item 3).
// SCENARIO INTENT: Confirm the legacy Quick/Balanced/Advanced disclosure axis
// is retired and capture the single Guided hardware surface.
import { loadAppDocument } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx) {
    const { page, baseUrl } = ctx;
    await page.setViewport({ width: 1280, height: 1400, deviceScaleFactor: 1 });
    // No real file needed: model metadata is injected into wizardState directly
    // below rather than resolved from disk, so this path only needs to look
    // plausible enough to pass the "local" source card's UI wiring.
    const localFixture = '/tmp/capture-fixtures/tier-matrix-capture-fixture.gguf';
    await loadAppDocument(page, baseUrl);

    await page.evaluate(async (localPath) => {
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard({
            localPath,
            localModel: { path: localPath, size_bytes: 4_920_000_000, source_kind: 'gguf_file' },
        });
    }, localFixture);
    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 10000 });
    await page.waitForFunction(
        () => document.getElementById('wizard-step-0')?.classList.contains('active'),
        { timeout: 5000 }
    );

    // Inject metadata so the VRAM bar renders without a real model on disk.
    await page.evaluate(async () => {
        const { wizardState } = await import('/js/features/spawn-wizard.js');
        wizardState.vram.available = 64 * 1024 * 1024 * 1024;
        wizardState.model.paramB = 8;
        wizardState.model.modelBytes = 4_920_000_000;
    });

    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-1')?.classList.contains('active'),
        { timeout: 8000 }
    ).catch(() => console.log('[CAPTURE] Hardware step wait timed out; continuing.'));
    await sleep(400);

    await page.evaluate(async () => {
        const { scheduleVramUpdate } = await import('/js/features/spawn-wizard.js');
        scheduleVramUpdate();
    });
    await sleep(300);

    const state = await page.evaluate(() => ({
        legacyCards: document.querySelectorAll('.profile-card[data-profile]').length,
        view: document.getElementById('view-mode-select')?.value,
    }));
    if (state.legacyCards || state.view !== 'guided') {
        throw new Error(`Legacy disclosure axis still reachable: ${JSON.stringify(state)}`);
    }
    await page.evaluate(() => {
        document.querySelector('.wizard-body')?.scrollTo({ top: 0, behavior: 'instant' });
    });
    await sleep(200);
    await captureShot(page, 'spawn-wizard-tier-matrix-guided.png', {
        fullPage: true,
        runtimeTag: 'llamacpp-local',
        expandSelector: '.wizard-body',
    });

    console.log('[CAPTURE] Scenario "spawn-wizard-tier-matrix" complete.');
}
