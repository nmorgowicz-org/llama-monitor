#!/usr/bin/env node

/**
 * fill-context.mjs — Fill context then test manual compaction
 *
 * Usage:
 *   node tests/ui/fill-context.mjs [ENDPOINT]
 *
 * Example:
 *   node tests/ui/fill-context.mjs http://192.168.2.16:8001
 *
 * Sends enough exchanges to build up a substantial context (~32k tokens),
 * then clicks the manual compact button to verify it works.
 */

import { chromium } from 'playwright';
import { argv } from 'node:process';

const ENDPOINT = argv[2] || 'http://localhost:7778';

// Long prompts designed to generate large responses (~1-2k tokens each)
const PROMPTS = [
  'Write a comprehensive tutorial on building a REST API with Node.js and Express, including authentication, error handling, and database integration.',
  'Explain the entire history of artificial intelligence from the Turing Test to modern LLMs, covering every major milestone and paradigm shift.',
  'Write a detailed guide to computer networking: OSI model layers, TCP/IP stack, DNS resolution, BGP routing, and firewall configuration.',
  'Describe the complete human digestive system from mouth to intestines, including all enzymes, hormones, and physiological processes involved.',
  'Implement a full e-commerce checkout flow in Python: cart management, payment processing with Stripe, order confirmation, and inventory updates.',
  'Explain quantum computing from first principles: qubits, superposition, entanglement, quantum gates, and Shor\'s algorithm.',
  'Write a comprehensive security guide covering encryption at rest and in transit, key management, zero-trust architecture, and incident response.',
  'Describe the evolution of programming languages: from machine code to assembly, COBOL, C, Java, Python, and modern TypeScript.',
];

async function main() {
  console.log(`\n🎯 Target: ${ENDPOINT}`);
  console.log('📦 Sending exchanges to build up context...\n');

  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();

  await page.goto(ENDPOINT);
  await page.waitForSelector('.top-nav-bar');
  await page.evaluate(() => switchView('monitor'));
  await page.getByRole('button', { name: /chat/i }).click();
  await page.waitForSelector('#page-chat');

  const input = page.locator('#chat-input');
  const sendBtn = page.locator('.chat-send-btn');

  for (let i = 0; i < PROMPTS.length; i++) {
    const round = i + 1;
    console.log(`📝 Round ${round}/${PROMPTS.length}...`);

    await input.fill(PROMPTS[i]);
    await sendBtn.click();

    // Wait for response to complete
    await page.waitForFunction(() => window.chatBusy === false, { timeout: 120000 });

    // Report context status
    const stats = await page.evaluate(() => {
      const tab = activeChatTab();
      const capacity = window.lastLlamaMetrics?.context_capacity_tokens || 0;
      const total = (tab?.totalInputTokens || 0) + (tab?.totalOutputTokens || 0);
      return {
        messages: tab?.messages.length ?? 0,
        totalTokens: total,
        capacity,
        ctxPct: capacity > 0 ? Math.round((total / capacity) * 100) : 0,
      };
    });

    console.log(`   msgs: ${stats.messages} | tokens: ${stats.totalTokens.toLocaleString()} | ctx: ${stats.ctxPct}%\n`);
  }

  console.log('✂️  Clicking manual compact...');

  // Click the compact button
  await page.locator('#btn-compact').click();

  // Wait for compaction to complete (button un-disables)
  await page.waitForFunction(() => !document.getElementById('btn-compact')?.disabled, { timeout: 30000 });

  // Verify results
  const results = await page.evaluate(() => {
    const tab = activeChatTab();
    const tombstones = tab.messages.filter(m => m.compaction_marker);
    return {
      finalMessages: tab.messages.length,
      tombstones: tombstones.length,
      tombstoneText: tombstones[0]?.content?.slice(0, 120),
    };
  });

  console.log(`\n📊 Results:`);
  console.log(`   Final messages: ${results.finalMessages}`);
  console.log(`   Tombstones: ${results.tombstones}`);
  if (results.tombstoneText) {
    console.log(`   Tombstone: "${results.tombstoneText}..."`);
  }

  if (results.tombstones > 0) {
    console.log('\n✅ Manual compaction succeeded!');
  } else {
    console.log('\n⚠️  No compaction occurred (not enough messages to drop).');
  }

  await browser.close();
  console.log('\n🏁 Done.\n');
}

main().catch(err => {
  console.error('❌', err.message);
  process.exit(1);
});
