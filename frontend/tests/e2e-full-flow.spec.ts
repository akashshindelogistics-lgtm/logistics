import { test, expect } from '@playwright/test';
import { registerOrg, loginOrg, uid } from './helpers';

/**
 * A single, narrated walk through the whole product: register an org, sign
 * out and back in, build up a warehouse and fleet, dispatch a multi-item
 * shipment to a customer, and carry it through its delivery lifecycle.
 *
 * This is meant to be watched, not just asserted on — run it with
 * `npm run test:e2e:demo` (playwright.demo.config.ts), which always opens a
 * real, slowed-down browser window instead of running headless. Each
 * test.step() below shows up as its own line in the list reporter and as a
 * labelled section in the trace, so progress is easy to follow either way.
 */
test('full logistics workflow: register, login, warehouse, fleet, and a shipment through delivery', async ({ page }) => {
  test.slow();

  const orgName = `Demo Logistics Co ${uid()}`;
  const godownA = `Central Warehouse ${uid()}`;
  const godownB = `Overflow Warehouse ${uid()}`;
  const stockA = `Cement Bags ${uid()}`;
  const stockB = `Steel Rods ${uid()}`;
  const vehicleReg = `MH12DM${uid().toUpperCase().slice(0, 4)}`;
  const driverName = `Ramesh Kulkarni ${uid()}`;
  const custName = `Sunrise Traders ${uid()}`;

  const org = await test.step('Register a new organization', async () => {
    const created = await registerOrg(page, orgName);
    await expect(page).toHaveURL(`/orgs/${created.id}`);
    await expect(page.getByRole('heading', { level: 1 })).toContainText(orgName);
    return created;
  });

  await test.step('Sign out and log back in', async () => {
    await page.getByRole('button', { name: /sign out/i }).click();
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
    await loginOrg(page, org);
    await expect(page).toHaveURL(`/orgs/${org.id}`);
  });

  await test.step('Create two godowns', async () => {
    await page.getByLabel('Godown Name').fill(godownA);
    await page.getByLabel('Address').fill('Plot 5, MIDC Industrial Area, Pune');
    await page.getByRole('button', { name: /add godown/i }).click();
    await expect(page.getByText(godownA)).toBeVisible({ timeout: 8000 });

    await page.getByLabel('Godown Name').fill(godownB);
    await page.getByLabel('Address').fill('Plot 9, MIDC Industrial Area, Pune');
    await page.getByRole('button', { name: /add godown/i }).click();
    await expect(page.getByText(godownB)).toBeVisible({ timeout: 8000 });
  });

  await test.step('Stock the first godown with two items', async () => {
    const card = page.getByTestId('godown-card').filter({ hasText: godownA });

    await card.getByLabel('Stock Item').fill(stockA);
    await card.getByLabel('Stock Quantity').fill('500');
    await card.getByLabel('Volume').fill('1');
    await card.getByRole('button', { name: /add stock/i }).click();
    await expect(page.getByText(stockA).first()).toBeVisible({ timeout: 8000 });

    await card.getByLabel('Stock Item').fill(stockB);
    await card.getByLabel('Stock Quantity').fill('200');
    await card.getByLabel('Volume').fill('1');
    await card.getByRole('button', { name: /add stock/i }).click();
    await expect(page.getByText(stockB).first()).toBeVisible({ timeout: 8000 });
  });

  await test.step('Transfer part of the stock to the overflow godown', async () => {
    const card = page.getByTestId('godown-card').filter({ hasText: godownA });
    await card.getByLabel('Transfer Item').selectOption({ label: stockA });
    await card.getByLabel('To Godown').selectOption({ label: godownB });
    await card.getByLabel('Transfer Quantity').fill('50');
    await card.getByRole('button', { name: /^transfer$/i }).click();
    await expect(page.getByText(/stock transferred between godowns/i)).toBeVisible({ timeout: 8000 });

    await expect(page.getByText('Stock Transfers')).toBeVisible();
    const historyRow = page.locator('tr', { has: page.getByText(stockA) }).last();
    await expect(historyRow.getByText(godownA)).toBeVisible();
    await expect(historyRow.getByText(godownB)).toBeVisible();
  });

  await test.step('Register a fleet vehicle', async () => {
    await page.getByLabel('Registration Number').fill(vehicleReg);
    await page.getByLabel('Capacity (MT)').fill('60');
    await page.getByRole('button', { name: /add vehicle/i }).click();
    await expect(page.getByTestId('fleet-table').getByText(vehicleReg)).toBeVisible({ timeout: 8000 });
  });

  await test.step('Record the vehicle’s insurance document', async () => {
    const expiry = new Date(Date.now() + 20 * 86_400_000).toISOString().slice(0, 10);
    await page.getByRole('button', { name: /add compliance document/i }).click();
    await page.getByLabel('Vehicle', { exact: true }).selectOption(vehicleReg);
    await page.getByLabel('Document', { exact: true }).selectOption('Insurance');
    await page.getByLabel('Document Number').fill(`POL-${uid()}`);
    await page.getByLabel('Expiry Date').fill(expiry);
    await page.getByRole('button', { name: /save document/i }).click();
    await expect(page.getByText(/compliance document recorded/i)).toBeVisible({ timeout: 8000 });
  });

  await test.step('Add a driver and assign them to the vehicle', async () => {
    await page.getByLabel('Driver Name').fill(driverName);
    await page.getByLabel('Licence Number').fill(`DL-${uid().toUpperCase()}`);
    await page.getByLabel('Phone').fill('+91 98200 00000');
    await page.getByRole('button', { name: /add driver/i }).click();
    await expect(page.getByRole('link', { name: driverName })).toBeVisible({ timeout: 8000 });

    await page.getByLabel(`Driver for ${vehicleReg}`).selectOption({ label: driverName });
    await expect(page.getByLabel(`Driver for ${vehicleReg}`)).toHaveValue(/.+/);
  });

  const custId = await test.step('Create a customer and set their delivery location', async () => {
    await page.goto('/customers');
    await page.getByRole('button', { name: /new customer/i }).click();
    await page.getByLabel('Customer Name').fill(custName);
    await page.getByLabel('Address').fill('221 Market Road, Bengaluru');
    await page.getByRole('button', { name: /^create customer$/i }).click();
    await expect(page.getByText(custName)).toBeVisible({ timeout: 8000 });

    // The dashboard has no location picker yet (todo.org still lists that as
    // open work), so set it via the API the UI itself calls, using the same
    // auth token the browser session holds.
    const token = await page.evaluate(() => localStorage.getItem('logi_token'));
    const listResp = await page.request.get('/api/customers', {
      headers: { Authorization: `Bearer ${token}` },
    });
    const list = (await listResp.json()) as { data: Array<{ id: string; name: string }> };
    const created = list.data.find(c => c.name === custName);
    if (!created) throw new Error('created customer not found in listing');

    await page.request.put(`/api/customers/${created.id}/location`, {
      data: { latitude: 12.9716, longitude: 77.5946, address: '221 Market Road, Bengaluru' },
      headers: { Authorization: `Bearer ${token}` },
    });

    return created.id;
  });

  await test.step('Dispatch a two-item shipment to the customer', async () => {
    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ value: custId });
    await page.getByLabel('Stock Description').fill(stockA);
    await page.getByLabel('Quantity', { exact: true }).fill('30');
    await page.getByRole('button', { name: /add another line/i }).click();
    await page.getByLabel('Stock Description 2').fill(stockB);
    await page.getByLabel('Quantity 2').fill('15');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });
  });

  await test.step('Carry the dispatch through its delivery lifecycle', async () => {
    await page.goto('/dispatches');
    const row = page.locator('tbody tr').filter({ hasText: stockA });
    await expect(row).toBeVisible({ timeout: 8000 });
    await expect(row.getByText('PENDING', { exact: true })).toBeVisible();

    await row.getByRole('button', { name: 'Confirm' }).click();
    await expect(row.getByText('CONFIRMED', { exact: true })).toBeVisible({ timeout: 8000 });

    await row.getByRole('button', { name: 'Mark Loaded' }).click();
    await expect(row.getByText('LOADED', { exact: true })).toBeVisible({ timeout: 8000 });

    await row.getByRole('button', { name: 'Mark In Transit' }).click();
    await expect(row.getByText('IN TRANSIT', { exact: true })).toBeVisible({ timeout: 8000 });

    await row.getByRole('button', { name: 'Mark Delivered' }).click();
    const confirmDeliveryBtn = page.getByRole('button', { name: /confirm delivery/i });
    await expect(confirmDeliveryBtn).toBeDisabled();
    await page.getByLabel(/receiver name/i).fill('Anita Rao');
    await page.getByLabel(/signature.*photo url/i).fill('https://example.com/pod/sig.png');
    await expect(confirmDeliveryBtn).toBeEnabled();
    await confirmDeliveryBtn.click();

    await expect(row.getByText('DELIVERED', { exact: true })).toBeVisible({ timeout: 8000 });
    await expect(row.getByRole('button', { name: /confirm|cancel|mark/i })).toHaveCount(0);

    await row.getByRole('button', { name: /ai status/i }).click();
    await expect(page.getByText(/received by/i)).toBeVisible();
    await expect(page.getByText('Anita Rao').last()).toBeVisible();
  });

  await test.step('Edit the vehicle, driver and godown from their detail pages', async () => {
    await page.goto('/vehicles');
    await page.getByRole('link', { name: vehicleReg }).click();
    await expect(page).toHaveURL(new RegExp(`/vehicles/${encodeURIComponent(vehicleReg)}$`));
    await page.getByLabel('Capacity').fill('30');
    await page.getByRole('button', { name: /^save$/i }).click();
    await expect(page.getByText(/vehicle updated/i)).toBeVisible({ timeout: 8000 });

    await page.goto(`/orgs/${org.id}`);
    await page.getByRole('link', { name: driverName }).click();
    // Wait for the driver detail page to actually render (not just the URL to
    // change) before touching its form — otherwise this can race the org
    // dashboard's own "Driver Name" field, which briefly coexists mid-transition.
    await expect(page.getByRole('heading', { level: 1, name: driverName })).toBeVisible();
    await page.getByLabel('Licence Number').fill(`DL-UPDATED-${uid()}`);
    await page.getByRole('button', { name: /^save$/i }).click();
    await expect(page.getByText(/driver updated/i)).toBeVisible({ timeout: 8000 });

    await page.goto(`/orgs/${org.id}`);
    await page.getByRole('link', { name: godownB }).click();
    // Same race as above: "Name" also matches the dashboard's "Godown Name"
    // and "Driver Name" fields until the godown detail page has fully mounted.
    await expect(page.getByRole('heading', { level: 1, name: godownB })).toBeVisible();
    await expect(page.getByLabel('Name')).toHaveValue(godownB);
    await page.getByLabel(/max capacity/i).fill('10000');
    await page.getByRole('button', { name: /^save$/i }).click();
    await expect(page.getByText(/godown updated/i)).toBeVisible({ timeout: 8000 });
  });

  await test.step('Confirm everything shows up on the fleet, customer and dashboard views', async () => {
    await page.goto('/vehicles');
    await expect(page.getByText(vehicleReg)).toBeVisible({ timeout: 8000 });

    await page.goto('/customers');
    await expect(page.getByText(custName)).toBeVisible({ timeout: 8000 });

    await page.goto('/');
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible({ timeout: 8000 });
    await expect(page.locator('.stat-card').filter({ hasText: 'Fleet Vehicles' })).toBeVisible();
    await expect(page.locator('.stat-card').filter({ hasText: 'Dispatches' })).toBeVisible();
  });

  await test.step('Sign out', async () => {
    await page.getByRole('button', { name: /sign out/i }).click();
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });
});
