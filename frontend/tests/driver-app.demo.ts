import { test, expect } from '@playwright/test';
import { createHash } from 'node:crypto';
import { loginOrg, registerOrg, uid, type TestOrg } from './helpers';

/**
 * The office's side of **distributing the driver app**: a driver's page shows
 * the release published on the server, with the download link, a QR code to
 * scan with the phone, and the server address for pairing. The link really
 * downloads the APK.
 *
 * Normally run at the end of `android/scripts/demo-update.sh`, which
 * publishes a release and passes DRIVER_APP_DEMO_ORG_NAME,
 * DRIVER_APP_DEMO_ORG_PASSWORD, DRIVER_APP_DEMO_DRIVER_ID,
 * DRIVER_APP_DEMO_VERSION and DRIVER_APP_DEMO_SHA256. On its own
 * (`npm run test:e2e:demo:driver-app`) it shows whatever the server publishes.
 */
const env = process.env;

test('driver app download on the dashboard', async ({ page }) => {
  test.slow();
  let driverId: string;

  if (env.DRIVER_APP_DEMO_DRIVER_ID) {
    const org: TestOrg = { name: env.DRIVER_APP_DEMO_ORG_NAME!, password: env.DRIVER_APP_DEMO_ORG_PASSWORD!, address: '', id: '' };
    await test.step(`Sign in as ${org.name}`, () => loginOrg(page, org));
    driverId = env.DRIVER_APP_DEMO_DRIVER_ID;
  } else {
    const org = await registerOrg(page, `Driver App Demo ${uid()}`);
    const token = await page.evaluate(() => localStorage.getItem('logi_token'));
    const res = await page.request.post(`/api/orgs/${org.id}/drivers`, {
      data: { name: 'Ramesh Patil', license_number: `L${uid()}`, phone: '+91 98220 00042' },
      headers: { Authorization: `Bearer ${token}` },
    });
    driverId = (await res.json()).data.id;
  }

  const card = page.getByTestId('driver-app-card');
  await test.step("The driver's page: download link, QR code and server address", async () => {
    await page.goto(`/drivers/${driverId}`);
    await card.scrollIntoViewIfNeeded();
    if (env.DRIVER_APP_DEMO_VERSION) {
      await expect(card.getByTestId('driver-app-version')).toHaveText(`v${env.DRIVER_APP_DEMO_VERSION}`);
      await expect(card.getByTestId('driver-app-qr')).toBeVisible();
    } else {
      await expect(card.getByTestId('driver-app-version').or(card.getByTestId('driver-app-none'))).toBeVisible();
    }
    await page.waitForTimeout(3000);
  });

  if (env.DRIVER_APP_DEMO_SHA256) {
    await test.step('The link downloads the published APK', async () => {
      const download = page.waitForEvent('download');
      await card.getByTestId('driver-app-link').click();
      const file = await download;
      expect(file.suggestedFilename()).toBe(`logistics-driver-${env.DRIVER_APP_DEMO_VERSION}.apk`);
      const stream = await file.createReadStream();
      const hash = createHash('sha256');
      for await (const chunk of stream) hash.update(chunk as Buffer);
      expect(hash.digest('hex')).toBe(env.DRIVER_APP_DEMO_SHA256);
      await page.waitForTimeout(1500);
    });
  }
});
