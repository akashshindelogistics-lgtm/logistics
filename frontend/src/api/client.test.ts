import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import api from './client';

// axios keeps registered interceptors on `.handlers`; exercising them directly
// is the least brittle way to unit-test this module without a real HTTP layer.
interface InterceptorHandler<T> {
  fulfilled: (value: T) => T;
  rejected: (error: unknown) => unknown;
}

const requestInterceptor = (
  api.interceptors.request as unknown as { handlers: InterceptorHandler<{ headers: Record<string, unknown> }>[] }
).handlers[0];

const responseInterceptor = (
  api.interceptors.response as unknown as { handlers: InterceptorHandler<unknown>[] }
).handlers[0];

const TOKEN_KEY = 'logi_token';

describe('client request interceptor', () => {
  beforeEach(() => localStorage.clear());

  it('attaches a Bearer Authorization header when a token is stored', () => {
    localStorage.setItem(TOKEN_KEY, 'tok-42');
    const config = requestInterceptor.fulfilled({ headers: {} });
    expect(config.headers.Authorization).toBe('Bearer tok-42');
  });

  it('leaves the Authorization header unset when there is no token', () => {
    const config = requestInterceptor.fulfilled({ headers: {} });
    expect(config.headers.Authorization).toBeUndefined();
  });
});

describe('client response interceptor', () => {
  let originalLocation: Location;

  beforeEach(() => {
    localStorage.clear();
    localStorage.setItem(TOKEN_KEY, 'tok-42');
    localStorage.setItem('logi_org_id', 'org-1');
    localStorage.setItem('logi_org_name', 'Acme');
    originalLocation = window.location;
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: { href: '' },
    });
  });

  afterEach(() => {
    Object.defineProperty(window, 'location', { configurable: true, value: originalLocation });
  });

  it('passes successful responses straight through', () => {
    const response = { status: 200, data: {} };
    expect(responseInterceptor.fulfilled(response)).toBe(response);
  });

  it('on a 401 for a non-auth request, clears the session and redirects to /login', async () => {
    const err = { response: { status: 401 }, config: { url: '/orgs' } };
    await expect(responseInterceptor.rejected(err)).rejects.toBe(err);
    expect(localStorage.getItem(TOKEN_KEY)).toBeNull();
    expect(localStorage.getItem('logi_org_id')).toBeNull();
    expect(localStorage.getItem('logi_org_name')).toBeNull();
    expect(window.location.href).toBe('/login');
  });

  it('on a 401 for an /auth/ request, leaves the session alone so the page can show the error', async () => {
    const err = { response: { status: 401 }, config: { url: '/auth/login' } };
    await expect(responseInterceptor.rejected(err)).rejects.toBe(err);
    expect(localStorage.getItem(TOKEN_KEY)).toBe('tok-42');
    expect(window.location.href).toBe('');
  });

  it('leaves non-401 errors untouched', async () => {
    const err = { response: { status: 500 }, config: { url: '/orgs' } };
    await expect(responseInterceptor.rejected(err)).rejects.toBe(err);
    expect(localStorage.getItem(TOKEN_KEY)).toBe('tok-42');
    expect(window.location.href).toBe('');
  });
});
