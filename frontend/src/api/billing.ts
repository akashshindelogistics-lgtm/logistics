import api from './client';
import type { ApiResponse, CustomerBillingSummary, Invoice } from '../types';

export interface InvoiceInput {
  amount: number;
  dueOn: string;
}

const toPayload = (input: InvoiceInput) => ({ amount: input.amount, due_on: input.dueOn });

export const createDispatchInvoice = (dispatchId: string, input: InvoiceInput) =>
  api
    .post<ApiResponse<Invoice>>(`/dispatches/${dispatchId}/invoice`, toPayload(input))
    .then(r => r.data);

export const getDispatchInvoice = (dispatchId: string) =>
  api.get<ApiResponse<Invoice>>(`/dispatches/${dispatchId}/invoice`).then(r => r.data);

export const updateInvoice = (invoiceId: string, input: InvoiceInput) =>
  api.put<ApiResponse<Invoice>>(`/invoices/${invoiceId}`, toPayload(input)).then(r => r.data);

export const payInvoice = (invoiceId: string) =>
  api.post<ApiResponse<Invoice>>(`/invoices/${invoiceId}/pay`, {}).then(r => r.data);

export const listOrgInvoices = (orgId: string) =>
  api.get<ApiResponse<Invoice[]>>(`/orgs/${orgId}/invoices`).then(r => r.data);

export const getCustomerBilling = (customerId: string) =>
  api
    .get<ApiResponse<CustomerBillingSummary>>(`/customers/${customerId}/billing`)
    .then(r => r.data);
