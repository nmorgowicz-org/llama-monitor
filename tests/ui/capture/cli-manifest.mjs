// capture:manifest — print filename + INTENT pairs for every screenshot capture
// call site across all scenario files, without launching a browser or server.
// See docs/plans/20260804-branch_audit_capture_split_chat_template_ux.md Phase A4.
//
// INTENT is taken from a `// INTENT: ...` comment on the line(s) immediately
// above the capture call when present, otherwise from the nearest preceding
// comment, otherwise reported as "(unannotated)" so gaps are visible instead
// of hidden.
import fs from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join, relative } from 'path';
import { SCENARIOS } from './index.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SCENARIOS_DIR = join(__dirname, 'scenarios');

const CAPTURE_CALL = /\b(captureShot|captureCloseUp|captureElementScreenshot)\s*\(\s*page\s*,\s*(?:[a-zA-Z0-9_.]+\s*,\s*)?['"`]([^'"`]+)['"`]/;

function walk(dir) {
    const out = [];
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = join(dir, entry.name);
        if (entry.isDirectory()) out.push(...walk(full));
        else if (entry.name.endsWith('.mjs')) out.push(full);
    }
    return out;
}

function intentFor(lines, idx, scenarioIntent) {
    for (let i = idx - 1; i >= Math.max(0, idx - 5); i -= 1) {
        const line = lines[i].trim();
        if (line.startsWith('// INTENT:')) return line.replace('// INTENT:', '').trim();
        if (line.startsWith('//')) return line.replace(/^\/\/\s*/, '').trim();
        if (line !== '') break; // stop at first non-comment, non-blank line
    }
    return scenarioIntent || '(unannotated)';
}

function main() {
    const strict = process.argv.includes('--strict');
    const files = walk(SCENARIOS_DIR);
    let total = 0;
    let annotated = 0;
    const violations = [];
    const seenWizardOutputs = new Map();
    for (const file of files.sort()) {
        const text = fs.readFileSync(file, 'utf8');
        const lines = text.split('\n');
        const scenarioIntent = text.match(/^\/\/ SCENARIO INTENT:\s*(.+)$/m)?.[1] || null;
        const scenarioPath = relative(SCENARIOS_DIR, file);
        const scenarioName = file.slice(file.lastIndexOf('/') + 1, -4);
        const registered = SCENARIOS[scenarioName];
        if (strict && scenarioPath.includes('spawn-wizard')) {
            if (!registered) violations.push(`${scenarioPath} is not registered`);
            const expectedCategory = scenarioPath.startsWith('wizard-rapidmlx/') ? 'wizard-rapidmlx' : 'wizard-llamacpp';
            if (registered && registered.category !== expectedCategory) {
                violations.push(`${scenarioPath} registers category ${registered.category}, expected ${expectedCategory}`);
            }
        }
        const rows = [];
        lines.forEach((line, idx) => {
            const m = CAPTURE_CALL.exec(line);
            if (m) {
                const intent = intentFor(lines, idx, scenarioIntent);
                rows.push({ filename: m[2], intent });
                total += 1;
                if (intent !== '(unannotated)') annotated += 1;
                if (strict && scenarioPath.includes('spawn-wizard') && intent === '(unannotated)') {
                    violations.push(`${scenarioPath}:${idx + 1} missing INTENT for ${m[2]}`);
                }
                if (strict && scenarioPath.includes('spawn-wizard') && !m[2].includes('${')) {
                    const prior = seenWizardOutputs.get(m[2]);
                    if (prior) violations.push(`${scenarioPath}:${idx + 1} duplicates ${m[2]} from ${prior}`);
                    else seenWizardOutputs.set(m[2], `${scenarioPath}:${idx + 1}`);
                }
            }
        });
        if (rows.length === 0) continue;
        console.log(`\n${relative(SCENARIOS_DIR, file)}`);
        for (const row of rows) {
            console.log(`  ${row.filename}\t${row.intent}`);
        }
    }
    console.log(`\n[CAPTURE MANIFEST] ${annotated}/${total} capture call sites have an INTENT comment.`);
    if (strict && violations.length) {
        for (const violation of violations) console.error(`[CAPTURE MANIFEST] ${violation}`);
        process.exitCode = 1;
    }
}

main();
