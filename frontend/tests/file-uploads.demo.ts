import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A short narrated walk through **real file storage for proof-of-delivery
 * photos** on its own — advance a dispatch to IN_TRANSIT, then confirm
 * delivery by uploading a real small image through the delivery-confirmation
 * form (instead of pasting a URL), and follow the "View signature/photo"
 * link to see the backend actually serving the uploaded bytes back.
 *
 * Watch it with `npm run test:e2e:demo:uploads` (headed, slowed).
 */
const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post' | 'put', path: string, data: unknown) {
  const res = await page.request[method](path, { data, headers: { Authorization: `Bearer ${await token(page)}` } });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return res.json();
}

test('proof-of-delivery photo upload', async ({ page }) => {
  test.slow();
  const org = await registerOrg(page, `Upload Demo ${uid()}`);
  const stock = `Fittings ${uid()}`;
  const custName = `Upload Demo Customer ${uid()}`;
  const reg = `UP${uid().toUpperCase()}`;
  let custId = '';

  await test.step('Set up a truck, driver, stock and a located customer', async () => {
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, { registration_number: reg, capacity: 100000, unit: 'MetricTon' });
    const driver = await api(page, 'post', `/api/orgs/${org.id}/drivers`, { name: 'Suresh Patil', license_number: `L${uid()}`, phone: '0' });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(reg)}/driver`, { driver_id: driver.data.id });
    const godown = await api(page, 'post', `/api/orgs/${org.id}/godowns`, { name: 'Central Godown', address: '1 Dock Rd' });
    await api(page, 'post', `/api/godowns/${godown.data.id}/stock`, { description: stock, quantity: 100, volume_in_size: 1 });
    const customer = await api(page, 'post', `/api/orgs/${org.id}/customers`, { name: custName, address: '5 Delivery Lane' });
    custId = customer.data.id;
    await api(page, 'put', `/api/customers/${custId}/location`, { latitude: 18.5204, longitude: 73.8567 });
  });

  await test.step('Dispatch stock and advance to IN_TRANSIT', async () => {
    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ value: custId });
    await page.getByLabel('Stock Description').fill(stock);
    await page.getByLabel('Quantity', { exact: true }).fill('10');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });
    await page.waitForTimeout(600);

    await page.goto('/dispatches');
    const row = page.locator('tbody tr').filter({ hasText: stock });
    await expect(row).toBeVisible({ timeout: 8000 });
    await row.getByRole('button', { name: 'Confirm' }).click();
    await expect(row.getByText('CONFIRMED', { exact: true })).toBeVisible({ timeout: 8000 });
    await row.getByRole('button', { name: 'Mark Loaded' }).click();
    await expect(row.getByText('LOADED', { exact: true })).toBeVisible({ timeout: 8000 });
    await row.getByRole('button', { name: 'Mark In Transit' }).click();
    await expect(row.getByText('IN TRANSIT', { exact: true })).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(800);
  });

  await test.step('Upload a real photo as proof of delivery, instead of pasting a URL', async () => {
    const row = page.locator('tbody tr').filter({ hasText: stock });
    await row.getByRole('button', { name: 'Mark Delivered' }).click();
    const confirmBtn = page.getByRole('button', { name: /confirm delivery/i });
    await expect(confirmBtn).toBeDisabled();

    await page.getByLabel(/receiver name/i).fill('Lakshmi Iyer');
    await page.waitForTimeout(400);
    await page.getByLabel(/signature \/ photo/i).setInputFiles({
      name: 'delivery-photo.png',
      mimeType: 'image/png',
      buffer: Buffer.from('fake png bytes for the demo'),
    });
    await expect(page.getByText(/uploaded/i)).toBeVisible({ timeout: 8000 });
    await expect(confirmBtn).toBeEnabled();
    await page.waitForTimeout(500);
    await confirmBtn.click();
    await expect(row.getByText('DELIVERED', { exact: true })).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(800);
  });

  await test.step('Follow "View signature/photo" — the backend serves the uploaded bytes back', async () => {
    const row = page.locator('tbody tr').filter({ hasText: stock });
    await row.getByRole('button', { name: /ai status/i }).click();
    const podLink = page.getByRole('link', { name: /view signature\/photo/i });
    await expect(podLink).toBeVisible();

    const href = await podLink.getAttribute('href');
    expect(href).toContain('/api/uploads/');
    const fileResp = await page.request.get(href!, { headers: { Authorization: `Bearer ${await token(page)}` } });
    expect(fileResp.ok()).toBeTruthy();
    expect(fileResp.headers()['content-type']).toBe('image/png');
    await page.waitForTimeout(1000);
  });
});
