import { test, expect } from '@playwright/test';
import { registerOrg, uid } from './helpers';

test.describe('Organization', () => {
  test('org detail page shows name, address and stat cards', async ({ page }) => {
    const name = `Org Detail ${uid()}`;
    const org = await registerOrg(page, name);

    await page.goto(`/orgs/${org.id}`);

    // Header
    await expect(page.getByRole('heading', { level: 1 })).toContainText(name);
    await expect(page.getByText(org.address)).toBeVisible();

    // Stat cards (scoped to .page-header .muted labels to avoid sidebar nav ambiguity)
    const statLabels = page.locator('.page-header .muted');
    await expect(statLabels.filter({ hasText: 'Vehicles' })).toBeVisible();
    await expect(statLabels.filter({ hasText: 'Stock items' })).toBeVisible();
  });

  test('sidebar shows org name and truncated id', async ({ page }) => {
    const name = `Sidebar Org ${uid()}`;
    const org = await registerOrg(page, name);

    // Sidebar org badge
    await expect(page.locator('.sidebar-org-name')).toContainText(name);
    await expect(page.locator('.sidebar-org-id')).toContainText(org.id.slice(0, 8));
  });

  test('My Organization sidebar link navigates to org detail', async ({ page }) => {
    const org = await registerOrg(page, `Nav Org ${uid()}`);

    // Navigate away first
    await page.goto('/customers');
    await page.getByRole('link', { name: /my organization/i }).click();

    await expect(page).toHaveURL(`/orgs/${org.id}`);
  });

  test('breadcrumb navigates back to organizations list', async ({ page }) => {
    const org = await registerOrg(page, `Breadcrumb Org ${uid()}`);

    await page.goto(`/orgs/${org.id}`);
    await page.getByRole('link', { name: 'Organizations' }).click();

    // Redirects to /orgs which then redirects to /orgs/<id> since user is authenticated
    await expect(page).toHaveURL(`/orgs/${org.id}`);
  });

  test('org detail shows Fleet Vehicles section with add-vehicle form', async ({ page }) => {
    const org = await registerOrg(page, `Fleet Org ${uid()}`);
    await page.goto(`/orgs/${org.id}`);

    await expect(page.getByText('Fleet Vehicles')).toBeVisible();
    await expect(page.getByLabel('Registration Number')).toBeVisible();
    await expect(page.getByLabel('Capacity (MT)')).toBeVisible();
    await expect(page.getByRole('button', { name: /add vehicle/i })).toBeVisible();
  });

  test('org detail shows Stock Inventory section', async ({ page }) => {
    const org = await registerOrg(page, `Stock Section Org ${uid()}`);
    await page.goto(`/orgs/${org.id}`);

    await expect(page.getByText('Stock Inventory')).toBeVisible();
  });

  test('org detail shows Dispatch Stock form', async ({ page }) => {
    const org = await registerOrg(page, `Dispatch Form Org ${uid()}`);
    await page.goto(`/orgs/${org.id}`);

    await expect(page.getByText('Dispatch Stock').first()).toBeVisible();
    await expect(page.getByLabel('Customer')).toBeVisible();
    await expect(page.getByLabel('Stock Description')).toBeVisible();
    await expect(page.getByLabel('Quantity')).toBeVisible();
    await expect(page.getByRole('button', { name: /dispatch stock/i })).toBeVisible();
  });
});
