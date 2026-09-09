import { test, expect } from '@playwright/test';
import { registerOrg, loginOrg, uid } from './helpers';

/**
 * A single, narrated walk through the whole product: register an org, sign
 * out and back in, build up a warehouse and fleet, dispatch a multi-item
 * shipment to a customer, glance at the ops report while it's out, carry it
 * through its delivery lifecycle, raise and pay a freight invoice, and
 * dispatch + return a second shipment to see its stock credited back into a
 * godown.
 *
 * This is meant to be watched, not just asserted on — run it with
 * `npm run test:e2e:demo` (playwright.demo.config.ts), which always opens a
 * real, slowed-down browser window instead of running headless. Each
 * test.step() below shows up as its own line in the list reporter and as a
 * labelled section in the trace, so progress is easy to follow either way.
 */
test('full logistics workflow: register, login, warehouse, fleet, delivery, billing, and a return', async ({ page }) => {
  test.slow();

  const orgName = `Demo Logistics Co ${uid()}`;
  const godownA = `Central Warehouse ${uid()}`;
  const godownB = `Overflow Warehouse ${uid()}`;
  const stockA = `Cement Bags ${uid()}`;
  const stockB = `Steel Rods ${uid()}`;
  const stockC = `Cable Reels ${uid()}`;
  // Full uid (not a 4-char slice) so a re-run never reuses a plate — a plate
  // still tied to an unfinished dispatch from a previous run would leave this
  // run's only vehicle "busy" and the dispatch step with nothing to send on.
  const vehicleReg = `MH12DM${uid().toUpperCase()}`;
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

  await test.step('Add a Dispatcher team member', async () => {
    await page.getByRole('link', { name: /team/i }).click();
    await expect(page.getByRole('heading', { level: 1, name: 'Team' })).toBeVisible();
    await page.getByRole('button', { name: /add member/i }).click();
    await page.getByLabel('Name').fill(`Dispatcher ${uid()}`);
    await page.getByLabel('Email').fill(`dispatcher-${uid()}@example.com`);
    await page.getByLabel(/temporary password/i).fill('team-member-pw');
    await page.getByLabel('Role', { exact: true }).selectOption('DISPATCHER');
    await page.getByRole('button', { name: /^add member$/i }).click();
    await expect(page.locator('.table-toolbar .badge')).toHaveText('1', { timeout: 8000 });
    await page.goto(`/orgs/${org.id}`);
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

  await test.step('A GPS tracker reports the vehicle’s live position', async () => {
    // The vehicle detail page shows the device push URL (the tracker key is
    // the whole credential — no login).
    await page.goto('/vehicles');
    await page.getByRole('link', { name: vehicleReg }).click();
    const pushPath = await page.getByLabel(/tracker push url/i).inputValue();
    expect(pushPath).toMatch(/^\/api\/track\/[0-9a-f-]{36}$/);

    // A device on the truck POSTs its coordinates, unauthenticated.
    const res = await page.request.post(pushPath, {
      data: { latitude: 18.5204, longitude: 73.8567 },
    });
    expect(res.ok()).toBeTruthy();

    // That position is now on the fleet list.
    await page.goto('/vehicles');
    const row = page.getByRole('row', { name: new RegExp(vehicleReg) });
    await expect(row.getByText('18.52040')).toBeVisible({ timeout: 8000 });
  });

  await test.step('Create a customer with a delivery location', async () => {
    await page.goto('/customers');
    await page.getByRole('button', { name: /new customer/i }).click();
    await page.getByLabel('Customer Name').fill(custName);
    await page.getByLabel('Address').fill('221 Market Road, Bengaluru');
    // The create form now carries optional coordinates, so the delivery
    // location is captured in the same step instead of a follow-up API call.
    await page.getByLabel(/latitude/i).fill('12.9716');
    await page.getByLabel(/longitude/i).fill('77.5946');
    await page.getByRole('button', { name: /^create customer$/i }).click();

    const custRow = page.getByRole('row', { name: new RegExp(custName) });
    await expect(custRow).toBeVisible({ timeout: 8000 });
    // The coordinates the form just captured show in the Location column
    // (rounded to 4dp) and put the customer on the locations map.
    await expect(custRow.getByText('12.9716, 77.5946')).toBeVisible();
    await expect(page.getByText(/1 mapped/i)).toBeVisible();
  });

  await test.step('Dispatch a two-item shipment to the customer', async () => {
    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ label: custName });
    await page.getByLabel('Stock Description').fill(stockA);
    await page.getByLabel('Quantity', { exact: true }).fill('30');
    await page.getByRole('button', { name: /add another line/i }).click();
    await page.getByLabel('Stock Description 2').fill(stockB);
    await page.getByLabel('Quantity 2').fill('15');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });
  });

  await test.step('Check the operations report while the shipment is out', async () => {
    await page.goto('/reports');
    await expect(page.getByRole('heading', { level: 1, name: 'Reports' })).toBeVisible();

    // The shipment we just created is still PENDING, so the one vehicle is on
    // a trip: 1 of 1 = 100% fleet utilization.
    const util = page.locator('.stat-card', { hasText: 'Fleet utilization' });
    await expect(util.getByText('100.0%')).toBeVisible({ timeout: 8000 });
    await expect(util.getByText('1 of 1 on a trip')).toBeVisible();

    // Nothing delivered yet.
    await expect(
      page.locator('.stat-card', { hasText: 'Delivered' }).getByText('0', { exact: true }),
    ).toBeVisible();

    // Today's dispatch is the only bar on the 14-day volume chart with a count.
    await expect(page.getByTitle(/^\d{4}-\d{2}-\d{2}: 1$/)).toBeVisible();

    // Both godowns show in the inventory table.
    await expect(page.getByRole('row', { name: new RegExp(godownA) })).toBeVisible();
    await expect(page.getByRole('row', { name: new RegExp(godownB) })).toBeVisible();
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

  await test.step('Raise a freight invoice for the delivered dispatch and mark it paid', async () => {
    const row = page.locator('tbody tr').filter({ hasText: stockA });
    const due = new Date(Date.now() + 30 * 86_400_000).toISOString().slice(0, 10);

    await row.getByRole('button', { name: 'Invoice' }).click();
    await page.getByLabel('Freight Amount').fill('4500');
    await page.getByLabel('Due Date').fill(due);
    await page.getByRole('button', { name: /raise invoice/i }).click();

    const billing = row.getByTestId('billing-cell');
    await expect(billing.getByText('PENDING', { exact: true })).toBeVisible({ timeout: 8000 });
    await billing.getByRole('button', { name: /mark paid/i }).click();
    await expect(billing.getByText('PAID', { exact: true })).toBeVisible({ timeout: 8000 });

    // A paid-up customer shows as settled on the Customers page.
    await page.goto('/customers');
    const custRow = page.locator('tbody tr', { hasText: custName });
    await expect(custRow.getByText('Settled')).toBeVisible({ timeout: 8000 });
  });

  await test.step('Dispatch a shipment, then return it and see the stock credited back', async () => {
    await page.goto(`/orgs/${org.id}`);
    // hasText would also match the overflow godown's card here — its own
    // "To Godown" transfer option now reads "Central Warehouse …" since it
    // holds stock post-transfer. Filter on the card's own heading link
    // instead, which only ever matches the card it names.
    const card = page.getByTestId('godown-card').filter({ has: page.getByRole('link', { name: godownA, exact: true }) });
    await card.getByLabel('Stock Item').fill(stockC);
    await card.getByLabel('Stock Quantity').fill('60');
    await card.getByLabel('Volume').fill('1');
    await card.getByRole('button', { name: /add stock/i }).click();
    await expect(page.getByText(stockC).first()).toBeVisible({ timeout: 8000 });

    await page.getByLabel('Customer').selectOption({ label: custName });
    await page.getByLabel('Stock Description').fill(stockC);
    await page.getByLabel('Quantity', { exact: true }).fill('20');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });

    // 60 - 20 = 40 left in the godown after the dispatch.
    await expect(page.getByTestId('godown-card').filter({ hasText: stockC }).getByText('40')).toBeVisible({ timeout: 8000 });

    await page.goto('/dispatches');
    const row = page.locator('tbody tr').filter({ hasText: stockC });
    await row.getByRole('button', { name: 'Confirm' }).click();
    await expect(row.getByText('CONFIRMED', { exact: true })).toBeVisible({ timeout: 8000 });
    await row.getByRole('button', { name: 'Mark Loaded' }).click();
    await expect(row.getByText('LOADED', { exact: true })).toBeVisible({ timeout: 8000 });
    await row.getByRole('button', { name: 'Mark In Transit' }).click();
    await expect(row.getByText('IN TRANSIT', { exact: true })).toBeVisible({ timeout: 8000 });

    await row.getByRole('button', { name: 'Mark Returned' }).click();
    await page.getByRole('button', { name: /confirm return/i }).click();
    await expect(row.getByText('RETURNED', { exact: true })).toBeVisible({ timeout: 8000 });

    // The 20 returned units are back in the godown: 40 + 20 = 60.
    await page.goto(`/orgs/${org.id}`);
    await expect(page.getByTestId('godown-card').filter({ hasText: stockC }).getByText('60')).toBeVisible({ timeout: 8000 });
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
