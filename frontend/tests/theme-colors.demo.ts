import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * A narrated walk through the **Peacock & Marigold** colour theme on its own:
 * the teal sidebar, the gradient greeting banner, stat cards washed in
 * cyan / marigold / magenta / olive, dispatch status tags in their lifecycle
 * colours, and the same pages again in dark mode, ending on the login screen.
 *
 * Watch it with `npm run test:e2e:demo:theme-colors` (headed, slowed).
 * The `.demo.ts` name keeps it out of the default headless suite;
 * `theme.spec.ts` is the assertion-focused coverage.
 */
const token = (page: Page) => page.evaluate(() => localStorage.getItem('logi_token'));
async function api(page: Page, method: 'post' | 'put', path: string, data?: unknown) {
  const res = await page.request[method](path, { data, headers: { Authorization: `Bearer ${await token(page)}` } });
  if (!res.ok()) throw new Error(`${method} ${path} -> ${res.status()} ${await res.text()}`);
  return (await res.json()).data;
}

test('Peacock & Marigold colour theme', async ({ page }) => {
  test.slow();
  const tag = uid().toUpperCase().slice(-4);
  const stock = `Red Clay Bricks ${uid()}`;
  const org = await registerOrg(page, `Palette Demo ${uid()}`);

  await test.step('Seed a fleet and dispatches in several lifecycle stages', async () => {
    for (const [i, name] of ['Sunil Jadhav', 'Prakash More', 'Anil Pawar', 'Ravi Shinde'].entries()) {
      const reg = `MH12PC${tag}${i}`;
      await api(page, 'post', `/api/orgs/${org.id}/vehicles`, { registration_number: reg, capacity: 100000, unit: 'MetricTon', vehicle_type: 'Truck' });
      const driver = await api(page, 'post', `/api/orgs/${org.id}/drivers`, { name, license_number: `L${uid()}${i}`, phone: '0' });
      await api(page, 'put', `/api/vehicles/${encodeURIComponent(reg)}/driver`, { driver_id: driver.id });
      await api(page, 'put', `/api/vehicles/${encodeURIComponent(reg)}/location`, { latitude: 18.52 + i * 0.01, longitude: 73.85, address: null });
    }
    const godown = await api(page, 'post', `/api/orgs/${org.id}/godowns`, { name: 'Chakan Kiln Yard', address: 'Chakan MIDC' });
    await api(page, 'post', `/api/godowns/${godown.id}/stock`, { description: stock, quantity: 50000, volume_in_size: 1 });
    const customer = await api(page, 'post', `/api/orgs/${org.id}/customers`, { name: 'Hinjewadi Site', address: 'Hinjewadi, Pune', latitude: 18.59, longitude: 73.74 });

    // Each dispatch is advanced to a different stage so every status colour shows.
    const stages: string[][] = [
      [],
      ['CONFIRMED'],
      ['CONFIRMED', 'LOADED', 'IN_TRANSIT'],
      ['CONFIRMED', 'LOADED', 'IN_TRANSIT', 'DELIVERED'],
    ];
    for (const [i, path] of stages.entries()) {
      const order = await api(page, 'post', `/api/orgs/${org.id}/dispatch`, {
        customer_id: customer.id,
        line_items: [{ stock_description: stock, requested_quantity: 1000 * (i + 1) }],
      });
      for (const status of path) {
        await api(page, 'put', `/api/dispatches/${order.id}/status`, status === 'DELIVERED'
          ? { status, proof_of_delivery: { receiver_name: 'Anita Rao', signature_or_photo_url: 'https://example.com/pod/sig.png' } }
          : { status });
      }
    }
  });

  await test.step('Dashboard in light mode: teal banner, tinted stat cards, coloured statuses', async () => {
    await page.goto('/');
    await expect(page.locator('.dash-hero')).toBeVisible();
    await expect(page.getByText('DELIVERED').first()).toHaveClass(/tag-green/);
    await expect(page.getByText('IN TRANSIT').first()).toHaveClass(/tag-purple/);
    await page.waitForTimeout(3500);
    await page.locator('.stat-card').nth(1).hover();
    await page.waitForTimeout(1200);
  });

  await test.step('Dispatches and Reports pick up the same palette', async () => {
    await page.goto('/dispatches');
    await page.waitForTimeout(2500);
    await page.goto('/reports');
    await page.waitForTimeout(2500);
  });

  await test.step('Switch to dark mode and revisit the dashboard', async () => {
    await page.getByRole('button', { name: /switch to dark theme/i }).click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
    await page.waitForTimeout(2000);
    await page.goto('/');
    await expect(page.locator('.dash-hero')).toBeVisible();
    await page.waitForTimeout(3500);
    await page.goto('/dispatches');
    await page.waitForTimeout(2500);
  });

  await test.step('Back to light, then the login screen', async () => {
    await page.getByRole('button', { name: /switch to light theme/i }).click();
    await page.waitForTimeout(1500);
    await page.getByRole('button', { name: /sign out/i }).click();
    await expect(page).toHaveURL(/login/);
    await page.waitForTimeout(3000);
  });
});
