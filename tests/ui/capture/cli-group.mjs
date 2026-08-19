// capture:group -- <group> — run every scenario registered under
// scenarios/<group>/ in one invocation. See docs/plans/
// 20260804-branch_audit_capture_split_chat_template_ux.md Phase A4.
//
// Usage: npm run capture:group -- wizard-rapidmlx
//        node capture/cli-group.mjs wizard-rapidmlx [--no-attach ...any other capture flags]
import fs from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { runCli } from './index.mjs';
import { TEMP_APP_CONFIG_DIR } from './harness/paths.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SCENARIOS_DIR = join(__dirname, 'scenarios');

function listGroups() {
    return fs.readdirSync(SCENARIOS_DIR, { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .map((entry) => entry.name)
        .sort();
}

function scenariosInGroup(group) {
    const groupDir = join(SCENARIOS_DIR, group);
    if (!fs.existsSync(groupDir)) return [];
    return fs.readdirSync(groupDir)
        .filter((f) => f.endsWith('.mjs'))
        .map((f) => f.replace(/\.mjs$/, ''))
        .sort();
}

async function main() {
    const argv = process.argv.slice(2);
    const group = argv[0];
    const rest = argv.slice(1);

    if (!group || group === '--help' || group === '-h') {
        console.log('Usage: node capture/cli-group.mjs <group> [capture flags]');
        console.log('Groups:');
        for (const g of listGroups()) {
            console.log(`  ${g}  (${scenariosInGroup(g).join(', ')})`);
        }
        return;
    }

    const scenarios = scenariosInGroup(group);
    if (scenarios.length === 0) {
        throw new Error(`Unknown or empty group "${group}". Known groups: ${listGroups().join(', ')}`);
    }

    console.log(`[CAPTURE GROUP] Running ${scenarios.length} scenario(s) in group "${group}": ${scenarios.join(', ')}`);
    for (const scenario of scenarios) {
        console.log(`\n[CAPTURE GROUP] --- ${scenario} ---`);
        // runCli()'s per-scenario teardown deletes TEMP_HOME (a module-level
        // singleton), so it must be recreated before each scenario when reusing
        // the same process across a whole group.
        fs.mkdirSync(TEMP_APP_CONFIG_DIR, { recursive: true });
        await runCli({ scenario, argv: rest });
    }
}

await main();
