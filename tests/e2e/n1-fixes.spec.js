import { test } from '@playwright/test';
import { expect, freshPage, placePaperBuy, searchFor, interceptFeedsWithPairs, pairsFixture } from './helpers.js';

// N6.2 — regression suite for the N1.1–N1.5 fixes (plus N1.6/N2.1 adjacent in n1-adjacent.spec.js).
// Every test starts from freshPage(): feeds intercepted with recorded fixtures,
// storage cleared, initial paper state ($10,000 / 0 holdings / 0 orders).

test.describe('T1 — N1.1 reset paper account', () => {
  test('T1.1 place-then-reset restores the initial paper state', async ({ page }) => {
    await freshPage(page);
    await placePaperBuy(page, 25);
    await expect(page.getByTestId('paper-cash')).not.toContainText('$10,000.00');
    await expect(page.getByTestId('orders-list')).toContainText(/buy/i);
    // N1.1 two-step confirm: first click arms the button inline (no native dialog).
    await page.getByTestId('reset-btn').click();
    await expect(page.getByTestId('reset-btn')).toContainText(/confirm reset/i);
    await page.getByTestId('reset-btn').click();
    await expect(page.getByTestId('toast')).toContainText(/paper account reset/i);
    await expect(page.getByTestId('paper-cash')).toContainText('$10,000.00');
    await expect(page.getByTestId('holdings-list')).toContainText(/no paper positions yet/i);
    await expect(page.getByTestId('orders-list')).toContainText(
      /your simulated paper orders will appear here/i
    );
    // Reset persists across reload (N1.1 + N2.1).
    await page.reload();
    await expect(page.getByTestId('paper-cash')).toContainText('$10,000.00');
  });

  test('T1.2 reset confirm step can be cancelled without changing state', async ({ page }) => {
    await freshPage(page);
    await placePaperBuy(page, 25);
    const cashBefore = await page.getByTestId('paper-cash').textContent();
    await page.getByTestId('reset-btn').click();
    await expect(page.getByTestId('reset-btn')).toContainText(/confirm reset/i);
    // Cancel = let the 5 s arm expire (the documented in-app cancel path).
    await expect(page.getByTestId('reset-btn')).toContainText(/reset paper account/i, {
      timeout: 10000,
    });
    await expect(page.getByTestId('paper-cash')).toHaveText(cashBefore);
    await expect(page.getByTestId('orders-list')).toContainText(/buy/i);
  });

  test('T1.3 reset on an empty account toasts and leaves balances untouched', async ({ page }) => {
    await freshPage(page);
    await page.getByTestId('reset-btn').click();
    await expect(page.getByTestId('toast')).toContainText(/click again within 5 s|order history/i);
    await expect(page.getByTestId('paper-cash')).toContainText('$10,000.00');
    await expect(page.getByTestId('orders-list')).toContainText(
      /your simulated paper orders will appear here/i
    );
  });
});

test.describe('T2 — N1.2 MAX preset', () => {
  test('T2.1 MAX fills the order size with available cash (plain decimal)', async ({ page }) => {
    await freshPage(page);
    await page.getByTestId('max-btn').click();
    const input = page.getByTestId('order-size-input');
    await expect(input).toHaveValue('10000.00');
  });

  test('T2.2 MAX on sell fills the full position value', async ({ page }) => {
    await freshPage(page);
    await placePaperBuy(page, 25);
    await page.locator('#sellBtn').click();
    await page.getByTestId('max-btn').click();
    const raw = await page.getByTestId('order-size-input').inputValue();
    const v = parseFloat(raw);
    expect(raw.length).toBeGreaterThan(0);
    expect(Math.abs(v - 25)).toBeLessThan(0.5);
  });

  test('T2.3 MAX recomputes after a max-sized buy (no stale value)', async ({ page }) => {
    await freshPage(page);
    await page.getByTestId('max-btn').click();
    await expect(page.getByTestId('order-size-input')).toHaveValue('10000.00');
    // Place the max-sized buy through the review sheet.
    await page.locator('#submitTrade').click();
    const confirm = page.locator('#reviewConfirm');
    await expect(confirm).toBeVisible({ timeout: 10000 });
    await confirm.click();
    await expect(page.getByTestId('toast')).toContainText(/Paper buy order recorded/i, {
      timeout: 10000,
    });
    const cash = await page.getByTestId('paper-cash').textContent();
    expect(parseFloat(cash.replace(/[^0-9.]/g, ''))).toBeLessThan(1);
    // MAX again must reflect the drained cash, not the stale 10000.00.
    await page.getByTestId('max-btn').click();
    const raw = await page.getByTestId('order-size-input').inputValue();
    expect(raw).not.toBe('10000.00');
    expect(parseFloat(raw)).toBeLessThan(1);
  });
});

test.describe('T3 — N1.3 watchlist remove + display', () => {
  test('T3.1 watch toggle adds and removes; header card follows', async ({ page }) => {
    await freshPage(page);
    await searchFor(page, 'BONK');
    const btn = page.getByTestId('watch-btn').first();
    await expect(btn).toContainText(/^watch$/i);
    await btn.click();
    await expect(btn).toContainText(/added ✓/i);
    await expect(page.getByTestId('toast')).toContainText(/added .* to/i);
    await expect(page.getByTestId('watchlist-card')).toContainText(/bonk/i);
    // N1.3 pack guard: rapid successive toggles (<=350 ms) are debounced to a
    // single intentional click, so wait out the guard window before the remove.
    await page.waitForTimeout(400);
    await btn.click();
    await expect(btn).toContainText(/^watch$/i);
    await expect(page.getByTestId('toast')).toContainText(/removed .* from/i);
    await expect(page.getByTestId('watchlist-card')).toContainText(/no tokens saved/i);
  });

  test('T3.2 header card lists watched tokens with symbol and live price', async ({ page }) => {
    await freshPage(page);
    await searchFor(page, 'BONK');
    // Top two discovery rows are distinct mints (mint-keyed identity, N5.3).
    await page.getByTestId('watch-btn').nth(0).click();
    await page.getByTestId('watch-btn').nth(1).click();
    const card = page.getByTestId('watchlist-card');
    const text = await card.textContent();
    expect((text.match(/Bonk/gi) || []).length).toBe(2);
    // Prices render on the N1.5 contract: real figures, never a bare $0.00.
    expect(text).not.toMatch(/\$0\.00([^0-9]|$)/);
    expect(text).toMatch(/0\.0000\d+/);
  });

  test('T3.3 double-click guard: rapid double-click yields a single add', async ({ page }) => {
    await freshPage(page);
    await searchFor(page, 'BONK');
    await page.getByTestId('watch-btn').first().dblclick();
    const text = await page.getByTestId('watchlist-card').textContent();
    expect((text.match(/Bonk/gi) || []).length).toBe(1);
    await expect(page.getByTestId('watch-btn').first()).toContainText(/added ✓/i);
  });
});

test.describe('T4 — N1.4 real 24 h change', () => {
  test('T4.1 nonzero 24 h changes render (the all-+0.00% signature is the failure)', async ({
    page,
  }) => {
    await freshPage(page);
    const cells = page.locator('[data-testid="pair-row"] [data-testid="chg-24h"]');
    await expect(cells.first()).toBeVisible();
    const texts = await cells.allTextContents();
    expect(texts.length).toBeGreaterThan(0);
    // Failure signature of A4: every row reads +0.00%.
    expect(texts.some((t) => !/^\+0\.00%$/.test(t.trim()))).toBe(true);
    // And at least one row shows a genuinely signed nonzero value.
    expect(texts.some((t) => /^[+-]\d+\.\d+%$/.test(t.trim()) && !/^\+0\.00%$/.test(t.trim()))).toBe(
      true
    );
  });

  test('T4.2 sign and color agree on both paths', async ({ page }) => {
    await freshPage(page);
    // Negative: pairs-table rows (the fixture's top-8 h24 values are all negative).
    const negCell = page.locator('[data-testid="pair-row"] [data-testid="chg-24h"]').first();
    const negText = (await negCell.textContent()).trim();
    expect(negText).toMatch(/^-\d+\.\d+%$/);
    expect(await negCell.getAttribute('class')).toMatch(/negative/);
    // Positive: discovery rows (the fixture has positive-h24 BONK rows).
    await searchFor(page, 'BONK');
    const cards = page.getByTestId('search-results').locator('.result');
    const n = await cards.count();
    expect(n).toBeGreaterThan(0);
    let sawPositive = false;
    for (let i = 0; i < n; i++) {
      const change = cards.nth(i).locator('small').last();
      const text = (await change.textContent()).trim();
      const cls = (await change.getAttribute('class')) || '';
      if (text === '— 24h') {
        expect(cls).not.toMatch(/positive|negative/);
        continue;
      }
      const m = text.match(/^([+-])(\d+\.\d+)% 24h$/);
      expect(m, `change cell format: ${text}`).not.toBeNull();
      expect(cls).toMatch(m[1] === '+' ? /positive/ : /negative/);
      if (m[1] === '+') sawPositive = true;
    }
    expect(sawPositive).toBe(true);
  });

  test('T4.3 null 24 h change renders "—", never "+0.00%" (null ≠ zero)', async ({ page }) => {
    const mutated = JSON.parse(JSON.stringify(pairsFixture));
    mutated.pairs[0].priceChange.h24 = null;
    await page.goto('/', { waitUntil: 'domcontentloaded' });
    await page.evaluate(() => localStorage.clear());
    await interceptFeedsWithPairs(page, mutated);
    await page.goto('/');
    await expect(page.getByTestId('pair-row').first()).toBeVisible({ timeout: 30000 });
    const first = await page
      .locator('[data-testid="pair-row"] [data-testid="chg-24h"]')
      .first()
      .textContent();
    expect(first.trim()).toBe('—');
  });

  test('T4.4 discovery and table paths share the 24 h rendering contract', async ({ page }) => {
    await freshPage(page);
    await searchFor(page, 'BONK');
    // Fixture row Bonk/SOL @ 0.000003129 carries h24 = 0.56 -> "+0.56% 24h" on both paths.
    const cards = page.getByTestId('search-results').locator('.result');
    const target = cards.filter({ hasText: '+0.56% 24h' });
    await expect(target.first()).toBeVisible();
    const cls = await target.first().locator('small').last().getAttribute('class');
    expect(cls).toMatch(/positive/);
  });
});

test.describe('T5 — N1.5 sub-cent formatting', () => {
  test('T5.1 sub-cent prices keep significant figures, never "$0.00"', async ({ page }) => {
    await freshPage(page);
    await searchFor(page, 'BONK');
    const prices = page.getByTestId('search-results').locator('.result strong');
    const n = await prices.count();
    expect(n).toBeGreaterThan(0);
    const texts = await prices.allTextContents();
    for (const t of texts) expect(t.trim()).not.toBe('$0.00');
    // ≥1 row renders ≥4 significant figures below $0.01 (the BONK-shaped contract).
    expect(texts.some((t) => /0\.0000\d{2,}/.test(t))).toBe(true);
  });

  test('T5.2 precision tiers: $100+ pairs at 2dp, sub-cent discovery at sig figs', async ({
    page,
  }) => {
    await freshPage(page);
    // Tier $100+: the SOL/USDC pairs rows render two decimals.
    const pairPrices = page.locator('[data-testid="pair-row"] td:nth-child(2)');
    const pairTexts = await pairPrices.allTextContents();
    expect(pairTexts.length).toBeGreaterThan(0);
    for (const t of pairTexts) expect(t.trim()).toMatch(/^\$\d{1,3}(,\d{3})*\.\d{2}$/);
    // Tier <$0.01: discovery rows keep significant figures.
    await searchFor(page, 'BONK');
    const discPrices = page.getByTestId('search-results').locator('.result strong');
    const discTexts = await discPrices.allTextContents();
    expect(discTexts.some((t) => /0\.0000\d{2,}/.test(t))).toBe(true);
    // NOTE: the $1–100 and $0.01–1 tiers are not present in the recorded fixtures;
    // they stay covered by the N1.5 live acceptance, not by this suite.
  });
});
