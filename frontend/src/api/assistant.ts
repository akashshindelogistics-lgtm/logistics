import api from './client';
import type { ApiResponse, AssistantAnswer, AssistantReindexResult } from '../types';

/** Ask the org-scoped "ask your data" assistant a natural-language question,
 *  grounded in that org's own indexed notifications and vehicle compliance
 *  documents. */
export const askAssistant = (orgId: string, question: string) =>
  api.post<ApiResponse<AssistantAnswer>>(`/orgs/${orgId}/assistant/ask`, { question }).then(r => r.data);

/** Rebuild every assistant chunk for an org from scratch — for data that
 *  predates this feature and so never triggered a write-through index hook. */
export const reindexAssistant = (orgId: string) =>
  api.post<ApiResponse<AssistantReindexResult>>(`/orgs/${orgId}/assistant/reindex`).then(r => r.data);
