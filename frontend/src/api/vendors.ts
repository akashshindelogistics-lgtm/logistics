import api from './client';
import type {
  ApiResponse, HireAssignmentInput, HireStatus, VehicleHire, VehicleVendor, VendorInput, VendorPayment,
} from '../types';

export const listVendors = (orgId: string) =>
  api.get<ApiResponse<VehicleVendor[]>>(`/orgs/${orgId}/vendors`).then(r => r.data);

export const createVendor = (orgId: string, input: VendorInput) =>
  api.post<ApiResponse<VehicleVendor>>(`/orgs/${orgId}/vendors`, input).then(r => r.data);

export const updateVendor = (vendorId: string, input: VendorInput & { is_active: boolean }) =>
  api.put<ApiResponse<VehicleVendor>>(`/vendors/${vendorId}`, input).then(r => r.data);

export const deleteVendor = (vendorId: string) =>
  api.delete<ApiResponse<null>>(`/vendors/${vendorId}`).then(r => r.data);

/** The org's vehicle hires, newest first, optionally only one status. */
export const listVehicleHires = (orgId: string, status?: HireStatus) =>
  api
    .get<ApiResponse<VehicleHire[]>>(`/orgs/${orgId}/vehicle-hires`, status ? { params: { status } } : undefined)
    .then(r => r.data);

/** Record the vendor's truck, driver and rate; its dispatches move to PENDING. */
export const assignVehicleHire = (hireId: string, input: HireAssignmentInput) =>
  api.put<ApiResponse<VehicleHire>>(`/vehicle-hires/${hireId}/assign`, input).then(r => r.data);

/** Pay the vendor against a hire; returns the hire with its new balance. */
export const recordVendorPayment = (
  hireId: string,
  input: { amount: number; paid_on?: string; note?: string },
) => api.post<ApiResponse<VehicleHire>>(`/vehicle-hires/${hireId}/payments`, input).then(r => r.data);

export const listVendorPayments = (hireId: string) =>
  api.get<ApiResponse<VendorPayment[]>>(`/vehicle-hires/${hireId}/payments`).then(r => r.data);
