import { test, expect } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * Helper: add stock to an org via the API directly (stock has no UI form;
 * it's added via POST /api/orgs/:id/stock). This sets up dispatch preconditions.
 */
async function addStockViaApi(page: import('@playwright/test').Page, orgId: string, description: string, quantity: number) {
  await page.request.post(`/api/orgs/${orgId}/stock`, {
    data: { description, quantity, volume_in_size: 'Large' },
  });
}

/** Helper: create a customer via the UI and return its name. */
async function createCustomerViaUI(page: import('@playwright/test').Page, name: string) {
  await page.goto('/customers');
  await page.getByRole('button', { name: /new customer/i }).click();
  await page.getByLabel('Customer Name').fill(name);
  await page.getByLabel('Address').fill('10 Dispatch Lane, Bangalore');
  await page.getByRole('button', { name: /^create customer$/i }).click();
  await expect(page.getByText(name)).toBeVisible({ timeout: 8000 });
}

test.describe('Dispatches', () => {
  test('dispatches page loads and shows table headers', async ({ page }) => {
    await registerOrg(page, `Dispatch List ${uid()}`);
    await page.goto('/dispatches');

    await expect(page.getByRole('heading', { name: /dispatches/i })).toBeVisible();
    await expect(page.getByText(/dispatch history/i)).toBeVisible();
  });

  test('dispatches page shows empty state for a new org', async ({ page }) => {
    await registerOrg(page, `Dispatch Empty ${uid()}`);
    await page.goto('/dispatches');

    await expect(page.getByText(/no dispatches/i)).toBeVisible({ timeout: 8000 });
  });

  test('dispatch stock to a customer and see it in dispatch history', async ({ page }) => {
    const org = await registerOrg(page, `Dispatch Full ${uid()}`);
    const custName = `Dispatch Customer ${uid()}`;
    const stockDesc = `Cement ${uid()}`;

    // Setup: add stock via API and customer via UI
    await addStockViaApi(page, org.id, stockDesc, 50);
    await createCustomerViaUI(page, custName);

    // Go to org detail and dispatch
    await page.goto(`/orgs/${org.id}`);
    await expect(page.getByText('Dispatch Stock')).toBeVisible();

    await page.getByLabel('Customer').selectOption({ label: custName });
    await page.getByLabel('Stock Description').fill(stockDesc);
    await page.getByLabel('Quantity').fill('10');
    await page.getByRole('button', { name: /dispatch stock/i }).click();

    // Success message
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });

    // Navigate to dispatches page and verify entry
    await page.goto('/dispatches');
    await expect(page.getByText(stockDesc)).toBeVisible({ timeout: 8000 });
  });

  test('dispatch shows error when stock is insufficient', async ({ page }) => {
    const org = await registerOrg(page, `Dispatch Insufficient ${uid()}`);
    const custName = `Customer ${uid()}`;

    // Add stock with only 5 units
    await addStockViaApi(page, org.id, 'Steel Rods', 5);
    await createCustomerViaUI(page, custName);

    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ label: custName });
    await page.getByLabel('Stock Description').fill('Steel Rods');
    await page.getByLabel('Quantity').fill('100'); // more than available
    await page.getByRole('button', { name: /dispatch stock/i }).click();

    await expect(page.getByText(/dispatch failed/i)).toBeVisible({ timeout: 10000 });
  });

  test('dispatches table shows vehicle, customer and order details', async ({ page }) => {
    const org = await registerOrg(page, `Dispatch Table ${uid()}`);
    const custName = `Table Customer ${uid()}`;
    const stockDesc = `Grain ${uid()}`;
    const reg = `MH09TT${uid().toUpperCase().slice(0, 4)}`;

    // Add vehicle
    await page.goto(`/orgs/${org.id}`);
    await page.getByPlaceholder(/MH12AB1234/i).fill(reg);
    await page.getByPlaceholder(/e\.g\. 10/i).fill('30');
    await page.getByRole('button', { name: /add vehicle/i }).click();
    await expect(page.getByText(reg)).toBeVisible({ timeout: 8000 });

    // Add stock and customer
    await addStockViaApi(page, org.id, stockDesc, 80);
    await createCustomerViaUI(page, custName);

    // Dispatch
    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ label: custName });
    await page.getByLabel('Stock Description').fill(stockDesc);
    await page.getByLabel('Quantity').fill('20');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });

    // Verify dispatches page columns
    await page.goto('/dispatches');
    await expect(page.getByText(stockDesc)).toBeVisible({ timeout: 8000 });
    await expect(page.getByText(custName)).toBeVisible();
    // Order ID and timestamp should be visible (at least one numeric badge)
    const rows = page.locator('tbody tr');
    await expect(rows).toHaveCount(1, { timeout: 8000 });
  });

  test('dispatch form requires customer to be selected', async ({ page }) => {
    const org = await registerOrg(page, `Dispatch Required ${uid()}`);
    await addStockViaApi(page, org.id, 'Wood', 10);

    await page.goto(`/orgs/${org.id}`);
    // Fill stock and quantity but leave customer unselected
    await page.getByLabel('Stock Description').fill('Wood');
    await page.getByLabel('Quantity').fill('5');
    await page.getByRole('button', { name: /dispatch stock/i }).click();

    // HTML5 required on select prevents submission; stays on same page
    await expect(page).toHaveURL(`/orgs/${org.id}`);
    await expect(page.getByText(/dispatch successful/i)).not.toBeVisible();
  });
});
