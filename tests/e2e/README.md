# N6.2 — E2E regression suite

Executable form of the N1.1–N1.5 regression plans (pack: `hidden_files/prep/N6.2-e2e-regression-suite.md`),
plus the two highest-value adjacent suites (N1.6 feed resilience, N2.1 persistence).

**19 tests, 2 files:**

| File | Tests | Covers |
|---|---|---|
| `n1-fixes.spec.js` | T1.1–T1.3, T2.1–T2.3, T3.1–T3.3, T4.1–T4.4, T5.1–T5.2 | N1.1 reset, N1.2 MAX, N1.3 watchlist, N1.4 24 h change, N1.5 sub-cent formatting |
| `n1-adjacent.spec.js` | T6.1–T6.4 | N1.6 timeout/retry/stale, N2.1 persistence roundtrip + corrupt-store safety |

## Run

```bash
cd tests/e2e
npm install          # once; installs @playwright/test + chromium (npx playwright install)
BASE_URL=https://gecko-solana-terminal.vercel.app npx playwright test --project=chromium
```

`BASE_URL` defaults to production. Point it at any preview URL to test a deployment
before promotion, or at a local static server for development.

The suite only **reads** deployments: it intercepts the DexScreener feeds with the
recorded fixtures, never connects a wallet, never signs, never sends transactions.

## Fixtures

Recorded 2026-10-23 ~04:38 UTC from the live endpoints the app actually calls:

- `fixtures/pairs-response.json` — `GET https://api.dexscreener.com/latest/dex/tokens/So11111111111111111111111111111111111111112`
  (24 rows pass the app's real-WSOL filter; top 8 are SOL/USDC with nonzero 24 h changes)
- `fixtures/search-response.json` — `GET https://api.dexscreener.com/latest/dex/search?q=BONK`
  (20 Solana rows; top rows are sub-cent BONK with mixed 24 h change signs)

Refresh a fixture only when the feed schema changes (pack §4: the nightly job diffs the
live schema against these files). Both files are byte-identical to the live responses
at recording time — no rows were hand-edited. Tests that need a mutated feed (T4.3's
null-24 h row) build the mutation in memory at runtime.

## Selector contract

Tests locate elements via `data-testid` attributes defined in `src/index.html`
(pack §1.3): `pair-row`, `chg-24h`, `pairs-table`, `order-size-input`, `max-btn`,
`reset-btn`, `paper-cash`, `orders-list`, `holdings-list`, `watchlist-card`,
`watch-btn`, `search-results`, `toast`, `feed-badge`, `retry-btn`, `hero-chg-24h`.
The attributes are inert (no styling or behavior keys off them).

## Known scoping notes

- T3.3 (double-click guard): the app debounces watch toggles within 350 ms so a rapid
  double-click is a single add; deliberate slower clicks still toggle normally.
- T4.4 pins the discovery/table 24 h rendering contract against the fixture's
  Bonk/SOL row (h24 = 0.56 → "+0.56% 24h", positive class).
- T5.2 covers the price tiers present in the fixtures ($100+ pairs at 2dp, sub-cent
  discovery at significant figures); the $1–100 and $0.01–1 tiers stay covered by the
  N1.5 live acceptance, not by this suite.
- T6.1 relies on the app's 10 s feed timeout; the test allows 30 s.
