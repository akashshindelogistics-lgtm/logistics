import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import { listCustomers, createCustomer } from './customers';

vi.mock('./client', () => ({
  default: { get: vi.fn(), post: vi.fn() },
}));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('customers api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('listCustomers GETs /customers and unwraps the envelope', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([{ id: 'c1' }]));
    const res = await listCustomers();
    expect(api.get).toHaveBeenCalledWith('/customers');
    expect(res.data).toEqual([{ id: 'c1' }]);
  });

  it('createCustomer POSTs name/address and unwraps', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({ id: 'c2' }));
    const res = await createCustomer('TechHub', '5 Market St');
    expect(api.post).toHaveBeenCalledWith('/customers', {
      name: 'TechHub',
      address: '5 Market St',
    });
    expect(res.data).toEqual({ id: 'c2' });
  });
});
