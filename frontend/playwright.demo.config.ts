import { defineConfig, devices } from '@playwright/test';

/**
 * Config for the visual, headed "demo" run of the full logistics workflow
 * (registration -> login -> godowns -> fleet -> drivers -> customers ->
 * dispatch -> delivery). Unlike playwright.config.ts (headless, used for the
 * regular test suite), this always opens a real browser window, slowed down,
 * so a human can watch the whole flow happen instead of running it by hand.
 *
 * Run with: npm run test:e2e:demo
 * It starts the Vite dev server and the Rust API itself if they aren't
 * already running (reusing them if they are), so this is a single command.
 */
export default defineConfig({
  testDir: './tests',
  testMatch: 'e2e-full-flow.spec.ts',
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
