import api from './client';
import type { ApiResponse, ComplianceDocType, Unit, Vehicle, VehicleDocument } from '../types';

export const listVehicles = () =>
  api.get<ApiResponse<Vehicle[]>>('/vehicles').then(r => r.data);

export const updateVehicle = (reg: string, capacity: number, unit: Unit) =>
  api
    .put<ApiResponse<Vehicle>>(`/vehicles/${encodeURIComponent(reg)}`, { capacity, unit })
    .then(r => r.data);

export const addVehicle = (orgId: string, registrationNumber: string, capacity: number) =>
  api
    .post<ApiResponse<Vehicle>>(`/orgs/${orgId}/vehicles`, {
      registration_number: registrationNumber,
      capacity,
      unit: 'MetricTon',
    })
    .then(r => r.data);

export const deleteVehicle = (reg: string) =>
  api.delete<ApiResponse<null>>(`/vehicles/${encodeURIComponent(reg)}`).then(r => r.data);

// ── Vehicle compliance documents ────────────────────────────────────────────

export interface VehicleDocumentInput {
  doc_type: ComplianceDocType;
  document_number: string;
  issued_on?: string | null;
  expires_on: string;
  notes?: string | null;
}

export const listVehicleDocuments = (reg: string) =>
  api
    .get<ApiResponse<VehicleDocument[]>>(`/vehicles/${encodeURIComponent(reg)}/documents`)
    .then(r => r.data);

export const listOrgVehicleDocuments = (orgId: string) =>
  api
    .get<ApiResponse<VehicleDocument[]>>(`/orgs/${orgId}/vehicle-documents`)
    .then(r => r.data);

export const addVehicleDocument = (reg: string, input: VehicleDocumentInput) =>
  api
    .post<ApiResponse<VehicleDocument>>(`/vehicles/${encodeURIComponent(reg)}/documents`, input)
    .then(r => r.data);

export const updateVehicleDocument = (id: string, input: VehicleDocumentInput) =>
  api.put<ApiResponse<VehicleDocument>>(`/vehicle-documents/${id}`, input).then(r => r.data);

export const deleteVehicleDocument = (id: string) =>
  api.delete<ApiResponse<null>>(`/vehicle-documents/${id}`).then(r => r.data);
