import { test, expect, type Page } from '@playwright/test';
import { registerOrg, uid } from './helpers';

/**
 * The "Driver app (Android)" card on a driver's page: where the office finds
 * the download link, QR code and server address to set up a driver's phone.
 *
 * The release itself is stubbed in the browser: what the backend publishes
 * depends on files on the server (covered by the Rust route tests in
 * src/logistics/server/routes/tests/driver_app.rs), and the end-to-end path
 * with a real APK is android/scripts/demo-update.sh.
 */
const release = {
  version_code: 2,
  version_name: '0.2.0',
  sha256: '0f'.repeat(32),
  size_bytes: 12_300_000,
  released_at: 1_793_000_000,
  file_name: 'logistics-driver-0.2.0.apk',
  download_path: '/api/driver-app/download',
};

async function openDriverPage(page: Page) {
  const org = await registerOrg(page, `Driver App ${uid()}`);
  const token = await page.evaluate(() => localStorage.getItem('logi_token'));
  const res = await page.request.post(`/api/orgs/${org.id}/drivers`, {
    data: { name: 'Ramesh Patil', license_number: `L${uid()}`, phone: '+91 98220 00042' },
    headers: { Authorization: `Bearer ${token}` },
  });
  expect(res.ok()).toBeTruthy();
  const driverId = (await res.json()).data.id;
  await page.goto(`/drivers/${driverId}`);
  await expect(page.getByRole('heading', { level: 1, name: 'Ramesh Patil' })).toBeVisible();
}

test.describe('Driver app card', () => {
  test('shows the published release with its download link, QR code and server address', async ({ page }) => {
    await page.route('**/api/driver-app/latest', route =>
      route.fulfill({ json: { success: true, message: 'Driver app 0.2.0', data: release } }));
    await page.route('**/api/driver-app/download', route =>
      route.fulfill({
        body: 'apk bytes',
        headers: {
          'Content-Type': 'application/vnd.android.package-archive',
          'Content-Disposition': 'attachment; filename="logistics-driver-0.2.0.apk"',
        },
      }));
    await openDriverPage(page);

    const card = page.getByTestId('driver-app-card');
    await expect(card.getByTestId('driver-app-version')).toHaveText('v0.2.0');
    const origin = new URL(page.url()).origin;
    await expect(card.getByTestId('driver-app-link')).toHaveAttribute('href', `${origin}/api/driver-app/download`);
    await expect(card.getByTestId('driver-app-server')).toHaveText(origin);
    await expect(card.getByText(/12\.3 MB/)).toBeVisible();
    await expect(card.getByTestId('driver-app-qr')).toHaveAttribute('src', /^data:image\/png;base64,/);

    // The link downloads the APK under its release name.
    const download = page.waitForEvent('download');
    await card.getByTestId('driver-app-link').click();
    expect((await download).suggestedFilename()).toBe('logistics-driver-0.2.0.apk');
  });

  test('says so when no release has been published', async ({ page }) => {
    await page.route('**/api/driver-app/latest', route =>
      route.fulfill({ status: 404, json: { success: false, message: 'No driver app release has been published on this server', data: null } }));
    await openDriverPage(page);

    await expect(page.getByTestId('driver-app-none')).toContainText('No driver app has been published');
  });
});
