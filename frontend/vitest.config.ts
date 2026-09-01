import { defineConfig, mergeConfig } from 'vitest/config';
import viteConfig from './vite.config.ts';

// Unit/component tests (Vitest + React Testing Library), separate from the
// Playwright end-to-end suite in ./tests (testDir there keeps Playwright
// from ever looking under src/, and `include` below keeps Vitest from ever
// looking under ./tests).
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      globals: true,
      setupFiles: ['./src/test/setup.ts'],
      include: ['src/**/*.{test,spec}.{ts,tsx}'],
      css: true,
    },
  }),
);
