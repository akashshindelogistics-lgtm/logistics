import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import {
  listOrgs,
  getOrg,
  createOrg,
  updateOrg,
  deleteOrg,
  dispatchStock,
} from './orgs';

vi.mock('./client', () => ({
  default: { get: vi.fn(), post: vi.fn(), put: vi.fn(), delete: vi.fn() },
}));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('orgs api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('listOrgs GETs /orgs and unwraps the response envelope', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([{ id: 'o1' }]));
    const res = await listOrgs();
    expect(api.get).toHaveBeenCalledWith('/orgs');
    expect(res).toEqual({ success: true, message: '', data: [{ id: 'o1' }] });
  });

  it('getOrg GETs /orgs/{id} and unwraps', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope({ id: 'o1' }));
    const res = await getOrg('o1');
    expect(api.get).toHaveBeenCalledWith('/orgs/o1');
    expect(res.data).toEqual({ id: 'o1' });
  });

  it('createOrg POSTs name/address/password and returns the raw axios response', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({ id: 'o2' }));
    const res = await createOrg('Acme', '1 Dock Rd', 'pw123456');
    expect(api.post).toHaveBeenCalledWith('/orgs', {
      name: 'Acme',
      address: '1 Dock Rd',
      password: 'pw123456',
    });
    // createOrg does not unwrap — callers read res.data.data
    expect(res.data.data).toEqual({ id: 'o2' });
  });

  it('updateOrg PUTs name/address only (no password) and unwraps', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({ id: 'o1', name: 'New' }));
    await updateOrg('o1', 'New', '2 Dock Rd');
    expect(api.put).toHaveBeenCalledWith('/orgs/o1', { name: 'New', address: '2 Dock Rd' });
  });

  it('deleteOrg DELETEs /orgs/{id} and unwraps', async () => {
    vi.mocked(api.delete).mockResolvedValue(envelope(null));
    await deleteOrg('o1');
    expect(api.delete).toHaveBeenCalledWith('/orgs/o1');
  });

  it('dispatchStock POSTs the snake_cased line-item payload to /orgs/{id}/dispatch', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({}));
    await dispatchStock('o1', 'cust-9', [
      { stockDescription: 'Cement', requestedQuantity: 40 },
      { stockDescription: 'Sand', requestedQuantity: 10 },
    ]);
    expect(api.post).toHaveBeenCalledWith('/orgs/o1/dispatch', {
      customer_id: 'cust-9',
      line_items: [
        { stock_description: 'Cement', requested_quantity: 40 },
        { stock_description: 'Sand', requested_quantity: 10 },
      ],
    });
  });
});
