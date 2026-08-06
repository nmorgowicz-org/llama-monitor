#!/usr/bin/env node
// Lint the spawn-wizard control-tier registry (plan §3.1/§5 Phase 4 item 5):
//   1. Every Quick-tier control carries a quickValue (I2), except the
//      documented pre-existing exceptions.
//   2. Every registered control id resolves to a real DOM node in
//      static/index.html for at least one loader.
// Usage: node scripts/validate-wizard-groups.mjs

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const root = path.resolve(__dirname, "..");

const { CONTROLS, assertQuickValueCoverage } = await import(
  path.join(root, "static", "js", "features", "spawn-wizard-groups.js")
);

let failed = false;

try {
  assertQuickValueCoverage();
  console.log(`✓ quickValue coverage (I2): ${CONTROLS.length} controls checked`);
} catch (err) {
  console.error(`✗ ${err.message}`);
  failed = true;
}

const html = fs.readFileSync(path.join(root, "static", "index.html"), "utf-8");
const missing = CONTROLS.filter((c) => !html.includes(`id="${c.id}"`));
if (missing.length) {
  console.error(`✗ registry ids missing from index.html: ${missing.map((c) => c.id).join(", ")}`);
  failed = true;
} else {
  console.log(`✓ all ${CONTROLS.length} registry ids resolve in index.html`);
}

if (failed) {
  console.log("");
  console.log("spawn-wizard-groups validation FAILED.");
  process.exit(1);
}

console.log("");
console.log("spawn-wizard-groups validated successfully.");
process.exit(0);
