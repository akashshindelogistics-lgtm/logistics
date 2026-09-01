import api from './client';
import type { ApiResponse, Driver, Vehicle } from '../types';

export const listDrivers = () =>
  api.get<ApiResponse<Driver[]>>('/drivers').then(r => r.data);

export const addDriver = (
  orgId: string,
  name: string,
  licenseNumber: string,
  phone: string,
) =>
  api
    .post<ApiResponse<Driver>>(`/orgs/${orgId}/drivers`, {
      name,
      license_number: licenseNumber,
      phone,
    })
    .then(r => r.data);

export const updateDriver = (
  driverId: string,
  name: string,
  licenseNumber: string,
  phone: string,
  isActive: boolean,
) =>
  api
    .put<ApiResponse<Driver>>(`/drivers/${driverId}`, {
      name,
      license_number: licenseNumber,
      phone,
      is_active: isActive,
    })
    .then(r => r.data);

export const deleteDriver = (driverId: string) =>
  api.delete<ApiResponse<null>>(`/drivers/${driverId}`).then(r => r.data);

/** Assign a driver to a vehicle, or pass `null` to clear the assignment. */
export const assignVehicleDriver = (reg: string, driverId: string | null) =>
  api
    .put<ApiResponse<Vehicle>>(`/vehicles/${encodeURIComponent(reg)}/driver`, {
      driver_id: driverId,
    })
    .then(r => r.data);
