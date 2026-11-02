import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import DriverAppCard from './DriverAppCard';
import * as driverAppApi from '../api/driverApp';
import type { DriverAppRelease } from '../types';

vi.mock('../api/driverApp', async importOriginal => {
  const actual = await importOriginal<typeof import('../api/driverApp')>();
  return { ...actual, getDriverAppRelease: vi.fn(), serverAddress: () => 'https://logi-api.example.org' };
});

const release: DriverAppRelease = {
  version_code: 2, version_name: '0.2.0', sha256: '0f'.repeat(32), size_bytes: 12_300_000,
  released_at: 1_793_000_000, file_name: 'logistics-driver-0.2.0.apk', download_path: '/api/driver-app/download',
};

describe('DriverAppCard', () => {
  // Each test sets the mock's answer. (Resetting it in a beforeEach made the
  // rejection in the last test surface as an unhandled error under Vitest 4.)

  it('shows the version, download link, QR code and server address', async () => {
    vi.mocked(driverAppApi.getDriverAppRelease).mockResolvedValue(release);
    render(<DriverAppCard />);

    expect(await screen.findByTestId('driver-app-version')).toHaveTextContent('v0.2.0');
    const link = screen.getByTestId('driver-app-link');
    expect(link).toHaveAttribute('href', 'https://logi-api.example.org/api/driver-app/download');
    expect(screen.getByTestId('driver-app-server')).toHaveTextContent('https://logi-api.example.org');
    expect(screen.getByText(/12\.3 MB/)).toBeInTheDocument();
    const qr = await screen.findByTestId('driver-app-qr');
    expect(qr.getAttribute('src')).toMatch(/^data:image\/png;base64,/);
  });

  it('copies the download link', async () => {
    const user = userEvent.setup();
    vi.mocked(driverAppApi.getDriverAppRelease).mockResolvedValue(release);
    render(<DriverAppCard />);

    await user.click(await screen.findByRole('button', { name: /copy link/i }));

    expect(await navigator.clipboard.readText()).toBe('https://logi-api.example.org/api/driver-app/download');
    expect(screen.getByRole('button', { name: /copied/i })).toBeInTheDocument();
  });

  it('says when nothing has been published', async () => {
    vi.mocked(driverAppApi.getDriverAppRelease).mockResolvedValue(null);
    render(<DriverAppCard />);
    expect(await screen.findByTestId('driver-app-none')).toHaveTextContent(/no driver app has been published/i);
  });

  it('says when the check failed', async () => {
    vi.mocked(driverAppApi.getDriverAppRelease).mockRejectedValue(new Error('down'));
    render(<DriverAppCard />);
    expect(await screen.findByText(/couldn't check for a published driver app/i)).toBeInTheDocument();
  });
});
