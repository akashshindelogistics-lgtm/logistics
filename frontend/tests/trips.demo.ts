import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A short narrated walk through **multi-stop trips** on its own — one truck
 * planned to visit three customers in a row, each getting their own stock.
 *
 * Watch it with `npm run test:e2e:demo:trips` (headed, slowed).
 */
const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post' | 'put', path: string, data: unknown) {
  const res = await page.request[method](path, { data, headers: { Authorization: `Bearer ${await token(page)}` } });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return res.json();
}

test('multi-stop trips', async ({ page }) => {
  test.slow();
  const org = await registerOrg(page, `Trips Demo ${uid()}`);
  const stock = `Cement ${uid()}`;
  const names = [`North Store ${uid()}`, `East Store ${uid()}`, `South Store ${uid()}`];

  await test.step('Set up a truck and stock, and three customers', async () => {
    const reg = `TD${uid().toUpperCase()}`;
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, { registration_number: reg, capacity: 100000, unit: 'MetricTon' });
    const d = await api(page, 'post', `/api/orgs/${org.id}/drivers`, { name: 'Ravi Kumar', license_number: `L${uid()}`, phone: '0' });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(reg)}/driver`, { driver_id: d.data.id });
    const g = await api(page, 'post', `/api/orgs/${org.id}/godowns`, { name: 'Central Godown', address: '1 Dock Rd' });
    await api(page, 'post', `/api/godowns/${g.data.id}/stock`, { description: stock, quantity: 500, volume_in_size: 1 });
    for (const [i, name] of names.entries()) {
      await api(page, 'post', `/api/orgs/${org.id}/customers`, {
        name, address: `${i} Market Rd`, latitude: 19 + i * 0.02, longitude: 72.8,
      });
    }
  });

  await test.step('Open the Trips page', async () => {
    await page.getByRole('link', { name: /trips/i }).click();
    await expect(page).toHaveURL(/\/trips$/);
    await expect(page.getByRole('heading', { level: 1, name: 'Multi-stop Trips' })).toBeVisible();
    await page.waitForTimeout(700);
  });

  await test.step('Plan a three-stop trip', async () => {
    await page.getByRole('button', { name: /plan a trip/i }).click();
    await page.getByRole('button', { name: /add another stop/i }).click();
    for (const [i, name] of names.entries()) {
      await page.getByLabel(new RegExp(`stop ${i + 1} — customer`, 'i')).selectOption({ label: name });
      await page.getByLabel(/stock item/i).nth(i).fill(stock);
      await page.getByLabel(/quantity/i).nth(i).fill(String((i + 1) * 10));
      await page.waitForTimeout(200);
    }
    await page.getByRole('button', { name: /^plan trip$/i }).click();
    await page.waitForTimeout(800);
  });

  await test.step('The trip lists one vehicle and three sequenced stops', async () => {
    const card = page.locator('.section-card').first();
    await expect(card).toBeVisible({ timeout: 8000 });
    await expect(card.getByText('PLANNED')).toBeVisible();
    for (const name of names) {
      await expect(card.getByText(name)).toBeVisible();
    }
    // Stops numbered 1, 2, 3.
    await expect(card.getByRole('row', { name: new RegExp(names[0]) }).getByText('1', { exact: true })).toBeVisible();
    await page.waitForTimeout(1200);
  });

  await test.step('Both stops are tagged as trip stops on the Dispatches page', async () => {
    await page.goto('/dispatches');
    await expect(page.getByText('Trip · stop 1')).toBeVisible({ timeout: 8000 });
    await expect(page.getByText('Trip · stop 3')).toBeVisible();
    await page.waitForTimeout(1200);
  });
});
