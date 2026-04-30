#!/usr/bin/env node

/**
 * debug-compact.mjs — Step-by-step compaction debugger
 *
 * Usage:
 *   node tests/ui/debug-compact.mjs [options]
 *
 * Options:
 *   --summarize        Call real model for summarization (default: truncate only)
 *   --url <url>        llama-monitor UI URL (default: http://localhost:7778)
 *   --endpoint <url>   Model endpoint to attach to (required with --summarize)
 *
 * Prints detailed internal state at each step so you can see exactly what
 * the compaction logic is doing.
 */

import { chromium } from 'playwright';
import { argv } from 'node:process';

function parseArgs(args) {
  const result = {
    summarize: false,
    url: process.env.LLAMA_MONITOR_URL || 'http://localhost:7778',
    endpoint: process.env.LLAMA_ENDPOINT || null,
  };
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--summarize') result.summarize = true;
    else if (args[i] === '--url' && args[i + 1]) result.url = args[++i];
    else if (args[i] === '--endpoint' && args[i + 1]) result.endpoint = args[++i];
  }
  return result;
}

const opts = parseArgs(argv.slice(2));

if (opts.summarize && !opts.endpoint) {
  console.error('❌ --summarize requires --endpoint <url>');
  process.exit(1);
}

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

function printState(label, state) {
  console.log(`\n[${label}]`);
  console.log(`  total      : ${state.total}`);
  console.log(`  tombstones : ${state.tombstones}`);
  console.log(`  conversational: ${state.conversational}`);
  if (state.tombstoneTexts?.length) {
    state.tombstoneTexts.forEach((t, i) => console.log(`  tombstone[${i}]: "${t}"`));
  }
}

(async () => {
  console.log('\n🔍 debug-compact — detailed compaction trace');
  console.log('🎯 Target: ' + opts.url);
  console.log('✂️  Mode: ' + (opts.summarize ? `summarize (${opts.endpoint})` : 'truncate only'));

  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  const page = await browser.newPage();
  await page.setViewportSize({ width: 1440, height: 900 });

  page.on('console', msg => {
    const text = msg.text();
    if (text.startsWith('[COMPACT]')) console.log('  browser:', text);
  });

  const getState = () => page.evaluate(() => {
    const tab = activeChatTab();
    const tombstones = tab.messages.filter(m => m.compaction_marker);
    const conversational = tab.messages.filter(m => m.role !== 'system' && !m.compaction_marker);
    return {
      total: tab.messages.length,
      tombstones: tombstones.length,
      conversational: conversational.length,
      tombstoneTexts: tombstones.map(t => t.content.slice(0, 120)),
    };
  });

  try {
    await page.goto(opts.url, { waitUntil: 'networkidle' });
    await sleep(2000);

    if (opts.summarize && opts.endpoint) {
      await page.evaluate((url) => {
        const input = document.getElementById('setup-endpoint-url');
        if (input) { input.value = url; input.dispatchEvent(new Event('input', { bubbles: true })); }
      }, opts.endpoint);
      const connectBtn = page.locator('#btn-connect, button:has-text("Connect"), button:has-text("Attach")').first();
      if (await connectBtn.count() > 0) { await connectBtn.click(); await sleep(3000); }
    }

    // Set up fresh tab with 30 messages
    await page.evaluate(() => {
      addChatTab();
      const tab = activeChatTab();
      for (let i = 0; i < 30; i++) {
        tab.messages.push({ role: 'user', content: 'Q' + i, timestamp_ms: Date.now() });
        tab.messages.push({ role: 'assistant', content: 'A' + i, timestamp_ms: Date.now() });
      }
      renderChatMessages();
    });

    printState('after inject (round 1)', await getState());

    // Predict what compaction will do before running it
    const prediction = await page.evaluate(() => {
      const tab = activeChatTab();
      const msgs = tab.messages;
      const systemMsg = msgs[0]?.role === 'system' ? msgs[0] : null;
      const tombstones = msgs.filter(m => m.compaction_marker);
      const conversational = msgs.filter(m => m.role !== 'system' && !m.compaction_marker);
      const keepTail = 10;
      const dropped = conversational.slice(0, conversational.length - keepTail);
      const kept = conversational.slice(-keepTail);
      return {
        willDrop: dropped.length,
        willKeep: kept.length,
        existingTombstones: tombstones.length,
        expectedTotal: (systemMsg ? 1 : 0) + tombstones.length + 1 + kept.length,
      };
    });
    console.log('\n[prediction — round 1 compact]');
    console.log(`  will drop  : ${prediction.willDrop} messages`);
    console.log(`  will keep  : ${prediction.willKeep} messages`);
    console.log(`  new tombstones: ${prediction.existingTombstones} + 1 = ${prediction.existingTombstones + 1}`);
    console.log(`  expected total: ${prediction.expectedTotal}`);

    const summarize = opts.summarize;
    await page.evaluate(({ summarize }) => compactChatTab(activeChatTab(), 10, summarize), { summarize });
    await sleep(opts.summarize ? 15000 : 2000);

    const after1 = await getState();
    printState('after compact (round 1)', after1);

    const ok1 = after1.total === prediction.expectedTotal && after1.tombstones === 1;
    console.log(ok1 ? '\n✅ Round 1 matches prediction' : `\n❌ Mismatch — got ${after1.total}, expected ${prediction.expectedTotal}`);

    // Add 30 more and compact again
    await page.evaluate(() => {
      const tab = activeChatTab();
      for (let i = 0; i < 30; i++) {
        tab.messages.push({ role: 'user', content: 'R2Q' + i, timestamp_ms: Date.now() });
        tab.messages.push({ role: 'assistant', content: 'R2A' + i, timestamp_ms: Date.now() });
      }
      renderChatMessages();
    });

    printState('after inject (round 2)', await getState());

    await page.evaluate(({ summarize }) => compactChatTab(activeChatTab(), 10, summarize), { summarize });
    await sleep(opts.summarize ? 15000 : 2000);

    const after2 = await getState();
    printState('after compact (round 2)', after2);

    const ok2 = after2.tombstones === 2;
    console.log(ok2 ? '\n✅ Round 2: both tombstones preserved' : `\n❌ Expected 2 tombstones, got ${after2.tombstones}`);

  } catch (err) {
    console.error('\n❌', err.message);
    console.error(err.stack);
    process.exit(1);
  } finally {
    await browser.close();
    console.log('\n🏁 Done.\n');
  }
})();
