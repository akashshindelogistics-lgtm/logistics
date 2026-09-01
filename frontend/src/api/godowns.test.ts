import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import {
  listGodowns,
  createGodown,
  updateGodown,
  deleteGodown,
  updateGodownLocation,
  addGodownStock,
  deleteGodownStock,
} from './godowns';

vi.mock('./client', () => ({
  default: { get: vi.fn(), post: vi.fn(), put: vi.fn(), delete: vi.fn() },
}));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('godowns api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('listGodowns GETs /orgs/{id}/godowns and unwraps', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([{ id: 'g1' }]));
    const res = await listGodowns('o1');
    expect(api.get).toHaveBeenCalledWith('/orgs/o1/godowns');
    expect(res.data).toEqual([{ id: 'g1' }]);
  });

  it('createGodown POSTs name/address to /orgs/{id}/godowns', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({ id: 'g2' }));
    await createGodown('o1', 'North Godown', 'Plot 5');
    expect(api.post).toHaveBeenCalledWith('/orgs/o1/godowns', {
      name: 'North Godown',
      address: 'Plot 5',
    });
  });

  it('updateGodown PUTs name/address to /godowns/{gid}', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({ id: 'g1' }));
    await updateGodown('g1', 'Renamed', 'New Addr');
    expect(api.put).toHaveBeenCalledWith('/godowns/g1', { name: 'Renamed', address: 'New Addr' });
  });

  it('deleteGodown DELETEs /godowns/{gid}', async () => {
    vi.mocked(api.delete).mockResolvedValue(envelope(null));
    await deleteGodown('g1');
    expect(api.delete).toHaveBeenCalledWith('/godowns/g1');
  });

  it('updateGodownLocation PUTs latitude/longitude/address to /godowns/{gid}/location', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({}));
    await updateGodownLocation('g1', 19.07, 72.87, 'Mumbai');
    expect(api.put).toHaveBeenCalledWith('/godowns/g1/location', {
      latitude: 19.07,
      longitude: 72.87,
      address: 'Mumbai',
    });
  });

  it('updateGodownLocation sends address undefined when omitted', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({}));
    await updateGodownLocation('g1', 1, 2);
    expect(api.put).toHaveBeenCalledWith('/godowns/g1/location', {
      latitude: 1,
      longitude: 2,
      address: undefined,
    });
  });

  it('addGodownStock POSTs the snake_cased stock payload to /godowns/{gid}/stock', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({}));
    await addGodownStock('g1', 'Cement Bags', 100, 2.5);
    expect(api.post).toHaveBeenCalledWith('/godowns/g1/stock', {
      description: 'Cement Bags',
      quantity: 100,
      volume_in_size: 2.5,
    });
  });

  it('deleteGodownStock URL-encodes the stock description in the path', async () => {
    vi.mocked(api.delete).mockResolvedValue(envelope(null));
    await deleteGodownStock('g1', 'Steel Rods 12mm / bundle');
    expect(api.delete).toHaveBeenCalledWith(
      '/godowns/g1/stock/Steel%20Rods%2012mm%20%2F%20bundle',
    );
  });
});
