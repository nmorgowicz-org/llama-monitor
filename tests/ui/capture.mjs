// Backward-compat shim: tests/ui/capture.mjs was split into tests/ui/capture/
// (index.mjs + harness/*.mjs + scenarios/**/*.mjs) per
// docs/plans/20260804-branch_audit_capture_split_chat_template_ux.md Phase A.
//
// This file is kept so existing invocations (`node tests/ui/capture.mjs ...`,
// the `capture:*` npm scripts, and any external tooling) keep working
// unchanged. Do not add new logic here — add it under tests/ui/capture/.
//
// Retire this shim in Phase A5, once Phase C has exercised the new layout.
export { runCli } from './capture/index.mjs';

import { runCli } from './capture/index.mjs';

if (import.meta.url === `file://${process.argv[1]}`) {
    await runCli();
}
