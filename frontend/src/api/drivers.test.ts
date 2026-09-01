import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import { listDrivers, addDriver, updateDriver, deleteDriver, assignVehicleDriver } from './drivers';

vi.mock('./client', () => ({
  default: { get: vi.fn(), post: vi.fn(), put: vi.fn(), delete: vi.fn() },
}));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('drivers api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('listDrivers GETs /drivers and unwraps', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([{ id: 'd1' }]));
    const res = await listDrivers();
    expect(api.get).toHaveBeenCalledWith('/drivers');
    expect(res.data).toEqual([{ id: 'd1' }]);
  });

  it('addDriver POSTs a snake_cased payload to /orgs/{id}/drivers', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({ id: 'd2' }));
    await addDriver('o1', 'Ravi Kumar', 'DL-123', '+91 90000 00000');
    expect(api.post).toHaveBeenCalledWith('/orgs/o1/drivers', {
      name: 'Ravi Kumar',
      license_number: 'DL-123',
      phone: '+91 90000 00000',
    });
  });

  it('updateDriver PUTs name/license/phone/is_active to /drivers/{id}', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({ id: 'd1' }));
    await updateDriver('d1', 'Ravi K', 'DL-123', '000', false);
    expect(api.put).toHaveBeenCalledWith('/drivers/d1', {
      name: 'Ravi K',
      license_number: 'DL-123',
      phone: '000',
      is_active: false,
    });
  });

  it('deleteDriver DELETEs /drivers/{id}', async () => {
    vi.mocked(api.delete).mockResolvedValue(envelope(null));
    await deleteDriver('d1');
    expect(api.delete).toHaveBeenCalledWith('/drivers/d1');
  });

  it('assignVehicleDriver PUTs { driver_id } to the URL-encoded vehicle path', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({}));
    await assignVehicleDriver('MH 12 AB 1234', 'd1');
    expect(api.put).toHaveBeenCalledWith('/vehicles/MH%2012%20AB%201234/driver', {
      driver_id: 'd1',
    });
  });

  it('assignVehicleDriver sends driver_id null to clear the assignment', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({}));
    await assignVehicleDriver('REG1', null);
    expect(api.put).toHaveBeenCalledWith('/vehicles/REG1/driver', { driver_id: null });
  });
});
