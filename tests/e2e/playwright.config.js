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
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
  reporter: [['list'], ['html', { open: 'never', outputFolder: 'playwright-report' }]],
});
