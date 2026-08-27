import api from './client';
import type { ApiResponse, Vehicle } from '../types';

export const listVehicles = () =>
  api.get<ApiResponse<Vehicle[]>>('/vehicles').then(r => r.data);

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
