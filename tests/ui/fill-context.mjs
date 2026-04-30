#!/usr/bin/env node

/**
 * fill-context.mjs — Test compaction with injected messages
 *
 * Usage:
 *   node tests/ui/fill-context.mjs [options]
 *
 * Options:
 *   --summarize        Call real model for summarization (default: truncate only)
 *   --url <url>        llama-monitor UI URL (default: http://localhost:7778)
 *   --endpoint <url>   Model endpoint to attach to (required with --summarize)
 *   --keep <n>         Messages to keep after compaction (default: 10)
 *   --count <n>        Messages to inject per round (default: 50)
 *
 * Examples:
 *   # Fast: truncation only, no model needed
 *   node tests/ui/fill-context.mjs
 *
 *   # Live: real summarization via model
 *   node tests/ui/fill-context.mjs --summarize --endpoint http://192.168.2.16:8001
 */

import { chromium } from 'playwright';
import { argv } from 'node:process';

function parseArgs(args) {
  const result = {
    summarize: false,
    url: process.env.LLAMA_MONITOR_URL || 'http://localhost:7778',
    endpoint: process.env.LLAMA_ENDPOINT || null,
    keep: 10,
    count: 50,
  };
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--summarize') result.summarize = true;
    else if (args[i] === '--url' && args[i + 1]) result.url = args[++i];
    else if (args[i] === '--endpoint' && args[i + 1]) result.endpoint = args[++i];
    else if (args[i] === '--keep' && args[i + 1]) result.keep = parseInt(args[++i], 10);
    else if (args[i] === '--count' && args[i + 1]) result.count = parseInt(args[++i], 10);
  }
  return result;
}

const opts = parseArgs(argv.slice(2));

if (opts.summarize && !opts.endpoint) {
  console.error('❌ --summarize requires --endpoint <url> (e.g. http://192.168.2.16:8001)');
  process.exit(1);
}

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

async function injectMessages(page, count, prefix = '') {
  await page.evaluate(({ count, prefix }) => {
    const tab = activeChatTab();
    if (!tab) return;
    for (let i = 0; i < count; i++) {
      tab.messages.push({
        role: 'user',
        content: `${prefix}Question ${i + 1}: This contains enough text to simulate a real conversation exchange with meaningful content about various topics.`,
        timestamp_ms: Date.now() - (count - i) * 60000,
      });
      tab.messages.push({
        role: 'assistant',
        content: `${prefix}Response ${i + 1}: This is a detailed reply with explanations, examples, and thorough analysis of the topic at hand.`,
        timestamp_ms: Date.now() - (count - i) * 60000 + 500,
      });
    }
    renderChatMessages();
  }, { count, prefix });
}

async function attachEndpoint(page, endpoint) {
  // Fill the endpoint input and connect
  await page.evaluate((url) => {
    const input = document.getElementById('setup-endpoint-url');
    if (input) {
      input.value = url;
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }
  }, endpoint);

  const connectBtn = page.locator('#btn-connect, button:has-text("Connect"), button:has-text("Attach")').first();
  if (await connectBtn.count() > 0) {
    await connectBtn.click();
    await sleep(3000);
  }
}

(async () => {
  console.log('\n🎯 Target: ' + opts.url);
  if (opts.summarize) {
    console.log('🤖 Mode: summarize (real model at ' + opts.endpoint + ')');
  } else {
    console.log('✂️  Mode: truncate only (no model needed)');
  }
  console.log('');

  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  const page = await browser.newPage();
  await page.setViewportSize({ width: 1440, height: 900 });

  page.on('console', msg => {
    const text = msg.text();
    if (text.startsWith('[COMPACT]')) console.log('  browser:', text);
  });

  try {
    await page.goto(opts.url, { waitUntil: 'networkidle' });
    await sleep(2000);

    if (opts.summarize && opts.endpoint) {
      console.log('🔌 Attaching to endpoint...');
      await attachEndpoint(page, opts.endpoint);
      console.log('   done\n');
    }

    // Round 1
    console.log(`📦 Injecting ${opts.count * 2} messages (round 1)...`);
    await page.evaluate(() => { addChatTab(); });
    await sleep(500);
    await injectMessages(page, opts.count);

    const beforeCount = await page.evaluate(() => activeChatTab().messages.length);
    console.log(`✅ Before: ${beforeCount} messages`);

    console.log('✂️  Compacting...');
    const summarize = opts.summarize;
    const keep = opts.keep;
    await page.evaluate(({ summarize, keep }) => compactChatTab(activeChatTab(), keep, summarize), { summarize, keep });

    // Wait for compaction — longer if summarizing (model call)
    await sleep(opts.summarize ? 15000 : 2000);

    const r1 = await page.evaluate(() => {
      const tab = activeChatTab();
      const tombstones = tab.messages.filter(m => m.compaction_marker);
      return {
        count: tab.messages.length,
        tombstones: tombstones.length,
        tombstoneText: tombstones[0]?.content?.slice(0, 200) ?? null,
      };
    });

    console.log('\n📊 Round 1 results:');
    console.log(`   Before: ${beforeCount} → After: ${r1.count} messages`);
    console.log(`   Tombstones: ${r1.tombstones}`);
    if (r1.tombstoneText) console.log(`   Tombstone preview: "${r1.tombstoneText}..."`);

    if (r1.tombstones > 0 && r1.count < beforeCount) {
      console.log('   ✅ Compaction succeeded');
    } else {
      console.log('   ❌ Compaction failed — no tombstone or count unchanged');
    }

    // Round 2 — verify tombstones accumulate
    console.log(`\n🔄 Round 2: injecting ${opts.count * 2} more messages...`);
    await injectMessages(page, opts.count, 'R2-');
    const before2 = await page.evaluate(() => activeChatTab().messages.length);
    console.log(`✅ Before: ${before2} messages`);

    console.log('✂️  Compacting...');
    await page.evaluate(({ summarize, keep }) => compactChatTab(activeChatTab(), keep, summarize), { summarize, keep });
    await sleep(opts.summarize ? 15000 : 2000);

    const r2 = await page.evaluate(() => {
      const tab = activeChatTab();
      const tombstones = tab.messages.filter(m => m.compaction_marker);
      return { count: tab.messages.length, tombstones: tombstones.length };
    });

    console.log('\n📊 Round 2 results:');
    console.log(`   Before: ${before2} → After: ${r2.count} messages`);
    console.log(`   Tombstones: ${r2.tombstones}`);

    if (r2.tombstones === 2) {
      console.log('   ✅ Multiple compactions preserved old tombstones');
    } else {
      console.log(`   ⚠️  Expected 2 tombstones, got ${r2.tombstones}`);
    }

  } catch (err) {
    console.error('\n❌', err.message);
    console.error(err.stack);
    process.exit(1);
  } finally {
    await browser.close();
    console.log('\n🏁 Done.\n');
  }
})();
