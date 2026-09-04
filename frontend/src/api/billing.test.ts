import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import {
  createDispatchInvoice,
  getDispatchInvoice,
  updateInvoice,
  payInvoice,
  listOrgInvoices,
  getCustomerBilling,
} from './billing';

vi.mock('./client', () => ({
  default: { get: vi.fn(), post: vi.fn(), put: vi.fn() },
}));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('billing api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('createDispatchInvoice POSTs the snake_cased payload', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({}));
    await createDispatchInvoice('d1', { amount: 4500, dueOn: '2027-01-15' });
    expect(api.post).toHaveBeenCalledWith('/dispatches/d1/invoice', {
      amount: 4500,
      due_on: '2027-01-15',
    });
  });

  it('getDispatchInvoice GETs the per-dispatch route and unwraps', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope({ id: 'i1' }));
    const res = await getDispatchInvoice('d1');
    expect(api.get).toHaveBeenCalledWith('/dispatches/d1/invoice');
    expect(res.data).toEqual({ id: 'i1' });
  });

  it('updateInvoice PUTs to /invoices/{id}', async () => {
    vi.mocked(api.put).mockResolvedValue(envelope({}));
    await updateInvoice('i1', { amount: 999, dueOn: '2027-02-01' });
    expect(api.put).toHaveBeenCalledWith('/invoices/i1', { amount: 999, due_on: '2027-02-01' });
  });

  it('payInvoice POSTs to /invoices/{id}/pay', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({}));
    await payInvoice('i1');
    expect(api.post).toHaveBeenCalledWith('/invoices/i1/pay', {});
  });

  it('listOrgInvoices GETs the org-wide route', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope([]));
    await listOrgInvoices('o1');
    expect(api.get).toHaveBeenCalledWith('/orgs/o1/invoices');
  });

  it('getCustomerBilling GETs the customer billing route', async () => {
    vi.mocked(api.get).mockResolvedValue(envelope({ customer_id: 'c1' }));
    await getCustomerBilling('c1');
    expect(api.get).toHaveBeenCalledWith('/customers/c1/billing');
  });
});
