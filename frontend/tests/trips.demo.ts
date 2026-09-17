import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A short narrated walk through **multi-stop trips** on its own — one truck
 * planned to visit three customers in a row, each getting their own stock,
 * with the "Optimize stop order" route-optimization checkbox reordering the
 * two stops after the first by proximity — then a look at the trip's live
 * route map, showing the truck's GPS position alongside its planned stops.
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
  // Given to the form in this order: South (the fixed start), North (far),
  // East (near) — so "Optimize stop order" has something real to fix,
  // visiting East before the farther-away North.
  const names = [`South Store ${uid()}`, `North Store ${uid()}`, `East Store ${uid()}`];
  const latitudes = [19.0, 19.08, 19.01];
  const reg = `TD${uid().toUpperCase()}`;

  await test.step('Set up a truck and stock, and three customers', async () => {
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, { registration_number: reg, capacity: 100000, unit: 'MetricTon' });
    const d = await api(page, 'post', `/api/orgs/${org.id}/drivers`, { name: 'Ravi Kumar', license_number: `L${uid()}`, phone: '0' });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(reg)}/driver`, { driver_id: d.data.id });
    const g = await api(page, 'post', `/api/orgs/${org.id}/godowns`, { name: 'Central Godown', address: '1 Dock Rd' });
    await api(page, 'post', `/api/godowns/${g.data.id}/stock`, { description: stock, quantity: 500, volume_in_size: 1 });
    for (const [i, name] of names.entries()) {
      await api(page, 'post', `/api/orgs/${org.id}/customers`, {
        name, address: `${i} Market Rd`, latitude: latitudes[i], longitude: 72.8,
      });
    }
    // A GPS tracker fitted to the truck reports it's already on the road,
    // just north of the fixed starting stop — same push a real device would
    // make to POST /api/track/{tracker_key}, done here as the equivalent
    // manual update so the demo doesn't need a real device.
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(reg)}/location`, { latitude: 19.02, longitude: 72.8 });
  });

  await test.step('Open the Trips page', async () => {
    await page.getByRole('link', { name: /trips/i }).click();
    await expect(page).toHaveURL(/\/trips$/);
    await expect(page.getByRole('heading', { level: 1, name: 'Multi-stop Trips' })).toBeVisible();
    await page.waitForTimeout(700);
  });

  await test.step('Plan a three-stop trip, given out of the nearest-first order', async () => {
    await page.getByRole('button', { name: /plan a trip/i }).click();
    await page.getByRole('button', { name: /add another stop/i }).click();
    for (const [i, name] of names.entries()) {
      await page.getByLabel(new RegExp(`stop ${i + 1} — customer`, 'i')).selectOption({ label: name });
      await page.getByLabel(/stock item/i).nth(i).fill(stock);
      await page.getByLabel(/quantity/i).nth(i).fill(String((i + 1) * 10));
      await page.waitForTimeout(200);
    }
    await page.waitForTimeout(400);
  });

  await test.step('Check "Optimize stop order" and submit', async () => {
    await page.getByRole('checkbox', { name: /optimize stop order/i }).check();
    await page.waitForTimeout(500);
    await page.getByRole('button', { name: /^plan trip$/i }).click();
    await page.waitForTimeout(800);
  });

  await test.step('The trip keeps South as stop 1, but visits East before the farther North', async () => {
    const card = page.locator('.section-card').first();
    await expect(card).toBeVisible({ timeout: 8000 });
    await expect(card.getByText('PLANNED')).toBeVisible();
    for (const name of names) {
      await expect(card.getByText(name)).toBeVisible();
    }
    const rows = card.locator('tbody tr');
    await expect(rows.nth(0)).toContainText(names[0]); // South — fixed start
    await expect(rows.nth(1)).toContainText(names[2]); // East — nearer, visited 2nd
    await expect(rows.nth(2)).toContainText(names[1]); // North — farther, visited 3rd
    await page.waitForTimeout(1200);
  });

  await test.step('Both stops are tagged as trip stops on the Dispatches page', async () => {
    await page.goto('/dispatches');
    await expect(page.getByText('Trip · stop 1')).toBeVisible({ timeout: 8000 });
    await expect(page.getByText('Trip · stop 3')).toBeVisible();
    await page.waitForTimeout(1200);
  });

  await test.step("Back on Trips, open the route map to see the truck's live position alongside its stops", async () => {
    await page.goto('/trips');
    const card = page.locator('.section-card').first();
    await expect(card).toBeVisible({ timeout: 8000 });
    await card.getByRole('button', { name: /route map/i }).click();
    await expect(card.locator('.leaflet-container')).toBeVisible();
    await expect(card.locator('.leaflet-marker-icon')).toHaveCount(1 + names.length);
    await page.waitForTimeout(1800);
  });
});
