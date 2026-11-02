import api from './client';
import type { ApiResponse, DevicePairing, Driver, DriverDeviceToken, Vehicle } from '../types';

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

/** Whether the driver's phone is paired, and when it last reported. */
export const getDevicePairing = (driverId: string) =>
  api.get<ApiResponse<DevicePairing>>(`/drivers/${driverId}/device-token`).then(r => r.data);

/**
 * Issue a new device token, disconnecting any phone paired before. The plain
 * token is in this response only (Admin/Dispatcher).
 */
export const rotateDeviceToken = (driverId: string) =>
  api.post<ApiResponse<DriverDeviceToken>>(`/drivers/${driverId}/device-token/rotate`).then(r => r.data);

/** Unpair the driver's phone; its token stops working at once (Admin/Dispatcher). */
export const unpairDevice = (driverId: string) =>
  api.delete<ApiResponse<DevicePairing>>(`/drivers/${driverId}/device-token`).then(r => r.data);
