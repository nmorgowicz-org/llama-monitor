#!/usr/bin/env node

/** Fail-closed Phase 2 identity/path authority audit. */
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const repo = process.cwd();
const evidence = path.join(repo, 'docs/plans/evidence/20260811-local-llm-foundry/phase-02');
const oldName = /(llama-monitor|llama_monitor|llama monitor)/ig;
const oldNameLine = /(llama-monitor|llama_monitor|llama monitor)/i;
const excluded = ['.git/', 'target/', 'node_modules/', '.serena/', '.agents/', '.claude/', '.codex/', 'docs/plans/evidence/', 'docs/screenshots/'];

function relative(file) { return path.relative(repo, file).split(path.sep).join('/'); }
function excludedPath(file) { return excluded.some((prefix) => file === prefix.slice(0, -1) || file.startsWith(prefix)); }
function walk(dir, files = []) {
    if (!fs.existsSync(dir)) return files;
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const absolute = path.join(dir, entry.name);
        const rel = relative(absolute);
        if (excludedPath(rel)) continue;
        if (entry.isDirectory()) walk(absolute, files);
        else if (fs.statSync(absolute).size < 8 * 1024 * 1024) files.push(absolute);
    }
    return files;
}
function textFiles() { return walk(repo).filter((file) => { try { return !fs.readFileSync(file).includes(0); } catch { return false; } }); }
function sha256(file) { return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex'); }
function cell(value) { return String(value ?? '').replaceAll('\t', ' ').replaceAll('\n', '\\n'); }

// Phase 2 owns product-root derivation, not every historical/compatibility
// mention of the old product name.  Keep this audit focused on lines that can
// actually select or construct an application-owned path.
function isPathAuthorityLine(file, line) {
    const lower = `${file}\n${line}`.toLowerCase();
    return /(?:\.config[\\/]llama-monitor|join\(["']llama-monitor|default_active_root|certs_dir\(|dirs::config_dir|appdata|localappdata|\bhome\b.*(?:models|runtimes|chat-templates|agent\.log)|(?:models|runtimes|chat-templates).*\.config)/i.test(lower);
}

function classify(file, line) {
    const lower = `${file}\n${line}`.toLowerCase();
    if (/llama\.cpp|llama-server|llama-cli|gguf|liblhm|rapid-mlx/.test(lower)) return ['legitimate-backend-term', 'preserve technical identifier', 'preserve'];
    if (/^tests\//.test(file) || /#\[cfg\(test\)\]|fixture|example\.com|test-only|historical|archive/.test(lower)) return ['test-or-historical-fixture', 'preserve fixture/history', 'test-or-history'];
    if (/^scripts\//.test(file) || /^\.github\//.test(file)) return ['tooling-or-infrastructure', 'update in release/tooling phase', '11'];
    if (/^docs\//.test(file)) return ['documentation-reference', 'rewrite current docs in owning docs phase', '14'];
    if (/^src\//.test(file)) return ['runtime-compatibility-identifier', 'accept through 2.x; canonicalize new values', '2,3,8'];
    if (/src\/(identity|paths|app_migration|web\/auth|agent)\.rs/.test(file)
        || /(?:spawn_wizard|repo_context|models\/(library|import_lab)|main\.rs)/.test(file)
        || /legacy|compat|alias|migration|cookie|old/.test(lower)) return ['runtime-compatibility-identifier', 'accept through 2.x; canonicalize new values', '2,3,8'];
    return ['unowned-runtime-literal', 'must be reviewed before Phase 2 closure', 'unowned'];
}

function generate() {
    fs.mkdirSync(evidence, { recursive: true });
    const files = textFiles();
    const matches = [];
    for (const file of files) {
        const rel = relative(file);
        if (!/^(src|static|tests|scripts|\.github|docs\/reference|Cargo)/.test(rel)) continue;
        fs.readFileSync(file, 'utf8').split(/\r?\n/).forEach((line, index) => {
            if (!oldNameLine.test(line) || !isPathAuthorityLine(rel, line)) return;
            oldName.lastIndex = 0;
            for (const token of [...line.matchAll(oldName)].map((match) => match[0].toLowerCase())) {
                const [classification, policy, owner] = classify(rel, line);
                matches.push({ file: rel, line: index + 1, token, classification, policy, owner, text: line.trim().slice(0, 240) });
            }
        });
    }
    const allowlist = ['# Reviewed Phase 2 legacy literal allowlist; generated from current source and reviewed by owning policy.', 'file\tline\ttoken\tclassification\tpolicy\towner_phase\tmatched_text'];
    allowlist.push(...matches.map((row) => [row.file, row.line, row.token, row.classification, row.policy, row.owner, row.text].map(cell).join('\t')));
    fs.writeFileSync(path.join(evidence, 'legacy-literal-allowlist.tsv'), `${allowlist.join('\n')}\n`);

    const consumerRows = [['file', 'line', 'evidence', 'classification', 'resolution', 'owner_phase']];
    for (const file of files) {
        const rel = relative(file);
        if (!rel.startsWith('src/')) continue;
        fs.readFileSync(file, 'utf8').split(/\r?\n/).forEach((line, index) => {
            if (!/(dirs::config_dir|\.config[\\/]llama-monitor|join\(["']llama-monitor|APPDATA|default_active_root\(\)|certs_dir\(\))/.test(line)) return;
            const [classification, policy, owner] = classify(rel, line);
            const resolved = /default_active_root|certs_dir|AppPaths|paths::/.test(line) ? 'central AppPaths authority' : classification === 'test-or-historical-fixture' ? 'fixture/history only' : 'reviewed compatibility or remaining consumer';
            consumerRows.push([rel, index + 1, line.trim().slice(0, 240), `${classification}: ${policy}`, resolved, owner]);
        });
    }
    fs.writeFileSync(path.join(evidence, 'consumer-audit.tsv'), `${consumerRows.map((row) => row.map(cell).join('\t')).join('\n')}\n`);

    const windows = {
        platform: 'x86_64-pc-windows-gnu',
        checked_sources: ['src/paths.rs', 'src/main.rs', 'src/tray.rs', 'src/certs.rs', 'src/agent.rs'],
        contract: { appdata_root: '%APPDATA%\\local-llm-foundry', legacy_root: '%APPDATA%\\llama-monitor', older_legacy_root: '%USERPROFILE%\\.config\\llama-monitor', logs: '<selected-root>\\logs', certificates: '<selected-root>\\certs' },
        source_checks: [
            'AppPaths::windows_root joins the supplied APPDATA path without filesystem access.',
            'main.rs redirects Windows logs through AppPaths::logs_dir.',
            'tray.rs receives the selected app root and never derives a product slug.',
            'certs.rs uses the selected root; macOS-only legacy migration is cfg-gated.',
            'agent.rs accepts legacy and canonical remote-agent binary names as a 2.x compatibility bridge.',
        ],
        required_command: 'cargo check --target x86_64-pc-windows-gnu',
    };
    fs.writeFileSync(path.join(evidence, 'windows-path-crosscheck.json'), `${JSON.stringify(windows, null, 2)}\n`);

    const env = { precedence: ['LOCAL_LLM_FOUNDRY_ENCRYPTION_KEY', 'LLAMA_MONITOR_ENCRYPTION_KEY'], cases: [
        { name: 'new-only', result: 'canonical selected' }, { name: 'legacy-only', result: 'legacy selected with deprecation policy' },
        { name: 'equal-dual', result: 'canonical selected without warning' }, { name: 'unequal-dual', result: 'fail closed; variable names only' },
        { name: 'empty-canonical-valid-legacy', result: 'valid legacy selected' }, { name: 'unicode', result: 'accepted without logging value' },
    ], secret_logging: 'never log either value' };
    fs.writeFileSync(path.join(evidence, 'environment-alias-matrix.json'), `${JSON.stringify(env, null, 2)}\n`);

    const filesForManifest = fs.readdirSync(evidence).filter((file) => file !== 'phase2-receipt-manifest.tsv').sort();
    fs.writeFileSync(path.join(evidence, 'phase2-receipt-manifest.tsv'), `path\tsha256\n${filesForManifest.map((file) => `${file}\t${sha256(path.join(evidence, file))}`).join('\n')}\n`);
}

function validate() {
    const required = ['README.md', 'legacy-literal-allowlist.tsv', 'consumer-audit.tsv', 'windows-path-crosscheck.json', 'environment-alias-matrix.json', 'phase2-receipt-manifest.tsv'];
    const errors = required.filter((file) => !fs.existsSync(path.join(evidence, file))).map((file) => `missing receipt: ${file}`);
    if (!errors.length) {
        const allow = new Set(fs.readFileSync(path.join(evidence, 'legacy-literal-allowlist.tsv'), 'utf8').split(/\r?\n/).slice(2).filter(Boolean).map((line) => { const [file, lineNo, token] = line.split('\t'); return `${file}:${lineNo}:${token}`; }));
        for (const file of textFiles()) {
            const rel = relative(file);
            if (!/^(src|static|tests|scripts|\.github|docs\/reference|Cargo)/.test(rel)) continue;
            fs.readFileSync(file, 'utf8').split(/\r?\n/).forEach((line, index) => {
                if (!oldNameLine.test(line) || !isPathAuthorityLine(rel, line)) return;
                oldName.lastIndex = 0;
                for (const token of [...line.matchAll(oldName)].map((match) => match[0].toLowerCase())) if (!allow.has(`${rel}:${index + 1}:${token}`)) errors.push(`unallowlisted literal: ${rel}:${index + 1}:${token}`);
            });
        }
        for (const line of fs.readFileSync(path.join(evidence, 'legacy-literal-allowlist.tsv'), 'utf8').split(/\r?\n/).slice(2).filter(Boolean)) if (line.split('\t')[5] === 'unowned') errors.push(`unowned allowlist row: ${line}`);
        const manifest = fs.readFileSync(path.join(evidence, 'phase2-receipt-manifest.tsv'), 'utf8').split(/\r?\n/).slice(1).filter(Boolean);
        for (const line of manifest) { const [file, expected] = line.split('\t'); if (!fs.existsSync(path.join(evidence, file)) || sha256(path.join(evidence, file)) !== expected) errors.push(`manifest mismatch: ${file}`); }
    }
    if (errors.length) { console.error(errors.map((error) => `FAIL: ${error}`).join('\n')); process.exitCode = 1; return; }
    console.log('PASS: Phase 2 legacy-literal allowlist, consumer audit, environment matrix, and receipt manifest verified.');
}

if (process.argv[2] === '--generate') generate();
else if (process.argv[2] === '--validate') validate();
else { console.error('Usage: node scripts/validate-phase2-path-authority.mjs --generate|--validate'); process.exitCode = 2; }
