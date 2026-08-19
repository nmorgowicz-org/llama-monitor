// Scenario: spawn-wizard-mmproj-selection
// INTENT: Capture the hardware vision/mmproj selector with typed family-backed recommendation evidence.
import { loadAppDocument } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx) {
    const { page, baseUrl } = ctx;
    await page.setViewport({ width: 1280, height: 1400, deviceScaleFactor: 1 });
    const localFixture = '/tmp/capture-fixtures/mmproj-selection-capture-fixture.gguf';
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

    await page.evaluate(async () => {
        const { wizardState } = await import('/js/features/spawn-wizard.js');
        wizardState.model.family = 'qwen3.5';
        wizardState.model.paramB = 8;
        wizardState.model.modelBytes = 4_920_000_000;
        wizardState.model.mmprojFiles = [
            {
                path: '/tmp/capture-fixtures/mmproj-selection-capture-fixture-mmproj-f16.gguf',
                size: 420_000_000,
                is_mmproj: true,
                label: 'F16',
                mmproj_recommendation: 'F16 projector selected from GGUF family metadata',
            },
            {
                path: '/tmp/capture-fixtures/mmproj-selection-capture-fixture-mmproj-q8_0.gguf',
                size: 220_000_000,
                is_mmproj: true,
                label: 'Q8_0',
            },
        ];
        wizardState.vram.available = 64 * 1024 * 1024 * 1024;
    });

    await page.evaluate(async () => {
        const { showStep, scheduleVramUpdate } = await import('/js/features/spawn-wizard.js');
        const { renderMmprojSection } = await import('/js/features/spawn-wizard-mmproj.js');
        showStep(1);
        renderMmprojSection();
        scheduleVramUpdate();
    });
    await page.waitForFunction(
        () => document.getElementById('wizard-step-1')?.classList.contains('active'),
        { timeout: 8000 }
    );
    await sleep(600);
    await page.evaluate(() => {
        document.getElementById('wizard-binary-prereq')?.style.setProperty('display', 'none');
        document.querySelector('.wizard-body')?.scrollTo({ top: 0, behavior: 'instant' });
    });
    await sleep(300);

    // INTENT: Capture the hardware vision/mmproj selector with the recommended F16 projector selected.
    await captureShot(page, 'spawn-wizard-mmproj-selection-vision.png', {
        fullPage: true,
        runtimeTag: 'llamacpp-local',
        expandSelector: '.wizard-body',
    });
}
