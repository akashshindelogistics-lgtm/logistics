import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import { listVendors, createVendor, updateVendor, deleteVendor } from './vendors';

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
});
