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
