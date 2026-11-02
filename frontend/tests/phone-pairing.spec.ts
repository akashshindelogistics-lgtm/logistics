import { test, expect } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * The "Driver's phone" panel on a driver's page, against the real backend:
 * pairing issues a token that works the way the phone uses it, the panel
 * shows when the phone last reported, and unpairing makes the token stop
 * working.
 */
test.describe("Driver's phone pairing", () => {
  test('pair, see the phone report, and unpair', async ({ page }) => {
    const org = await registerOrg(page, `Phone Pairing ${uid()}`);
    const auth = { Authorization: `Bearer ${await page.evaluate(() => localStorage.getItem('logi_token'))}` };
    const res = await page.request.post(`/api/orgs/${org.id}/drivers`, {
      data: { name: 'Ramesh Patil', license_number: `L${uid()}`, phone: '+91 98220 00042' },
      headers: auth,
    });
    const driverId = (await res.json()).data.id;
    const asPhone = (token: string) => page.request.get('/api/driver/me', { headers: { Authorization: `Bearer ${token}` } });

    await page.goto(`/drivers/${driverId}`);
    const card = page.getByTestId('phone-pairing-card');
    await expect(card.getByTestId('pairing-status')).toHaveText('Not paired');

    await card.getByRole('button', { name: 'Pair a phone' }).click();
    const token = (await card.getByTestId('pairing-token').innerText()).trim();
    expect(token).toMatch(/^[0-9a-f-]{36}$/);
    await expect(card.getByTestId('pairing-qr')).toBeVisible();
    await expect(card.getByTestId('pairing-server')).toHaveText(new URL(page.url()).origin);
    await expect(card.getByTestId('pairing-status')).toHaveText('Paired');

    // The token works as the phone uses it.
    const me = await asPhone(token);
    expect(me.status()).toBe(200);
    expect((await me.json()).data.name).toBe('Ramesh Patil');

    await card.getByRole('button', { name: 'Done' }).click();
    await expect(card.getByTestId('pairing-token')).toHaveCount(0);
    await page.reload();
    await expect(card.getByTestId('pairing-detail')).toContainText('last reported just now');

    await card.getByRole('button', { name: 'Unpair' }).click();
    await card.getByTestId('pairing-confirm').getByRole('button', { name: 'Unpair' }).click();
    await expect(card.getByTestId('pairing-status')).toHaveText('Not paired');
    expect((await asPhone(token)).status()).toBe(401);
  });
});
