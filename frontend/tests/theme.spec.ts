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

  test('applies the Oxford & Claret palette in both themes', async ({ page }) => {
    await registerOrg(page, `Theme Palette ${uid()}`);
    await page.goto('/');

    // Oxford navy brand, a navy gradient sidebar and a gold "Customers" stat card in light mode…
    const brand = () => page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--brand').trim());
    expect(await brand()).toBe('#1f3a6b');
    await expect(page.locator('.sidebar')).toHaveCSS('background-image', /linear-gradient\(rgb\(18, 29, 54\)/);
    await expect(page.locator('.card-amber .stat-value')).toHaveCSS('color', 'rgb(154, 106, 18)');

    // …and their brighter counterparts in dark mode.
    await page.getByRole('button', { name: /switch to dark theme/i }).click();
    expect(await brand()).toBe('#93abe3');
    await expect(page.locator('.card-amber .stat-value')).toHaveCSS('color', 'rgb(227, 184, 90)');
  });
});
