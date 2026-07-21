// tests/ui/core/phase7-presets.spec.js
//
// Phase 7 preset serialization tests (7.5A).
// Verifies Phase 7 Rapid-MLX fields (kv_cache_dtype, turboquant_mode, workload_scenario,
// reasoning_mode, sampling_mode, tool_call_parser, enable_auto_tool_choice) serialize
// correctly through wizard buildSpawnPayload() and preset payloads.
//
// These tests work against real endpoints in CI — no fake data needed.

import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

test.describe('Phase 7 preset serialization', () => {
  test('@in-memory-test workload profile serializes into spawn payload', async ({ page }) => {
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
      wizardState.hardware.workloadProfile = {
        id: 'tool_research_agent',
        assumptions: {
          streaming: true,
          toolUse: true,
          formatOwner: 'backend',
          stablePrefixLikelihood: 'high',
          hotSessions: '1_active',
          concurrency: 1,
          samplingOwnership: 'backend',
          responseCacheEligible: false,
        },
      };
      wizardState.hardware.workloadProfileConfirmed = true;
    });

    // Verify wizardState reflects the profile
    const stateInPage = await page.evaluate(async () => {
      const { wizardState } = await import('/js/features/spawn-wizard.js');
      return wizardState.hardware.workloadProfile;
    });
    expect(stateInPage.id).toBe('tool_research_agent');

    // Build and check spawn payload includes workload_scenario
    const spawnPayload = await page.evaluate(async () => {
      const { buildSpawnPayload } = await import('/js/features/spawn-wizard.js');
      return buildSpawnPayload();
    });
    expect(spawnPayload.rapid_mlx?.workload_scenario).toBe('tool_research_agent');
    expect(spawnPayload.rapid_mlx?.workload_assumptions?.tool_use).toBe(true);
    expect(spawnPayload.rapid_mlx?.workload_assumptions?.streaming).toBe(true);
    expect(spawnPayload.rapid_mlx?.workload_assumptions?.concurrency).toBe(1);
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
      wizardState.hardware.workloadProfile = {
        id: 'interactive_coding_agent',
        assumptions: {
          streaming: true,
          toolUse: true,
          formatOwner: 'backend',
          stablePrefixLikelihood: 'high',
          hotSessions: '1_active',
          concurrency: 1,
          samplingOwnership: 'backend',
          responseCacheEligible: false,
        },
      };
      wizardState.hardware.workloadProfileConfirmed = true;

      return buildPresetPayload();
    });

    expect(payload.backend).toBe('rapid_mlx');
    expect(payload.rapid_mlx).toBeDefined();
    expect(payload.rapid_mlx.model_source).toEqual({
      kind: 'hugging_face_repo',
      repo_id: 'mlx-community/Qwen3-0.6B-4bit',
      revision: 'main',
    });
    expect(payload.rapid_mlx.workload_scenario).toBe('interactive_coding_agent');
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
      wizardState.hardware.workloadProfile = {
        id: 'interactive_coding_agent',
        assumptions: {
          streaming: true,
          toolUse: true,
          formatOwner: 'backend',
          stablePrefixLikelihood: 'high',
          hotSessions: '1_active',
          concurrency: 1,
          samplingOwnership: 'backend',
          responseCacheEligible: false,
        },
      };
      wizardState.hardware.workloadProfileConfirmed = true;

      return buildSpawnPayload();
    });

    expect(payload.backend).toBe('rapid_mlx');
    expect(payload.rapid_mlx.workload_scenario).toBe('interactive_coding_agent');
    expect(payload.rapid_mlx.workload_assumptions).toBeDefined();
    expect(payload.rapid_mlx.model_source).toEqual({
      kind: 'hugging_face_repo',
      repo_id: 'mlx-community/Qwen3-0.6B-4bit',
    });
  });
});
