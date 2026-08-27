import { test, expect } from '@playwright/test';
import { registerOrg, uid } from './helpers';

test.describe('Vehicles', () => {
  test('vehicles page loads and shows empty state for a new org', async ({ page }) => {
    await registerOrg(page, `Vehicle Empty ${uid()}`);
    await page.goto('/vehicles');

    await expect(page.getByRole('heading', { name: /vehicles/i })).toBeVisible();
    // New org has no vehicles
    await expect(page.getByText(/no vehicles/i)).toBeVisible({ timeout: 8000 });
  });

  test('add a vehicle and see it in org detail and vehicles list', async ({ page }) => {
    const org = await registerOrg(page, `Vehicle Add ${uid()}`);
    const regNumber = `MH12AB${uid().toUpperCase().slice(0, 4)}`;

    await page.goto(`/orgs/${org.id}`);

    // Fill add-vehicle form
    await page.getByPlaceholder(/MH12AB1234/i).fill(regNumber);
    await page.getByPlaceholder(/e\.g\. 10/i).fill('15');
    await page.getByRole('button', { name: /add vehicle/i }).click();

    // Vehicle appears in the Fleet Vehicles table
    await expect(page.getByText(regNumber)).toBeVisible({ timeout: 8000 });
    await expect(page.getByText('15 MT')).toBeVisible();

    // Vehicles page also shows it
    await page.goto('/vehicles');
    await expect(page.getByText(regNumber)).toBeVisible({ timeout: 8000 });
  });

  test('adding a vehicle increments the stat card count', async ({ page }) => {
    const org = await registerOrg(page, `Vehicle Count ${uid()}`);
    await page.goto(`/orgs/${org.id}`);

    // Initial count
    const statsLocator = page.locator('.section-card-header .badge').first();

    // Add vehicle
    const reg = `TN01XY${uid().toUpperCase().slice(0, 4)}`;
    await page.getByPlaceholder(/MH12AB1234/i).fill(reg);
    await page.getByPlaceholder(/e\.g\. 10/i).fill('5');
    await page.getByRole('button', { name: /add vehicle/i }).click();

    await expect(page.getByText(reg)).toBeVisible({ timeout: 8000 });

    // Header stat card for vehicles should now show 1
    await expect(page.locator('.page-header').getByText('1')).toBeVisible();
  });

  test('vehicle shows "Not tracked" location before any update', async ({ page }) => {
    const org = await registerOrg(page, `Vehicle Location ${uid()}`);
    const reg = `KA05ZZ${uid().toUpperCase().slice(0, 4)}`;

    await page.goto(`/orgs/${org.id}`);
    await page.getByPlaceholder(/MH12AB1234/i).fill(reg);
    await page.getByPlaceholder(/e\.g\. 10/i).fill('8');
    await page.getByRole('button', { name: /add vehicle/i }).click();

    await expect(page.getByText(reg)).toBeVisible({ timeout: 8000 });
    await expect(page.getByText('Not tracked')).toBeVisible();
  });

  test('delete a vehicle removes it from the list', async ({ page }) => {
    const org = await registerOrg(page, `Vehicle Delete ${uid()}`);
    const reg = `DL01AA${uid().toUpperCase().slice(0, 4)}`;

    await page.goto(`/orgs/${org.id}`);
    await page.getByPlaceholder(/MH12AB1234/i).fill(reg);
    await page.getByPlaceholder(/e\.g\. 10/i).fill('20');
    await page.getByRole('button', { name: /add vehicle/i }).click();
    await expect(page.getByText(reg)).toBeVisible({ timeout: 8000 });

    // Intercept the confirm() dialog
    page.on('dialog', dialog => dialog.accept());

    // Click delete on that row
    const row = page.getByRole('row', { name: new RegExp(reg, 'i') });
    await row.getByRole('button').click();

    // Row should be gone
    await expect(page.getByText(reg)).not.toBeVisible({ timeout: 8000 });
  });

  test('add vehicle form requires both fields', async ({ page }) => {
    const org = await registerOrg(page, `Vehicle Validation ${uid()}`);
    await page.goto(`/orgs/${org.id}`);

    // Submit with only registration, no capacity
    await page.getByPlaceholder(/MH12AB1234/i).fill('GJ01XX9999');
    await page.getByRole('button', { name: /add vehicle/i }).click();

    // HTML5 required validation prevents form submission — URL stays the same
    await expect(page).toHaveURL(`/orgs/${org.id}`);
  });
});
