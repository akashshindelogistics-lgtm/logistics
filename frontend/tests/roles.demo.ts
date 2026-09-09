import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A short narrated walk through **role-scoped team members** on its own —
 * an Admin adds a Dispatcher and a Warehouse-staff member on the Team page,
 * then we sign in as one of them and see the Team page is off-limits.
 *
 * Watch it with `npm run test:e2e:demo:roles` (headed, slowed).
 */
const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));

test('role-scoped team members', async ({ page }) => {
  test.slow();
  const org = await registerOrg(page, `Roles Demo ${uid()}`);
  const dispatcherEmail = `dispatcher-${uid()}@example.com`;
  const warehouseEmail = `warehouse-${uid()}@example.com`;

  await test.step('Admin opens the Team page', async () => {
    await page.getByRole('link', { name: /team/i }).click();
    await expect(page).toHaveURL(/\/team$/);
    await expect(page.getByRole('heading', { level: 1, name: 'Team' })).toBeVisible();
    await page.waitForTimeout(600);
  });

  await test.step('Add a Dispatcher', async () => {
    await page.getByRole('button', { name: /add member/i }).click();
    await page.getByLabel('Name').fill('Dana Dispatcher');
    await page.getByLabel('Email').fill(dispatcherEmail);
    await page.getByLabel(/temporary password/i).fill('team-member-pw');
    await page.getByLabel('Role', { exact: true }).selectOption('DISPATCHER');
    await page.getByRole('button', { name: /^add member$/i }).click();
    await expect(page.getByRole('row', { name: /Dana Dispatcher/ })).toBeVisible({ timeout: 8000 });
    await page.waitForTimeout(600);
  });

  await test.step('Add a Warehouse-staff member', async () => {
    await page.getByRole('button', { name: /add member/i }).click();
    await page.getByLabel('Name').fill('Wes Warehouse');
    await page.getByLabel('Email').fill(warehouseEmail);
    await page.getByLabel(/temporary password/i).fill('team-member-pw');
    await page.getByLabel('Role', { exact: true }).selectOption('WAREHOUSE_STAFF');
    await page.getByRole('button', { name: /^add member$/i }).click();
    const row = page.getByRole('row', { name: /Wes Warehouse/ });
    await expect(row).toBeVisible({ timeout: 8000 });
    await expect(row.getByLabel(/role for wes warehouse/i)).toHaveValue('WAREHOUSE_STAFF');
    await page.waitForTimeout(800);
  });

  await test.step('The role gates hold on the API', async () => {
    const adminToken = await token(page);
    const godown = await page.request.post(`/api/orgs/${org.id}/godowns`, {
      data: { name: 'Demo Godown', address: '1 Rd' },
      headers: { Authorization: `Bearer ${adminToken}` },
    }).then(r => r.json());

    const asUser = async (email: string) => (await (await page.request.post('/api/auth/user-login', {
      data: { email, password: 'team-member-pw' },
    })).json()).data.token as string;

    const whToken = await asUser(warehouseEmail);
    // Warehouse staff can add stock…
    expect((await page.request.post(`/api/godowns/${godown.data.id}/stock`, {
      data: { description: 'Bricks', quantity: 5, volume_in_size: 1 },
      headers: { Authorization: `Bearer ${whToken}` },
    })).status()).toBe(201);
    // …but not dispatch.
    expect((await page.request.post(`/api/orgs/${org.id}/dispatch`, {
      data: { customer_id: '00000000-0000-0000-0000-000000000000', line_items: [{ stock_description: 'Bricks', requested_quantity: 1 }] },
      headers: { Authorization: `Bearer ${whToken}` },
    })).status()).toBe(403);
  });

  await test.step('Sign in as the Warehouse-staff member — no Team page for them', async () => {
    await page.getByRole('button', { name: /sign out/i }).click();
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
    await page.getByRole('tab', { name: /team member/i }).click();
    await page.getByLabel('Email').fill(warehouseEmail);
    await page.getByLabel('Password').fill('team-member-pw');
    await page.getByRole('button', { name: /sign in/i }).click();
    await expect(page).toHaveURL(/\/orgs\//, { timeout: 8000 });

    await expect(page.getByRole('link', { name: /team/i })).toHaveCount(0);
    await page.goto('/team');
    await expect(page.getByText(/admins only/i)).toBeVisible();
    await page.waitForTimeout(1000);
  });
});
