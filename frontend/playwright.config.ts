import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  fullyParallel: false, // tests share a DB; run sequentially to avoid races
  retries: 0,
  workers: 1,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: 'http://localhost:5173',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    headless: true,
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        // In CI, let Playwright manage the browser path after `playwright install`.
        // Locally, point at the full Chromium binary (headless shell may need sudo).
        ...(process.env.CI
          ? {}
          : {
              executablePath: `${process.env.HOME}/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome`,
              channel: undefined,
            }),
      },
    },
  ],
  // Assumes `vite dev` and the Rust backend are already running.
  // Start them manually: cargo run  &&  vite dev
});
