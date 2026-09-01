import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import { listVehicles, addVehicle, deleteVehicle } from './vehicles';

vi.mock('./client', () => ({
  default: { get: vi.fn(), post: vi.fn(), delete: vi.fn() },
}));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('vehicles api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('listVehicles GETs /vehicles and unwraps', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([{ registration_number: 'MH01AB1234' }]));
    const res = await listVehicles();
    expect(api.get).toHaveBeenCalledWith('/vehicles');
    expect(res.data).toEqual([{ registration_number: 'MH01AB1234' }]);
  });

  it('addVehicle POSTs to /orgs/{id}/vehicles with a hardcoded MetricTon unit', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({}));
    await addVehicle('o1', 'MH01AB1234', 12);
    expect(api.post).toHaveBeenCalledWith('/orgs/o1/vehicles', {
      registration_number: 'MH01AB1234',
      capacity: 12,
      unit: 'MetricTon',
    });
  });

  it('deleteVehicle URL-encodes the registration number', async () => {
    vi.mocked(api.delete).mockResolvedValue(envelope(null));
    await deleteVehicle('MH 01/AB 1234');
    expect(api.delete).toHaveBeenCalledWith('/vehicles/MH%2001%2FAB%201234');
  });

  it('deleteVehicle leaves a plain registration number untouched', async () => {
    vi.mocked(api.delete).mockResolvedValue(envelope(null));
    await deleteVehicle('MH01AB1234');
    expect(api.delete).toHaveBeenCalledWith('/vehicles/MH01AB1234');
  });
});
