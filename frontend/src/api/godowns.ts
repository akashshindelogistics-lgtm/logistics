import api from './client';
import type { ApiResponse, Godown, Location, Stock } from '../types';

export const listGodowns = (orgId: string) =>
  api.get<ApiResponse<Godown[]>>(`/orgs/${orgId}/godowns`).then(r => r.data);

export const createGodown = (orgId: string, name: string, address: string) =>
  api.post<ApiResponse<Godown>>(`/orgs/${orgId}/godowns`, { name, address }).then(r => r.data);

export const updateGodown = (godownId: string, name: string, address: string) =>
  api.put<ApiResponse<Godown>>(`/godowns/${godownId}`, { name, address }).then(r => r.data);

export const deleteGodown = (godownId: string) =>
  api.delete<ApiResponse<null>>(`/godowns/${godownId}`).then(r => r.data);

export const updateGodownLocation = (
  godownId: string,
  latitude: number,
  longitude: number,
  address?: string,
) =>
  api
    .put<ApiResponse<Location>>(`/godowns/${godownId}/location`, { latitude, longitude, address })
    .then(r => r.data);

export const addGodownStock = (
  godownId: string,
  description: string,
  quantity: number,
  volumeInSize: number,
) =>
  api
    .post<ApiResponse<Stock>>(`/godowns/${godownId}/stock`, {
      description,
      quantity,
      volume_in_size: volumeInSize,
    })
    .then(r => r.data);

export const deleteGodownStock = (godownId: string, description: string) =>
  api
    .delete<ApiResponse<null>>(`/godowns/${godownId}/stock/${encodeURIComponent(description)}`)
    .then(r => r.data);
