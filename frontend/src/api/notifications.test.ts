import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import { listDispatchNotifications, listOrgNotifications } from './notifications';

vi.mock('./client', () => ({ default: { get: vi.fn() } }));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('notifications api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('listDispatchNotifications GETs /dispatches/{id}/notifications and unwraps', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([{ id: 'n1' }]));
    const res = await listDispatchNotifications('d1');
    expect(api.get).toHaveBeenCalledWith('/dispatches/d1/notifications');
    expect(res.data).toEqual([{ id: 'n1' }]);
  });

  it('listOrgNotifications GETs /orgs/{id}/notifications and unwraps', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([]));
    await listOrgNotifications('o1');
    expect(api.get).toHaveBeenCalledWith('/orgs/o1/notifications');
  });
});
