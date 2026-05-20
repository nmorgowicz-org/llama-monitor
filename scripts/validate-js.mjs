#!/usr/bin/env node
// Validate all JavaScript files for syntax errors.
// Cross-platform replacement for scripts/validate-js.sh.
// Usage: node scripts/validate-js.mjs

import fs from "fs";
import os from "os";
import path from "path";
import { fileURLToPath } from "url";
import { execSync } from "child_process";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const root = path.resolve(__dirname, "..");
const jsDir = path.join(root, "static", "js");

function walkJs(dir) {
  const out = [];
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...walkJs(full));
    } else if (entry.isFile() && entry.name.endsWith(".js")) {
      out.push(full);
    }
  }
  return out;
}

const files = walkJs(jsDir).sort();
let failed = false;

console.log("Validating JavaScript files...");

for (const file of files) {
  const rel = path.relative(root, file);
  const tmp = path.join(os.tmpdir(), "test-esm.mjs");
  fs.writeFileSync(tmp, fs.readFileSync(file, "utf-8"), "utf-8");

  try {
    execSync(`node --check "${tmp}"`, { stdio: "pipe" });
    console.log(`✓ ${rel}`);
  } catch (err) {
    console.error(`✗ ${rel} - SYNTAX ERROR`);
    const stderr = (err.stderr || "").toString();
    const stdout = (err.stdout || "").toString();
    const lines = (stderr || stdout).split("\n").slice(0, 5);
    for (const line of lines) {
      console.error(line);
    }
    failed = true;
  } finally {
    try {
      fs.unlinkSync(tmp);
    } catch {
      // ignore cleanup errors
    }
  }
}

if (failed) {
  console.log("");
  console.log(
    "JavaScript validation FAILED. Fix syntax errors before committing."
  );
  process.exit(1);
}

console.log("");
console.log("All JavaScript files validated successfully.");
process.exit(0);
