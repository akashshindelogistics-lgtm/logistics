import { describe, it, expect, vi, beforeEach } from 'vitest';
import { AxiosError, type AxiosResponse } from 'axios';
import api from './client';
import { downloadUrl, getDriverAppRelease, serverAddress } from './driverApp';
import type { DriverAppRelease } from '../types';

vi.mock('./client', () => ({ default: { get: vi.fn() } }));

const release: DriverAppRelease = {
  version_code: 2, version_name: '0.2.0', sha256: '0f'.repeat(32), size_bytes: 12_300_000,
  released_at: 1_793_000_000, file_name: 'logistics-driver-0.2.0.apk', download_path: '/api/driver-app/download',
};

const httpError = (status: number) =>
  new AxiosError('fail', String(status), undefined, undefined, { status } as AxiosResponse);

describe('driver app api', () => {
  beforeEach(() => vi.resetAllMocks());

  it('getDriverAppRelease GETs /driver-app/latest and unwraps the release', async () => {
    vi.mocked(api.get).mockResolvedValue({ data: { success: true, message: '', data: release } });
    expect(await getDriverAppRelease()).toEqual(release);
    expect(api.get).toHaveBeenCalledWith('/driver-app/latest');
  });

  it('getDriverAppRelease is null when nothing is published, and rethrows other errors', async () => {
    vi.mocked(api.get).mockRejectedValueOnce(httpError(404));
    expect(await getDriverAppRelease()).toBeNull();

    vi.mocked(api.get).mockRejectedValueOnce(httpError(500));
    await expect(getDriverAppRelease()).rejects.toBeInstanceOf(AxiosError);
  });

  it('serverAddress is the API origin when the API is elsewhere, else the page origin', () => {
    expect(serverAddress('https://logi-api.example.org/api', 'https://dash.example.org')).toBe('https://logi-api.example.org');
    expect(serverAddress('/api', 'http://localhost:5173')).toBe('http://localhost:5173');
    expect(serverAddress(undefined, 'http://localhost:5173')).toBe('http://localhost:5173');
  });

  it('downloadUrl joins the server address and the fixed download path', () => {
    expect(downloadUrl(release, 'https://logi-api.example.org')).toBe('https://logi-api.example.org/api/driver-app/download');
  });
});
