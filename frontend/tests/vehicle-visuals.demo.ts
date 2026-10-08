import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A short narrated walk through the **3D visuals** on their own: the 3D
 * icons on the login page and dashboard, a fleet with one vehicle of every
 * body type, those vehicles drawn as 3D markers on the live map, and the
 * vehicle page's turntable model — idle (green ring), on the road (blue
 * ring, wheels turning) and previewing a different body type before saving.
 *
 * Watch it with `npm run test:e2e:demo:vehicle-visuals` (headed, slowed, real
 * WebGL). The `.demo.ts` name keeps it out of the default headless suite;
 * `vehicle-visuals.spec.ts` is the assertion-focused coverage that runs there
 * (with the 3D view switched off, showing the still render instead).
 */
const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post' | 'put', path: string, data: unknown) {
  const res = await page.request[method](path, { data, headers: { Authorization: `Bearer ${await token(page)}` } });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return res.json();
}

const FLEET = [
  { type: 'Truck', cap: '20', lat: 18.5204, lng: 73.8567 },
  { type: 'Tipper', cap: '16', lat: 18.5590, lng: 73.7868 },
  { type: 'Trailer', cap: '32', lat: 18.6298, lng: 73.7997 },
  { type: 'Tempo', cap: '3', lat: 18.5018, lng: 73.9260 },
  { type: 'Pickup', cap: '1', lat: 18.4655, lng: 73.8640 },
  { type: 'Tanker', cap: '24', lat: 18.5913, lng: 73.9064 },
];

test('3D icons and vehicle models', async ({ page }) => {
  test.slow();
  const tag = uid().toUpperCase().slice(-4);
  const regOf = (type: string) => `MH12V${FLEET.findIndex(v => v.type === type)}${tag}`;

  await test.step('The login page greets you with 3D icons', async () => {
    await page.goto('/login');
    await expect(page.locator('.login-brand img[data-icon3d="lorry"]')).toBeVisible();
    await page.waitForTimeout(2000);
  });

  const org = await registerOrg(page, `3D Fleet ${uid()}`);

  await test.step('Add one vehicle of every body type from the org page', async () => {
    for (const v of FLEET) {
      await page.getByLabel('Registration Number').fill(regOf(v.type));
      await page.getByLabel('Capacity (MT)').fill(v.cap);
      await page.getByLabel('Type', { exact: true }).selectOption(v.type);
      await page.getByRole('button', { name: /add vehicle/i }).click();
      await expect(page.getByTestId('fleet-table').getByText(regOf(v.type))).toBeVisible({ timeout: 8000 });
    }
    await page.getByTestId('fleet-table').scrollIntoViewIfNeeded();
    await page.waitForTimeout(2000);
  });

  await test.step('GPS trackers report in, and the fleet map draws each vehicle as its own 3D model', async () => {
    for (const v of FLEET) {
      await api(page, 'put', `/api/vehicles/${encodeURIComponent(regOf(v.type))}/location`, {
        latitude: v.lat, longitude: v.lng, address: null,
      });
    }
    await page.goto('/vehicles');
    await expect(page.locator('.map-pin-3d')).toHaveCount(FLEET.length, { timeout: 10000 });
    await page.waitForTimeout(2500);
    await page.getByRole('cell', { name: 'Tanker', exact: true }).scrollIntoViewIfNeeded();
    await page.waitForTimeout(2000);
  });

  await test.step('Open the tanker: its model turns on a green "Available" ring — drag to look around', async () => {
    await page.getByRole('link', { name: regOf('Tanker') }).click();
    const showcase = page.getByTestId('vehicle-showcase');
    await expect(showcase).toHaveAttribute('data-mode', '3d');
    await expect(showcase).toHaveClass(/is-ready/, { timeout: 30000 });
    await expect(page.getByTestId('vehicle-activity')).toHaveText('Available');
    await page.waitForTimeout(3500);

    // Grab the model and spin it by hand.
    const box = (await showcase.locator('canvas').boundingBox())!;
    await page.mouse.move(box.x + box.width * 0.35, box.y + box.height * 0.5);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width * 0.75, box.y + box.height * 0.42, { steps: 25 });
    await page.mouse.up();
    await page.waitForTimeout(2500);
  });

  await test.step('Send the truck out on a delivery: its ring turns blue and the wheels roll', async () => {
    const truck = regOf('Truck');
    const driver = await api(page, 'post', `/api/orgs/${org.id}/drivers`, { name: 'Sunil Jadhav', license_number: `L${uid()}`, phone: '0' });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(truck)}/driver`, { driver_id: driver.data.id });
    const godown = await api(page, 'post', `/api/orgs/${org.id}/godowns`, { name: 'Block Yard', address: 'Chakan MIDC' });
    await api(page, 'post', `/api/godowns/${godown.data.id}/stock`, { description: 'AAC Blocks', quantity: 500, volume_in_size: 1 });
    const customer = await api(page, 'post', `/api/orgs/${org.id}/customers`, {
      name: 'Hinjewadi Site', address: 'Phase 2, Hinjewadi', latitude: 18.59, longitude: 73.73,
    });
    // Only the truck has a driver, so the dispatch lands on it.
    const dispatch = await api(page, 'post', `/api/orgs/${org.id}/dispatch`, {
      customer_id: customer.data.id,
      line_items: [{ stock_description: 'AAC Blocks', requested_quantity: 12 }],
    });
    expect(dispatch.data.vehicle_registration_number).toBe(truck);
    for (const status of ['CONFIRMED', 'LOADED', 'IN_TRANSIT']) {
      await api(page, 'put', `/api/dispatches/${dispatch.data.id}/status`, { status });
    }

    await page.goto(`/vehicles/${encodeURIComponent(truck)}`);
    await expect(page.getByTestId('vehicle-showcase')).toHaveClass(/is-ready/, { timeout: 30000 });
    await expect(page.getByTestId('vehicle-activity')).toHaveText('In transit');
    await page.waitForTimeout(5000);
  });

  await test.step('Pick a different body type: the model swaps before you save', async () => {
    const type = page.getByLabel('Type', { exact: true });
    for (const t of ['Tipper', 'Tempo', 'Tanker']) {
      await type.selectOption(t);
      await expect(page.getByTestId('vehicle-showcase')).toHaveClass(/is-ready/, { timeout: 30000 });
      await page.waitForTimeout(2200);
    }
    await type.selectOption('Truck');
    await page.getByRole('button', { name: /^save$/i }).click();
    await expect(page.getByText(/vehicle updated/i)).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(1500);
  });

  await test.step('Back on the dashboard, every stat card carries a 3D icon', async () => {
    await page.goto('/');
    await expect(page.locator('.stat-card img[data-icon3d]')).toHaveCount(4, { timeout: 8000 });
    await page.locator('.stat-card').filter({ hasText: 'Fleet Vehicles' }).hover();
    await page.waitForTimeout(2500);
  });
});
