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
 * for every other status. `returnToGodownId` is only used when `status` is
 * `RETURNED` — the godown the shipment's stock is credited back into; the
 * server picks a sensible default when it is omitted.
 */
export const updateDispatchStatus = (
  id: string,
  status: DispatchStatus,
  proofOfDelivery?: ProofOfDeliveryInput,
  returnToGodownId?: string,
) =>
  api
    .put<ApiResponse<DispatchOrder>>(`/dispatches/${id}/status`, {
      status,
      proof_of_delivery: proofOfDelivery,
      return_to_godown_id: returnToGodownId || undefined,
    })
    .then(r => r.data);
