import { test, expect } from '@playwright/test';
import { registerOrg, uid } from './helpers';

test.describe('Vehicle vendors', () => {
  test('add, edit, deactivate and delete a vendor', async ({ page }) => {
    await registerOrg(page, `Vendor Org ${uid()}`);
    const name = `Balaji Transport ${uid()}`;

    await page.getByRole('link', { name: 'Vendors' }).click();
    await expect(page).toHaveURL(/\/vendors$/);
    await expect(page.getByText(/no vendors yet/i)).toBeVisible();

    await page.getByRole('button', { name: /add vendor/i }).click();
    await page.getByLabel('Vendor name').fill(name);
    await page.getByLabel('Phone').fill('022 2345 6789');
    await page.getByLabel('GSTIN').fill('27aapfu0939f1zv');
    await page.getByRole('button', { name: /^add vendor$/i }).click();

    const row = page.getByRole('row', { name: new RegExp(name) });
    await expect(row).toBeVisible({ timeout: 8000 });
    // The server upper-cases the GSTIN.
    await expect(row.getByText('27AAPFU0939F1ZV')).toBeVisible();
    await expect(row.getByText('Active')).toBeVisible();

    await row.getByRole('button', { name: `Edit ${name}` }).click();
    await page.getByLabel('Contact person').fill('Meena Iyer');
    await page.getByLabel('Notes').fill('Pune–Nashik lane, 10T trucks');
    await page.getByRole('button', { name: /save vendor/i }).click();
    await expect(row.getByText('Meena Iyer')).toBeVisible({ timeout: 8000 });
    await expect(row.getByText('Pune–Nashik lane, 10T trucks')).toBeVisible();

    await row.getByRole('button', { name: `Deactivate ${name}` }).click();
    await expect(row.getByText('Inactive')).toBeVisible({ timeout: 8000 });

    page.once('dialog', d => d.accept());
    await row.getByRole('button', { name: `Delete ${name}` }).click();
    await expect(page.getByText(/no vendors yet/i)).toBeVisible({ timeout: 8000 });
  });

  test('shows the server error for a malformed GSTIN', async ({ page }) => {
    await registerOrg(page, `Vendor Validation ${uid()}`);
    await page.goto('/vendors');
    await page.getByRole('button', { name: /add vendor/i }).click();
    await page.getByLabel('Vendor name').fill('Bad GSTIN Carriers');
    await page.getByLabel('Phone').fill('1');
    await page.getByLabel('GSTIN').fill('ABC123');
    await page.getByRole('button', { name: /^add vendor$/i }).click();
    await expect(page.getByText(/GSTIN must be 15 letters and digits/i)).toBeVisible({ timeout: 8000 });
  });
});
