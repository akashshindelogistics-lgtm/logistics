import { test, expect } from '@playwright/test';
import { registerOrg, uid } from './helpers';

test.describe('Theme toggle', () => {
  test('switches to dark mode and persists across a reload', async ({ page }) => {
    await registerOrg(page, `Theme Toggle ${uid()}`);

    const html = page.locator('html');
    await expect(html).toHaveAttribute('data-theme', 'light');

    const toggle = page.getByRole('button', { name: /switch to dark theme/i });
    await toggle.click();

    await expect(html).toHaveAttribute('data-theme', 'dark');
    await expect(page.getByRole('button', { name: /switch to light theme/i })).toBeVisible();

    await page.reload();
    await expect(html).toHaveAttribute('data-theme', 'dark');
  });
});
