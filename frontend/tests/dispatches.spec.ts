import { test, expect } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * Helper: add stock to an org via the API directly (stock lives in a godown,
 * which this creates on the fly, and dispatch sums stock across all of an
 * org's godowns so tests don't need to care which one).
 * Reads the JWT from localStorage so the request is authenticated.
 */
async function addStockViaApi(page: import('@playwright/test').Page, orgId: string, description: string, quantity: number) {
  const token = await page.evaluate(() => localStorage.getItem('logi_token'));
  const godownResp = await page.request.post(`/api/orgs/${orgId}/godowns`, {
    data: { name: `Godown ${description}`, address: '1 Warehouse Road' },
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!godownResp.ok()) {
    throw new Error(`addStockViaApi: create godown failed: ${godownResp.status()} ${await godownResp.text()}`);
  }
  const godownBody = await godownResp.json() as { data: { id: string } };

  const resp = await page.request.post(`/api/godowns/${godownBody.data.id}/stock`, {
    // volume_in_size 1 so a modest vehicle capacity comfortably covers the shipment.
    data: { description, quantity, volume_in_size: 1 },
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!resp.ok()) {
    throw new Error(`addStockViaApi failed: ${resp.status()} ${await resp.text()}`);
  }
}

/**
 * Create a driver and assign them to a vehicle via the API. A vehicle needs
 * an active assigned driver before it can be selected for a dispatch.
 */
async function assignActiveDriver(
  page: import('@playwright/test').Page,
  orgId: string,
  vehicleReg: string,
) {
  const token = await page.evaluate(() => localStorage.getItem('logi_token'));
  const driverResp = await page.request.post(`/api/orgs/${orgId}/drivers`, {
    data: { name: 'E2E Driver', license_number: 'E2E-LIC-001', phone: '+91 90000 00000' },
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!driverResp.ok()) {
    throw new Error(`assignActiveDriver: create driver failed: ${driverResp.status()} ${await driverResp.text()}`);
  }
  const driver = (await driverResp.json()) as { data: { id: string } };

  const assignResp = await page.request.put(`/api/vehicles/${encodeURIComponent(vehicleReg)}/driver`, {
    data: { driver_id: driver.data.id },
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!assignResp.ok()) {
    throw new Error(`assignActiveDriver: assign failed: ${assignResp.status()} ${await assignResp.text()}`);
  }
}

/**
 * Create a customer under the org via API and set their location (required for
 * dispatch). Returns the customer id extracted from the network response.
 */
async function createCustomerWithLocation(
  page: import('@playwright/test').Page,
  orgId: string,
  name: string,
): Promise<string> {
  const token = await page.evaluate(() => localStorage.getItem('logi_token'));

  const resp = await page.request.post(`/api/orgs/${orgId}/customers`, {
    data: { name, address: '10 Dispatch Lane, Bangalore' },
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!resp.ok()) {
    throw new Error(`createCustomerWithLocation: ${resp.status()} ${await resp.text()}`);
  }
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

    // Setup: vehicle + active driver (both required by dispatch), stock and
    // customer with location
    const vehReg = `DF${uid().toUpperCase().slice(0, 6)}`;
    await page.request.post(`/api/orgs/${org.id}/vehicles`, {
      data: { registration_number: vehReg, capacity: 20, unit: 'MetricTon' },
      headers: { Authorization: `Bearer ${token}` },
    });
    await assignActiveDriver(page, org.id, vehReg);
    await addStockViaApi(page, org.id, stockDesc, 50);
    const custId = await createCustomerWithLocation(page, org.id, custName);

    // Go to org detail and dispatch
    await page.goto(`/orgs/${org.id}`);
    await expect(page.getByText('Dispatch Stock').first()).toBeVisible();

    await page.getByLabel('Customer').selectOption({ value: custId });
    await page.getByLabel('Stock Description').fill(stockDesc);
    await page.getByLabel('Quantity', { exact: true }).fill('10');
    await page.getByRole('button', { name: /dispatch stock/i }).click();

    // Success message
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });

    // Navigate to dispatches page and verify entry
    await page.goto('/dispatches');
    await expect(page.getByText(stockDesc)).toBeVisible({ timeout: 8000 });
  });

  test('raise a freight invoice for a dispatch and mark it paid', async ({ page }) => {
    const org = await registerOrg(page, `Billing Flow ${uid()}`);
    const custName = `Billing Customer ${uid()}`;
    const stockDesc = `Pipes ${uid()}`;
    const token = await page.evaluate(() => localStorage.getItem('logi_token'));

    const vehReg = `BF${uid().toUpperCase().slice(0, 6)}`;
    await page.request.post(`/api/orgs/${org.id}/vehicles`, {
      data: { registration_number: vehReg, capacity: 100, unit: 'MetricTon' },
      headers: { Authorization: `Bearer ${token}` },
    });
    await assignActiveDriver(page, org.id, vehReg);
    await addStockViaApi(page, org.id, stockDesc, 50);
    const custId = await createCustomerWithLocation(page, org.id, custName);

    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ value: custId });
    await page.getByLabel('Stock Description').fill(stockDesc);
    await page.getByLabel('Quantity', { exact: true }).fill('10');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });

    await page.goto('/dispatches');
    const row = page.locator('tbody tr').first();
    await expect(row.getByText(stockDesc)).toBeVisible({ timeout: 8000 });

    // Raise an invoice due in 30 days.
    const due = new Date(Date.now() + 30 * 86_400_000).toISOString().slice(0, 10);
    await row.getByRole('button', { name: 'Invoice' }).click();
    await page.getByLabel('Freight Amount').fill('4500');
    await page.getByLabel('Due Date').fill(due);
    await page.getByRole('button', { name: /raise invoice/i }).click();

    const billing = row.getByTestId('billing-cell');
    await expect(billing.getByText('PENDING', { exact: true })).toBeVisible({ timeout: 8000 });
    await billing.getByRole('button', { name: /mark paid/i }).click();
    await expect(billing.getByText('PAID', { exact: true })).toBeVisible({ timeout: 8000 });

    // The customer's billing shows settled once paid.
    await page.goto('/customers');
    const custRow = page.locator('tbody tr', { hasText: custName });
    await expect(custRow.getByText('Settled')).toBeVisible({ timeout: 8000 });
  });

  test('dispatch shows error when stock is insufficient', async ({ page }) => {
    const org = await registerOrg(page, `Dispatch Insufficient ${uid()}`);
    const custName = `Customer ${uid()}`;

    // Add stock with only 5 units and customer with location
    await addStockViaApi(page, org.id, 'Steel Rods', 5);
    const custId = await createCustomerWithLocation(page, org.id, custName);

    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ value: custId });
    await page.getByLabel('Stock Description').fill('Steel Rods');
    await page.getByLabel('Quantity', { exact: true }).fill('100'); // more than available
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

    // Assign an active driver, add stock and a located customer (all required for dispatch)
    await assignActiveDriver(page, org.id, reg);
    await addStockViaApi(page, org.id, stockDesc, 80);
    const custId = await createCustomerWithLocation(page, org.id, custName);

    // Dispatch
    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ value: custId });
    await page.getByLabel('Stock Description').fill(stockDesc);
    await page.getByLabel('Quantity', { exact: true }).fill('20');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });

    // Verify dispatches page columns (shows stock desc, vehicle reg, status — not customer name)
    await page.goto('/dispatches');
    await expect(page.getByText(stockDesc)).toBeVisible({ timeout: 8000 });
    await expect(page.getByText(reg)).toBeVisible();
    // A freshly dispatched order starts its lifecycle at PENDING (stock and a
    // vehicle are reserved, but nothing has physically moved yet).
    await expect(page.getByText('PENDING', { exact: true })).toBeVisible();
    const rows = page.locator('tbody tr');
    await expect(rows).toHaveCount(1, { timeout: 8000 });
  });

  test('dispatch several stock lines in one order and see them all in the table', async ({ page }) => {
    const org = await registerOrg(page, `Multi Line ${uid()}`);
    const custName = `Multi Customer ${uid()}`;
    const itemA = `Bricks ${uid()}`;
    const itemB = `Tiles ${uid()}`;
    const reg = `MH19ML${uid().toUpperCase().slice(0, 4)}`;

    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Registration Number').fill(reg);
    await page.getByLabel('Capacity (MT)').fill('500');
    await page.getByRole('button', { name: /add vehicle/i }).click();
    await expect(page.getByTestId('fleet-table').getByText(reg)).toBeVisible({ timeout: 8000 });

    await assignActiveDriver(page, org.id, reg);
    await addStockViaApi(page, org.id, itemA, 200);
    await addStockViaApi(page, org.id, itemB, 200);
    const custId = await createCustomerWithLocation(page, org.id, custName);

    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ value: custId });
    await page.getByLabel('Stock Description').fill(itemA);
    await page.getByLabel('Quantity', { exact: true }).fill('30');
    await page.getByRole('button', { name: /add another line/i }).click();
    await page.getByLabel('Stock Description 2').fill(itemB);
    await page.getByLabel('Quantity 2').fill('12');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });

    await page.goto('/dispatches');
    const row = page.locator('tbody tr').first();
    await expect(row.getByText(itemA)).toBeVisible({ timeout: 8000 });
    await expect(row.getByText(itemB)).toBeVisible();
    // Qty column shows the combined total (30 + 12).
    await expect(row.getByText('42', { exact: true })).toBeVisible();
  });

  test('dispatch form requires customer to be selected', async ({ page }) => {
    const org = await registerOrg(page, `Dispatch Required ${uid()}`);
    await addStockViaApi(page, org.id, 'Wood', 10);

    await page.goto(`/orgs/${org.id}`);
    // Fill stock and quantity but leave customer unselected
    await page.getByLabel('Stock Description').fill('Wood');
    await page.getByLabel('Quantity', { exact: true }).fill('5');
    await page.getByRole('button', { name: /dispatch stock/i }).click();

    // HTML5 required on select prevents submission; stays on same page
    await expect(page).toHaveURL(`/orgs/${org.id}`);
    await expect(page.getByText(/dispatch successful/i)).not.toBeVisible();
  });

  test('advance a dispatch through its lifecycle to DELIVERED, requiring proof of delivery', async ({ page }) => {
    const org = await registerOrg(page, `Dispatch Lifecycle ${uid()}`);
    const custName = `Lifecycle Customer ${uid()}`;
    const stockDesc = `Pipes ${uid()}`;
    const token = await page.evaluate(() => localStorage.getItem('logi_token'));

    const vehReg = `LC${uid().toUpperCase().slice(0, 6)}`;
    await page.request.post(`/api/orgs/${org.id}/vehicles`, {
      data: { registration_number: vehReg, capacity: 20, unit: 'MetricTon' },
      headers: { Authorization: `Bearer ${token}` },
    });
    await assignActiveDriver(page, org.id, vehReg);
    await addStockViaApi(page, org.id, stockDesc, 50);
    const custId = await createCustomerWithLocation(page, org.id, custName);

    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ value: custId });
    await page.getByLabel('Stock Description').fill(stockDesc);
    await page.getByLabel('Quantity', { exact: true }).fill('10');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });

    await page.goto('/dispatches');
    const row = page.locator('tbody tr').filter({ hasText: stockDesc });
    await expect(row).toBeVisible({ timeout: 8000 });
    await expect(row.getByText('PENDING', { exact: true })).toBeVisible();

    // PENDING -> CONFIRMED -> LOADED -> IN_TRANSIT, each a plain click with
    // no extra input required.
    await row.getByRole('button', { name: 'Confirm' }).click();
    await expect(row.getByText('CONFIRMED', { exact: true })).toBeVisible({ timeout: 8000 });

    await row.getByRole('button', { name: 'Mark Loaded' }).click();
    await expect(row.getByText('LOADED', { exact: true })).toBeVisible({ timeout: 8000 });

    await row.getByRole('button', { name: 'Mark In Transit' }).click();
    await expect(row.getByText('IN TRANSIT', { exact: true })).toBeVisible({ timeout: 8000 });

    // IN_TRANSIT -> DELIVERED requires proof of delivery — the inline form
    // must appear, and the confirm button must stay disabled until both
    // fields are filled.
    await row.getByRole('button', { name: 'Mark Delivered' }).click();
    const confirmDeliveryBtn = page.getByRole('button', { name: /confirm delivery/i });
    await expect(confirmDeliveryBtn).toBeDisabled();

    await page.getByLabel(/receiver name/i).fill('Priya Sharma');
    await page.getByLabel(/signature.*photo url/i).fill('https://example.com/pod/sig.png');
    await expect(confirmDeliveryBtn).toBeEnabled();
    await confirmDeliveryBtn.click();

    await expect(row.getByText('DELIVERED', { exact: true })).toBeVisible({ timeout: 8000 });
    // Terminal status — no further action buttons on this row.
    await expect(row.getByRole('button', { name: /confirm|cancel|mark/i })).toHaveCount(0);

    // The details panel surfaces the full status history and the proof of
    // delivery that was just captured.
    await row.getByRole('button', { name: /ai status/i }).click();
    await expect(page.getByText(/received by/i)).toBeVisible();
    await expect(page.getByText('Priya Sharma').last()).toBeVisible();
  });

  test('marking a dispatch RETURNED credits its stock back into a godown', async ({ page }) => {
    const org = await registerOrg(page, `Returns Flow ${uid()}`);
    const custName = `Return Customer ${uid()}`;
    const stockDesc = `Panels ${uid()}`;
    const token = await page.evaluate(() => localStorage.getItem('logi_token'));

    const vehReg = `RT${uid().toUpperCase().slice(0, 6)}`;
    await page.request.post(`/api/orgs/${org.id}/vehicles`, {
      data: { registration_number: vehReg, capacity: 100, unit: 'MetricTon' },
      headers: { Authorization: `Bearer ${token}` },
    });
    await assignActiveDriver(page, org.id, vehReg);
    await addStockViaApi(page, org.id, stockDesc, 60);
    const custId = await createCustomerWithLocation(page, org.id, custName);

    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ value: custId });
    await page.getByLabel('Stock Description').fill(stockDesc);
    await page.getByLabel('Quantity', { exact: true }).fill('25');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });

    // 60 - 25 = 35 left in the godown after the dispatch.
    await expect(page.getByTestId('godown-card').filter({ hasText: stockDesc }).getByText('35')).toBeVisible({ timeout: 8000 });

    await page.goto('/dispatches');
    const row = page.locator('tbody tr').filter({ hasText: stockDesc });
    await row.getByRole('button', { name: 'Confirm' }).click();
    await expect(row.getByText('CONFIRMED', { exact: true })).toBeVisible({ timeout: 8000 });
    await row.getByRole('button', { name: 'Mark Loaded' }).click();
    await expect(row.getByText('LOADED', { exact: true })).toBeVisible({ timeout: 8000 });
    await row.getByRole('button', { name: 'Mark In Transit' }).click();
    await expect(row.getByText('IN TRANSIT', { exact: true })).toBeVisible({ timeout: 8000 });

    await row.getByRole('button', { name: 'Mark Returned' }).click();
    await page.getByRole('button', { name: /confirm return/i }).click();
    await expect(row.getByText('RETURNED', { exact: true })).toBeVisible({ timeout: 8000 });

    // The 25 units are back in the godown: 35 + 25 = 60.
    await page.goto(`/orgs/${org.id}`);
    await expect(page.getByTestId('godown-card').filter({ hasText: stockDesc }).getByText('60')).toBeVisible({ timeout: 8000 });
  });
});
