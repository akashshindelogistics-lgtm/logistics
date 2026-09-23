/**
 * Mirrors the backend contract in `src/logistics/server/routes.rs`
 * (`DriverLocationFix`, `DriverLocationPayload`, `DriverLocationResult`) and
 * `docs/driver-phone-tracking.md`. Keep this in sync if the backend's shape
 * changes — there is no shared schema between the two projects.
 */

/** One position captured by the phone. `recordedAt` is unix seconds, not
 * milliseconds — the backend stores `Location.timestamp` the same way. */
export interface DriverLocationFix {
  latitude: number;
  longitude: number;
  recordedAt: number;
  accuracyM?: number;
  speedMps?: number;
}

/** A fix waiting in the offline queue. `id` is local-only, used to remove
 * exactly the fixes a successful upload covered. */
export interface QueuedFix extends DriverLocationFix {
  id: string;
}

export interface DriverLocationApiFix {
  latitude: number;
  longitude: number;
  recorded_at: number;
  accuracy_m?: number;
  speed_mps?: number;
}

export interface DriverLocationResult {
  accepted: number;
  location_updated: boolean;
  vehicle_registration_number: string;
  location: {
    latitude: number;
    longitude: number;
    timestamp: number;
    address: string | null;
  } | null;
}

export interface ApiResponse<T> {
  success: boolean;
  message: string;
  data: T | null;
}

/** Saved on the device after pairing. Both fields come from the dashboard's
 * "Regenerate device token" panel (see docs/driver-phone-tracking.md). */
export interface Pairing {
  deviceToken: string;
  /** Base URL of the logistics API, e.g. `https://api.example.com` — no
   * trailing slash, no `/api` suffix. */
  apiBaseUrl: string;
}
