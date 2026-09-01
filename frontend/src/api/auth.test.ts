import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import {
  getToken,
  getOrgId,
  getOrgName,
  isLoggedIn,
  storeAuth,
  clearAuth,
  login,
  listAuthOrgs,
} from './auth';

vi.mock('./client', () => ({
  default: { get: vi.fn(), post: vi.fn() },
}));

describe('auth token/org storage helpers', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetAllMocks();
  });

  it('reads back what storeAuth wrote', () => {
    storeAuth({ token: 'tok-1', org_id: 'org-1', org_name: 'Acme Freight' });
    expect(getToken()).toBe('tok-1');
    expect(getOrgId()).toBe('org-1');
    expect(getOrgName()).toBe('Acme Freight');
  });

  it('isLoggedIn reflects whether a token is stored', () => {
    expect(isLoggedIn()).toBe(false);
    storeAuth({ token: 'tok-1', org_id: 'org-1', org_name: 'Acme' });
    expect(isLoggedIn()).toBe(true);
  });

  it('clearAuth removes every stored key', () => {
    storeAuth({ token: 'tok-1', org_id: 'org-1', org_name: 'Acme' });
    clearAuth();
    expect(getToken()).toBeNull();
    expect(getOrgId()).toBeNull();
    expect(getOrgName()).toBeNull();
    expect(isLoggedIn()).toBe(false);
  });

  it('getters return null when nothing is stored', () => {
    expect(getToken()).toBeNull();
    expect(getOrgId()).toBeNull();
    expect(getOrgName()).toBeNull();
  });
});

describe('auth api calls', () => {
  beforeEach(() => vi.resetAllMocks());

  it('login POSTs org_id + password to /auth/login', () => {
    vi.mocked(api.post).mockResolvedValue({ data: {} });
    login('org-7', 's3cret');
    expect(api.post).toHaveBeenCalledWith('/auth/login', {
      org_id: 'org-7',
      password: 's3cret',
    });
  });

  it('listAuthOrgs GETs /auth/orgs', () => {
    vi.mocked(api.get).mockResolvedValue({ data: {} });
    listAuthOrgs();
    expect(api.get).toHaveBeenCalledWith('/auth/orgs');
  });
});
