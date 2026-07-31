// tests/ui/core/phase7-presets.spec.js
//
// Phase 7 preset serialization tests (7.5A).
// Verifies concrete Phase 7 Rapid-MLX fields serialize through wizard/preset payloads,
// canonical estimates preserve Rapid prefill vs llama ubatch, and stale keys stay absent.
//
// workload_scenario is deliberately not in that list. It is an estimator input, not a
// launch setting, and `RapidMlxConfig` has no field for it.
//
// These tests work against real endpoints in CI — no fake data needed.

import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

test.describe('Phase 7 preset serialization', () => {
  test('@in-memory-test canonical estimates keep Rapid prefill separate from llama ubatch', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    const bodies = await page.evaluate(async () => {
      const {
        buildEstimateBody,
        rapidEstimatePolicyFromConfig,
        rapidEstimatePolicyFromWizardHardware,
      } = await import('/js/features/vram-estimate.js');
      return {
        rapidFromPreset: buildEstimateBody({
          backend: 'rapid_mlx',
          model_path: 'mlx-community/model',
          ubatch_size: 2048,
          ...rapidEstimatePolicyFromConfig({ prefill_step_size: 1536 }),
        }),
        rapidFromWizard: buildEstimateBody({
          backend: 'rapid_mlx',
          model_path: 'mlx-community/model',
          ...rapidEstimatePolicyFromWizardHardware({ prefillStepSize: 1024 }),
        }),
        llama: buildEstimateBody({
          backend: 'llama_cpp',
          model_path: '/models/model.gguf',
          ubatch_size: 768,
          prefill_step_size: 1536,
        }),
      };
    });

    expect(bodies.rapidFromPreset.prefill_step_size).toBe(1536);
    expect(bodies.rapidFromPreset).not.toHaveProperty('ubatch_size');
    expect(bodies.rapidFromWizard.prefill_step_size).toBe(1024);
    expect(bodies.llama.ubatch_size).toBe(768);
    expect(bodies.llama).not.toHaveProperty('prefill_step_size');
  });

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
      wizardState.hardware.rapidReasoningMode = 'off';
      wizardState.hardware.toolCallParser = 'openai';
      wizardState.hardware.samplingMode = 'explicit_client';
      wizardState.hardware.prefillStepSize = 1536;
      wizardState.hardware.temperature = 0.42;
      wizardState.hardware.autoToolChoice = true;
      wizardState.hardware.speculativeEnabled = true;
      wizardState.hardware.speculativeSource = 'external';
      wizardState.hardware.speculativeModel = 'mlx-community/Qwen3-MTP-sidecar';
      wizardState.hardware.speculativeTokens = 3;
      wizardState.hardware.speculativeDisableAutoK = true;

      return buildSpawnPayload();
    });

    expect(payload.backend).toBe('rapid_mlx');
    expect(payload.rapid_mlx.model_source).toEqual({
      kind: 'hugging_face_repo',
      repo_id: 'mlx-community/Qwen3-0.6B-4bit',
    });
    expect(payload.rapid_mlx).toMatchObject({
      kv_cache_dtype: 'int8',
      reasoning_mode: 'off',
      tool_call_parser: 'openai',
      sampling_mode: 'explicit_client',
      prefill_step_size: 1536,
      default_temperature: 0.42,
      auto_tool_choice: true,
      no_thinking: true,
      speculative_config: {
        method: 'mtp',
        model: 'mlx-community/Qwen3-MTP-sidecar',
        num_speculative_tokens: 3,
        disable_auto_k: true,
      },
    });
    expect(payload.rapid_mlx).not.toHaveProperty('workload_scenario');
  });

  test('@in-memory-test Rapid MTP defaults off and embedded config never invents a model', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    const result = await page.evaluate(async () => {
      const { buildSpawnPayload, wizardState } = await import('/js/features/spawn-wizard.js');
      wizardState.engine.selected = 'rapid_mlx';
      wizardState.model.rapidMlxSource = { kind: 'hugging_face_repo', repo_id: 'mlx-community/model' };
      const off = buildSpawnPayload().rapid_mlx;
      wizardState.hardware.speculativeEnabled = true;
      wizardState.hardware.speculativeSource = 'embedded';
      wizardState.hardware.speculativeModel = 'stale/sidecar';
      const embedded = buildSpawnPayload().rapid_mlx;
      return { off, embedded };
    });
    expect(result.off).not.toHaveProperty('speculative_config');
    expect(result.embedded.speculative_config).toEqual({
      method: 'mtp', num_speculative_tokens: 2, disable_auto_k: false,
    });
  });
});
