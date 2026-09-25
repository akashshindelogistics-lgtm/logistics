import { Page, expect } from '@playwright/test';

/** Unique suffix so parallel test runs or re-runs don't collide on names */
export const uid = () => Date.now().toString(36);

export interface TestOrg {
  name: string;
  address: string;
  password: string;
  id: string;
}

/**
 * Register a brand-new org via the Register page and return its credentials.
 * Leaves the browser on the dashboard after a successful registration.
 */
export async function registerOrg(page: Page, orgName: string, password = 'Test@12345'): Promise<TestOrg> {
  const address = '123 Test Street, Mumbai';

  await page.goto('/register');
  await page.getByLabel('Organization Name').fill(orgName);
  await page.getByLabel('Address').fill(address);
  await page.getByLabel('Password', { exact: true }).fill(password);
  await page.getByLabel('Confirm Password').fill(password);
  await page.getByRole('button', { name: /create organization/i }).click();

  // After register the app auto-logs in and redirects to /orgs/<id>
  await expect(page).toHaveURL(/\/orgs\/[0-9a-f-]{36}/, { timeout: 10000 });

  const orgId = page.url().split('/orgs/')[1];
  return { name: orgName, address, password, id: orgId };
}

/**
 * Login via the Login page. Leaves the browser on the dashboard (/orgs/<id>).
 */
export async function loginOrg(page: Page, org: TestOrg) {
  await page.goto('/login');

  // Wait for the org dropdown to populate
  const select = page.getByLabel('Organization');
  await expect(select).toBeAttached({ timeout: 8000 });

  // Select the org by its name
  await select.selectOption({ label: org.name });
  await page.getByLabel('Password').fill(org.password);
  await page.getByRole('button', { name: /sign in/i }).click();

  await expect(page).toHaveURL(/\/orgs\//, { timeout: 10000 });
}

/**
 * Clear localStorage auth so each test starts logged out.
 */
export async function clearAuth(page: Page) {
  await page.goto('/');
  await page.evaluate(() => {
    localStorage.removeItem('logi_token');
    localStorage.removeItem('logi_org_id');
    localStorage.removeItem('logi_org_name');
  });
}

/**
 * Over the API: a fleetless org with stock, a customer and a vendor, one
 * dispatch on a hired truck assigned at 9,000 with a 6,000 advance, and a
 * 12,500 invoice on it. Returns the vendor's name.
 */
export async function seedPaidHire(page: Page, org: TestOrg): Promise<string> {
  const token = await page.evaluate(() => localStorage.getItem('logi_token'));
  const headers = { Authorization: `Bearer ${token}` };
  const post = async (url: string, data: object) => (await page.request.post(url, { data, headers })).json();
  const put = async (url: string, data: object) => (await page.request.put(url, { data, headers })).json();

  const vendorName = `Sharma Roadlines ${uid()}`;
  const godown = await post(`/api/orgs/${org.id}/godowns`, { name: 'Central Warehouse', address: 'MIDC' });
  await post(`/api/godowns/${godown.data.id}/stock`, { description: 'Cement Bags', quantity: 100, volume_in_size: 1 });
  const customer = await post(`/api/orgs/${org.id}/customers`, { name: `Buyer ${uid()}`, address: 'Baner' });
  const vendor = await post(`/api/orgs/${org.id}/vendors`, { name: vendorName, phone: '+91 98200 11111' });
  const dispatch = await post(`/api/orgs/${org.id}/dispatch`, {
    customer_id: customer.data.id,
    line_items: [{ stock_description: 'Cement Bags', requested_quantity: 10 }],
    vehicle_source: 'HIRED',
    vendor_id: vendor.data.id,
  });
  const assigned = await put(`/api/vehicle-hires/${dispatch.data.hire_id}/assign`, {
    registration_number: 'MH12 HR 7788', capacity: 20, driver_name: 'Suresh Patil',
    driver_phone: '+91 97000 12345', freight_amount: 9000, advance_paid: 6000,
  });
  expect(assigned.success).toBeTruthy();
  const due = new Date(Date.now() + 30 * 86_400_000).toISOString().slice(0, 10);
  const invoice = await post(`/api/dispatches/${dispatch.data.id}/invoice`, { amount: 12500, due_on: due });
  expect(invoice.success).toBeTruthy();
  return vendorName;
}
