import { test, expect, type Page } from '@playwright/test';
import { existsSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import jsQR from 'jsqr';
import { loginOrg, registerOrg, uid, type TestOrg } from './helpers';

/**
 * The dispatcher's side of **pairing a driver's phone**: the driver's page
 * issues a device token and shows it as a QR code. The demo reads the QR code
 * off the screen (jsQR), so what the phone gets is exactly what's drawn.
 *
 * Normally run by `android/scripts/demo-phone-pairing.sh`, which hands the
 * decoded link to the app on the emulator. They take turns through files in
 * PAIRING_DEMO_HANDOFF: this writes `link.txt`, waits for the phone's
 * `phone-paired`, shows "last reported", unpairs, and writes `unpaired`.
 * Also takes PAIRING_DEMO_ORG_NAME, PAIRING_DEMO_ORG_PASSWORD and
 * PAIRING_DEMO_DRIVER_ID.
 *
 * On its own (`npm run test:e2e:demo:phone-pairing`) the token from the QR
 * code is used through the API, the way the phone uses it.
 */
const env = process.env;
const handoff = env.PAIRING_DEMO_HANDOFF;

/** Decode the QR code image on the page. */
async function readQr(page: Page): Promise<string> {
  const pixels = await page.getByTestId('pairing-qr').evaluate(async (img: HTMLImageElement) => {
    await img.decode();
    const canvas = document.createElement('canvas');
    canvas.width = img.naturalWidth;
    canvas.height = img.naturalHeight;
    const ctx = canvas.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const data = ctx.getImageData(0, 0, canvas.width, canvas.height);
    return { width: data.width, height: data.height, rgba: Array.from(data.data) };
  });
  const code = jsQR(Uint8ClampedArray.from(pixels.rgba), pixels.width, pixels.height);
  if (!code) throw new Error('No QR code could be read from the pairing panel');
  return code.data;
}

async function waitForFile(path: string, timeoutMs: number) {
  await expect.poll(() => existsSync(path), { timeout: timeoutMs, intervals: [500] }).toBe(true);
}

test('pairing a driver phone from the dashboard', async ({ page }) => {
  test.setTimeout(300_000);
  let driverId: string;

  if (env.PAIRING_DEMO_DRIVER_ID) {
    const org: TestOrg = { name: env.PAIRING_DEMO_ORG_NAME!, password: env.PAIRING_DEMO_ORG_PASSWORD!, address: '', id: '' };
    await test.step(`Sign in as ${org.name}`, () => loginOrg(page, org));
    driverId = env.PAIRING_DEMO_DRIVER_ID;
  } else {
    const org = await registerOrg(page, `Phone Pairing Demo ${uid()}`);
    const token = await page.evaluate(() => localStorage.getItem('logi_token'));
    const res = await page.request.post(`/api/orgs/${org.id}/drivers`, {
      data: { name: 'Ramesh Patil', license_number: `L${uid()}`, phone: '+91 98220 00042' },
      headers: { Authorization: `Bearer ${token}` },
    });
    driverId = (await res.json()).data.id;
  }

  const card = page.getByTestId('phone-pairing-card');
  let link = '';
  await test.step("Pair a phone: the token as a QR code", async () => {
    await page.goto(`/drivers/${driverId}`);
    await card.scrollIntoViewIfNeeded();
    await expect(card.getByTestId('pairing-status')).toHaveText('Not paired');
    await page.waitForTimeout(1500);
    await card.getByRole('button', { name: 'Pair a phone' }).click();
    await expect(card.getByTestId('pairing-qr')).toBeVisible();
    await card.getByTestId('pairing-qr').scrollIntoViewIfNeeded();

    link = await readQr(page);
    const url = new URL(link);
    expect(url.protocol).toBe('logisticsdriver:');
    expect(url.searchParams.get('token')).toBe((await card.getByTestId('pairing-token').innerText()).trim());
    expect(url.searchParams.get('server')).toBe(new URL(page.url()).origin);
    console.log(`QR code reads: ${link}`);
    await page.waitForTimeout(2000);
  });

  await test.step('The phone pairs from the code', async () => {
    if (handoff) {
      writeFileSync(join(handoff, 'link.txt'), link);
      await waitForFile(join(handoff, 'phone-paired'), 240_000);
    } else {
      const token = new URL(link).searchParams.get('token')!;
      const me = await page.request.get('/api/driver/me', { headers: { Authorization: `Bearer ${token}` } });
      expect(me.status()).toBe(200);
    }
    await card.getByRole('button', { name: 'Done' }).click();
    await page.reload();
    await expect(card.getByTestId('pairing-status')).toHaveText('Paired');
    await expect(card.getByTestId('pairing-detail')).toContainText('last reported just now');
    await card.scrollIntoViewIfNeeded();
    await page.waitForTimeout(2500);
  });

  await test.step('Unpair: the phone is disconnected', async () => {
    await card.getByRole('button', { name: 'Unpair' }).click();
    await page.waitForTimeout(1000);
    await card.getByTestId('pairing-confirm').getByRole('button', { name: 'Unpair' }).click();
    await expect(card.getByTestId('pairing-status')).toHaveText('Not paired');
    if (handoff) writeFileSync(join(handoff, 'unpaired'), '');
    await page.waitForTimeout(2500);
  });
});
