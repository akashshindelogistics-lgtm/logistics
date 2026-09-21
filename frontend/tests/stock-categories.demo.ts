import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A short narrated walk through **stock categories** on its own — tag stock
 * with a free-text category when it's added, watch the category ride along
 * unchanged when it's transferred to another godown, and watch it show up on
 * a dispatched shipment's line item, since that's the actual point of the
 * feature: the transport side reflects what *kind* of goods are moving.
 *
 * Watch it with `npm run test:e2e:demo:stock-categories` (headed, slowed).
 * The `.demo.ts` name keeps it out of the default headless suite
 * (`npm run test:e2e`); `organization.spec.ts` is the assertion-focused
 * coverage that runs there.
 */
const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post' | 'put', path: string, data: unknown) {
  const res = await page.request[method](path, { data, headers: { Authorization: `Bearer ${await token(page)}` } });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return res.json();
}

test('stock categories', async ({ page }) => {
  test.slow();
  const org = await registerOrg(page, `Categories Demo ${uid()}`);
  const cement = `Cement ${uid()}`;
  const chairs = `Office Chairs ${uid()}`;
  const custName = `Buyer ${uid()}`;
  const reg = `CAT${uid().toUpperCase()}`;

  await test.step('Set up a truck, an active driver, and a customer', async () => {
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, { registration_number: reg, capacity: 5000, unit: 'MetricTon' });
    const driver = await api(page, 'post', `/api/orgs/${org.id}/drivers`, {
      name: 'Suresh Patil', license_number: `L${uid()}`, phone: '0',
    });
    await api(page, 'put', `/api/vehicles/${encodeURIComponent(reg)}/driver`, { driver_id: driver.data.id });
    await api(page, 'post', `/api/orgs/${org.id}/customers`, {
      name: custName, address: '9 Market Rd', latitude: 18.52, longitude: 73.85,
    });
  });

  await test.step('Create two godowns', async () => {
    await page.getByLabel('Godown Name').fill('Main Warehouse');
    await page.getByLabel('Address').fill('Plot 5, Industrial Area');
    await page.getByRole('button', { name: /add godown/i }).click();
    await expect(page.getByText('Main Warehouse')).toBeVisible({ timeout: 8000 });

    await page.getByLabel('Godown Name').fill('Overflow Warehouse');
    await page.getByLabel('Address').fill('Plot 9, Industrial Area');
    await page.getByRole('button', { name: /add godown/i }).click();
    await expect(page.getByText('Overflow Warehouse')).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(500);
  });

  const mainCard = () => page.getByTestId('godown-card').filter({ has: page.getByRole('link', { name: 'Main Warehouse', exact: true }) });

  await test.step('Add stock with an explicit category', async () => {
    const card = mainCard();
    await card.getByLabel('Stock Item').fill(cement);
    await card.getByLabel('Stock Quantity').fill('300');
    await card.getByLabel('Volume').fill('5');
    await card.getByLabel('Category').fill('Building Materials');
    await page.waitForTimeout(400);
    await card.getByRole('button', { name: /add stock/i }).click();
    await expect(card.locator('table').getByText(cement)).toBeVisible({ timeout: 8000 });
    await expect(card.locator('tr', { has: page.getByText(cement) }).getByText('Building Materials')).toBeVisible();
    await page.waitForTimeout(1000);
  });

  await test.step('Add a second item with no category, and watch it default to "General"', async () => {
    const card = mainCard();
    await card.getByLabel('Stock Item').fill(chairs);
    await card.getByLabel('Stock Quantity').fill('40');
    await card.getByLabel('Volume').fill('1');
    // Category left blank on purpose.
    await page.waitForTimeout(400);
    await card.getByRole('button', { name: /add stock/i }).click();
    await expect(card.locator('table').getByText(chairs)).toBeVisible({ timeout: 8000 });
    await expect(card.locator('tr', { has: page.getByText(chairs) }).getByText('General')).toBeVisible();
    await page.waitForTimeout(1200);
  });

  await test.step('Transfer some of the categorized stock to the overflow godown', async () => {
    const card = mainCard();
    await card.getByLabel('Transfer Item').selectOption({ label: cement });
    await card.getByLabel('To Godown').selectOption({ label: 'Overflow Warehouse' });
    await card.getByLabel('Transfer Quantity').fill('80');
    await page.waitForTimeout(400);
    await card.getByRole('button', { name: /^transfer$/i }).click();
    await expect(page.getByText(/stock transferred between godowns/i)).toBeVisible({ timeout: 8000 });

    // The new row created in the overflow godown carries the same category.
    const overflowCard = page.getByTestId('godown-card').filter({
      has: page.getByRole('link', { name: 'Overflow Warehouse', exact: true }),
    });
    await expect(overflowCard.locator('tr', { has: page.getByText(cement) }).getByText('Building Materials')).toBeVisible();
    await page.waitForTimeout(1400);
  });

  await test.step('Dispatch the categorized stock to a customer', async () => {
    await page.getByLabel('Customer').selectOption({ label: custName });
    await page.getByLabel('Stock Description').fill(cement);
    await page.getByLabel('Quantity', { exact: true }).fill('25');
    await page.waitForTimeout(400);
    await page.getByRole('button', { name: /dispatch stock/i }).click();
    await expect(page.getByText(/dispatch successful/i)).toBeVisible({ timeout: 10000 });
  });

  await test.step('The dispatch shows the category next to the stock item', async () => {
    await page.getByRole('link', { name: /dispatches/i }).click();
    await expect(page).toHaveURL(/\/dispatches$/);
    const row = page.locator('tbody tr').filter({ hasText: cement });
    await expect(row).toBeVisible({ timeout: 8000 });
    await expect(row.getByText('(Building Materials)')).toBeVisible();
    await page.waitForTimeout(1800);
  });
});
