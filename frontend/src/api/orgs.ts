import api from './client';
import type { ApiResponse, DispatchOrder, Organization } from '../types';

export interface DispatchLineItemInput {
  stockDescription: string;
  requestedQuantity: number;
}

export const listOrgs = () =>
  api.get<ApiResponse<Organization[]>>('/orgs').then(r => r.data);

export const getOrg = (id: string) =>
  api.get<ApiResponse<Organization>>(`/orgs/${id}`).then(r => r.data);

export const createOrg = (name: string, address: string, password: string) =>
  api.post<ApiResponse<Organization>>('/orgs', { name, address, password });

export const updateOrg = (id: string, name: string, address: string) =>
  api.put<ApiResponse<Organization>>(`/orgs/${id}`, { name, address }).then(r => r.data);

export const deleteOrg = (id: string) =>
  api.delete<ApiResponse<null>>(`/orgs/${id}`).then(r => r.data);

/**
 * Dispatch stock to a customer. With no `hireVendorId` a free vehicle from
 * the org's own fleet is picked. With one, the stock is reserved and the
 * dispatch waits (AWAITING_VEHICLE) for a truck hired from that vendor.
 */
export const dispatchStock = (
  orgId: string,
  customerId: string,
  lineItems: DispatchLineItemInput[],
  hireVendorId?: string,
) =>
  api
    .post<ApiResponse<DispatchOrder>>(`/orgs/${orgId}/dispatch`, {
      customer_id: customerId,
      line_items: lineItems.map(li => ({
        stock_description: li.stockDescription,
        requested_quantity: li.requestedQuantity,
      })),
      ...(hireVendorId ? { vehicle_source: 'HIRED', vendor_id: hireVendorId } : {}),
    })
    .then(r => r.data);
