import api from './client';
import type { ApiResponse } from '../types';

export interface UploadedFileRef {
  id: string;
  url: string;
}

/**
 * Upload an image file (proof-of-delivery photo/signature) for an org and
 * get back the URL it can be fetched from. The backend actually stores the
 * bytes (on local disk) — this replaces the old free-form "paste a URL"
 * text field.
 *
 * The shared `api` client sets a default `Content-Type: application/json`
 * header; that's explicitly unset here so the browser can compute the
 * correct `multipart/form-data; boundary=...` header for the `FormData`
 * body itself.
 */
export const uploadFile = (orgId: string, file: File) => {
  const formData = new FormData();
  formData.append('file', file);
  return api
    .post<ApiResponse<UploadedFileRef>>(`/orgs/${orgId}/uploads`, formData, {
      headers: { 'Content-Type': undefined },
    })
    .then(r => r.data);
};

/**
 * Turn the backend-relative path an upload responds with (always
 * `/api/uploads/{id}`) into a URL usable directly as an `<a href>`,
 * regardless of deployment shape:
 *  - Dev / same-origin prod (no `VITE_API_BASE_URL`): the path as-is,
 *    resolved against the frontend's own origin (the Vite proxy, or the
 *    same server, forwards it to the API).
 *  - Cross-origin prod (`VITE_API_BASE_URL` points at a separate API
 *    domain, e.g. GitHub Pages + a DuckDNS API host): prefixed with that
 *    domain's origin, so the link points at the API host that actually
 *    stores the file rather than the frontend's own origin.
 */
export const uploadedFileHref = (path: string): string => {
  const base = import.meta.env.VITE_API_BASE_URL;
  if (!base) return path;
  try {
    return new URL(base).origin + path;
  } catch {
    return path;
  }
};
