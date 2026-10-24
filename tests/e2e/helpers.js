import { expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

// N6.2 shared harness: fixture feeds, fresh-state isolation, common flows.
// The suite only READS deployments: it intercepts the market feeds with recorded
// fixtures so tests never depend on live market values, and it never connects a
// wallet, signs, or sends anything.

const here = path.dirname(fileURLToPath(import.meta.url));

export const pairsFixture = JSON.parse(
  readFileSync(path.join(here, 'fixtures/pairs-response.json'), 'utf8')
);
export const searchFixture = JSON.parse(
  readFileSync(path.join(here, 'fixtures/search-response.json'), 'utf8')
);

const TOKENS_RE = /dexscreener\.com\/latest\/dex\/tokens/;
const SEARCH_RE = /dexscreener\.com\/latest\/dex\/search/;
const ANY_FEED_RE = /dexscreener\.com/;

/** Happy path: serve the recorded fixtures for both feed endpoints. */
export async function interceptFeeds(page) {
  await page.route(TOKENS_RE, (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(pairsFixture),
    })
  );
  await page.route(SEARCH_RE, (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(searchFixture),
    })
  );
}

/** Happy path with a mutated pairs fixture (e.g. to inject a null 24h-change row). */
export async function interceptFeedsWithPairs(page, mutatedPairs) {
  await page.unrouteAll({ behavior: 'wait' });
  await page.route(TOKENS_RE, (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(mutatedPairs),
    })
  );
  await page.route(SEARCH_RE, (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(searchFixture),
    })
  );
}

/** Failure path: every feed request aborts (drives the N1.6 timeout/error states). */
export async function failFeeds(page) {
  await page.unrouteAll({ behavior: 'wait' });
  await page.route(ANY_FEED_RE, (route) => route.abort());
}

/**
 * Fresh-state isolation (N2.1 interaction): clear storage, intercept the feeds,
 * then load the home page and wait for the pairs table to render from the fixture.
 * Every test starts from the N1.1 initial state ($10,000 / 0 holdings / 0 orders).
 */
export async function freshPage(page) {
  const consoleErrors = [];
  page.on('console', (msg) => {
    if (msg.type() === 'error') consoleErrors.push(msg.text().slice(0, 300));
  });
  page.on('pageerror', (err) => consoleErrors.push('pageerror: ' + String((err && err.message) || err).slice(0, 300)));
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  await page.evaluate(() => localStorage.clear());
  await interceptFeeds(page);
  await page.goto('/');
  await waitForFeedBadge(page, consoleErrors);
  await expect(page.getByTestId('pair-row').first()).toBeVisible({ timeout: 30000 });
}

/** Snapshot of what the page actually looked like, for badge-absence diagnostics. */
export async function badgeDiagnostics(page, consoleErrors) {
  let url = '',
    title = '',
    htmlLen = 0,
    hasBadge = false,
    bodyHead = '';
  try {
    url = page.url();
  } catch (_) {}
  try {
    title = await page.title();
  } catch (_) {}
  try {
    const html = await page.content();
    htmlLen = html.length;
    hasBadge = html.includes('data-testid="feed-badge"');
    bodyHead = html.replace(/\s+/g, ' ').slice(0, 400);
  } catch (_) {}
  return (
    `url=${url} title=${JSON.stringify(title)} htmlLen=${htmlLen} ` +
    `rawHtmlHasBadge=${hasBadge} consoleErrors=${JSON.stringify(consoleErrors.slice(0, 5))} ` +
    `bodyHead=${JSON.stringify(bodyHead)}`
  );
}

/**
 * Wait for the feed badge, resilient to bad page loads.
 *
 * The badge is STATIC HTML (src/index.html header; app JS only ever sets its
 * textContent/style, never removes it — verified 2026-10-23): its absence from
 * the DOM proves a bad page load (serving/network/rendering transient), never a
 * product state. CI runs #10–#13 (2026-10-23) showed exactly this signature —
 * `getByTestId('feed-badge')` "element(s) not found" for the full 30 s on a
 * shifting contiguous block of end-of-run tests, each failing twice
 * (initial + retry), while the same static badge was found minutes earlier and
 * later in the same run. So on element-absence we capture diagnostics and take
 * ONE reload before failing. The LIVE FEED text assertion below still runs at
 * full strictness afterward — no product signal is weakened: a genuinely broken
 * header would fail every load, not transiently.
 */
export async function waitForFeedBadge(page, consoleErrors) {
  const badge = () => page.getByTestId('feed-badge');
  try {
    await expect(badge()).toBeAttached({ timeout: 30000 });
  } catch (attachErr) {
    const diag = await badgeDiagnostics(page, consoleErrors || []);
    await page.reload();
    try {
      await expect(badge()).toBeAttached({ timeout: 30000 });
    } catch (attachErr2) {
      const diag2 = await badgeDiagnostics(page, consoleErrors || []);
      throw new Error(
        'feed-badge static element never attached (initial load + one reload). ' +
          'This is a page-load/serving failure, not a product assertion. ' +
          `diag-initial=[${diag}] diag-reload=[${diag2}] lastError=${attachErr2.message}`
      );
    }
  }
  await expect(badge()).toContainText(/live feed/i, { timeout: 30000 });
}

/** Paper ticket flow: fill size -> submit (opens the N4.4 review sheet) -> confirm. */
export async function placePaperBuy(page, usd = 25) {
  await page.getByTestId('order-size-input').fill(String(usd));
  await page.locator('#submitTrade').click();
  const confirm = page.locator('#reviewConfirm');
  await expect(confirm).toBeVisible({ timeout: 10000 });
  await expect(confirm).toBeEnabled({ timeout: 10000 });
  await confirm.click();
  await expect(page.getByTestId('toast')).toContainText(/Paper buy order recorded/i, {
    timeout: 10000,
  });
}

/** Discovery path: search a query and wait for result cards to render. */
export async function searchFor(page, query) {
  await page.locator('#tokenSearch').fill(query);
  await page.locator('#searchBtn').click();
  await expect(page.getByTestId('search-results').locator('.result').first()).toBeVisible({
    timeout: 20000,
  });
}

export { expect };
