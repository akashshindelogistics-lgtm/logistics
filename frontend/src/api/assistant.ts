import api from './client';
import type { ApiResponse, AssistantAnswer } from '../types';

/** Ask the org-scoped "ask your data" assistant a natural-language question,
 *  grounded in that org's own indexed notifications and vehicle compliance
 *  documents. */
export const askAssistant = (orgId: string, question: string) =>
  api.post<ApiResponse<AssistantAnswer>>(`/orgs/${orgId}/assistant/ask`, { question }).then(r => r.data);
