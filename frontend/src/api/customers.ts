import api from './client';
import type { ApiResponse, Customer } from '../types';

// Customers belong to a single org. The backend scopes the list to the
// authenticated org's token, so no org id is needed here.
export const listCustomers = () =>
  api.get<ApiResponse<Customer[]>>('/customers').then(r => r.data);

export const createCustomer = (orgId: string, name: string, address: string) =>
  api
    .post<ApiResponse<Customer>>(`/orgs/${orgId}/customers`, { name, address })
    .then(r => r.data);

export const deleteCustomer = (customerId: string) =>
  api.delete<ApiResponse<null>>(`/customers/${customerId}`).then(r => r.data);
