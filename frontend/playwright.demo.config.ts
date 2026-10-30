import { defineConfig, devices } from '@playwright/test';

/**
 * Config for the visual, headed "demo" runs. Unlike playwright.config.ts
 * (headless, the regular suite) this always opens a real browser window,
 * slowed down, so a human can watch the flow instead of running it by hand.
 *
 * Matches the full-workflow demo (`e2e-full-flow.spec.ts`) and every focused
 * per-feature demo (`*.demo.ts` — kept out of the headless suite by not
 * ending in `.spec.ts`).
 *
 *   npm run test:e2e:demo          — the whole workflow
 *   npm run test:e2e:demo:reports  — just the ops-reporting flow
 *
 * It starts the Vite dev server and the Rust API itself if they aren't
 * already running (reusing them if they are), so this is a single command.
 */
// Ports default to the usual 5173 (Vite) and 8080 (API). Set E2E_WEB_PORT and
// E2E_API_PORT to run a worktree's demo beside servers already on those ports.
const webPort = process.env.E2E_WEB_PORT || '5173';
const apiPort = process.env.E2E_API_PORT || '8080';
const webURL = process.env.E2E_BASE_URL || `http://localhost:${webPort}`;

export default defineConfig({
  testDir: './tests',
  testMatch: /(e2e-full-flow\.spec|\.demo)\.ts$/,
  fullyParallel: false,
  retries: 0,
  workers: 1,
  timeout: 120_000,
  reporter: 'list',
  use: {
    baseURL: webURL,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'on',
    headless: false,
    launchOptions: {
      // Slow every Playwright-driven action down so the flow is easy to
      // follow with the naked eye instead of flashing by.
      slowMo: 400,
    },
    viewport: { width: 1440, height: 900 },
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        ...(process.env.CI
          ? {}
          : {
              executablePath: `${process.env.HOME}/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome`,
              channel: undefined,
            }),
      },
    },
  ],
  webServer: [
    {
      command: `npm run dev -- --port ${webPort} --strictPort`,
      url: webURL,
      env: { API_PROXY_TARGET: `http://localhost:${apiPort}` },
      reuseExistingServer: true,
      timeout: 60_000,
    },
    {
      command: 'cargo run --bin logistics-system',
      cwd: '..',
      url: `http://127.0.0.1:${apiPort}/api/orgs`,
      env: { PORT: apiPort },
      reuseExistingServer: true,
      timeout: 300_000,
    },
  ],
});
