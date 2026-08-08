import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import { join } from 'node:path';
import { currentArtifactsDir, setArtifactCategory } from './harness/paths.mjs';
import { beginCaptureReceipt, finishCaptureReceipt, recordCapture } from './harness/receipt.mjs';

const CATEGORY = '__capture-receipt-test__';

function reset() {
    setArtifactCategory(CATEGORY);
    fs.rmSync(currentArtifactsDir(), { recursive: true, force: true });
    fs.mkdirSync(currentArtifactsDir(), { recursive: true });
}

test('receipt rejects a missing expected screenshot', () => {
    reset();
    beginCaptureReceipt({ scenario: 'missing', category: CATEGORY, runtime: 'neutral', intent: 'negative test', expectedOutputs: ['expected.png'] });
    assert.throws(() => finishCaptureReceipt(), /missing=\[expected\.png\]/);
});

test('receipt rejects an unexpected screenshot', () => {
    reset();
    beginCaptureReceipt({ scenario: 'unexpected', category: CATEGORY, runtime: 'neutral', intent: 'negative test', expectedOutputs: ['expected.png'] });
    fs.writeFileSync(join(currentArtifactsDir(), 'other.png'), 'test');
    recordCapture('other.png', { width: 1440, height: 900 });
    assert.throws(() => finishCaptureReceipt(), /unexpected=\[other\.png\]/);
});

test('receipt writes a manifest for an expected realistic screenshot', () => {
    reset();
    beginCaptureReceipt({ scenario: 'complete', category: CATEGORY, runtime: 'neutral', intent: 'positive test', expectedOutputs: ['expected.png'] });
    fs.writeFileSync(join(currentArtifactsDir(), 'expected.png'), 'test');
    recordCapture('expected.png', { width: 430, height: 900 });
    const receipt = finishCaptureReceipt();
    assert.equal(receipt.produced[0].filename, 'expected.png');
    assert.ok(fs.existsSync(join(currentArtifactsDir(), 'complete--receipt.json')));
});
