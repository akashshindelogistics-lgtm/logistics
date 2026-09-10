import api from './client';
import type { ApiResponse, OrgRole } from '../types';

export interface LoginData {
  token: string;
  org_id: string;
  org_name: string;
  /** Always present from the server; optional here so older fixtures compile. */
  role?: OrgRole;
  user_name?: string | null;
}

export interface OrgSummary {
  id: string;
  name: string;
}

const TOKEN_KEY = 'logi_token';
const ORG_ID_KEY = 'logi_org_id';
const ORG_NAME_KEY = 'logi_org_name';
const ROLE_KEY = 'logi_role';
const USER_NAME_KEY = 'logi_user_name';

export const getToken = (): string | null => localStorage.getItem(TOKEN_KEY);
export const getOrgId = (): string | null => localStorage.getItem(ORG_ID_KEY);
export const getOrgName = (): string | null => localStorage.getItem(ORG_NAME_KEY);
export const getRole = (): OrgRole => (localStorage.getItem(ROLE_KEY) as OrgRole) || 'ADMIN';
export const getUserName = (): string | null => localStorage.getItem(USER_NAME_KEY);
export const isLoggedIn = (): boolean => !!getToken();
export const isAdmin = (): boolean => getRole() === 'ADMIN';

export const storeAuth = (data: LoginData) => {
  localStorage.setItem(TOKEN_KEY, data.token);
  localStorage.setItem(ORG_ID_KEY, data.org_id);
  localStorage.setItem(ORG_NAME_KEY, data.org_name);
  localStorage.setItem(ROLE_KEY, data.role ?? 'ADMIN');
  if (data.user_name) localStorage.setItem(USER_NAME_KEY, data.user_name);
  else localStorage.removeItem(USER_NAME_KEY);
};

export const clearAuth = () => {
  [TOKEN_KEY, ORG_ID_KEY, ORG_NAME_KEY, ROLE_KEY, USER_NAME_KEY].forEach(k => localStorage.removeItem(k));
};

/** Sign in as an organization (the org-owner login — always Admin). */
export const login = (orgId: string, password: string) =>
  api.post<ApiResponse<LoginData>>('/auth/login', { org_id: orgId, password });

/** Sign in as a team member with their own email + password. */
export const userLogin = (email: string, password: string) =>
  api.post<ApiResponse<LoginData>>('/auth/user-login', { email, password });

export const listAuthOrgs = () =>
  api.get<ApiResponse<OrgSummary[]>>('/auth/orgs');
