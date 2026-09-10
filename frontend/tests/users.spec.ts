import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));

async function addUser(page: Page, orgId: string, email: string, role: string) {
  const res = await page.request.post(`/api/orgs/${orgId}/users`, {
    data: { name: `User ${email}`, email, password: 'team-member-pw', role },
    headers: { Authorization: `Bearer ${await token(page)}` },
  });
  if (!res.ok()) throw new Error(`addUser ${email}: ${res.status()} ${await res.text()}`);
}

test.describe('Team members & roles', () => {
  test('an admin adds a team member on the Team page and they can sign in by email', async ({ page }) => {
    await registerOrg(page, `Team Page ${uid()}`);
    const email = `member-${uid()}@example.com`;

    await page.goto('/team');
    await expect(page.getByRole('heading', { level: 1, name: 'Team' })).toBeVisible();

    await page.getByRole('button', { name: /add member/i }).click();
    await page.getByLabel('Name').fill('Dispatch Dan');
    await page.getByLabel('Email').fill(email);
    await page.getByLabel(/temporary password/i).fill('team-member-pw');
    await page.getByLabel('Role', { exact: true }).selectOption('DISPATCHER');
    await page.getByRole('button', { name: /^add member$/i }).click();

    const row = page.getByRole('row', { name: /Dispatch Dan/ });
    await expect(row).toBeVisible({ timeout: 8000 });
    await expect(row.getByLabel(/role for dispatch dan/i)).toHaveValue('DISPATCHER');

    // Sign out, then sign in as the new member via the "Team member" tab.
    await page.getByRole('button', { name: /sign out/i }).click();
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
    await page.getByRole('tab', { name: /team member/i }).click();
    await page.getByLabel('Email').fill(email);
    await page.getByLabel('Password').fill('team-member-pw');
    await page.getByRole('button', { name: /sign in/i }).click();

    await expect(page).toHaveURL(/\/orgs\//, { timeout: 8000 });
    // A non-admin has no Team link in the sidebar.
    await expect(page.getByRole('link', { name: /team/i })).toHaveCount(0);
  });

  test('a non-admin cannot open the Team page', async ({ page }) => {
    const org = await registerOrg(page, `Team Guard ${uid()}`);
    const email = `wh-${uid()}@example.com`;
    await addUser(page, org.id, email, 'WAREHOUSE_STAFF');

    await page.getByRole('button', { name: /sign out/i }).click();
    await page.getByRole('tab', { name: /team member/i }).click();
    await page.getByLabel('Email').fill(email);
    await page.getByLabel('Password').fill('team-member-pw');
    await page.getByRole('button', { name: /sign in/i }).click();
    await expect(page).toHaveURL(/\/orgs\//, { timeout: 8000 });

    await page.goto('/team');
    await expect(page.getByText(/admins only/i)).toBeVisible();
  });

  test('role gates: warehouse staff can stock but not dispatch; dispatcher the reverse', async ({ page }) => {
    const org = await registerOrg(page, `Role Gates ${uid()}`);
    const adminToken = await token(page);
    const dispEmail = `disp-${uid()}@example.com`;
    const whEmail = `wh-${uid()}@example.com`;
    await addUser(page, org.id, dispEmail, 'DISPATCHER');
    await addUser(page, org.id, whEmail, 'WAREHOUSE_STAFF');

    // A godown to aim stock writes at.
    const godown = await page.request.post(`/api/orgs/${org.id}/godowns`, {
      data: { name: 'Gate Godown', address: '1 Rd' },
      headers: { Authorization: `Bearer ${adminToken}` },
    }).then(r => r.json());

    const loginAs = async (email: string) => {
      const res = await page.request.post('/api/auth/user-login', {
        data: { email, password: 'team-member-pw' },
      });
      return (await res.json()).data.token as string;
    };
    const dispToken = await loginAs(dispEmail);
    const whToken = await loginAs(whEmail);

    const stockBody = { description: 'Bricks', quantity: 5, volume_in_size: 1 };
    const dispatchBody = { customer_id: '00000000-0000-0000-0000-000000000000', line_items: [{ stock_description: 'Bricks', requested_quantity: 1 }] };

    // Warehouse staff: stock OK, dispatch forbidden.
    expect((await page.request.post(`/api/godowns/${godown.data.id}/stock`, {
      data: stockBody, headers: { Authorization: `Bearer ${whToken}` },
    })).status()).toBe(201);
    expect((await page.request.post(`/api/orgs/${org.id}/dispatch`, {
      data: dispatchBody, headers: { Authorization: `Bearer ${whToken}` },
    })).status()).toBe(403);

    // Dispatcher: the reverse.
    expect((await page.request.post(`/api/godowns/${godown.data.id}/stock`, {
      data: stockBody, headers: { Authorization: `Bearer ${dispToken}` },
    })).status()).toBe(403);
    // Not 403 for the dispatcher (it fails later on the bogus customer, but the role gate passed).
    expect((await page.request.post(`/api/orgs/${org.id}/dispatch`, {
      data: dispatchBody, headers: { Authorization: `Bearer ${dispToken}` },
    })).status()).not.toBe(403);
  });
});
