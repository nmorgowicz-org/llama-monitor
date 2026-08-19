// tests/ui/core/test-tags.js
//
// Tagging convention for Playwright E2E tests.
//
// Add tags as a leading prefix in test names:
//
//   test('@in-memory-test spawn payload builds correctly', async ({ page }) => { ... });
//   test('@fake-data-bypass model inventory renders badges', async ({ page }) => { ... });
//   test('@runtime-required chat roundtrip completes', async ({ page }) => { ... });
//
// Tags are informational only — they do not affect CI execution (no auto-skips).
// Runtime-dependent tests MUST use test.skip(!hasRuntime, ...) explicitly.
//
// See playwright.config.js for tag definitions and CI behavior.
