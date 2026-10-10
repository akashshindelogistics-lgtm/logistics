import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A narrated walk through the **motion** in the dashboard on its own: stat
 * cards that rise and count up, the sidebar highlight sliding between pages
 * as they cross-fade, table rows sliding in and out, a vehicle's icon
 * morphing from the fleet list into its page, the dispatch lifecycle track
 * filling as a shipment advances, a truck gliding across the live map between
 * GPS fixes, a trip's route drawing itself, and the assistant springing open.
 *
 * Watch it with `npm run test:e2e:demo:motion` (headed, slowed, motion on).
 * The `.demo.ts` name keeps it out of the default headless suite, which runs
 * with reduced motion; `motion.spec.ts` is the assertion-focused coverage.
 */
const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post' | 'put' | 'delete', path: string, data?: unknown) {
  const res = await page.request[method](path, { data, headers: { Authorization: `Bearer ${await token(page)}` } });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return (await res.json()).data;
}

test('motion in the UI', async ({ page }) => {
  test.slow();
  const tag = uid().toUpperCase().slice(-4);
  const truck = `MH12MT${tag}`;
  const tipper = `MH12MP${tag}`;
  const tempo = `MH12MO${tag}`;
  const stock = `AAC Blocks ${uid()}`;
  const org = await registerOrg(page, `Motion Demo ${uid()}`);
  const customerIds: string[] = [];

  await test.step('Set up a small fleet, stock and three delivery sites', async () => {
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, { registration_number: truck, capacity: 100000, unit: 'MetricTon', vehicle_type: 'Truck' });
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, { registration_number: tipper, capacity: 40, unit: 'MetricTon', vehicle_type: 'Tipper' });
    const driver = await api(page, 'post', `/api/orgs/${org.id}/drivers`, { name: 'Sunil Jadhav', license_number: `L${uid()}`, phone: '0' });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(truck)}/driver`, { driver_id: driver.id });
    // A second driven vehicle, free for the trip once the truck is out on its dispatch.
    const driver2 = await api(page, 'post', `/api/orgs/${org.id}/drivers`, { name: 'Prakash More', license_number: `L${uid()}`, phone: '0' });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(tipper)}/driver`, { driver_id: driver2.id });
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, { registration_number: tempo, capacity: 3, unit: 'MetricTon', vehicle_type: 'Tempo' });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(truck)}/location`, { latitude: 18.5204, longitude: 73.8567, address: null });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(tipper)}/location`, { latitude: 18.56, longitude: 73.79, address: null });
    const godown = await api(page, 'post', `/api/orgs/${org.id}/godowns`, { name: 'Chakan Yard', address: 'Chakan MIDC' });
    await api(page, 'post', `/api/godowns/${godown.id}/stock`, { description: stock, quantity: 1000, volume_in_size: 1 });
    for (const [i, site] of ['Hinjewadi', 'Wakad', 'Baner'].entries()) {
      const c = await api(page, 'post', `/api/orgs/${org.id}/customers`, {
        name: `${site} Site`, address: `${site}, Pune`, latitude: 18.59 - i * 0.02, longitude: 73.74 + i * 0.03,
      });
      customerIds.push(c.id);
    }
  });

  await test.step('The org page: sections rise in, counts tick up, new rows and cards slide in', async () => {
    await page.goto(`/orgs/${org.id}`);
    await expect(page.getByTestId('fleet-table')).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(1800);

    // A new vehicle's row slides into the fleet table, and the header count ticks up.
    const pickup = `MH12MK${tag}`;
    await page.getByLabel('Registration Number').fill(pickup);
    await page.getByLabel('Capacity (MT)').fill('2');
    await page.getByLabel('Type', { exact: true }).selectOption('Pickup');
    await page.getByRole('button', { name: /add vehicle/i }).click();
    await expect(page.getByTestId('fleet-table').getByText(pickup)).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(1500);

    // A new godown's card slides in under the existing one.
    await page.getByLabel('Godown Name').fill('Overflow Yard');
    await page.getByLabel('Address').fill('Talegaon MIDC');
    await page.getByRole('button', { name: /add godown/i }).click();
    await expect(page.getByTestId('godown-card')).toHaveCount(2, { timeout: 8000 });
    await page.getByTestId('godown-card').last().scrollIntoViewIfNeeded();
    await page.waitForTimeout(1500);

    // Dispatch form: an extra stock line slides in, and out again when removed.
    await page.getByRole('button', { name: /add another line/i }).scrollIntoViewIfNeeded();
    await page.getByRole('button', { name: /add another line/i }).click();
    await expect(page.getByTestId('dispatch-line-item')).toHaveCount(2);
    await page.waitForTimeout(1000);
    await page.getByRole('button', { name: 'Remove stock line 2' }).click();
    await expect(page.getByTestId('dispatch-line-item')).toHaveCount(1);
    await page.waitForTimeout(1000);
  });

  await test.step('The dashboard: stat cards rise in one after another and count up', async () => {
    await page.goto('/');
    await expect(page.locator('.stat-card')).toHaveCount(4);
    await page.waitForTimeout(2500);
  });

  await test.step('Move around with the sidebar: the highlight slides, pages cross-fade', async () => {
    for (const name of ['Vehicles', 'Customers', 'Dispatches', 'Trips', 'Vehicles']) {
      await page.locator('.sidebar').getByRole('link', { name }).click();
      await expect(page.locator('.sidebar-link.active')).toHaveText(name);
      await page.waitForTimeout(700);
    }
  });

  await test.step('Open the truck: its icon morphs out of the fleet list into the vehicle page', async () => {
    await page.getByRole('link', { name: truck }).click();
    await expect(page.getByRole('heading', { level: 1, name: truck })).toBeVisible();
    await page.waitForTimeout(2000);
    await page.getByRole('link', { name: 'Fleet Vehicles' }).click();
    await expect(page.getByRole('link', { name: truck })).toBeVisible();
    await page.waitForTimeout(1200);
  });

  await test.step('Watch the truck glide across the live map as its GPS reports in', async () => {
    await page.locator('.leaflet-container').scrollIntoViewIfNeeded();
    // A tracker on the truck reports two fresh fixes; the page re-reads
    // positions every 10 s and the marker glides to each one.
    for (const [lat, lng] of [[18.5420, 73.8250], [18.5600, 73.8000]]) {
      await api(page, 'put', `/api/vehicles/${encodeURIComponent(truck)}/location`, { latitude: lat, longitude: lng, address: null });
      await page.waitForTimeout(11_500);
    }
  });

  await test.step('Retire the tempo: its row slides out of the table', async () => {
    page.once('dialog', d => d.accept());
    await page.getByRole('row', { name: new RegExp(tempo) }).getByRole('button', { name: /remove/i }).click();
    await expect(page.getByRole('link', { name: tempo })).toHaveCount(0);
    await page.waitForTimeout(1500);
  });

  await test.step('Dispatch a load and walk it through its lifecycle: the track fills, the tag pops', async () => {
    await api(page, 'post', `/api/orgs/${org.id}/dispatch`, {
      customer_id: customerIds[0],
      line_items: [{ stock_description: stock, requested_quantity: 20 }],
    });
    await page.goto('/dispatches');
    const row = page.locator('tbody tr').filter({ has: page.getByTestId('lifecycle-track') }).first();
    await expect(row.getByTestId('lifecycle-track')).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(1200);
    for (const action of [/^confirm$/i, /mark loaded/i, /mark in transit/i]) {
      await row.getByRole('button', { name: action }).click();
      await page.waitForTimeout(1600);
    }
    await expect(row.getByTestId('lifecycle-track')).toHaveClass(/is-moving/);
    await page.waitForTimeout(2500);
  });

  await test.step("Plan a trip to the three sites and open its map: the route draws itself", async () => {
    await api(page, 'post', `/api/orgs/${org.id}/trips`, {
      stops: customerIds.map(id => ({ customer_id: id, line_items: [{ stock_description: stock, requested_quantity: 5 }] })),
      optimize_route: false,
    });
    await page.locator('.sidebar').getByRole('link', { name: 'Trips' }).click();
    await page.getByRole('button', { name: /route map/i }).first().click();
    await expect(page.locator('path.map-route')).toHaveCount(1, { timeout: 8000 });
    await page.locator('.leaflet-container').first().scrollIntoViewIfNeeded();
    await page.waitForTimeout(3000);
  });

  await test.step('Open and close the assistant: it springs out of its corner', async () => {
    await page.getByRole('button', { name: 'Open assistant' }).click();
    await expect(page.getByRole('dialog', { name: 'Ask your data' })).toBeVisible();
    await page.waitForTimeout(1500);
    await page.getByRole('button', { name: 'Collapse assistant' }).click();
    await page.waitForTimeout(1200);
  });
});
