import puppeteer from 'puppeteer';

const browser = await puppeteer.launch({
  headless: 'new',
  args: ['--no-sandbox', '--disable-setuid-sandbox'],
});

const page = await browser.newPage();
console.log('Browser launched, navigating to http://127.0.0.1:7778...');

try {
  await page.goto('http://127.0.0.1:7778', { 
    waitUntil: 'networkidle0',
    timeout: 15000 
  });
  console.log('Page loaded successfully!');
  await page.screenshot({ path: '/tmp/test-connect.png' });
  console.log('Screenshot saved to /tmp/test-connect.png');
} catch (err) {
  console.error('Error:', err.message);
}

await browser.close();
