import { DriverLocationApiError, reportLocation } from '../api';
import type { QueuedFix } from '../types';

function mockFetchOnce(status: number, body: unknown) {
  (globalThis.fetch as jest.Mock).mockResolvedValueOnce({
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  });
}

const FIX: QueuedFix = { id: 'a', latitude: 18.5, longitude: 73.8, recordedAt: 1_700_000_000 };

beforeEach(() => {
  globalThis.fetch = jest.fn();
});

describe('reportLocation', () => {
  it('POSTs to /api/driver/location with the Bearer token and snake_case fixes', async () => {
    mockFetchOnce(200, {
      success: true,
      message: 'Location recorded',
      data: {
        accepted: 1,
        location_updated: true,
        vehicle_registration_number: 'MH14 GP 0001',
        location: { latitude: 18.5, longitude: 73.8, timestamp: 1_700_000_000, address: null },
      },
    });

    const result = await reportLocation('https://api.example.com', 'token-1', [
      { ...FIX, accuracyM: 8, speedMps: 5 },
    ]);

    expect(globalThis.fetch).toHaveBeenCalledWith(
      'https://api.example.com/api/driver/location',
      expect.objectContaining({
        method: 'POST',
        headers: expect.objectContaining({
          Authorization: 'Bearer token-1',
          'Content-Type': 'application/json',
        }),
      }),
    );
    const [, init] = (globalThis.fetch as jest.Mock).mock.calls[0];
    const body = JSON.parse(init.body);
    expect(body).toEqual({
      fixes: [
        {
          latitude: 18.5,
          longitude: 73.8,
          recorded_at: 1_700_000_000,
          accuracy_m: 8,
          speed_mps: 5,
        },
      ],
    });
    expect(result.vehicle_registration_number).toBe('MH14 GP 0001');
  });

  it('omits accuracy_m/speed_mps when not provided, rather than sending null', async () => {
    mockFetchOnce(200, {
      success: true,
      message: 'ok',
      data: { accepted: 1, location_updated: true, vehicle_registration_number: 'X', location: null },
    });

    await reportLocation('https://api.example.com', 'token-1', [FIX]);

    const [, init] = (globalThis.fetch as jest.Mock).mock.calls[0];
    const body = JSON.parse(init.body);
    expect(body.fixes[0]).not.toHaveProperty('accuracy_m');
    expect(body.fixes[0]).not.toHaveProperty('speed_mps');
  });

  it.each([401, 403, 409, 400, 500])('throws DriverLocationApiError with status %d on a non-2xx response', async (status) => {
    mockFetchOnce(status, { success: false, message: 'nope', data: null });
    await expect(reportLocation('https://api.example.com', 't', [FIX])).rejects.toMatchObject({
      status,
    });
  });

  it('throws a status-0 DriverLocationApiError on a network failure', async () => {
    (globalThis.fetch as jest.Mock).mockRejectedValue(new Error('Network request failed'));
    await expect(reportLocation('https://api.example.com', 't', [FIX])).rejects.toBeInstanceOf(
      DriverLocationApiError,
    );
    await expect(reportLocation('https://api.example.com', 't', [FIX])).rejects.toMatchObject({ status: 0 });
  });

  it('throws when the response is ok but carries no data', async () => {
    mockFetchOnce(200, { success: true, message: 'huh', data: null });
    await expect(reportLocation('https://api.example.com', 't', [FIX])).rejects.toBeInstanceOf(
      DriverLocationApiError,
    );
  });
});
