import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import { createTrip, listOrgTrips, getTrip } from './trips';

vi.mock('./client', () => ({ default: { get: vi.fn(), post: vi.fn() } }));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('trips api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('createTrip POSTs the stops to /orgs/{id}/trips and unwraps', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({ id: 't1' }));
    const stops = [
      { customer_id: 'c1', line_items: [{ stock_description: 'Cement', requested_quantity: 10 }] },
      { customer_id: 'c2', line_items: [{ stock_description: 'Cement', requested_quantity: 5 }] },
    ];
    const res = await createTrip('o1', stops);
    expect(api.post).toHaveBeenCalledWith('/orgs/o1/trips', { stops });
    expect(res.data).toEqual({ id: 't1' });
  });

  it('listOrgTrips GETs /orgs/{id}/trips', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([]));
    await listOrgTrips('o1');
    expect(api.get).toHaveBeenCalledWith('/orgs/o1/trips');
  });

  it('getTrip GETs /trips/{id}', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope({ id: 't1' }));
    await getTrip('t1');
    expect(api.get).toHaveBeenCalledWith('/trips/t1');
  });
});
