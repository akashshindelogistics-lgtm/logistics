import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid, type TestOrg } from './helpers';

/**
 * Seed an org that owns **no vehicles**: one godown of stock, a customer and
 * an active vehicle vendor, all over the API.
 */
async function seedFleetlessOrg(page: Page, org: TestOrg, stock: string, customer: string, vendor: string) {
  const token = await page.evaluate(() => localStorage.getItem('logi_token'));
  const headers = { Authorization: `Bearer ${token}` };
  const godown = await (await page.request.post(`/api/orgs/${org.id}/godowns`, {
    data: { name: `Hire Godown ${uid()}`, address: 'MIDC' }, headers,
  })).json();
  expect((await page.request.post(`/api/godowns/${godown.data.id}/stock`, {
    data: { description: stock, quantity: 100, volume_in_size: 1 }, headers,
  })).status()).toBe(201);
  expect((await page.request.post(`/api/orgs/${org.id}/customers`, {
    data: { name: customer, address: 'Baner, Pune' }, headers,
  })).ok()).toBeTruthy();
  expect((await page.request.post(`/api/orgs/${org.id}/vendors`, {
    data: { name: vendor, phone: '+91 98200 11111' }, headers,
  })).status()).toBe(201);
}

async function reserveOnHiredTruck(page: Page, org: TestOrg, stock: string, customer: string, vendor: string, qty: string) {
  await page.goto(`/orgs/${org.id}`);
  await page.getByLabel('Vehicle source').selectOption('HIRED');
  await page.getByLabel('Vendor', { exact: true }).selectOption({ label: `${vendor} · +91 98200 11111` });
  await page.getByLabel('Customer').selectOption({ label: customer });
  await page.getByLabel('Stock Description').fill(stock);
  await page.getByLabel('Quantity', { exact: true }).fill(qty);
  await page.getByRole('button', { name: /reserve & request truck/i }).click();
  // Wait for whichever outcome message the form shows, so a failure reports its text.
  const outcome = page.getByText(/stock reserved|dispatch failed|pick the vendor/i);
  await expect(outcome).toBeVisible({ timeout: 10000 });
  await expect(outcome).toHaveText(/stock reserved/i);
}

test.describe('Hired vehicles', () => {
  test('an org with no fleet dispatches on a hired truck', async ({ page }) => {
    const org = await registerOrg(page, `No Fleet ${uid()}`);
    const stock = `Cement ${uid()}`;
    const customer = `Buyer ${uid()}`;
    const vendor = `Sharma Roadlines ${uid()}`;
    await seedFleetlessOrg(page, org, stock, customer, vendor);

    // Own fleet: nothing to send, and the form offers to hire instead.
    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ label: customer });
    await page.getByLabel('Stock Description').fill(stock);
    await page.getByLabel('Quantity', { exact: true }).fill('20');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await page.getByRole('button', { name: /hire a truck from a vendor instead/i }).click({ timeout: 10000 });
    await expect(page.getByLabel('Vehicle source')).toHaveValue('HIRED');

    await reserveOnHiredTruck(page, org, stock, customer, vendor, '20');

    await page.goto('/dispatches');
    const row = page.locator('tbody tr').filter({ hasText: stock }).first();
    await expect(row.getByText('AWAITING VEHICLE', { exact: true })).toBeVisible({ timeout: 8000 });
    await expect(row.getByText(new RegExp(`Hired · ${vendor}`))).toBeVisible();

    await row.getByRole('button', { name: 'Assign truck' }).click();
    await page.getByLabel('Truck number').fill('MH12 HR 4455');
    await page.getByLabel('Capacity').fill('10');
    await page.getByLabel('Driver name').fill('Suresh Patil');
    await page.getByLabel('Driver phone').fill('+91 97000 12345');
    await page.getByLabel('Hire cost').fill('9000');
    await page.getByLabel('Advance paid').fill('7000');
    await page.getByRole('button', { name: /save truck/i }).click();
    await expect(page.getByText(/too small/i)).toBeVisible({ timeout: 8000 });

    await page.getByLabel('Capacity').fill('25');
    await page.getByRole('button', { name: /save truck/i }).click();
    await expect(row.getByText('MH12 HR 4455')).toBeVisible({ timeout: 8000 });
    await expect(row.getByText('PENDING', { exact: true })).toBeVisible();
    await expect(row.getByRole('button', { name: 'Confirm' })).toBeVisible();
  });

  test('cancelling before a truck is assigned puts the stock back', async ({ page }) => {
    const org = await registerOrg(page, `Hire Cancel ${uid()}`);
    const stock = `Steel ${uid()}`;
    const customer = `Buyer ${uid()}`;
    const vendor = `Balaji Transport ${uid()}`;
    await seedFleetlessOrg(page, org, stock, customer, vendor);
    await reserveOnHiredTruck(page, org, stock, customer, vendor, '30');

    const token = await page.evaluate(() => localStorage.getItem('logi_token'));
    const stockLeft = async () => {
      const res = await (await page.request.get(`/api/orgs/${org.id}/godowns`, {
        headers: { Authorization: `Bearer ${token}` },
      })).json();
      return res.data.flatMap((g: { stock: { description: string; quantity: number }[] }) => g.stock)
        .find((s: { description: string }) => s.description === stock).quantity;
    };
    expect(await stockLeft()).toBe(70);

    await page.goto('/dispatches');
    const row = page.locator('tbody tr').filter({ hasText: stock }).first();
    await row.getByRole('button', { name: 'Cancel' }).click();
    await expect(row.getByText('CANCELLED', { exact: true })).toBeVisible({ timeout: 8000 });
    expect(await stockLeft()).toBe(100);
  });
});
