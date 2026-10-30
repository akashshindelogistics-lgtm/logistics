import { test, expect, type Page } from '@playwright/test';
import { loginOrg, registerOrg, uid, type TestOrg } from './helpers';

/**
 * The dispatcher's side of **driver phone tracking**: the position the
 * driver's phone reported shows on the Vehicles page (fleet map and the
 * Latitude / Longitude columns) and on the vehicle's own page.
 *
 * Normally run at the end of `android/scripts/demo-tracking.sh`, which drives
 * the Android app on the emulator and passes what it seeded:
 * DRIVER_DEMO_ORG_NAME, DRIVER_DEMO_ORG_PASSWORD, DRIVER_DEMO_REG and the
 * position the phone ended at, DRIVER_DEMO_LAT / DRIVER_DEMO_LON.
 *
 * On its own (`npm run test:e2e:demo:driver-tracking`) it seeds a driver and
 * reports one fix through POST /api/driver/location with the device token,
 * the same request the app makes.
 */
const env = process.env;

async function seedWithoutPhone(page: Page) {
  const org = await registerOrg(page, `Driver Tracking Demo ${uid()}`);
  const reg = `DT${uid().toUpperCase()}`;
  const auth = { Authorization: `Bearer ${await page.evaluate(() => localStorage.getItem('logi_token'))}` };
  const post = async (path: string, data?: unknown) => {
    const res = await page.request.post(path, { data, headers: auth });
    if (!res.ok()) throw new Error(`POST ${path} -> ${res.status()} ${await res.text()}`);
    return res.json();
  };
  await post(`/api/orgs/${org.id}/vehicles`, { registration_number: reg, capacity: 20, unit: 'MetricTon' });
  const driver = await post(`/api/orgs/${org.id}/drivers`, { name: 'Ramesh Patil', license_number: `L${uid()}`, phone: '0' });
  const assigned = await page.request.put(`/api/vehicles/${reg}/driver`, { data: { driver_id: driver.data.id }, headers: auth });
  expect(assigned.ok()).toBeTruthy();
  const token = (await post(`/api/drivers/${driver.data.id}/device-token/rotate`)).data.device_token;
  const [lat, lon] = [18.7210, 73.8330];
  const fix = await page.request.post('/api/driver/location', {
    data: { fixes: [{ latitude: lat, longitude: lon, recorded_at: Math.floor(Date.now() / 1000), accuracy_m: 8 }] },
    headers: { Authorization: `Bearer ${token}` },
  });
  expect(fix.ok()).toBeTruthy();
  return { org, reg, lat, lon };
}

/** Within about 10 m: the dashboard rounds, and the database stores a float. */
const near = (text: string, want: number) => Math.abs(parseFloat(text) - want) < 0.0001;

test('driver phone tracking on the dashboard', async ({ page }) => {
  test.slow();
  let org: TestOrg, reg: string, lat: number, lon: number;

  if (env.DRIVER_DEMO_REG) {
    org = { name: env.DRIVER_DEMO_ORG_NAME!, password: env.DRIVER_DEMO_ORG_PASSWORD!, address: '', id: '' };
    reg = env.DRIVER_DEMO_REG;
    lat = Number(env.DRIVER_DEMO_LAT);
    lon = Number(env.DRIVER_DEMO_LON);
    await test.step(`Sign in as ${org.name}`, () => loginOrg(page, org));
  } else {
    await test.step('Seed a driver and report a fix as the phone would', async () => {
      ({ org, reg, lat, lon } = await seedWithoutPhone(page));
    });
  }

  await test.step('Vehicles page: the fleet map and the coordinates', async () => {
    await page.goto('/vehicles');
    await expect(page.getByText('Live Fleet Map')).toBeVisible();
    const row = page.getByRole('row').filter({ hasText: reg });
    await expect(row).toBeVisible();
    const cells = row.locator('td.coord-cell');
    await expect.poll(async () => near(await cells.nth(0).innerText(), lat)).toBe(true);
    await expect.poll(async () => near(await cells.nth(1).innerText(), lon)).toBe(true);
    await row.scrollIntoViewIfNeeded();
    await page.waitForTimeout(1500);

    // The org has one vehicle, so the map has one pin: open its popup.
    await page.getByText('Live Fleet Map').scrollIntoViewIfNeeded();
    await page.locator('.leaflet-marker-icon').first().click();
    await expect(page.locator('.leaflet-popup-content')).toContainText(reg);
    await page.waitForTimeout(2000);
  });

  await test.step("The vehicle's own page", async () => {
    await page.goto(`/vehicles/${encodeURIComponent(reg)}`);
    const location = page.getByText(`${lat.toFixed(4)}, ${lon.toFixed(4)}`);
    await expect(location).toBeVisible();
    await location.scrollIntoViewIfNeeded();
    await page.waitForTimeout(2500);
  });
});
