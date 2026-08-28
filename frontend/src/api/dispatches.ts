import api from './client';
import type { ApiResponse, DispatchOrder } from '../types';

export const listDispatches = () =>
  api.get<ApiResponse<DispatchOrder[]>>('/dispatches').then(r => r.data);

export const getDispatchSummary = (id: string) =>
  api.get<ApiResponse<string>>(`/dispatches/${id}/summary`).then(r => r.data);
