import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post' | 'put', path: string, data: unknown) {
  const res = await page.request[method](path, {
    data, headers: { Authorization: `Bearer ${await token(page)}` },
  });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return res.json();
}

test.describe('Dispatch notifications', () => {
  test('creating a customer with an email captures it, and a dispatch records notifications', async ({ page }) => {
    const org = await registerOrg(page, `Notify ${uid()}`);
    const stock = `Cement ${uid()}`;
    const custName = `Reachable ${uid()}`;
    const custEmail = `ops-${uid()}@example.com`;

    // Fleet + stock via API.
    const reg = `NT${uid().toUpperCase()}`;
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, { registration_number: reg, capacity: 500, unit: 'MetricTon' });
    const driver = await api(page, 'post', `/api/orgs/${org.id}/drivers`, { name: 'Notify Driver', license_number: `L${uid()}`, phone: '+91 98888 00000' });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(reg)}/driver`, { driver_id: driver.data.id });
    const godown = await api(page, 'post', `/api/orgs/${org.id}/godowns`, { name: 'NG', address: '1 Rd' });
    await api(page, 'post', `/api/godowns/${godown.data.id}/stock`, { description: stock, quantity: 200, volume_in_size: 1 });

    // Create the customer through the UI with an email.
    await page.goto('/customers');
    await page.getByRole('button', { name: /new customer/i }).click();
    await page.getByLabel('Customer Name').fill(custName);
    await page.getByLabel('Address').fill('5 Market Rd');
    await page.getByLabel(/latitude/i).fill('18.5204');
    await page.getByLabel(/longitude/i).fill('73.8567');
    await page.getByLabel(/email/i).fill(custEmail);
    await page.getByRole('button', { name: /^create customer$/i }).click();
    const custRow = page.getByRole('row', { name: new RegExp(custName) });
    await expect(custRow).toBeVisible({ timeout: 8000 });

    // Dispatch to them.
    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ label: custName });
    await page.getByLabel('Stock Description').fill(stock);
    await page.getByLabel('Quantity', { exact: true }).fill('20');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });

    // The Dispatches page shows the recorded notifications.
    await page.goto('/dispatches');
    const row = page.locator('tbody tr').filter({ hasText: stock });
    await expect(row).toBeVisible({ timeout: 8000 });
    await row.getByRole('button', { name: /notifications for/i }).click();

    await expect(page.getByText(/on its way/i)).toBeVisible({ timeout: 8000 });
    await expect(page.getByText(custEmail, { exact: false })).toBeVisible();
    // Customer email notification is QUEUED; the driver SMS too.
    await expect(page.getByText('QUEUED').first()).toBeVisible();

    // The org-wide feed carries them too.
    const feed = await page.request.get(`/api/orgs/${org.id}/notifications`, {
      headers: { Authorization: `Bearer ${await token(page)}` },
    }).then(r => r.json());
    expect(feed.data.length).toBe(2);
  });
});
