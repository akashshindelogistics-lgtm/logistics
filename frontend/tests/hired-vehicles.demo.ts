import { test, expect } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A narrated walk through **dispatching on a hired truck** on its own: an org
 * that owns no vehicles records a vendor, reserves stock for a customer
 * against that vendor, enters the truck the vendor sends, and runs the
 * delivery as usual.
 *
 * Phase 2 of docs/vehicle-vendors.md.
 * Watch it with `npm run test:e2e:demo:hired-vehicles` (headed, slowed).
 */
test('dispatching on a hired vehicle', async ({ page }) => {
  test.slow();
  const org = await registerOrg(page, `Hired Truck Demo ${uid()}`);
  const stock = `Cement Bags ${uid()}`;
  const customer = `Sunrise Traders ${uid()}`;
  const vendor = `Sharma Roadlines ${uid()}`;

  await test.step('Set up stock and a customer (the org has no vehicles of its own)', async () => {
    const token = await page.evaluate(() => localStorage.getItem('logi_token'));
    const headers = { Authorization: `Bearer ${token}` };
    const godown = await (await page.request.post(`/api/orgs/${org.id}/godowns`, {
      data: { name: 'Central Warehouse', address: 'MIDC, Pune' }, headers,
    })).json();
    await page.request.post(`/api/godowns/${godown.data.id}/stock`, {
      data: { description: stock, quantity: 200, volume_in_size: 1 }, headers,
    });
    await page.request.post(`/api/orgs/${org.id}/customers`, {
      data: { name: customer, address: '221 Market Road, Bengaluru' }, headers,
    });
  });

  await test.step('Record the transporter the dispatcher calls for trucks', async () => {
    await page.getByRole('link', { name: 'Vendors' }).click();
    await page.getByRole('button', { name: /add vendor/i }).click();
    await page.getByLabel('Vendor name').fill(vendor);
    await page.getByLabel('Contact person').fill('Anil Sharma');
    await page.getByLabel('Phone').fill('+91 98200 11111');
    await page.getByRole('button', { name: /^add vendor$/i }).click();
    await expect(page.getByRole('row', { name: new RegExp(vendor) })).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(800);
  });

  await test.step('Try to dispatch on the own fleet — there is none, so hire instead', async () => {
    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ label: customer });
    await page.getByLabel('Stock Description').fill(stock);
    await page.getByLabel('Quantity', { exact: true }).fill('40');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    const hireInstead = page.getByRole('button', { name: /hire a truck from a vendor instead/i });
    await expect(hireInstead).toBeVisible({ timeout: 10000 });
    await page.waitForTimeout(1200);
    await hireInstead.click();
  });

  await test.step('Reserve the stock and request a truck from the vendor', async () => {
    await page.getByLabel('Vendor', { exact: true }).selectOption({ label: `${vendor} · +91 98200 11111` });
    await page.waitForTimeout(600);
    await page.getByRole('button', { name: /reserve & request truck/i }).click();
    // Wait for whichever outcome message the form shows, so a failure reports its text.
    const outcome = page.getByText(/stock reserved|dispatch failed|pick the vendor/i);
    await expect(outcome).toBeVisible({ timeout: 10000 });
    await expect(outcome).toHaveText(/stock reserved/i);
    await page.waitForTimeout(1200);
  });

  const row = page.locator('tbody tr').filter({ hasText: stock }).first();

  await test.step('The dispatch waits for its truck', async () => {
    await page.getByRole('link', { name: 'Dispatches' }).click();
    await expect(row.getByText('AWAITING VEHICLE', { exact: true })).toBeVisible({ timeout: 8000 });
    await expect(row.getByText(new RegExp(`Hired · ${vendor}`))).toBeVisible();
    await page.waitForTimeout(1200);
  });

  await test.step('The vendor confirms: enter the truck, driver and agreed rate', async () => {
    await row.getByRole('button', { name: 'Assign truck' }).click();
    await page.getByLabel('Truck number').fill('MH12 HR 4455');
    await page.getByLabel('Capacity').fill('60');
    await page.getByLabel('Driver name').fill('Suresh Patil');
    await page.getByLabel('Driver phone').fill('+91 97000 12345');
    await page.getByLabel('Driver licence').fill('MH12 2019 0045678');
    await page.getByLabel('Hire cost').fill('9000');
    await page.getByLabel('Advance paid').fill('7000');
    await page.waitForTimeout(800);
    await page.getByRole('button', { name: /save truck/i }).click();
    await expect(row.getByText('MH12 HR 4455')).toBeVisible({ timeout: 8000 });
    await expect(row.getByText('PENDING', { exact: true })).toBeVisible();
    await page.waitForTimeout(1200);
  });

  await test.step('From here it is an ordinary dispatch: confirm, load, send out', async () => {
    await row.getByRole('button', { name: 'Confirm' }).click();
    await expect(row.getByText('CONFIRMED', { exact: true })).toBeVisible({ timeout: 8000 });
    await row.getByRole('button', { name: 'Mark Loaded' }).click();
    await expect(row.getByText('LOADED', { exact: true })).toBeVisible({ timeout: 8000 });
    await row.getByRole('button', { name: 'Mark In Transit' }).click();
    await expect(row.getByText('IN TRANSIT', { exact: true })).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(1500);
  });
});
