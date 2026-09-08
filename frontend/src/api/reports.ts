import api from './client';
import type { ApiResponse, OpsReport } from '../types';

/** Operational report for one organisation: fleet utilization, delivery
 *  performance, godown inventory, and dispatch volume over the last 14 days. */
export const getOpsReport = (orgId: string) =>
  api.get<ApiResponse<OpsReport>>(`/orgs/${orgId}/reports`).then(r => r.data);
