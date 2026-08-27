import { test, expect } from '@playwright/test';
import { registerOrg, uid } from './helpers';

test.describe('Customers', () => {
  test('customers page loads for authenticated user', async ({ page }) => {
    await registerOrg(page, `Customer Page ${uid()}`);
    await page.goto('/customers');

    await expect(page.getByRole('heading', { name: /customers/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /new customer/i })).toBeVisible();
  });

  test('New Customer button toggles create form', async ({ page }) => {
    await registerOrg(page, `Customer Form Toggle ${uid()}`);
    await page.goto('/customers');

    // Form hidden initially
    await expect(page.getByText('Create Customer')).not.toBeVisible();

    await page.getByRole('button', { name: /new customer/i }).click();
    await expect(page.getByText('Create Customer')).toBeVisible();
    await expect(page.getByLabel('Customer Name')).toBeVisible();
    await expect(page.getByLabel('Address')).toBeVisible();
  });

  test('create a customer and see it in the table', async ({ page }) => {
    await registerOrg(page, `Customer Create ${uid()}`);
    await page.goto('/customers');

    const custName = `Test Customer ${uid()}`;

    await page.getByRole('button', { name: /new customer/i }).click();
    await page.getByLabel('Customer Name').fill(custName);
    await page.getByLabel('Address').fill('456 Market Road, Pune');
    await page.getByRole('button', { name: /^create customer$/i }).click();

    // Customer should appear in the table
    await expect(page.getByText(custName)).toBeVisible({ timeout: 8000 });
    await expect(page.getByText('456 Market Road, Pune')).toBeVisible();
  });

  test('create multiple customers and count badge updates', async ({ page }) => {
    await registerOrg(page, `Customer Count ${uid()}`);
    await page.goto('/customers');

    for (let i = 1; i <= 2; i++) {
      await page.getByRole('button', { name: /new customer/i }).click();
      await page.getByLabel('Customer Name').fill(`Multi Customer ${i} ${uid()}`);
      await page.getByLabel('Address').fill(`${i} Lane, City`);
      await page.getByRole('button', { name: /^create customer$/i }).click();
      // Wait for form to close
      await expect(page.getByText('Create Customer')).not.toBeVisible({ timeout: 5000 });
    }

    // Badge should show at least 2
    const badge = page.locator('.table-toolbar .badge');
    await expect(badge).toHaveText(/[2-9]|\d{2,}/, { timeout: 8000 });
  });

  test('cancel button hides the create form', async ({ page }) => {
    await registerOrg(page, `Customer Cancel ${uid()}`);
    await page.goto('/customers');

    await page.getByRole('button', { name: /new customer/i }).click();
    await expect(page.getByLabel('Customer Name')).toBeVisible();

    await page.getByRole('button', { name: /cancel/i }).click();
    await expect(page.getByLabel('Customer Name')).not.toBeVisible();
  });

  test('customer name appears in dispatch customer dropdown', async ({ page }) => {
    const org = await registerOrg(page, `Customer Dispatch ${uid()}`);
    const custName = `Dispatch Customer ${uid()}`;

    // Create customer
    await page.goto('/customers');
    await page.getByRole('button', { name: /new customer/i }).click();
    await page.getByLabel('Customer Name').fill(custName);
    await page.getByLabel('Address').fill('789 Hub Street, Delhi');
    await page.getByRole('button', { name: /^create customer$/i }).click();
    await expect(page.getByText(custName)).toBeVisible({ timeout: 8000 });

    // Navigate to org detail and check dispatch dropdown
    await page.goto(`/orgs/${org.id}`);
    const select = page.getByLabel('Customer');
    await expect(select).toBeVisible();
    await expect(select.getByRole('option', { name: custName })).toBeAttached({ timeout: 8000 });
  });
});
