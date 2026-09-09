import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import { listOrgUsers, createOrgUser, updateOrgUser, deleteOrgUser } from './users';

vi.mock('./client', () => ({
  default: { get: vi.fn(), post: vi.fn(), put: vi.fn(), delete: vi.fn() },
}));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('users api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('listOrgUsers GETs /orgs/{id}/users and unwraps', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([{ id: 'u1' }]));
    const res = await listOrgUsers('o1');
    expect(api.get).toHaveBeenCalledWith('/orgs/o1/users');
    expect(res.data).toEqual([{ id: 'u1' }]);
  });

  it('createOrgUser POSTs the full payload to /orgs/{id}/users', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({ id: 'u2' }));
    await createOrgUser('o1', { name: 'Sam', email: 's@x.com', password: 'pw123456', role: 'DISPATCHER' });
    expect(api.post).toHaveBeenCalledWith('/orgs/o1/users', {
      name: 'Sam', email: 's@x.com', password: 'pw123456', role: 'DISPATCHER',
    });
  });

  it('updateOrgUser PUTs name/role/is_active to /users/{id}', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({ id: 'u2' }));
    await updateOrgUser('u2', { name: 'Sam', role: 'ADMIN', is_active: false });
    expect(api.put).toHaveBeenCalledWith('/users/u2', { name: 'Sam', role: 'ADMIN', is_active: false });
  });

  it('deleteOrgUser DELETEs /users/{id}', async () => {
    vi.mocked(api.delete).mockResolvedValue(envelope(null));
    await deleteOrgUser('u2');
    expect(api.delete).toHaveBeenCalledWith('/users/u2');
  });
});
