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
    await expect(statLabels.filter({ hasText: 'Godowns' })).toBeVisible();
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

  test('org detail shows Godowns section with add-godown form', async ({ page }) => {
    const org = await registerOrg(page, `Godown Section Org ${uid()}`);
    await page.goto(`/orgs/${org.id}`);

    await expect(page.getByText('Godowns').first()).toBeVisible();
    await expect(page.getByLabel('Godown Name')).toBeVisible();
    await expect(page.getByLabel('Address')).toBeVisible();
    await expect(page.getByRole('button', { name: /add godown/i })).toBeVisible();
  });

  test('create a godown and add stock to it via the UI', async ({ page }) => {
    const org = await registerOrg(page, `Godown Flow Org ${uid()}`);
    const godownName = `Warehouse ${uid()}`;
    const stockDesc = `Bags of Rice ${uid()}`;
    await page.goto(`/orgs/${org.id}`);

    await page.getByLabel('Godown Name').fill(godownName);
    await page.getByLabel('Address').fill('Plot 5, Industrial Area');
    await page.getByRole('button', { name: /add godown/i }).click();
    await expect(page.getByText(godownName)).toBeVisible({ timeout: 8000 });

    await page.getByLabel('Stock Item').fill(stockDesc);
    await page.getByLabel('Stock Quantity').fill('250');
    await page.getByLabel('Volume').fill('50');
    await page.getByRole('button', { name: /add stock/i }).click();

    await expect(page.getByText(stockDesc)).toBeVisible({ timeout: 8000 });
    await expect(page.getByText('250')).toBeVisible();
  });

  test('transfer stock from one godown to another and see it in the history', async ({ page }) => {
    const org = await registerOrg(page, `Transfer Flow Org ${uid()}`);
    const source = `Source WH ${uid()}`;
    const dest = `Dest WH ${uid()}`;
    const stockDesc = `Cartons ${uid()}`;
    await page.goto(`/orgs/${org.id}`);

    // Two godowns.
    await page.getByLabel('Godown Name').fill(source);
    await page.getByLabel('Address').fill('Plot 1, Industrial Area');
    await page.getByRole('button', { name: /add godown/i }).click();
    await expect(page.getByText(source)).toBeVisible({ timeout: 8000 });

    await page.getByLabel('Godown Name').fill(dest);
    await page.getByLabel('Address').fill('Plot 2, Industrial Area');
    await page.getByRole('button', { name: /add godown/i }).click();
    await expect(page.getByText(dest)).toBeVisible({ timeout: 8000 });

    // Stock into the source godown.
    const sourceCard = page.getByTestId('godown-card').filter({ hasText: source });
    await sourceCard.getByLabel('Stock Item').fill(stockDesc);
    await sourceCard.getByLabel('Stock Quantity').fill('100');
    await sourceCard.getByLabel('Volume').fill('2');
    await sourceCard.getByRole('button', { name: /add stock/i }).click();
    await expect(page.getByText(stockDesc).first()).toBeVisible({ timeout: 8000 });

    // Transfer 30 units to the destination godown.
    await sourceCard.getByLabel('Transfer Item').selectOption({ label: stockDesc });
    await sourceCard.getByLabel('To Godown').selectOption({ label: dest });
    await sourceCard.getByLabel('Transfer Quantity').fill('30');
    await sourceCard.getByRole('button', { name: /^transfer$/i }).click();

    await expect(page.getByText(/stock transferred between godowns/i)).toBeVisible({ timeout: 8000 });

    // The transfer shows up in the Stock Transfers history table.
    await expect(page.getByText('Stock Transfers')).toBeVisible();
    const historyRow = page.locator('tr', { has: page.getByText(stockDesc) }).last();
    await expect(historyRow.getByText(source)).toBeVisible();
    await expect(historyRow.getByText(dest)).toBeVisible();
  });

  test('record a vehicle compliance document and see it flagged as expiring soon', async ({ page }) => {
    const org = await registerOrg(page, `Compliance Flow Org ${uid()}`);
    const reg = `KA05MC${uid().slice(-4)}`;
    await page.goto(`/orgs/${org.id}`);

    // Add a vehicle.
    await page.getByLabel('Registration Number').fill(reg);
    await page.getByLabel('Capacity (MT)').fill('12');
    await page.getByRole('button', { name: /add vehicle/i }).click();
    await expect(page.getByTestId('fleet-table').getByText(reg)).toBeVisible({ timeout: 8000 });

    // Record an insurance policy that lapses in 10 days.
    const soon = new Date(Date.now() + 10 * 86_400_000).toISOString().slice(0, 10);
    await page.getByRole('button', { name: /add compliance document/i }).click();
    await page.getByLabel('Vehicle', { exact: true }).selectOption(reg);
    await page.getByLabel('Document', { exact: true }).selectOption('Insurance');
    await page.getByLabel('Document Number').fill('POL-E2E-1');
    await page.getByLabel('Expiry Date').fill(soon);
    await page.getByRole('button', { name: /save document/i }).click();

    await expect(page.getByText(/compliance document recorded/i)).toBeVisible({ timeout: 8000 });

    const complianceRow = page.getByTestId('compliance-row').filter({ hasText: 'POL-E2E-1' });
    await expect(complianceRow.getByText('Expiring soon')).toBeVisible();
    await expect(page.getByText(/1 expiring soon/i)).toBeVisible();
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
