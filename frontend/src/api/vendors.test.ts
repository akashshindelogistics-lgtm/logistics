import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import { listVendors, createVendor, updateVendor, deleteVendor, listVehicleHires, assignVehicleHire } from './vendors';

vi.mock('./client', () => ({
  default: { get: vi.fn(), post: vi.fn(), put: vi.fn(), delete: vi.fn() },
}));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

const input = {
  name: 'Sharma Roadlines', contact_person: 'Anil', phone: '+91 98200 00000',
  gstin: null, notes: null,
};

describe('vendors api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('listVendors GETs /orgs/{id}/vendors and unwraps', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([{ id: 'v1' }]));
    const res = await listVendors('o1');
    expect(api.get).toHaveBeenCalledWith('/orgs/o1/vendors');
    expect(res.data).toEqual([{ id: 'v1' }]);
  });

  it('createVendor POSTs the fields to /orgs/{id}/vendors', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({ id: 'v2' }));
    await createVendor('o1', input);
    expect(api.post).toHaveBeenCalledWith('/orgs/o1/vendors', input);
  });

  it('updateVendor PUTs the fields plus is_active to /vendors/{id}', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({ id: 'v2' }));
    await updateVendor('v2', { ...input, is_active: false });
    expect(api.put).toHaveBeenCalledWith('/vendors/v2', { ...input, is_active: false });
  });

  it('deleteVendor DELETEs /vendors/{id}', async () => {
    vi.mocked(api.delete).mockResolvedValue(envelope(null));
    await deleteVendor('v2');
    expect(api.delete).toHaveBeenCalledWith('/vendors/v2');
  });

  it('listVehicleHires GETs /orgs/{id}/vehicle-hires, with an optional status filter', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([]));
    await listVehicleHires('o1');
    expect(api.get).toHaveBeenCalledWith('/orgs/o1/vehicle-hires', undefined);
    await listVehicleHires('o1', 'REQUESTED');
    expect(api.get).toHaveBeenCalledWith('/orgs/o1/vehicle-hires', { params: { status: 'REQUESTED' } });
  });

  it('assignVehicleHire PUTs the truck details to /vehicle-hires/{id}/assign', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({ id: 'h1' }));
    const input = {
      registration_number: 'MH12 HR 1', capacity: 20, driver_name: 'Ravi', driver_phone: '1',
      freight_amount: 9000, advance_paid: 5000,
    };
    await assignVehicleHire('h1', input);
    expect(api.put).toHaveBeenCalledWith('/vehicle-hires/h1/assign', input);
  });
});
