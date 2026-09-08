import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));

async function api(page: Page, method: 'post' | 'put', path: string, data: unknown) {
  const auth = { Authorization: `Bearer ${await token(page)}` };
  const res = await page.request[method](path, { data, headers: auth });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return res.json();
}

test.describe('Reports', () => {
  test('a fresh org sees a zeroed report with a full 14-day volume axis', async ({ page }) => {
    await registerOrg(page, `Report Empty ${uid()}`);
    await page.goto('/reports');

    await expect(page.getByRole('heading', { level: 1, name: 'Reports' })).toBeVisible();

    const util = page.locator('.stat-card', { hasText: 'Fleet utilization' });
    await expect(util.getByText('0.0%')).toBeVisible({ timeout: 8000 });
    await expect(util.getByText('0 of 0 on a trip')).toBeVisible();

    // The volume chart always renders one cell per day of the window.
    await expect(page.getByText(/dispatch volume/i)).toBeVisible();
    await expect(page.getByTitle(/^\d{4}-\d{2}-\d{2}: \d+$/)).toHaveCount(14);

    await expect(page.getByText('No godowns yet.')).toBeVisible();
  });

  test('a dispatch shows up in fleet utilization, volume and inventory', async ({ page }) => {
    const org = await registerOrg(page, `Report Live ${uid()}`);
    const vehReg = `RP${uid().toUpperCase().slice(0, 6)}`;
    const stockDesc = `Cement ${uid()}`;
    const custName = `Report Customer ${uid()}`;

    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, {
      registration_number: vehReg, capacity: 50, unit: 'MetricTon',
    });
    const driver = await api(page, 'post', `/api/orgs/${org.id}/drivers`, {
      name: 'Rep Driver', license_number: `LIC-${uid()}`, phone: '+91 90000 00000',
    });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(vehReg)}/driver`, { driver_id: driver.data.id });

    const godown = await api(page, 'post', `/api/orgs/${org.id}/godowns`, {
      name: 'Report Godown', address: '1 Dock Rd', max_capacity: 1000,
    });
    await api(page, 'post', `/api/godowns/${godown.data.id}/stock`, {
      description: stockDesc, quantity: 200, volume_in_size: 1,
    });

    const customer = await api(page, 'post', `/api/orgs/${org.id}/customers`, {
      name: custName, address: '9 Buyer St', latitude: 18.52, longitude: 73.85,
    });

    await page.goto(`/orgs/${org.id}`);
    await page.getByLabel('Customer').selectOption({ value: customer.data.id });
    await page.getByLabel('Stock Description').fill(stockDesc);
    await page.getByLabel('Quantity', { exact: true }).fill('40');
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });

    await page.goto('/reports');

    const util = page.locator('.stat-card', { hasText: 'Fleet utilization' });
    await expect(util.getByText('100.0%')).toBeVisible({ timeout: 8000 });
    await expect(util.getByText('1 of 1 on a trip')).toBeVisible();

    // Today's bucket (the last cell) now has a count of 1.
    await expect(page.getByTitle(/^\d{4}-\d{2}-\d{2}: 1$/)).toBeVisible();
    await expect(page.getByText(/40 units dispatched in 30 days/)).toBeVisible();

    const godownRow = page.getByRole('row', { name: /Report Godown/ });
    await expect(godownRow.getByText('160')).toBeVisible(); // 200 - 40 dispatched
    await expect(godownRow.getByText('16.0%')).toBeVisible(); // 160 vol / 1000 cap
  });
});
