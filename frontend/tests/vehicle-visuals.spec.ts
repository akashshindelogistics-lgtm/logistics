import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

// Vehicle types, their rendered icons, the vehicle-detail showcase and the
// 3D icons around the app. The live WebGL view is switched off here
// (logitrack:disable-3d) so the suite never depends on software WebGL in
// headless Chromium; the showcase then shows the still render of the same
// model. vehicle-visuals.demo.ts shows the real 3D view.

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('logitrack:disable-3d', '1'));
});

const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));

test.describe('Vehicle visuals', () => {
  test('a vehicle added with a type shows that type everywhere, and can be retyped', async ({ page }) => {
    await registerOrg(page, `Visuals ${uid()}`);
    const reg = `MH14TP${uid().toUpperCase().slice(-4)}`;

    // Add a tipper from the org page.
    await page.getByLabel('Registration Number').fill(reg);
    await page.getByLabel('Capacity (MT)').fill('12');
    await page.getByLabel('Type', { exact: true }).selectOption('Tipper');
    await page.getByRole('button', { name: /add vehicle/i }).click();
    const fleetRow = page.getByTestId('fleet-table').getByRole('row', { name: new RegExp(reg) });
    await expect(fleetRow).toBeVisible({ timeout: 8000 });
    await expect(fleetRow.locator('img.vehicle-type-icon')).toHaveAttribute('title', 'Tipper');

    // The fleet list names the type next to its icon.
    await page.goto('/vehicles');
    const row = page.getByRole('row', { name: new RegExp(reg) });
    await expect(row.getByRole('cell', { name: 'Tipper', exact: true })).toBeVisible({ timeout: 8000 });
    await expect(row.locator('img.vehicle-type-icon')).toBeVisible();

    // Detail page: the showcase falls back to the still render with a status.
    await row.getByRole('link', { name: reg }).click();
    const showcase = page.getByTestId('vehicle-showcase');
    await expect(showcase).toHaveAttribute('data-mode', 'poster');
    await expect(page.getByAltText('Tipper illustration')).toBeVisible();
    await expect(page.getByTestId('vehicle-activity')).toHaveText('Available');
    await expect(page.getByLabel('Type', { exact: true })).toHaveValue('Tipper');

    // Picking a new type previews it at once, and saving persists it.
    await page.getByLabel('Type', { exact: true }).selectOption('Tanker');
    await expect(page.getByAltText('Tanker illustration')).toBeVisible();
    await page.getByRole('button', { name: /^save$/i }).click();
    await expect(page.getByText(/vehicle updated/i)).toBeVisible({ timeout: 8000 });

    const res = await page.request.get('/api/vehicles', { headers: { Authorization: `Bearer ${await token(page)}` } });
    const stored = (await res.json()).data.find((v: { registration_number: string }) => v.registration_number === reg);
    expect(stored.vehicle_type).toBe('Tanker');
    await expect(page.getByTestId('vehicle-showcase')).toBeVisible();
  });

  test('a vehicle on a dispatch shows as busy on its detail page', async ({ page }) => {
    const org = await registerOrg(page, `Visuals Busy ${uid()}`);
    const reg = `MH14BY${uid().toUpperCase().slice(-4)}`;
    const auth = { Authorization: `Bearer ${await token(page)}` };
    const post = async (path: string, data: unknown) => {
      const r = await page.request.post(path, { data, headers: auth });
      expect(r.ok(), `${path} -> ${r.status()} ${await r.text()}`).toBeTruthy();
      return (await r.json()).data;
    };

    await post(`/api/orgs/${org.id}/vehicles`, { registration_number: reg, capacity: 5000, unit: 'MetricTon', vehicle_type: 'Trailer' });
    const driver = await post(`/api/orgs/${org.id}/drivers`, { name: 'Ramesh', license_number: `L${uid()}`, phone: '0' });
    await page.request.put(`/api/vehicles/${encodeURIComponent(reg)}/driver`, { data: { driver_id: driver.id }, headers: auth });
    const godown = await post(`/api/orgs/${org.id}/godowns`, { name: 'Yard', address: '1 Yard Rd' });
    await post(`/api/godowns/${godown.id}/stock`, { description: 'Blocks', quantity: 100, volume_in_size: 1 });
    const customer = await post(`/api/orgs/${org.id}/customers`, { name: 'Site A', address: '2 Site Rd', latitude: 18.5, longitude: 73.8 });
    await post(`/api/orgs/${org.id}/dispatch`, {
      customer_id: customer.id,
      line_items: [{ stock_description: 'Blocks', requested_quantity: 10 }],
    });

    await page.goto(`/vehicles/${encodeURIComponent(reg)}`);
    await expect(page.getByAltText('Trailer illustration')).toBeVisible({ timeout: 8000 });
    await expect(page.getByTestId('vehicle-activity')).toHaveText('On a trip');
  });

  test('3D icons decorate the dashboard and login page', async ({ page }) => {
    await registerOrg(page, `Visuals Icons ${uid()}`);
    await page.goto('/');
    const card = page.locator('.stat-card').filter({ hasText: 'Fleet Vehicles' });
    await expect(card.locator('img[data-icon3d="truck"]')).toBeVisible({ timeout: 8000 });
    await expect(page.locator('.dash-hero-art img')).toHaveCount(3);

    await page.evaluate(() => localStorage.clear());
    await page.goto('/login');
    await expect(page.locator('.login-brand img[data-icon3d="lorry"]')).toBeVisible();
    await expect(page.locator('.feat-icon img')).toHaveCount(4);
  });
});
