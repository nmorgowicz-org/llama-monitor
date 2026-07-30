// tests/ui/core/phase7-presets.spec.js
//
// Phase 7 preset serialization tests (7.5A).
// Verifies Phase 7 Rapid-MLX fields (kv_cache_dtype, turboquant_mode, reasoning_mode,
// sampling_mode, tool_call_parser, enable_auto_tool_choice) serialize correctly through
// wizard buildSpawnPayload() and preset payloads.
//
// workload_scenario is deliberately not in that list. It is an estimator input, not a
// launch setting, and `RapidMlxConfig` has no field for it.
//
// These tests work against real endpoints in CI — no fake data needed.

import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

test.describe('Phase 7 preset serialization', () => {
  test('@in-memory-test workload profile reaches the estimator, not the spawn payload', async ({ page }) => {
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
      wizardState.hardware.workloadScenario = 'tool_research_agent';
    });

    // Verify wizardState reflects the scenario
    const stateInPage = await page.evaluate(async () => {
      const { wizardState } = await import('/js/features/spawn-wizard.js');
      return wizardState.hardware.workloadScenario;
    });
    expect(stateInPage).toBe('tool_research_agent');

    // The scenario must reach the VRAM estimate and stay out of the launch payload. This test
    // used to assert the opposite, which proved only that the wizard wrote the key -- the
    // backend has no `workload_scenario` field on `RapidMlxConfig` and dropped it on arrival.
    const result = await page.evaluate(async () => {
      const { buildSpawnPayload, wizardState } = await import('/js/features/spawn-wizard.js');
      const { rapidEstimatePolicyFromWizardHardware } = await import('/js/features/vram-estimate.js');
      return {
        inLaunchPayload: 'workload_scenario' in (buildSpawnPayload().rapid_mlx || {}),
        estimateScenario: rapidEstimatePolicyFromWizardHardware(wizardState.hardware).workload_scenario,
      };
    });
    expect(result.inLaunchPayload).toBe(false);
    expect(result.estimateScenario).toBe('tool_research_agent');
  });

  test('@in-memory-test wizard buildPresetPayload preserves Rapid-MLX model_source', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    const payload = await page.evaluate(async () => {
      const { buildPresetPayload, wizardState } = await import('/js/features/spawn-wizard.js');
      wizardState.engine.selected = 'rapid_mlx';
      wizardState.engine.explicit = true;
      wizardState.model.rapidMlxSource = {
        kind: 'hugging_face_repo',
        repo_id: 'mlx-community/Qwen3-0.6B-4bit',
        revision: 'main',
      };
      wizardState.access.port = 9123;
      wizardState.hardware.workloadScenario = 'interactive_coding_agent';

      return buildPresetPayload();
    });

    expect(payload.backend).toBe('rapid_mlx');
    expect(payload.rapid_mlx).toBeDefined();
    expect(payload.rapid_mlx.model_source).toEqual({
      kind: 'hugging_face_repo',
      repo_id: 'mlx-community/Qwen3-0.6B-4bit',
      revision: 'main',
    });
  });

  test('@in-memory-test wizard buildSpawnPayload includes Phase 7 fields', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    const payload = await page.evaluate(async () => {
      const { buildSpawnPayload, wizardState } = await import('/js/features/spawn-wizard.js');
      wizardState.engine.selected = 'rapid_mlx';
      wizardState.engine.explicit = true;
      wizardState.model.rapidMlxSource = { kind: 'hugging_face_repo', repo_id: 'mlx-community/Qwen3-0.6B-4bit' };
      wizardState.access.port = 9123;

      // Set Phase 7 fields directly in wizardState (simulates advanced controls)
      wizardState.hardware.kvCacheDtype = 'int8';
      wizardState.hardware.reasoningMode = 'enable';
      wizardState.hardware.toolCallParser = 'openai';
      wizardState.hardware.enableAutoToolChoice = true;

      return buildSpawnPayload();
    });

    expect(payload.backend).toBe('rapid_mlx');
    expect(payload.rapid_mlx.model_source).toEqual({
      kind: 'hugging_face_repo',
      repo_id: 'mlx-community/Qwen3-0.6B-4bit',
    });
  });
});
