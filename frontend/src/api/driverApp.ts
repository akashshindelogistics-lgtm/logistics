import axios from 'axios';
import api from './client';
import type { ApiResponse, DriverAppRelease } from '../types';

/** The driver app release published on the server, or `null` if none has been (404). */
export const getDriverAppRelease = (): Promise<DriverAppRelease | null> =>
  api
    .get<ApiResponse<DriverAppRelease>>('/driver-app/latest')
    .then(r => r.data.data ?? null)
    .catch(err => {
      if (axios.isAxiosError(err) && err.response?.status === 404) return null;
      throw err;
    });

/**
 * The server address drivers enter on the pairing screen: the API's origin.
 * A deployed dashboard reaches the API through VITE_API_BASE_URL (e.g.
 * https://logi-api.example.org/api); in development the page's own origin
 * proxies /api to it.
 */
export function serverAddress(
  apiBase: string | undefined = import.meta.env.VITE_API_BASE_URL,
  pageOrigin: string = window.location.origin,
): string {
  return apiBase && /^https?:\/\//.test(apiBase) ? new URL(apiBase).origin : pageOrigin;
}

/** Where a phone downloads the APK: the fixed path on the server address. */
export const downloadUrl = (release: DriverAppRelease, server: string = serverAddress()) =>
  server + release.download_path;
