import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import api from './client';
import { uploadFile, uploadedFileHref } from './uploads';

vi.mock('./client', () => ({ default: { get: vi.fn(), post: vi.fn() } }));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('uploads api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('uploadFile POSTs a FormData body to /orgs/{id}/uploads with Content-Type unset, and unwraps', async () => {
    vi.mocked(api.post).mockResolvedValue(envelope({ id: 'file-1', url: '/api/uploads/file-1' }));
    const file = new File(['bytes'], 'pod.png', { type: 'image/png' });

    const res = await uploadFile('org-1', file);

    expect(api.post).toHaveBeenCalledTimes(1);
    const [url, body, config] = vi.mocked(api.post).mock.calls[0];
    expect(url).toBe('/orgs/org-1/uploads');
    expect(body).toBeInstanceOf(FormData);
    expect((body as FormData).get('file')).toBe(file);
    expect(config).toEqual({ headers: { 'Content-Type': undefined } });
    expect(res.data).toEqual({ id: 'file-1', url: '/api/uploads/file-1' });
  });

  describe('uploadedFileHref', () => {
    afterEach(() => vi.unstubAllEnvs());

    it('returns the path as-is when VITE_API_BASE_URL is unset (same-origin/dev)', () => {
      vi.stubEnv('VITE_API_BASE_URL', '');
      expect(uploadedFileHref('/api/uploads/file-1')).toBe('/api/uploads/file-1');
    });

    it('prefixes the API origin when VITE_API_BASE_URL points at a separate domain', () => {
      vi.stubEnv('VITE_API_BASE_URL', 'https://logi-api.duckdns.org/api');
      expect(uploadedFileHref('/api/uploads/file-1')).toBe('https://logi-api.duckdns.org/api/uploads/file-1');
    });

    it('falls back to the bare path if VITE_API_BASE_URL is not a valid URL', () => {
      vi.stubEnv('VITE_API_BASE_URL', 'not-a-url');
      expect(uploadedFileHref('/api/uploads/file-1')).toBe('/api/uploads/file-1');
    });
  });
});
