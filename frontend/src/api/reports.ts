import api from './client';
import type { ApiResponse, OpsReport } from '../types';

/** Operational report for one organisation: fleet utilization, delivery
 *  performance, godown inventory, and dispatch volume over the last 14 days. */
export const getOpsReport = (orgId: string) =>
  api.get<ApiResponse<OpsReport>>(`/orgs/${orgId}/reports`).then(r => r.data);

/** An AI-narrated "what needs your attention" briefing over the same report. */
export const getOpsReportSummary = (orgId: string) =>
  api.get<ApiResponse<string>>(`/orgs/${orgId}/reports/summary`).then(r => r.data);
