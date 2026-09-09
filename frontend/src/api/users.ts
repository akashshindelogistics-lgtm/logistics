import api from './client';
import type { ApiResponse, OrgRole, OrgUser } from '../types';

export const listOrgUsers = (orgId: string) =>
  api.get<ApiResponse<OrgUser[]>>(`/orgs/${orgId}/users`).then(r => r.data);

export const createOrgUser = (
  orgId: string,
  input: { name: string; email: string; password: string; role: OrgRole },
) => api.post<ApiResponse<OrgUser>>(`/orgs/${orgId}/users`, input).then(r => r.data);

export const updateOrgUser = (
  userId: string,
  input: { name: string; role: OrgRole; is_active: boolean },
) => api.put<ApiResponse<OrgUser>>(`/users/${userId}`, input).then(r => r.data);

export const deleteOrgUser = (userId: string) =>
  api.delete<ApiResponse<null>>(`/users/${userId}`).then(r => r.data);
