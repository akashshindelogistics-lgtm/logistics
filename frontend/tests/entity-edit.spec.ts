import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/** Authenticated JSON POST/PUT helper using the token in localStorage. */
async function api(page: Page, method: 'post' | 'put', path: string, data: unknown) {
  const token = await page.evaluate(() => localStorage.getItem('logi_token'));
  const resp = await page.request[method](path, { data, headers: { Authorization: `Bearer ${token}` } });
  if (!resp.ok()) throw new Error(`${method} ${path} -> ${resp.status()} ${await resp.text()}`);
  return resp.json();
}

test.describe('Editing entity details', () => {
  test('edit a vehicle from its detail page', async ({ page }) => {
    const org = await registerOrg(page, `Edit Vehicle ${uid()}`);
    const reg = `MH20EE${uid().toUpperCase().slice(0, 4)}`;
    await api(page, 'post', `/api/orgs/${org.id}/vehicles`, { registration_number: reg, capacity: 10, unit: 'MetricTon' });

    await page.goto('/vehicles');
    await page.getByRole('link', { name: reg }).click();
    await expect(page).toHaveURL(new RegExp(`/vehicles/${encodeURIComponent(reg)}$`));

    const capacity = page.getByLabel('Capacity');
    await expect(capacity).toHaveValue('10');
    await capacity.fill('44');
    await page.getByLabel('Unit').selectOption('Box');
    await page.getByRole('button', { name: /^save$/i }).click();
    await expect(page.getByText(/vehicle updated/i)).toBeVisible({ timeout: 8000 });

    await page.goto('/vehicles');
    await expect(page.getByText('44 Box')).toBeVisible({ timeout: 8000 });
  });

  test('edit a driver from its detail page', async ({ page }) => {
    const org = await registerOrg(page, `Edit Driver ${uid()}`);
    const name = `Driver ${uid()}`;
    const created = await api(page, 'post', `/api/orgs/${org.id}/drivers`, {
      name, license_number: 'DL-OLD', phone: '+91 90000 00000',
    }) as { data: { id: string } };

    await page.goto(`/drivers/${created.data.id}`);
    await expect(page.getByLabel('Name')).toHaveValue(name);

    await page.getByLabel('Licence Number').fill('DL-NEW-123');
    await page.getByRole('checkbox').uncheck();
    await page.getByRole('button', { name: /^save$/i }).click();
    await expect(page.getByText(/driver updated/i)).toBeVisible({ timeout: 8000 });

    await page.goto(`/orgs/${org.id}`);
    await expect(page.getByText('DL-NEW-123')).toBeVisible({ timeout: 8000 });
  });

  test('edit a godown from its detail page', async ({ page }) => {
    const org = await registerOrg(page, `Edit Godown ${uid()}`);
    const gName = `Godown ${uid()}`;
    const created = await api(page, 'post', `/api/orgs/${org.id}/godowns`, {
      name: gName, address: '1 Old Road',
    }) as { data: { id: string } };

    await page.goto(`/godowns/${created.data.id}`);
    await expect(page.getByLabel('Name')).toHaveValue(gName);

    await page.getByLabel('Address').fill('99 New Avenue');
    await page.getByLabel(/max capacity/i).fill('7500');
    await page.getByRole('button', { name: /^save$/i }).click();
    await expect(page.getByText(/godown updated/i)).toBeVisible({ timeout: 8000 });

    await page.goto(`/orgs/${org.id}`);
    await expect(page.getByText('99 New Avenue')).toBeVisible({ timeout: 8000 });
  });

  test('a vehicle detail page 404s for another org', async ({ page }) => {
    const orgA = await registerOrg(page, `VD Owner ${uid()}`);
    const reg = `MH21FF${uid().toUpperCase().slice(0, 4)}`;
    await api(page, 'post', `/api/orgs/${orgA.id}/vehicles`, { registration_number: reg, capacity: 5, unit: 'MetricTon' });

    await registerOrg(page, `VD Other ${uid()}`);
    await page.goto(`/vehicles/${encodeURIComponent(reg)}`);
    await expect(page.getByText(/vehicle not found/i)).toBeVisible({ timeout: 8000 });
  });
});
