import api from './client';
import type { ApiResponse, ComplianceDocType, Unit, Vehicle, VehicleDocument, VehicleMaintenance } from '../types';

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

/**
 * Issue a fresh GPS tracker key for a vehicle, invalidating the previous one.
 * Every tracker device on the vehicle must then be reconfigured with the new key.
 */
export const rotateTrackerKey = (reg: string) =>
  api
    .post<ApiResponse<Vehicle>>(`/vehicles/${encodeURIComponent(reg)}/tracker-key/rotate`)
    .then(r => r.data);

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

// ── Vehicle preventive maintenance ──────────────────────────────────────────

export interface VehicleMaintenanceInput {
  description: string;
  due_on?: string | null;
  due_at_mileage_km?: number | null;
  last_service_on?: string | null;
  notes?: string | null;
}

export const listVehicleMaintenance = (reg: string) =>
  api
    .get<ApiResponse<VehicleMaintenance[]>>(`/vehicles/${encodeURIComponent(reg)}/maintenance`)
    .then(r => r.data);

export const listOrgVehicleMaintenance = (orgId: string) =>
  api
    .get<ApiResponse<VehicleMaintenance[]>>(`/orgs/${orgId}/vehicle-maintenance`)
    .then(r => r.data);

export const addVehicleMaintenance = (reg: string, input: VehicleMaintenanceInput) =>
  api
    .post<ApiResponse<VehicleMaintenance>>(`/vehicles/${encodeURIComponent(reg)}/maintenance`, input)
    .then(r => r.data);

export const updateVehicleMaintenance = (id: string, input: VehicleMaintenanceInput) =>
  api.put<ApiResponse<VehicleMaintenance>>(`/vehicle-maintenance/${id}`, input).then(r => r.data);

export const recordVehicleMaintenanceMileage = (id: string, currentMileageKm: number) =>
  api
    .put<ApiResponse<VehicleMaintenance>>(`/vehicle-maintenance/${id}/mileage`, {
      current_mileage_km: currentMileageKm,
    })
    .then(r => r.data);

export const deleteVehicleMaintenance = (id: string) =>
  api.delete<ApiResponse<null>>(`/vehicle-maintenance/${id}`).then(r => r.data);
