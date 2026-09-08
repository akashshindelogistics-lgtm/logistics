import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A short narrated walk through the operations **Reports** feature on its own
 * — set up a small org, put one shipment on the road, then read the report.
 *
 * Watch it with `npm run test:e2e:demo:reports` (headed, slowed). The
 * `.demo.ts` name keeps it out of the default headless suite
 * (`npm run test:e2e`); `reports.spec.ts` is the assertion-focused coverage
 * that runs there.
 */
const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post' | 'put', path: string, data: unknown) {
  const res = await page.request[method](path, {
    data,
    headers: { Authorization: `Bearer ${await token(page)}` },
  });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return res.json();
}

test('operations reporting', async ({ page }) => {
  test.slow();
  const org = await registerOrg(page, `Reports Demo ${uid()}`);
  const stock = `Cement ${uid()}`;
  const custName = `Buyer ${uid()}`;

  await test.step('Register a small fleet and stock two godowns', async () => {
    for (let i = 0; i < 3; i++) {
      const reg = `RPT${uid().toUpperCase()}${i}`;
      await api(page, 'post', `/api/orgs/${org.id}/vehicles`, {
        registration_number: reg, capacity: 500, unit: 'MetricTon',
      });
      const driver = await api(page, 'post', `/api/orgs/${org.id}/drivers`, {
        name: `Driver ${i}`, license_number: `L${uid()}${i}`, phone: '0',
      });
      await api(page, 'put', `/api/vehicles/${encodeURIComponent(reg)}/driver`, { driver_id: driver.data.id });
    }
    const north = await api(page, 'post', `/api/orgs/${org.id}/godowns`, {
      name: 'North Godown', address: 'Dock 1', max_capacity: 2000,
    });
    await api(page, 'post', `/api/godowns/${north.data.id}/stock`, { description: stock, quantity: 800, volume_in_size: 1 });
    const south = await api(page, 'post', `/api/orgs/${org.id}/godowns`, { name: 'South Godown', address: 'Dock 2' });
    await api(page, 'post', `/api/godowns/${south.data.id}/stock`, { description: `Rods ${uid()}`, quantity: 150, volume_in_size: 2 });
    await api(page, 'post', `/api/orgs/${org.id}/customers`, {
      name: custName, address: '9 Market Rd', latitude: 18.52, longitude: 73.85,
    });
  });

  await test.step('Dispatch a shipment to the customer', async () => {
    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ label: custName });
    await page.getByLabel('Stock Description').fill(stock);
    await page.getByLabel('Quantity', { exact: true }).fill('120');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });
  });

  await test.step('Open Reports from the sidebar', async () => {
    await page.getByRole('link', { name: /reports/i }).click();
    await expect(page).toHaveURL(/\/reports$/);
    await expect(page.getByRole('heading', { level: 1, name: 'Reports' })).toBeVisible();
    await page.waitForTimeout(800);
  });

  await test.step('Read the headline numbers', async () => {
    // One of three trucks is out with the shipment.
    const util = page.locator('.stat-card', { hasText: 'Fleet utilization' });
    await expect(util.getByText('33.3%')).toBeVisible({ timeout: 8000 });
    await expect(util.getByText('1 of 3 on a trip')).toBeVisible();
    await expect(
      page.locator('.stat-card', { hasText: 'Delivered' }).getByText('0', { exact: true }),
    ).toBeVisible();
    await page.waitForTimeout(800);
  });

  await test.step('See today on the 14-day dispatch-volume chart', async () => {
    await expect(page.getByText(/dispatch volume/i)).toBeVisible();
    await expect(page.getByTitle(/^\d{4}-\d{2}-\d{2}: 1$/)).toBeVisible();
    await expect(page.getByText(/120 units dispatched in 30 days/)).toBeVisible();
    await page.waitForTimeout(800);
  });

  await test.step('Check godown inventory', async () => {
    // North drew down 800 -> 680; volume 680 / 2000 cap = 34%.
    const north = page.getByRole('row', { name: /North Godown/ });
    await expect(north.getByText('680')).toBeVisible();
    await expect(north.getByText('34.0%')).toBeVisible();
    // South has no cap set.
    await expect(page.getByRole('row', { name: /South Godown/ }).getByText('no cap')).toBeVisible();
    await page.waitForTimeout(1000);
  });
});
