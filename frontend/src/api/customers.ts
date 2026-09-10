import api from './client';
import type { ApiResponse, Customer } from '../types';

// Customers belong to a single org. The backend scopes the list to the
// authenticated org's token, so no org id is needed here.
export const listCustomers = () =>
  api.get<ApiResponse<Customer[]>>('/customers').then(r => r.data);

export interface CustomerLocationInput {
  latitude: number;
  longitude: number;
  /** Optional label for the map pin. Defaults to the customer's address. */
  address?: string;
}

export interface CustomerContactInput {
  phone?: string;
  email?: string;
}

export const createCustomer = (
  orgId: string,
  name: string,
  address: string,
  location?: CustomerLocationInput,
  contact?: CustomerContactInput,
) =>
  api
    .post<ApiResponse<Customer>>(`/orgs/${orgId}/customers`, {
      name,
      address,
      ...(location
        ? {
            latitude: location.latitude,
            longitude: location.longitude,
            location_address: location.address,
          }
        : {}),
      ...(contact?.phone ? { phone: contact.phone } : {}),
      ...(contact?.email ? { email: contact.email } : {}),
    })
    .then(r => r.data);

export const deleteCustomer = (customerId: string) =>
  api.delete<ApiResponse<null>>(`/customers/${customerId}`).then(r => r.data);
