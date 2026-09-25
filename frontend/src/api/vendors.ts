import api from './client';
import type { ApiResponse, VehicleVendor, VendorInput } from '../types';

export const listVendors = (orgId: string) =>
  api.get<ApiResponse<VehicleVendor[]>>(`/orgs/${orgId}/vendors`).then(r => r.data);

export const createVendor = (orgId: string, input: VendorInput) =>
  api.post<ApiResponse<VehicleVendor>>(`/orgs/${orgId}/vendors`, input).then(r => r.data);

export const updateVendor = (vendorId: string, input: VendorInput & { is_active: boolean }) =>
  api.put<ApiResponse<VehicleVendor>>(`/vendors/${vendorId}`, input).then(r => r.data);

export const deleteVendor = (vendorId: string) =>
  api.delete<ApiResponse<null>>(`/vendors/${vendorId}`).then(r => r.data);
