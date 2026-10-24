import { defineConfig } from '@playwright/test';

// N6.2 — E2E regression suite runner.
// BASE_URL points at the deployment under test:
//   production: BASE_URL=https://gecko-solana-terminal.vercel.app npx playwright test
//   preview:    BASE_URL=<preview-url> npx playwright test
//   local:      serve the repo's dist/ (e.g. python3 -m http.server) and point BASE_URL at it.
export default defineConfig({
  testDir: '.',
  testMatch: ['*.spec.js'],
  fullyParallel: false,
  workers: 1,
  retries: 1, // pack flake policy: one retry; two consecutive failures on two deployments = real regression
  timeout: 120000,
  expect: { timeout: 15000 },
  use: {
    baseURL: process.env.BASE_URL || 'https://gecko-solana-terminal.vercel.app',
    viewport: { width: 1440, height: 900 },
    trace: 'retain-on-failure',
    video: 'retain-on-failure',
    // N6.2 CI reachability (Carlos's decision 2026-10-23: relax bot protection for CI):
    // when the VERCEL_AUTOMATION_BYPASS_SECRET env var is present (in CI it comes from
    // the GitHub Actions repo secret of the same name — the value is NEVER committed),
    // send Vercel's Protection Bypass for Automation headers on every request so the
    // runner gets the app instead of the Security Checkpoint interstitial. The
    // set-bypass-cookie header makes the bypass stick across in-session navigations.
    // Absent the env var (all local/VM runs) the config behaves exactly as before.
    ...(process.env.VERCEL_AUTOMATION_BYPASS_SECRET
      ? {
          extraHTTPHeaders: {
            'x-vercel-protection-bypass': process.env.VERCEL_AUTOMATION_BYPASS_SECRET,
            'x-vercel-set-bypass-cookie': 'true',
          },
        }
      : {}),
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
  reporter: [['list'], ['html', { open: 'never', outputFolder: 'playwright-report' }]],
});
