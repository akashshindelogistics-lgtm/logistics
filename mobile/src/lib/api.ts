import type { ApiResponse, DriverLocationApiFix, DriverLocationResult, QueuedFix } from './types';

/** Thrown by `reportLocation` for any non-2xx response. `status` lets the
 * caller in `flush.ts` decide whether the failure is worth retrying (a
 * network error or 5xx) or is a standing problem the retry loop should stop
 * hammering (401/403/409 — see that file). */
export class DriverLocationApiError extends Error {
  status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = 'DriverLocationApiError';
    this.status = status;
  }
}

function toApiFix(fix: QueuedFix): DriverLocationApiFix {
  return {
    latitude: fix.latitude,
    longitude: fix.longitude,
    recorded_at: Math.round(fix.recordedAt),
    ...(fix.accuracyM !== undefined ? { accuracy_m: fix.accuracyM } : {}),
    ...(fix.speedMps !== undefined ? { speed_mps: fix.speedMps } : {}),
  };
}

/**
 * POST a batch of fixes to `{apiBaseUrl}/api/driver/location`, authenticated
 * by the driver's device token (see `docs/driver-phone-tracking.md`).
 * `fixes` must be non-empty and at most `MAX_FIXES_PER_REQUEST` long — the
 * caller (`flush.ts`) is responsible for chunking the queue.
 */
export async function reportLocation(
  apiBaseUrl: string,
  deviceToken: string,
  fixes: readonly QueuedFix[],
): Promise<DriverLocationResult> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}/api/driver/location`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${deviceToken}`,
      },
      body: JSON.stringify({ fixes: fixes.map(toApiFix) }),
    });
  } catch (err) {
    // Network failure (offline, DNS, TLS, ...) — not a server response, so
    // there is no status code to classify. Treated as retryable by flush.ts.
    throw new DriverLocationApiError(0, err instanceof Error ? err.message : 'Network request failed');
  }

  let body: ApiResponse<DriverLocationResult> | null = null;
  try {
    body = (await response.json()) as ApiResponse<DriverLocationResult>;
  } catch {
    // Fall through with body still null; the status code alone is enough to
    // report a useful error below.
  }

  if (!response.ok) {
    throw new DriverLocationApiError(response.status, body?.message ?? `HTTP ${response.status}`);
  }
  if (!body?.data) {
    throw new DriverLocationApiError(response.status, body?.message ?? 'Empty response from server');
  }
  return body.data;
}
