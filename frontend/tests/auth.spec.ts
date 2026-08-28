import { test, expect } from '@playwright/test';
import { registerOrg, loginOrg, clearAuth, uid } from './helpers';

test.describe('Authentication', () => {
  test('redirects unauthenticated user to /login', async ({ page }) => {
    await clearAuth(page);
    await page.goto('/');
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });

  test('register page shows brand panel and form', async ({ page }) => {
    await page.goto('/register');
    await expect(page.getByRole('heading', { name: /register your organization/i })).toBeVisible();
    await expect(page.getByLabel('Organization Name')).toBeVisible();
    await expect(page.getByLabel('Address')).toBeVisible();
    await expect(page.getByLabel('Password', { exact: true })).toBeVisible();
    await expect(page.getByLabel('Confirm Password')).toBeVisible();
  });

  test('login page shows org dropdown and password field', async ({ page }) => {
    // The login page only renders the org <select> when at least one org exists,
    // so register one first (then log out) to guarantee a populated dropdown.
    const org = await registerOrg(page, `Login Form Org ${uid()}`);
    await clearAuth(page);

    await page.goto('/login');
    await expect(page.getByRole('heading', { name: /sign in to your organization/i })).toBeVisible();
    await expect(page.getByLabel('Organization')).toBeVisible();
    await expect(page.getByLabel('Organization')).toContainText(org.name);
    await expect(page.getByLabel('Password')).toBeVisible();
    await expect(page.getByRole('button', { name: /sign in/i })).toBeVisible();
  });

  test('register creates org and auto-logs in to dashboard', async ({ page }) => {
    const name = `Auth Org ${uid()}`;
    const org = await registerOrg(page, name);

    // Should land on the org detail page
    await expect(page).toHaveURL(`/orgs/${org.id}`);
    await expect(page.getByRole('heading', { level: 1 })).toContainText(name);
  });

  test('register shows error when passwords do not match', async ({ page }) => {
    await page.goto('/register');
    await page.getByLabel('Organization Name').fill('Mismatch Org');
    await page.getByLabel('Address').fill('1 Test Road');
    await page.getByLabel('Password', { exact: true }).fill('abc12345');
    await page.getByLabel('Confirm Password').fill('different');
    await page.getByRole('button', { name: /create organization/i }).click();

    // Should stay on /register and show an error
    await expect(page).toHaveURL(/\/register/);
    await expect(page.getByText(/passwords do not match/i)).toBeVisible();
  });

  test('login with valid credentials navigates to dashboard', async ({ page }) => {
    const org = await registerOrg(page, `Login Test ${uid()}`);
    await clearAuth(page);

    await loginOrg(page, org);
    await expect(page).toHaveURL(`/orgs/${org.id}`);
  });

  test('login with wrong password shows error', async ({ page }) => {
    const org = await registerOrg(page, `WrongPwd Org ${uid()}`);
    await clearAuth(page);

    await page.goto('/login');
    const select = page.getByLabel('Organization');
    await expect(select).toBeAttached({ timeout: 8000 });
    await select.selectOption({ label: org.name });
    await page.getByLabel('Password').fill('wrongpassword');
    await page.getByRole('button', { name: /sign in/i }).click();

    await expect(page.getByText(/invalid credentials/i)).toBeVisible({ timeout: 8000 });
    await expect(page).toHaveURL(/\/login/);
  });

  test('sign out clears session and redirects to login', async ({ page }) => {
    const org = await registerOrg(page, `Logout Org ${uid()}`);
    await expect(page).toHaveURL(`/orgs/${org.id}`);

    // Click sign out in sidebar
    await page.getByRole('button', { name: /sign out/i }).click();
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });

    // Navigating to / again should redirect back to /login
    await page.goto('/');
    await expect(page).toHaveURL(/\/login/);
  });
});
