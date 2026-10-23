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
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  await page.evaluate(() => localStorage.clear());
  await interceptFeeds(page);
  await page.goto('/');
  await expect(page.getByTestId('feed-badge')).toContainText(/live feed/i, { timeout: 30000 });
  await expect(page.getByTestId('pair-row').first()).toBeVisible({ timeout: 30000 });
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
