import { test, expect } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * Helper: add stock to an org via the API directly (stock has no UI form).
 * Reads the JWT from localStorage so the request is authenticated.
 */
async function addStockViaApi(page: import('@playwright/test').Page, orgId: string, description: string, quantity: number) {
  const token = await page.evaluate(() => localStorage.getItem('logi_token'));
  const resp = await page.request.post(`/api/orgs/${orgId}/stock`, {
    data: { description, quantity, volume_in_size: 100 },
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!resp.ok()) {
    throw new Error(`addStockViaApi failed: ${resp.status()} ${await resp.text()}`);
  }
}

/**
 * Create a customer via the UI and set their location via API (required for dispatch).
 * Returns the customer id extracted from the network response.
 */
async function createCustomerWithLocation(page: import('@playwright/test').Page, name: string): Promise<string> {
  const token = await page.evaluate(() => localStorage.getItem('logi_token'));

  // Create via API directly (faster and gives us the id)
  const resp = await page.request.post('/api/customers', {
    data: { name, address: '10 Dispatch Lane, Bangalore' },
    headers: { Authorization: `Bearer ${token}` },
  });
  const body = await resp.json() as { data: { id: string } };
  const customerId = body.data.id;

  // Set location (required for dispatch)
  await page.request.put(`/api/customers/${customerId}/location`, {
    data: { latitude: 18.5204, longitude: 73.8567, address: '10 Dispatch Lane, Bangalore' },
    headers: { Authorization: `Bearer ${token}` },
  });

  return customerId;
}

test.describe('Dispatches', () => {
  test('dispatches page loads and shows table headers', async ({ page }) => {
    await registerOrg(page, `Dispatch List ${uid()}`);
    await page.goto('/dispatches');

    // Scope to the page's <h1>; a regex match would also hit the empty-state
    // <h3>No dispatch orders yet</h3> and trip strict mode.
    await expect(page.getByRole('heading', { level: 1, name: 'Dispatch Orders' })).toBeVisible();
    await expect(page.getByText(/all orders/i)).toBeVisible();
  });

  test('dispatches page shows empty state for a new org', async ({ page }) => {
    await registerOrg(page, `Dispatch Empty ${uid()}`);
    await page.goto('/dispatches');

    await expect(page.getByText(/no dispatch orders yet/i)).toBeVisible({ timeout: 8000 });
  });

  test('dispatch stock to a customer and see it in dispatch history', async ({ page }) => {
    const org = await registerOrg(page, `Dispatch Full ${uid()}`);
    const custName = `Dispatch Customer ${uid()}`;
    const stockDesc = `Cement ${uid()}`;
    const token = await page.evaluate(() => localStorage.getItem('logi_token'));

    // Setup: vehicle (required by dispatch), stock and customer with location
    await page.request.post(`/api/orgs/${org.id}/vehicles`, {
      data: { registration_number: `DF${uid().toUpperCase().slice(0,6)}`, capacity: 20, unit: 'MetricTon' },
      headers: { Authorization: `Bearer ${token}` },
    });
    await addStockViaApi(page, org.id, stockDesc, 50);
    const custId = await createCustomerWithLocation(page, custName);

    // Go to org detail and dispatch
    await page.goto(`/orgs/${org.id}`);
    await expect(page.getByText('Dispatch Stock').first()).toBeVisible();

    await page.getByLabel('Customer').selectOption({ value: custId });
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

    // Add stock with only 5 units and customer with location
    await addStockViaApi(page, org.id, 'Steel Rods', 5);
    const custId = await createCustomerWithLocation(page, custName);

    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ value: custId });
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
    await page.getByLabel('Registration Number').fill(reg);
    await page.getByLabel('Capacity (MT)').fill('30');
    await page.getByRole('button', { name: /add vehicle/i }).click();
    await expect(page.getByText(reg)).toBeVisible({ timeout: 8000 });

    // Add stock and customer with location (required for dispatch)
    await addStockViaApi(page, org.id, stockDesc, 80);
    const custId = await createCustomerWithLocation(page, custName);

    // Dispatch
    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ value: custId });
    await page.getByLabel('Stock Description').fill(stockDesc);
    await page.getByLabel('Quantity').fill('20');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });

    // Verify dispatches page columns (shows stock desc, vehicle reg, status — not customer name)
    await page.goto('/dispatches');
    await expect(page.getByText(stockDesc)).toBeVisible({ timeout: 8000 });
    await expect(page.getByText(reg)).toBeVisible();
    await expect(page.getByText('DISPATCHED', { exact: true })).toBeVisible();
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
