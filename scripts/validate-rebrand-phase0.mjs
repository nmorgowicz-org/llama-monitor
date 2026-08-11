#!/usr/bin/env node

/**
 * Generate and validate the Phase 0 rebrand receipts.
 *
 * The generator intentionally records metadata only for live user roots. It
 * never reads file contents below those roots, follows symlinks, or emits
 * secret/token values. The validator is fail-closed: a new old-name match is
 * an error until it has an explicit classification row.
 */

import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const repo = process.cwd();
const evidence = path.join(repo, 'docs/plans/evidence/20260811-local-llm-foundry/phase-00');
const self = path.relative(repo, new URL(import.meta.url).pathname);
const oldNameRe = /llama-monitor|llama_monitor|llama monitor/ig;
const oldNameLineRe = /(llama-monitor|llama_monitor|llama monitor)/i;

const excluded = [
    '.git/', 'target/', 'node_modules/', '.serena/', '.agents/', '.claude/', '.codex/',
    'docs/plans/evidence/', 'docs/screenshots/',
    'scripts/validate-rebrand-phase0.mjs',
];

function rel(p) {
    return path.relative(repo, p).split(path.sep).join('/');
}

function isExcluded(relative) {
    return excluded.some((prefix) => relative === prefix.slice(0, -1) || relative.startsWith(prefix));
}

function walk(dir, out = []) {
    if (!fs.existsSync(dir)) return out;
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const absolute = path.join(dir, entry.name);
        const relative = rel(absolute);
        if (isExcluded(relative)) continue;
        if (entry.isDirectory()) walk(absolute, out);
        else out.push(absolute);
    }
    return out;
}

function textFiles() {
    return walk(repo).filter((file) => {
        try {
            if (fs.statSync(file).size > 8 * 1024 * 1024) return false;
            const data = fs.readFileSync(file);
            return !data.includes(0);
        } catch {
            return false;
        }
    });
}

function sha256(data) {
    return crypto.createHash('sha256').update(data).digest('hex');
}

function safeCell(value) {
    return String(value ?? '').replaceAll('\t', ' ').replaceAll('\r', '').replaceAll('\n', '\\n');
}

function redactResourcePath(relativePath) {
    const structural = new Set([
        '.', '..', 'models', 'cache', 'hf', 'rapid-mlx', 'bin', '.staging',
        'certs', 'backups', 'auto', 'chat.db', 'chat.db-wal', 'chat.db-shm',
        'api-token', 'db-admin-token', 'settings', 'sessions', 'presets',
        'templates', 'gpu', 'agent', '.DS_Store',
    ]);
    return relativePath.split('/').map((component) => {
        if (structural.has(component)) return component;
        const digest = sha256(component).slice(0, 12);
        const extension = path.extname(component);
        return extension ? `<redacted-${digest}>${extension}` : `<redacted-${digest}>`;
    }).join('/');
}

const commandLog = [];
function run(command, args) {
    const result = spawnSync(command, args, { cwd: repo, encoding: 'utf8' });
    commandLog.push({
        command: [command, ...args].join(' '),
        exit_code: result.status ?? 1,
        stdout: result.stdout ?? '',
        stderr: result.stderr ?? '',
    });
    return result;
}

function classifyMatch(file, line) {
    const lower = `${file}\n${line}`.toLowerCase();
    if (/(llama\.cpp|llama-server|llama-cli|llama-bench|llama_context|llama_model|llama_batch|llama_sampler|llama_backend|libllama|libllm|liblhm)/i.test(line)
        || file.startsWith('src/llama/')) {
        return ['legitimate-technology', 'preserve-technical-name', 'preserve'];
    }
    if (/historical|changelog|release note|benchmark receipt|archive|prior release/i.test(lower)
        || file.startsWith('docs/plans/evidence/')) {
        return ['historical-record', 'do-not-rewrite-history', '0'];
    }
    if (/compat|legacy|alias|migration|cookie|storage|fixture|backward|old name/i.test(lower)
        || file.startsWith('tests/')) {
        return ['compatibility-identifier', 'accept-through-2.x', '2,3,8'];
    }
    if (file.startsWith('.github/')) return ['infrastructure-only', 'update-release-infrastructure', '10,11'];
    if (file.startsWith('scripts/')) return ['infrastructure-only', 'update-release-tooling', '11'];
    if (file.startsWith('docs/')) return ['public-brand', 'rewrite-current-documentation', '14'];
    if (file.startsWith('static/') || file.startsWith('assets/') || file === 'Cargo.toml' || file === 'Cargo.lock') {
        return ['public-brand', 'rename-current-product-surface', '1,7'];
    }
    return ['public-brand', 'rename-current-product-surface', '7'];
}

function scanOldNameMatches(files = textFiles()) {
    const rows = [];
    for (const file of files) {
        const relative = rel(file);
        const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
        lines.forEach((line, index) => {
            if (!oldNameLineRe.test(line)) return;
            oldNameRe.lastIndex = 0;
            const tokens = [...line.matchAll(oldNameRe)].map((match) => match[0].toLowerCase());
            for (const token of tokens) {
                const [classification, policy, owner] = classifyMatch(relative, line);
                rows.push({ file: relative, line: index + 1, token, classification, policy, owner, text: line.trim().slice(0, 240) });
            }
        });
    }
    return rows;
}

function write(file, content) {
    fs.writeFileSync(path.join(evidence, file), content);
}

function inventoryRoot(root, label) {
    const result = { label, path: label, exists: fs.existsSync(root), content_read: false, entries: [] };
    if (!result.exists) return result;
    function visit(directory, relative = '.') {
        let entries;
        try { entries = fs.readdirSync(directory, { withFileTypes: true }); } catch (error) {
            result.entries.push({ relative_path: relative, type: 'unreadable', error: error.code ?? 'unknown' });
            return;
        }
        for (const entry of entries) {
            const absolute = path.join(directory, entry.name);
            const child = relative === '.' ? entry.name : `${relative}/${entry.name}`;
            let stat;
            try { stat = fs.lstatSync(absolute); } catch (error) {
                result.entries.push({ relative_path: child, type: 'unreadable', error: error.code ?? 'unknown' });
                continue;
            }
            const symlink = stat.isSymbolicLink();
            const lower = child.toLowerCase();
            let classification = 'unknown-user-or-app-data';
            let policy = 'retain-until-explicit-user-migration';
            let owner = '5,6';
            if (/^(chat\.db|chat\.db-(wal|shm)|api-token|db-admin-token|settings|sessions|presets|templates|gpu|certs|agent)/i.test(child)
                || /\.(json|db|db-wal|db-shm|toml|yaml|yml|pem|key)$/i.test(child)) {
                classification = 'app-state-or-secret';
                policy = 'copy-first; never read secrets; sqlite via ChatStorage::backup';
                owner = '4,5';
            } else if (/^(models|cache|hf|rapid-mlx|bin|\.staging)(\/|$)/i.test(child)) {
                classification = 'managed-model-or-runtime-tree';
                policy = 'retain until explicit follow-up migration; never delete external roots';
                owner = '5,6';
            }
            if (symlink) {
                classification = 'symlink-or-reparse-entry';
                policy = 'refuse traversal; report and retain';
                owner = '3,4';
            }
            result.entries.push({
                relative_path: redactResourcePath(child),
                type: stat.isDirectory() ? 'directory' : stat.isFile() ? 'file' : 'special',
                mode: (stat.mode & 0o7777).toString(8).padStart(4, '0'),
                size_bytes: stat.isFile() ? stat.size : 0,
                symlink,
                classification,
                policy,
                owner_phase: owner,
            });
            if (stat.isDirectory() && !symlink) visit(absolute, child);
        }
    }
    visit(root);
    return result;
}

function parseApiMatrix() {
    const rows = [];
    for (const file of walk(path.join(repo, 'src/web/api')).filter((item) => item.endsWith('.rs'))) {
        const relative = rel(file);
        const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
        lines.forEach((line, index) => {
            if (!line.includes('warp::path!')) return;
            const nextPath = lines.slice(index + 1).findIndex((candidate) => candidate.includes('warp::path!'));
            const end = nextPath === -1 ? lines.length : index + 1 + nextPath;
            const block = lines.slice(index, end).join('\n');
            const macro = block.match(/warp::path!\(([^)]*)\)/s);
            if (!macro) return;
            const pathMatch = [...macro[1].matchAll(/"([^"]+)"|\bString\b/g)];
            const pieces = [];
            for (const match of pathMatch) {
                if (match[1]) pieces.push(match[1]);
                else pieces.push(':param');
            }
            if (pieces.length === 0 || pieces[0] !== 'api') return;
            const route = `/${pieces.join('/')}`;
            const methods = [...block.matchAll(/warp::(get|post|put|delete|patch|options)\(\)/g)].map((match) => match[1].toUpperCase());
            const auth = route.startsWith('/api/auth/')
                ? 'public-form-auth'
                : route.startsWith('/api/internal/') || route === '/api/db/admin-token'
                    ? 'loopback-or-authenticated'
                    : block.includes('check_db_admin_token') ? 'db-admin-token'
                        : block.includes('check_api_token') || block.includes('bearer_matches_api_token') ? 'api-token'
                            : block.includes('authenticate_request') ? 'form-auth-or-api-token' : 'source-declared';
            rows.push({ route, method: methods.length ? [...new Set(methods)].join('|') : 'source-declared', auth, source: `${relative}:${index + 1}`, compatibility: 'route-stable-through-2.x' });
        });
    }
    return rows.sort((a, b) => `${a.route}\0${a.method}`.localeCompare(`${b.route}\0${b.method}`));
}

function parseScreenshots() {
    const index = fs.readFileSync(path.join(repo, 'tests/ui/capture/index.mjs'), 'utf8');
    const scenarios = [];
    for (const match of index.matchAll(/import\s+(\w+)\s+from\s+'\.\/scenarios\/([^']+)'/g)) {
        const [, symbol, scenarioPath] = match;
        const [group, file] = scenarioPath.split('/');
        const name = file.replace(/\.mjs$/, '');
        let surface = 'application-surface';
        if (/welcome|navbar|sidebar|smoke/i.test(name)) surface = 'identity-and-navigation';
        else if (/auth|login/i.test(name)) surface = 'authentication-shell';
        else if (/wizard/i.test(name) || group.startsWith('wizard')) surface = 'spawn-wizard';
        else if (/model|browser|filebrowser/i.test(name)) surface = 'model-library';
        else if (/preset|community|discussion|evidence/i.test(name)) surface = 'presets';
        else if (/dashboard|benchmark|tune|rapid/i.test(name)) surface = 'dashboard-and-runtime';
        scenarios.push({ scenario: name, group, source: `tests/ui/capture/scenarios/${scenarioPath}`, symbol, viewport: { width: 1440, height: 900 }, theme: 'dark-and-light-where-scenario-defines', target_surface: surface });
    }
    return scenarios;
}

function generate() {
    fs.mkdirSync(evidence, { recursive: true });
    const checkout = run('git', ['rev-parse', 'HEAD']).stdout.trim();
    const timestamp = run('date', ['-u', '+%Y-%m-%dT%H:%M:%SZ']).stdout.trim();
    const allTextFiles = textFiles();
    const groups = [
        ['rust-and-package', 'Llama Monitor|llama-monitor|llama_monitor', ['src', 'Cargo.toml', 'Cargo.lock']],
        ['frontend-and-assets', 'Llama Monitor|llama-monitor|llama_monitor', ['static', 'assets']],
        ['tests', 'Llama Monitor|llama-monitor|llama_monitor', ['tests']],
        ['documentation', 'Llama Monitor|llama-monitor|llama_monitor', ['README.md', 'docs']],
        ['workflows-and-scripts', 'Llama Monitor|llama-monitor|llama_monitor', ['.github', 'scripts']],
        ['environment-variables-and-roots', 'LLAMA_MONITOR|LOCAL_LLM_FOUNDRY|APPDATA|HOME|\\.config/llama-monitor', ['.']],
        ['urls-and-repositories', 'github\\.com/[^[:space:]]*llama-monitor|llama-monitor\\.(com|org|io)', ['.']],
        ['cookies-browser-keys', 'cookie|localStorage|sessionStorage|llama-monitor-chat|llama_monitor_', ['static', 'tests', 'docs']],
        ['tasks-services-processes', 'launchd|systemd|service|task scheduler|process|llama-monitor', ['.github', 'scripts', 'src', 'docs']],
        ['generated-assets-and-filenames', 'Llama Monitor|llama-monitor|llama_monitor', ['src/gen', 'build.rs', 'static', 'assets']],
    ];
    const source = [`Phase 0 source inventory; capture ${timestamp}; baseline checkout ${checkout}`, 'Pattern: Llama Monitor | llama-monitor | llama_monitor (case-insensitive)', 'Excluded: .git, target, phase-00 evidence, screenshot artifacts, this validator', ''];
    for (const [name, pattern, paths] of groups) {
        const args = ['-n', '-i', pattern, '--hidden', '--glob', '!.git/**', '--glob', '!target/**', '--glob', '!node_modules/**', '--glob', '!.serena/**', '--glob', '!.agents/**', '--glob', '!.claude/**', '--glob', '!.codex/**', '--glob', '!docs/plans/evidence/**', '--glob', '!docs/screenshots/**', '--glob', `!${self}`, ...paths];
        const result = run('rg', args);
        source.push(`===== ${name} =====`, `COMMAND: rg ${args.join(' ')}`, `EXIT: ${result.status ?? 1}`, result.stdout.trimEnd() || '(no matches)', '');
    }
    const filenameArgs = ['--files', '--hidden', '--glob', '!.git/**', '--glob', '!target/**', '--glob', '!node_modules/**', '--glob', '!.serena/**', '--glob', '!.agents/**', '--glob', '!.claude/**', '--glob', '!.codex/**', '--glob', '!docs/plans/evidence/**', '--glob', '!docs/screenshots/**', '--glob', `!${self}`];
    const filenameResult = run('rg', filenameArgs);
    const filenameMatches = filenameResult.stdout.split(/\r?\n/).filter((item) => oldNameLineRe.test(item));
    source.push('===== filenames =====', `COMMAND: rg ${filenameArgs.join(' ')}`, `EXIT: ${filenameResult.status ?? 1}`, filenameMatches.join('\n') || '(no matching filenames)', '');
    write('source-inventory.txt', `${source.join('\n')}\n`);

    const matches = scanOldNameMatches(allTextFiles);
    const classificationHeader = 'file\tline\ttoken\tclassification\tpolicy\towner_phase\tmatched_text';
    write('classification.tsv', `${classificationHeader}\n${matches.map((row) => [row.file, row.line, row.token, row.classification, row.policy, row.owner, row.text].map(safeCell).join('\t')).join('\n')}\n`);

    const pathRows = [['literal', 'file', 'line', 'platform', 'ownership', 'new_value', 'compatibility', 'owner_phase']];
    for (const file of allTextFiles) {
        const relative = rel(file);
        if (!/^(src|static|tests|scripts|\.github|Cargo|README|docs)/.test(relative)) continue;
        fs.readFileSync(file, 'utf8').split(/\r?\n/).forEach((line, index) => {
            if (!/(dirs::|APPDATA|HOME|\.config\/llama-monitor|llama-monitor|local-llm-foundry)/i.test(line)) return;
            const literal = line.match(/(?:dirs::\w+|APPDATA|HOME|(?:~|%APPDATA%)[^\s"'`]*llama-monitor[^\s"'`]*)/i)?.[0] ?? 'path-consumer';
            const platform = /APPDATA|windows/i.test(line) ? 'windows' : /macos|darwin|Library/i.test(line) ? 'macos' : 'unix-cross-platform';
            const ownership = /test|fixture/i.test(relative) ? 'test-fixture' : 'application';
            pathRows.push([literal, relative, index + 1, platform, ownership, 'AppPaths/local-llm-foundry', 'legacy value retained where compatibility requires', '2,4,5,6,7,9,11,12']);
        });
    }
    write('path-consumers.tsv', `${pathRows.map((row) => row.map(safeCell).join('\t')).join('\n')}\n`);

    const identity = {
        schema_version: 1,
        capture: { timestamp_utc: timestamp, baseline_checkout: checkout, source: 'Phase 0 discovery; no secrets read' },
        mappings: [
            { surface: 'product', legacy: 'Llama Monitor', canonical: 'Local LLM Foundry', classification: 'public-brand', owner_phase: '1,7,14' },
            { surface: 'slug', legacy: 'llama-monitor', canonical: 'local-llm-foundry', classification: 'public-brand', owner_phase: '1,7,11' },
            { surface: 'rust-library', legacy: 'llama_monitor', canonical: 'llama_monitor', classification: 'compatibility-identifier', owner_phase: '2,8', policy: 'retain internal crate namespace through 2.x' },
            { surface: 'unix-root', legacy: '~/.config/llama-monitor', canonical: '~/.config/local-llm-foundry', classification: 'public-brand', owner_phase: '4,5' },
            { surface: 'windows-root', legacy: '%APPDATA%\\llama-monitor', canonical: '%APPDATA%\\local-llm-foundry', classification: 'public-brand', owner_phase: '4,9,12' },
            { surface: 'runtime-technology', legacy: 'llama.cpp / llama-server', canonical: 'unchanged', classification: 'legitimate-technology', owner_phase: 'preserve' },
        ],
        compatibility: { browser_storage: 'stable-through-2.x', api_routes: 'stable-through-2.x', legacy_artifacts: 'dual-name-bridge-through-2.0.x', canonical_only: '2.1.0' },
    };
    write('identity-contract.json', `${JSON.stringify(identity, null, 2)}\n`);

    const home = os.homedir();
    const roots = [
        inventoryRoot(path.join(home, '.config/llama-monitor'), 'legacy-unix-config-root'),
        inventoryRoot(path.join(home, 'Library/Application Support/llama-monitor'), 'legacy-macos-application-support-root'),
    ];
    const resource = {
        schema_version: 1,
        capture: { timestamp_utc: timestamp, baseline_checkout: checkout, content_read: false, symlinks_followed: false },
        roots,
        windows_fixtures: {
            available: false,
            note: 'No checked-in Windows fixture existed at capture; synthetic cases below are the required Phase 0 contract for Phase 3 tests.',
            entries: [
                { relative_path: 'chat.db', classification: 'app-state-or-secret', policy: 'ChatStorage::backup' },
                { relative_path: 'chat.db-wal', classification: 'sqlite-sidecar', policy: 'checkpoint/backup; never raw-copy' },
                { relative_path: 'models/.staging/model.part', classification: 'partial-download', policy: 'retain and reconcile during explicit migration' },
                { relative_path: 'models/link-out', classification: 'symlink-or-reparse-entry', policy: 'refuse traversal' },
                { relative_path: 'unknown-user-file.bin', classification: 'unknown-user-or-app-data', policy: 'retain; require explicit decision' },
            ],
        },
    };
    write('resource-inventory.json', `${JSON.stringify(resource)}\n`);

    const apiRows = parseApiMatrix();
    write('api-auth-matrix.tsv', `route\tmethod\tauth_level\tsource\tcompatibility\n${apiRows.map((row) => [row.route, row.method, row.auth, row.source, row.compatibility].map(safeCell).join('\t')).join('\n')}\n`);

    const browserKeys = [
        'csp-collapsed', 'llama-monitor-chat-font', 'llama-monitor-chat-style', 'llama-monitor-chat-telemetry-pinned', 'llama-monitor-date-format', 'llama-monitor-enter-to-send', 'llama-monitor-gpu-viz', 'llama-monitor-group-by-family', 'llama-monitor-last-endpoint', 'llama-monitor-last-session', 'llama-monitor-preferences', 'llama-monitor-preset-sort', 'llama-monitor-previous-position', 'llama-monitor-system-viz', 'llama_monitor_context_notes_intro_hidden', 'llama_monitor_sidebar_expanded', 'sidebarCollapsed', 'spawn_wizard_tips_collapsed', 'suggestions_custom_categories', 'update-dismissed', 'wizard_view_mode', 'llama_monitor_sidebar_width', 'llama-monitor-notifications', 'appNavWidth', 'llama-monitor-chat-focus-mode', 'llama-monitor-log-font-size', 'llama-monitor-log-tail-enabled', 'llama-monitor-log-tail-lines', 'llama-monitor-models-prefs', 'template_autoupdater_lastCheck', 'template_autoupdater_busy', 'template_autoupdater_lastStatus',
    ];
    const browserRows = [['key', 'producer', 'consumer', 'storage', 'migration']];
    for (const key of browserKeys) {
        const hits = allTextFiles.filter((file) => fs.readFileSync(file, 'utf8').includes(key)).map(rel);
        browserRows.push([key, hits.join(','), hits.join(','), key.includes('wizard_view_mode') ? 'sessionStorage-or-localStorage' : 'localStorage', 'preserve exact key through 2.x; do not silently rename']);
    }
    write('browser-storage-inventory.tsv', `${browserRows.map((row) => row.map(safeCell).join('\t')).join('\n')}\n`);

    const releaseRows = [
        ['surface', 'current/legacy identifier', 'canonical 2.0 identifier', 'consumer', 'bridge policy', 'owner_phase'],
        ['cargo-package', 'llama-monitor', 'local-llm-foundry', 'Cargo/build/release-please', 'single package rename at coordinated cutover', '7,11'],
        ['primary-binary', 'llama-monitor[.exe]', 'local-llm-foundry[.exe]', 'updater/launcher/users', 'canonical plus legacy launcher through 2.x', '7,8,11'],
        ['archive-assets', 'llama-monitor-*', 'local-llm-foundry-*', 'release workflows/checksums', 'publish both names in 2.0.x; canonical only 2.1.0', '11'],
        ['updater-parser', 'legacy artifact names', 'canonical artifact names', 'src/agent.rs/self-update', 'accept both names through 2.x; canonical preferred', '8,11'],
        ['repository-url', 'nmorgowicz-org/llama-monitor', 'nmorgowicz-org/local-llm-foundry', 'README/docs/workflows', 'update after remote rename; preserve redirect', '11,14'],
        ['pwa-manifest', 'Llama Monitor / llama-monitor', 'Local LLM Foundry / local-llm-foundry', 'browser install/update', 'canonical identity; old API/storage keys stable', '1,7'],
    ];
    write('release-surface-inventory.tsv', `${releaseRows.map((row) => row.map(safeCell).join('\t')).join('\n')}\n`);

    const assetRows = [['asset', 'kind', 'current_role', 'classification', 'owner_phase', 'policy']];
    for (const file of [...walk(path.join(repo, 'assets')), ...walk(path.join(repo, 'static')), ...walk(path.join(repo, 'docs/brand'))]) {
        const relative = rel(file);
        if (!/\.(svg|png|ico|webp|jpg|jpeg)$/i.test(relative)) continue;
        const isTokenIngot = /token-ingot|static\/icon\.svg/i.test(relative);
        const isConcept = relative.startsWith('docs/brand/concepts/');
        assetRows.push([
            relative,
            path.extname(relative).slice(1).toLowerCase(),
            isConcept ? 'direction/reference artwork' : isTokenIngot ? 'selected brand mark' : 'functional or legacy UI mark',
            isTokenIngot ? 'approved-token-ingot' : isConcept ? 'concept-reference' : 'unrelated-functional-mark-or-legacy-mark',
            isTokenIngot ? '1' : isConcept ? '1' : '1,7',
            isTokenIngot ? 'deterministic SVG; derivative proofs required' : isConcept ? 'retain as historical/reference; do not ship as master' : 'review during frontend identity pass; preserve functional icons',
        ]);
    }
    write('brand-asset-inventory.tsv', `${assetRows.map((row) => row.map(safeCell).join('\t')).join('\n')}\n`);

    write('screenshot-scenario-manifest.json', `${JSON.stringify({ schema_version: 1, capture: { timestamp_utc: timestamp, source: 'tests/ui/capture/index.mjs', default_viewport: { width: 1440, height: 900 } }, scenarios: parseScreenshots() }, null, 2)}\n`);

    const inventorySummary = roots.map((root) => `${root.label}: exists=${root.exists}, entries=${root.entries.length}`).join('\n');
    commandLog.push({ command: 'filesystem-lstat (metadata-only resource inventory)', exit_code: 0, stdout: `${inventorySummary}\n`, stderr: '' });
    write('raw-inventory-commands.log', commandLog.map((entry) => [
        `COMMAND: ${entry.command}`,
        `EXIT: ${entry.exit_code}`,
        '--- STDOUT ---', entry.stdout.trimEnd(),
        '--- STDERR ---', entry.stderr.trimEnd(),
        '--- END ---', '',
    ].join('\n')).join('\n'));

    const files = fs.readdirSync(evidence).filter((file) => file !== 'produced-file-manifest.tsv').sort();
    write('produced-file-manifest.tsv', `path\tsha256\n${files.map((file) => `${file}\t${sha256(fs.readFileSync(path.join(evidence, file)))}`).join('\n')}\n`);
}

function validate() {
    const required = [
        'README.md', 'source-inventory.txt', 'path-consumers.tsv', 'identity-contract.json',
        'resource-inventory.json', 'api-auth-matrix.tsv', 'browser-storage-inventory.tsv',
        'release-surface-inventory.tsv', 'screenshot-scenario-manifest.json',
        'brand-asset-inventory.tsv', 'raw-inventory-commands.log', 'classification.tsv', 'validation-result.txt', 'produced-file-manifest.tsv',
    ];
    const errors = [];
    for (const file of required) {
        if (!fs.existsSync(path.join(evidence, file)) || fs.statSync(path.join(evidence, file)).size === 0) errors.push(`missing or empty receipt: ${file}`);
    }
    if (errors.length === 0) {
        const lines = fs.readFileSync(path.join(evidence, 'classification.tsv'), 'utf8').trimEnd().split(/\r?\n/).slice(1);
        const classified = new Set(lines.map((line) => {
            const [file, lineNumber, token] = line.split('\t');
            return `${file}:${lineNumber}:${token}`;
        }));
        for (const row of scanOldNameMatches()) {
            const key = `${row.file}:${row.line}:${row.token}`;
            if (!classified.has(key)) errors.push(`unclassified old-name match: ${key}`);
        }
        for (const line of lines) {
            const fields = line.split('\t');
            if (fields.length < 7 || !fields[3] || fields[3] === 'unclassified' || !fields[4] || !fields[5]) errors.push(`invalid classification row: ${line}`);
        }
        const manifest = fs.readFileSync(path.join(evidence, 'produced-file-manifest.tsv'), 'utf8').trimEnd().split(/\r?\n/).slice(1);
        const listed = new Set();
        for (const line of manifest) {
            const [file, expected] = line.split('\t');
            listed.add(file);
            const actualPath = path.join(evidence, file);
            if (!fs.existsSync(actualPath)) errors.push(`manifest file missing: ${file}`);
            else if (sha256(fs.readFileSync(actualPath)) !== expected) errors.push(`manifest hash mismatch: ${file}`);
        }
        for (const file of fs.readdirSync(evidence)) {
            if (file !== 'produced-file-manifest.tsv' && !listed.has(file)) errors.push(`unlisted produced file: ${file}`);
        }
        try {
            const resource = JSON.parse(fs.readFileSync(path.join(evidence, 'resource-inventory.json')));
            for (const root of resource.roots) for (const entry of root.entries) {
                if (!entry.classification || !entry.policy || !entry.owner_phase) errors.push(`unowned resource entry: ${root.label}/${entry.relative_path}`);
            }
        } catch (error) { errors.push(`resource inventory JSON invalid: ${error.message}`); }
    }
    if (errors.length) {
        console.error(errors.map((error) => `FAIL: ${error}`).join('\n'));
        process.exitCode = 1;
        return;
    }
    console.log(`PASS: Phase 0 receipts complete; ${scanOldNameMatches().length} old-name matches explicitly classified; manifest hashes verified.`);
}

const mode = process.argv[2] ?? '--validate';
if (mode === '--generate') generate();
else if (mode === '--validate') validate();
else {
    console.error('Usage: node scripts/validate-rebrand-phase0.mjs --generate|--validate');
    process.exitCode = 2;
}
