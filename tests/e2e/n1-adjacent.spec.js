import { test } from '@playwright/test';
import {
  expect,
  freshPage,
  placePaperBuy,
  searchFor,
  interceptFeeds,
  failFeeds,
  pairsFixture,
  searchFixture,
} from './helpers.js';

// N6.2 — the two highest-value adjacent suites: N1.6 feed resilience and N2.1
// persistence. Cheap once the harness exists; they pin the contracts the
// dashboard/history packs depend on.

test.describe('T6 — N1.6 feed resilience + N2.1 persistence', () => {
  test('T6.1 feed timeout surfaces an error with Retry; retry recovers the table', async ({
    page,
  }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' });
    await page.evaluate(() => localStorage.clear());
    await failFeeds(page);
    await page.goto('/');
    // The app aborts feed requests after a 10 s timeout, then shows FEED ERROR.
    await expect(page.getByTestId('feed-badge')).toContainText(/feed error/i, { timeout: 30000 });
    await expect(page.getByTestId('pairs-table')).toContainText(/did not load/i);
    await expect(page.getByTestId('retry-btn')).toBeVisible();
    // Restore the feed, then Retry: the table loads (no eternal spinner, the D3 signature).
    await interceptFeeds(page);
    await page.getByTestId('retry-btn').click();
    await expect(page.getByTestId('feed-badge')).toContainText(/live feed/i, { timeout: 30000 });
    await expect(page.getByTestId('pair-row').first()).toBeVisible({ timeout: 30000 });
  });

  test('T6.2 feed failure keeps last-good data with the stale badge', async ({ page }) => {
    await freshPage(page);
    const firstRowText = await page.getByTestId('pair-row').first().textContent();
    await failFeeds(page);
    await page.getByTestId('retry-btn').click();
    await expect(page.getByTestId('feed-badge')).toContainText(/stale data/i, { timeout: 30000 });
    // Rows remain on screen: nothing disappears or zeroes out.
    await expect(page.getByTestId('pair-row').first()).toBeVisible();
    const afterRowText = await page.getByTestId('pair-row').first().textContent();
    expect(afterRowText).toBe(firstRowText);
    expect(afterRowText).not.toMatch(/\$0\.00\s/);
  });

  test('T6.3 persistence roundtrip: orders, holdings, watchlist and search survive reload', async ({
    page,
  }) => {
    await freshPage(page);
    await placePaperBuy(page, 25);
    await searchFor(page, 'BONK');
    await page.getByTestId('watch-btn').first().click();
    await expect(page.getByTestId('watch-btn').first()).toContainText(/added ✓/i);
    await page.reload();
    await expect(page.getByTestId('pair-row').first()).toBeVisible({ timeout: 30000 });
    // Paper state restored (N2.1 versioned schema).
    await expect(page.getByTestId('paper-cash')).toContainText('$9,975.00');
    await expect(page.getByTestId('orders-list')).toContainText(/buy/i);
    await expect(page.getByTestId('holdings-list')).toContainText(/sol/i);
    // Watch state + search box restored.
    await expect(page.getByTestId('watchlist-card')).toContainText(/bonk/i);
    await expect(page.locator('#tokenSearch')).toHaveValue('BONK');
  });

  test('T6.4 corrupt store is safe: garbage under the versioned key boots to initial state', async ({
    page,
  }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' });
    await interceptFeeds(page);
    await page.evaluate(() => {
      localStorage.setItem('gecko-terminal-state-v2', 'garbage{{{not-json');
      localStorage.setItem('gecko-terminal:v1:history', 'garbage{{{not-json');
    });
    await page.reload();
    // No white screen: the table renders and the paper account is the initial one.
    await expect(page.getByTestId('pair-row').first()).toBeVisible({ timeout: 30000 });
    await expect(page.getByTestId('paper-cash')).toContainText('$10,000.00');
  });

  test('T6.5 three consecutive feed failures escalate STALE DATA to FEED OFFLINE; last-good data stays', async ({
    page,
  }) => {
    await freshPage(page);
    const firstRowText = await page.getByTestId('pair-row').first().textContent();
    await failFeeds(page);
    const retry = page.getByTestId('retry-btn');
    // Failure 1 -> STALE DATA (amber); failure 2 stays stale.
    await retry.click();
    await expect(page.getByTestId('feed-badge')).toContainText(/stale data/i, { timeout: 30000 });
    await retry.click();
    await expect(page.getByTestId('feed-badge')).toContainText(/stale data/i, { timeout: 30000 });
    // Failure 3 -> escalates to FEED OFFLINE (red), per the N1.6 spec.
    await retry.click();
    await expect(page.getByTestId('feed-badge')).toContainText(/feed offline/i, { timeout: 30000 });
    // Last-good data stays on screen through the escalation.
    await expect(page.getByTestId('pair-row').first()).toBeVisible();
    const afterRowText = await page.getByTestId('pair-row').first().textContent();
    expect(afterRowText).toBe(firstRowText);
  });

  test('T6.6 failed token search shows an error with Retry; retry recovers results', async ({
    page,
  }) => {
    await freshPage(page);
    // Abort only the search endpoint; the market feed stays on fixtures.
    await page.unrouteAll({ behavior: 'wait' });
    await page.route(/dexscreener\.com\/latest\/dex\/tokens/, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(pairsFixture),
      })
    );
    await page.route(/dexscreener\.com\/latest\/dex\/search/, (route) => route.abort());
    await page.locator('#tokenSearch').fill('BONK');
    await page.locator('#searchBtn').click();
    // Honest error state with an inline Retry — not the graceful-empty copy.
    await expect(page.getByTestId('search-results')).toContainText(/timed out or failed/i, {
      timeout: 20000,
    });
    const retry = page.getByTestId('search-retry');
    await expect(retry).toBeVisible();
    // Restore the search feed, then Retry: results render.
    await page.unrouteAll({ behavior: 'wait' });
    await page.route(/dexscreener\.com\/latest\/dex\/search/, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(searchFixture),
      })
    );
    await page.route(/dexscreener\.com\/latest\/dex\/tokens/, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(pairsFixture),
      })
    );
    await retry.click();
    await expect(page.getByTestId('search-results').locator('.result').first()).toBeVisible({
      timeout: 20000,
    });
  });
});
