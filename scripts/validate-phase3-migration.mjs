#!/usr/bin/env node

/** Fail-closed receipt validator for the pure migration specification phase. */
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const repo = process.cwd();
const evidence = path.join(repo, 'docs/plans/evidence/20260811-local-llm-foundry/phase-03');
const fixtureSource = path.join(repo, 'tests/integration/app_home_migration_fixtures.rs');
const requiredFixtures = [
    'fresh_empty_old_only_and_custom_root_states_are_explicit',
    'both_roots_collision_is_fail_closed',
    'partial_downloads_hf_cache_models_and_runtime_are_retained',
    'critical_tls_certs_tokens_chat_db_and_wal_are_copy_candidates',
    'symlink_and_special_entry_are_rejected_before_mutation',
    'cross_volume_simulation_has_stable_plan_id_and_no_preview_writes',
];
const sha256 = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');

function generate() {
    fs.mkdirSync(evidence, { recursive: true });
    const source = fs.readFileSync(fixtureSource, 'utf8');
    const fixtures = requiredFixtures.map(name => ({
        name,
        present: source.includes(`fn matrix_${name}`),
        policy: name.includes('symlink') ? 'refuse-before-mutation' : 'pure-plan-and-classify',
    }));
    if (fixtures.some(fixture => !fixture.present)) {
        throw new Error(`missing fixture: ${fixtures.filter(fixture => !fixture.present).map(fixture => fixture.name).join(', ')}`);
    }
    fs.writeFileSync(path.join(evidence, 'fixture-matrix.json'), `${JSON.stringify({ schema_version: 1, fixtures }, null, 2)}\n`);
    fs.writeFileSync(path.join(evidence, 'public-error-contract.json'), `${JSON.stringify({
        schema_version: 1,
        codes: ['invalid_root', 'unsafe_entry', 'destination_conflict', 'stale_plan', 'queue_conflict', 'permission_denied', 'io_failure'],
        rule: 'public responses expose error_code and generic error; local paths remain diagnostics only',
    }, null, 2)}\n`);
    const files = ['README.md', 'fixture-matrix.json', 'public-error-contract.json'].sort();
    fs.writeFileSync(path.join(evidence, 'phase3-receipt-manifest.tsv'), `path\tsha256\n${files.map(file => `${file}\t${sha256(path.join(evidence, file))}`).join('\n')}\n`);
}

function validate() {
    const required = ['README.md', 'fixture-matrix.json', 'public-error-contract.json', 'phase3-receipt-manifest.tsv'];
    const errors = required.filter(file => !fs.existsSync(path.join(evidence, file))).map(file => `missing receipt: ${file}`);
    if (!fs.existsSync(fixtureSource)) errors.push('missing fixture source');
    if (!errors.length) {
        const source = fs.readFileSync(fixtureSource, 'utf8');
        for (const name of requiredFixtures) if (!source.includes(`fn matrix_${name}`)) errors.push(`missing fixture: ${name}`);
        for (const line of fs.readFileSync(path.join(evidence, 'phase3-receipt-manifest.tsv'), 'utf8').split(/\r?\n/).slice(1).filter(Boolean)) {
            const [file, expected] = line.split('\t');
            if (!fs.existsSync(path.join(evidence, file)) || sha256(path.join(evidence, file)) !== expected) errors.push(`manifest mismatch: ${file}`);
        }
    }
    if (errors.length) { console.error(errors.map(error => `FAIL: ${error}`).join('\n')); process.exitCode = 1; return; }
    console.log('PASS: Phase 3 fixture matrix, public error contract, and receipt manifest verified.');
}

if (process.argv[2] === '--generate') generate();
else if (process.argv[2] === '--validate') validate();
else { console.error('Usage: node scripts/validate-phase3-migration.mjs --generate|--validate'); process.exitCode = 2; }
