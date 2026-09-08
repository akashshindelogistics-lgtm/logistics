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
export default defineConfig({
  testDir: './tests',
  testMatch: /(e2e-full-flow\.spec|\.demo)\.ts$/,
  fullyParallel: false,
  retries: 0,
  workers: 1,
  timeout: 120_000,
  reporter: 'list',
  use: {
    baseURL: 'http://localhost:5173',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
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
      command: 'npm run dev',
      url: 'http://localhost:5173',
      reuseExistingServer: true,
      timeout: 60_000,
    },
    {
      command: 'cargo run --bin logistics-system',
      cwd: '..',
      url: 'http://127.0.0.1:8080/api/orgs',
      reuseExistingServer: true,
      timeout: 300_000,
    },
  ],
});
