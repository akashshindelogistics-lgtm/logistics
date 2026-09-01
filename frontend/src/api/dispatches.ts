import api from './client';
import type { ApiResponse, DispatchOrder, DispatchStatus } from '../types';

export const listDispatches = () =>
  api.get<ApiResponse<DispatchOrder[]>>('/dispatches').then(r => r.data);

export const getDispatchSummary = (id: string) =>
  api.get<ApiResponse<string>>(`/dispatches/${id}/summary`).then(r => r.data);

export interface ProofOfDeliveryInput {
  receiver_name: string;
  signature_or_photo_url: string;
}

/**
 * Move a dispatch to `status`. `proofOfDelivery` is required by the backend
 * when `status` is `DELIVERED` (rejected with a 400 otherwise) and ignored
 * for every other status.
 */
export const updateDispatchStatus = (
  id: string,
  status: DispatchStatus,
  proofOfDelivery?: ProofOfDeliveryInput,
) =>
  api
    .put<ApiResponse<DispatchOrder>>(`/dispatches/${id}/status`, {
      status,
      proof_of_delivery: proofOfDelivery,
    })
    .then(r => r.data);
