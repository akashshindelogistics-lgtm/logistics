import { test, expect } from '@playwright/test';
import { registerOrg, seedPaidHire, uid } from './helpers';

/**
 * A narrated walk through **paying vendors** on its own: a truck hired for
 * 9,000 with a 6,000 advance is settled on the Vendors page, and the Reports
 * page shows spend, what's owed and the margin the hire made.
 *
 * Phase 3 of docs/vehicle-vendors.md.
 * Watch it with `npm run test:e2e:demo:vendor-payments` (headed, slowed).
 */
test('paying vendors for hired trucks', async ({ page }) => {
  test.slow();
  const org = await registerOrg(page, `Vendor Payments Demo ${uid()}`);
  let vendorName = '';

  await test.step('A dispatch went out on a hired truck: 9,000 agreed, 6,000 advance, invoiced 12,500', async () => {
    vendorName = await seedPaidHire(page, org);
  });

  const hireRow = page.getByTestId('hire-row').filter({ hasText: 'MH12 HR 7788' });

  await test.step('The Vendors page shows what is still owed', async () => {
    await page.getByRole('link', { name: 'Vendors' }).click();
    const vendorRow = page.getByRole('row', { name: new RegExp(vendorName) }).first();
    await expect(vendorRow.getByTestId('vendor-outstanding')).toHaveText('3,000', { timeout: 8000 });
    await expect(hireRow).toContainText('3,000');
    await page.waitForTimeout(1500);
  });

  await test.step('Pay the balance on delivery', async () => {
    await hireRow.getByRole('button', { name: /record payment/i }).click();
    await page.getByLabel('Note').fill('balance on delivery');
    await page.waitForTimeout(800);
    await page.getByRole('button', { name: /save payment/i }).click();
    await expect(hireRow.getByText('Paid', { exact: true })).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(1000);
  });

  await test.step('The payment history shows the advance and the balance', async () => {
    await hireRow.getByRole('button', { name: /^payments$/i }).click();
    await expect(page.getByTestId('payment-history')).toContainText('balance on delivery');
    await page.waitForTimeout(1500);
  });

  await test.step('Reports: hired share, vendor spend and the margin on this hire', async () => {
    await page.getByRole('link', { name: 'Reports' }).click();
    const section = page.getByTestId('hired-transport');
    await expect(section.getByTestId('hire-margin-row')).toContainText('3,500', { timeout: 8000 });
    await section.scrollIntoViewIfNeeded();
    await page.waitForTimeout(2500);
  });
});
