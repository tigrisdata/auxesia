// Run with a local Playwright installation:
// PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs node crates/auxesia/tests/search-browser.mjs
// Start Auxesia with ./docs as its root first. Override SEARCH_BASE_URL if needed.
import assert from 'node:assert/strict';
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const base = process.env.SEARCH_BASE_URL || 'http://127.0.0.1:8080';
const browser = await chromium.launch({ headless: true });
try {
  const context = await browser.newContext();
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(base);
  const form = page.locator('#site-search');
  await form.locator('input').fill('snapshot');
  await form.getByRole('button', { name: 'Search', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Search documentation' });
  await dialog.waitFor({ state: 'visible' });
  const input = dialog.getByRole('searchbox');
  await dialog.locator('.search-results li').first().waitFor();
  assert.equal(await input.inputValue(), 'snapshot');
  assert.equal(await input.evaluate(el => el === document.activeElement), true);
  await page.keyboard.press('Escape');
  await dialog.waitFor({ state: 'hidden' });
  assert.equal(await form.getByRole('button').evaluate(el => el === document.activeElement), true);
  await page.keyboard.press('Control+k');
  await dialog.waitFor({ state: 'visible' });
  // Native modal dialogs contain focus even after repeated Tab presses.
  for (let i = 0; i < 18; i++) {
    await page.keyboard.press('Tab');
    assert.equal(await dialog.evaluate(el => el.contains(document.activeElement)), true);
  }
  // Delay an older response, then return markup-like text for the newer query.
  let releaseOld;
  const oldPending = new Promise(resolve => { releaseOld = resolve; });
  let oldStarted;
  const started = new Promise(resolve => { oldStarted = resolve; });
  await page.route('**/_auxesia/search.json?*', async route => {
    const query = new URL(route.request().url()).searchParams.get('q');
    if (query === 'oldrequest') { oldStarted(); await oldPending; }
    const title = query === 'newrequest' ? '<img src=x onerror=alert(1)>' : 'Obsolete result';
    await route.fulfill({ json: { total: 1, results: [{ title, url: '/index.md', excerpt: '<script>bad()</script>' }] } });
  });
  await input.fill('oldrequest');
  await started;
  await input.fill('newrequest');
  await dialog.getByRole('link', { name: '<img src=x onerror=alert(1)>' }).waitFor();
  releaseOld();
  await page.waitForTimeout(300);
  assert.equal(await dialog.locator('.search-results img, .search-results script').count(), 0);
  assert.equal(await dialog.getByText('Obsolete result').count(), 0);
  await input.fill('');
  await page.waitForTimeout(250);
  assert.equal(await dialog.locator('.search-results li').count(), 0);
  await page.unrouteAll({ behavior: 'wait' });
  await page.route('**/_auxesia/search.json?*', route => route.fulfill({ status: 500, json: { error: 'Search unavailable' } }));
  await input.fill('failure');
  await dialog.getByText('Search unavailable').waitFor();
  await page.unrouteAll({ behavior: 'wait' });
  await input.fill('noresultsquasarxyz');
  await dialog.getByText('No results.').waitFor();
  assert.deepEqual(errors, []);
  const plain = await browser.newContext({ javaScriptEnabled: false });
  const nojs = await plain.newPage();
  await nojs.goto(base);
  await nojs.locator('#site-search input').fill('snapshot');
  await nojs.locator('#site-search button').click();
  await nojs.waitForURL('**/_auxesia/search?q=snapshot');
  assert.ok(await nojs.locator('.search-results li').count() > 0);
  console.log('Search browser checks passed: dialog, keyboard, stale responses, safe text, errors, empty results, and no-JavaScript form.');
} finally {
  await browser.close();
}
