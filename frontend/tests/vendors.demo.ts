import { test, expect } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A short narrated walk through **vehicle vendors** on its own: an org with
 * no trucks of its own records the transporters it phones for a hired
 * vehicle, edits one, and parks another as inactive.
 *
 * Phase 1 of docs/vehicle-vendors.md — hiring a vendor's truck for a
 * dispatch comes in phase 2.
 *
 * Watch it with `npm run test:e2e:demo:vendors` (headed, slowed).
 */
test('vehicle vendors', async ({ page }) => {
  test.slow();
  await registerOrg(page, `Vendors Demo ${uid()}`);
  const sharma = `Sharma Roadlines ${uid()}`;
  const balaji = `Balaji Transport ${uid()}`;

  await test.step('Open the Vendors page from the sidebar', async () => {
    await page.getByRole('link', { name: 'Vendors' }).click();
    await expect(page).toHaveURL(/\/vendors$/);
    await expect(page.getByText(/no vendors yet/i)).toBeVisible();
    await page.waitForTimeout(800);
  });

  await test.step('Add the transporter the dispatcher usually calls', async () => {
    await page.getByRole('button', { name: /add vendor/i }).click();
    await page.getByLabel('Vendor name').fill(sharma);
    await page.getByLabel('Contact person').fill('Anil Sharma');
    await page.getByLabel('Phone').fill('+91 98200 11111');
    await page.getByLabel('GSTIN').fill('27AAPFU0939F1ZV');
    await page.getByLabel('Notes').fill('Mumbai–Pune lane, 10T and 20T trucks, 80% advance');
    await page.getByRole('button', { name: /^add vendor$/i }).click();
    await expect(page.getByRole('row', { name: new RegExp(sharma) })).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(800);
  });

  await test.step('Add a second, backup vendor', async () => {
    await page.getByRole('button', { name: /add vendor/i }).click();
    await page.getByLabel('Vendor name').fill(balaji);
    await page.getByLabel('Phone').fill('022 2345 6789');
    await page.getByRole('button', { name: /^add vendor$/i }).click();
    await expect(page.getByRole('row', { name: new RegExp(balaji) })).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(800);
  });

  await test.step('A malformed GSTIN is rejected with a clear message', async () => {
    const row = page.getByRole('row', { name: new RegExp(balaji) });
    await row.getByRole('button', { name: `Edit ${balaji}` }).click();
    await page.getByLabel('GSTIN').fill('12345');
    await page.getByRole('button', { name: /save vendor/i }).click();
    await expect(page.getByText(/GSTIN must be 15 letters and digits/i)).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(1000);
  });

  await test.step('Fix the edit: add the contact person instead', async () => {
    await page.getByLabel('GSTIN').fill('');
    await page.getByLabel('Contact person').fill('Meena Iyer');
    await page.getByRole('button', { name: /save vendor/i }).click();
    const row = page.getByRole('row', { name: new RegExp(balaji) });
    await expect(row.getByText('Meena Iyer')).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(800);
  });

  await test.step('Park the backup vendor as inactive — it drops to the bottom', async () => {
    const row = page.getByRole('row', { name: new RegExp(balaji) });
    await row.getByRole('button', { name: `Deactivate ${balaji}` }).click();
    await expect(row.getByText('Inactive')).toBeVisible({ timeout: 8000 });
    await expect(page.locator('tbody tr').last()).toContainText(balaji);
    await page.waitForTimeout(1500);
  });
});
