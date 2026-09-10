import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A short narrated walk through **dispatch notifications** on its own —
 * add a customer with an email, dispatch to them, and see the customer +
 * driver notifications recorded on the Dispatches page.
 *
 * Watch it with `npm run test:e2e:demo:notifications` (headed, slowed).
 */
const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post' | 'put', path: string, data: unknown) {
  const res = await page.request[method](path, { data, headers: { Authorization: `Bearer ${await token(page)}` } });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return res.json();
}
async function dispatchId(page: Page, stockDescription: string): Promise<string> {
  const res = await page.request.get('/api/dispatches', { headers: { Authorization: `Bearer ${await token(page)}` } });
  const list = (await res.json()).data as Array<{ id: string; line_items: { stock_description: string }[] }>;
  return list.find(d => d.line_items.some(li => li.stock_description === stockDescription))!.id;
}

test('dispatch notifications', async ({ page }) => {
  test.slow();
  const org = await registerOrg(page, `Notify Demo ${uid()}`);
  const stock = `Cement ${uid()}`;
  const custName = `Reachable Retail ${uid()}`;
  const custEmail = `ops-${uid()}@example.com`;

  await test.step('Set up a fleet and a stocked godown', async () => {
    const reg = `ND${uid().toUpperCase()}`;
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, { registration_number: reg, capacity: 500, unit: 'MetricTon' });
    const driver = await api(page, 'post', `/api/orgs/${org.id}/drivers`, { name: 'Ravi Kumar', license_number: `L${uid()}`, phone: '+91 98888 00000' });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(reg)}/driver`, { driver_id: driver.data.id });
    const godown = await api(page, 'post', `/api/orgs/${org.id}/godowns`, { name: 'North Godown', address: '1 Dock Rd' });
    await api(page, 'post', `/api/godowns/${godown.data.id}/stock`, { description: stock, quantity: 200, volume_in_size: 1 });
  });

  await test.step('Add a customer with an email address', async () => {
    await page.goto('/customers');
    await page.getByRole('button', { name: /new customer/i }).click();
    await page.getByLabel('Customer Name').fill(custName);
    await page.getByLabel('Address').fill('221 Market Road, Bengaluru');
    await page.getByLabel(/latitude/i).fill('12.9716');
    await page.getByLabel(/longitude/i).fill('77.5946');
    await page.getByLabel(/phone/i).fill('+91 90000 12345');
    await page.getByLabel(/email/i).fill(custEmail);
    await page.getByRole('button', { name: /^create customer$/i }).click();
    await expect(page.getByRole('row', { name: new RegExp(custName) })).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(700);
  });

  await test.step('Dispatch a shipment to them', async () => {
    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ label: custName });
    await page.getByLabel('Stock Description').fill(stock);
    await page.getByLabel('Quantity', { exact: true }).fill('30');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });
    await page.waitForTimeout(700);
  });

  await test.step('See the notifications recorded on the Dispatches page', async () => {
    await page.goto('/dispatches');
    const row = page.locator('tbody tr').filter({ hasText: stock });
    await expect(row).toBeVisible({ timeout: 8000 });
    await row.getByRole('button', { name: /notifications for/i }).click();

    // Two rows: an email to the customer and an SMS to the driver.
    await expect(page.getByText(/on its way/i)).toBeVisible({ timeout: 8000 });
    await expect(page.getByText(custEmail, { exact: false })).toBeVisible();
    await expect(page.getByText('New trip assigned', { exact: false })).toBeVisible();
    await page.waitForTimeout(1500);
  });

  await test.step('Delivering the shipment adds a second customer notification', async () => {
    await page.request.put(`/api/dispatches/${await dispatchId(page, stock)}/status`, {
      data: { status: 'CONFIRMED' }, headers: { Authorization: `Bearer ${await token(page)}` },
    });
    // (skip straight through via the API for the demo's sake)
    for (const s of ['LOADED', 'IN_TRANSIT']) {
      await page.request.put(`/api/dispatches/${await dispatchId(page, stock)}/status`, {
        data: { status: s }, headers: { Authorization: `Bearer ${await token(page)}` },
      });
    }
    await page.request.put(`/api/dispatches/${await dispatchId(page, stock)}/status`, {
      data: { status: 'DELIVERED', proof_of_delivery: { receiver_name: 'Anita Rao', signature_or_photo_url: 'https://example.com/pod/sig.png' } },
      headers: { Authorization: `Bearer ${await token(page)}` },
    });

    await page.reload();
    const row = page.locator('tbody tr').filter({ hasText: stock });
    await row.getByRole('button', { name: /notifications for/i }).click();
    await expect(page.getByText(/has been delivered/i)).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(1500);
  });
});
