import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

// Motion features, checked for their end states. This suite runs with
// reducedMotion: 'reduce' (playwright.config.ts), so nothing animates and
// nothing races the assertions; tests/motion.demo.ts shows the motion itself.

const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post' | 'put', path: string, data: unknown) {
  const res = await page.request[method](path, { data, headers: { Authorization: `Bearer ${await token(page)}` } });
  expect(res.ok(), `${method} ${path} -> ${res.status()} ${await res.text()}`).toBeTruthy();
  return (await res.json()).data;
}

/** A truck with an active driver, stock in a godown, and customers with locations. */
async function seed(page: Page, orgId: string, customers = 1) {
  const reg = `MO${uid().toUpperCase().slice(-6)}`;
  const stock = `Blocks ${uid()}`;
  await api(page, 'post', `/api/orgs/${orgId}/vehicles`, { registration_number: reg, capacity: 100000, unit: 'MetricTon', vehicle_type: 'Tipper' });
  const driver = await api(page, 'post', `/api/orgs/${orgId}/drivers`, { name: 'Motion Driver', license_number: `L${uid()}`, phone: '0' });
  await api(page, 'put', `/api/vehicles/${encodeURIComponent(reg)}/driver`, { driver_id: driver.id });
  const godown = await api(page, 'post', `/api/orgs/${orgId}/godowns`, { name: 'Yard', address: '1 Yard Rd' });
  await api(page, 'post', `/api/godowns/${godown.id}/stock`, { description: stock, quantity: 500, volume_in_size: 1 });
  const customerIds: string[] = [];
  for (let i = 0; i < customers; i++) {
    const c = await api(page, 'post', `/api/orgs/${orgId}/customers`, {
      name: `Site ${i} ${uid()}`, address: `${i} Site Rd`, latitude: 18.5 + i * 0.03, longitude: 73.8 + i * 0.02,
    });
    customerIds.push(c.id);
  }
  return { reg, stock, customerIds };
}

test.describe('Motion', () => {
  test('the sidebar highlight sits on whichever link is active', async ({ page }) => {
    await registerOrg(page, `Motion Nav ${uid()}`);
    for (const name of ['Vehicles', 'Customers', 'Dashboard']) {
      await page.locator('.sidebar').getByRole('link', { name }).click();
      const active = page.locator('.sidebar-link.active');
      await expect(active).toHaveText(name);
      await expect(active.locator('.sidebar-link-highlight')).toHaveCount(1);
      await expect(page.locator('.sidebar-link-highlight')).toHaveCount(1);
    }
  });

  test('opening a vehicle from the fleet list lands on its header, and the breadcrumb leads back', async ({ page }) => {
    const org = await registerOrg(page, `Motion Fleet ${uid()}`);
    const { reg } = await seed(page, org.id);
    await page.goto('/vehicles');
    await page.getByRole('link', { name: reg }).click();
    await expect(page).toHaveURL(new RegExp(`/vehicles/${reg}$`));
    await expect(page.getByRole('heading', { level: 1, name: reg })).toBeVisible();
    await expect(page.getByAltText('Tipper illustration')).toBeAttached();

    await page.getByRole('link', { name: 'Fleet Vehicles' }).click();
    await expect(page).toHaveURL(/\/vehicles$/);
    await expect(page.getByRole('link', { name: reg })).toBeVisible();
  });

  test('a dispatch shows its lifecycle track, which advances with the status', async ({ page }) => {
    const org = await registerOrg(page, `Motion Lifecycle ${uid()}`);
    const { stock, customerIds } = await seed(page, org.id);
    await api(page, 'post', `/api/orgs/${org.id}/dispatch`, {
      customer_id: customerIds[0],
      line_items: [{ stock_description: stock, requested_quantity: 5 }],
    });

    await page.goto('/dispatches');
    const row = page.locator('tbody tr').filter({ has: page.getByTestId('lifecycle-track') }).first();
    const track = row.getByTestId('lifecycle-track');
    await expect(track).toHaveAttribute('aria-valuenow', '1', { timeout: 8000 });
    await expect(track).toHaveAccessibleName('Step 1 of 5: PENDING');

    await row.getByRole('button', { name: /^confirm$/i }).click();
    await expect(track).toHaveAttribute('aria-valuenow', '2', { timeout: 8000 });
    await expect(row.locator('.status-tag-pop')).toHaveText('CONFIRMED');
  });

  test('a trip map draws the route through its stops', async ({ page }) => {
    const org = await registerOrg(page, `Motion Trip ${uid()}`);
    const { stock, customerIds } = await seed(page, org.id, 3);
    await api(page, 'post', `/api/orgs/${org.id}/trips`, {
      stops: customerIds.map(id => ({ customer_id: id, line_items: [{ stock_description: stock, requested_quantity: 2 }] })),
      optimize_route: false,
    });

    await page.goto('/trips');
    const card = page.locator('.section-card').first();
    await card.getByRole('button', { name: /route map/i }).click();
    await expect(card.locator('path.map-route')).toHaveCount(1, { timeout: 8000 });
    await expect(card.locator('path.map-route')).toHaveAttribute('pathLength', '1');
  });

  test('the assistant panel opens and closes from its launcher', async ({ page }) => {
    await registerOrg(page, `Motion Assist ${uid()}`);
    await page.getByRole('button', { name: 'Open assistant' }).click();
    await expect(page.getByRole('dialog', { name: 'Ask your data' })).toBeVisible();
    await page.getByRole('button', { name: 'Collapse assistant' }).click();
    await expect(page.getByRole('dialog', { name: 'Ask your data' })).toHaveCount(0);
  });
});
