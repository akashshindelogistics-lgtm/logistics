import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post' | 'put', path: string, data: unknown) {
  const res = await page.request[method](path, { data, headers: { Authorization: `Bearer ${await token(page)}` } });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return res.json();
}

test.describe('Multi-stop trips', () => {
  test('plan a two-stop trip and see both stops share one vehicle', async ({ page }) => {
    const org = await registerOrg(page, `Trip ${uid()}`);
    const stock = `Cement ${uid()}`;

    const reg = `TP${uid().toUpperCase()}`;
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, { registration_number: reg, capacity: 100000, unit: 'MetricTon' });
    const driver = await api(page, 'post', `/api/orgs/${org.id}/drivers`, { name: 'Trip Driver', license_number: `L${uid()}`, phone: '0' });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(reg)}/driver`, { driver_id: driver.data.id });
    const godown = await api(page, 'post', `/api/orgs/${org.id}/godowns`, { name: 'G', address: '1 Rd' });
    await api(page, 'post', `/api/godowns/${godown.data.id}/stock`, { description: stock, quantity: 500, volume_in_size: 1 });

    const c1 = `North ${uid()}`;
    const c2 = `South ${uid()}`;
    await api(page, 'post', `/api/orgs/${org.id}/customers`, { name: c1, address: 'a', latitude: 19.0, longitude: 72.8 });
    await api(page, 'post', `/api/orgs/${org.id}/customers`, { name: c2, address: 'b', latitude: 19.1, longitude: 72.9 });

    await page.goto('/trips');
    await expect(page.getByRole('heading', { level: 1, name: 'Multi-stop Trips' })).toBeVisible();
    await expect(page.getByText(/no trips yet/i)).toBeVisible();

    await page.getByRole('button', { name: /plan a trip/i }).click();
    await page.getByLabel(/stop 1 — customer/i).selectOption({ label: c1 });
    await page.getByLabel(/stock item/i).first().fill(stock);
    await page.getByLabel(/quantity/i).first().fill('30');
    await page.getByLabel(/stop 2 — customer/i).selectOption({ label: c2 });
    await page.getByLabel(/stock item/i).nth(1).fill(stock);
    await page.getByLabel(/quantity/i).nth(1).fill('20');
    await page.getByRole('button', { name: /^plan trip$/i }).click();

    // The trip card appears with the one vehicle and both stops.
    const card = page.locator('.section-card', { hasText: reg });
    await expect(card).toBeVisible({ timeout: 8000 });
    await expect(card.getByText('PLANNED')).toBeVisible();
    await expect(card.getByText(c1)).toBeVisible();
    await expect(card.getByText(c2)).toBeVisible();

    // Both stops show on the Dispatches page tagged as trip stops.
    await page.goto('/dispatches');
    await expect(page.getByText('Trip · stop 1')).toBeVisible({ timeout: 8000 });
    await expect(page.getByText('Trip · stop 2')).toBeVisible();
  });
});
