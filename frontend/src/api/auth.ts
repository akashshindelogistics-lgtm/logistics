import api from './client';
import type { ApiResponse } from '../types';

export interface LoginData {
  token: string;
  org_id: string;
  org_name: string;
}

export interface OrgSummary {
  id: string;
  name: string;
}

const TOKEN_KEY = 'logi_token';
const ORG_ID_KEY = 'logi_org_id';
const ORG_NAME_KEY = 'logi_org_name';

export const getToken = (): string | null => localStorage.getItem(TOKEN_KEY);
export const getOrgId = (): string | null => localStorage.getItem(ORG_ID_KEY);
export const getOrgName = (): string | null => localStorage.getItem(ORG_NAME_KEY);
export const isLoggedIn = (): boolean => !!getToken();

export const storeAuth = (data: LoginData) => {
  localStorage.setItem(TOKEN_KEY, data.token);
  localStorage.setItem(ORG_ID_KEY, data.org_id);
  localStorage.setItem(ORG_NAME_KEY, data.org_name);
};

export const clearAuth = () => {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(ORG_ID_KEY);
  localStorage.removeItem(ORG_NAME_KEY);
};

export const login = (orgId: string, password: string) =>
  api.post<ApiResponse<LoginData>>('/auth/login', { org_id: orgId, password });

export const listAuthOrgs = () =>
  api.get<ApiResponse<OrgSummary[]>>('/auth/orgs');
