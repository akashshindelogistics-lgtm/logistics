import { test, expect } from '@playwright/test';
import { registerOrg, seedPaidHire, uid } from './helpers';

test.describe('Vendor payments', () => {
  test('pay off a hired truck and see it in the report', async ({ page }) => {
    const org = await registerOrg(page, `Vendor Pay ${uid()}`);
    const vendorName = await seedPaidHire(page, org);

    await page.goto('/vendors');
    const vendorRow = page.getByRole('row', { name: new RegExp(vendorName) }).first();
    await expect(vendorRow.getByTestId('vendor-outstanding')).toHaveText('3,000', { timeout: 8000 });

    const hireRow = page.getByTestId('hire-row').filter({ hasText: 'MH12 HR 7788' });
    await hireRow.getByRole('button', { name: /record payment/i }).click();
    await expect(page.getByLabel('Amount')).toHaveValue('3000');

    // More than is owed is refused.
    await page.getByLabel('Amount').fill('5000');
    await page.getByRole('button', { name: /save payment/i }).click();
    await expect(page.getByText(/more than the 3000 still owed/i)).toBeVisible({ timeout: 8000 });

    await page.getByLabel('Amount').fill('3000');
    await page.getByLabel('Note').fill('balance on delivery');
    await page.getByRole('button', { name: /save payment/i }).click();
    await expect(hireRow.getByText('Paid', { exact: true })).toBeVisible({ timeout: 8000 });
    await expect(vendorRow.getByTestId('vendor-outstanding')).toHaveText('—');

    await hireRow.getByRole('button', { name: /^payments$/i }).click();
    await expect(page.getByTestId('payment-history')).toContainText('balance on delivery');

    await page.goto('/reports');
    const section = page.getByTestId('hired-transport');
    await expect(section).toContainText('1 of 1 dispatches on hired trucks (100.0%)', { timeout: 8000 });
    await expect(section.getByTestId('owed-to-vendors')).toHaveText('0');
    await expect(section.getByTestId('hire-margin-row')).toContainText('3,500');
  });
});
