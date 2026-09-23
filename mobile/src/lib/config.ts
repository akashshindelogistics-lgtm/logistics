/** Name the background task is registered under. Must match between
 * `TaskManager.defineTask` and `Location.startLocationUpdatesAsync`. */
export const LOCATION_TASK_NAME = 'logistics-driver-location-report';

/** Mirrors `MAX_DRIVER_FIXES_PER_REQUEST` in `src/logistics/server/routes.rs`.
 * A queue longer than this is sent in several requests. */
export const MAX_FIXES_PER_REQUEST = 100;

/** Mirrors `MAX_FIX_CLOCK_SKEW_SECS` in `src/logistics/server/routes.rs`. A
 * fix timestamped further in the future than this is rejected by the server,
 * so it is dropped client-side instead of being queued forever. */
export const MAX_FIX_CLOCK_SKEW_SECS = 300;

/** Upper bound on how many fixes the offline queue keeps. At one fix every
 * ~30s this is roughly a day of driving; past this the oldest fixes are
 * dropped rather than growing the queue without limit on a phone that has
 * been offline a long time. */
export const MAX_QUEUED_FIXES = 2000;

/** How often the background task asks the OS for a location update, in
 * milliseconds, and the minimum distance (metres) between updates. */
export const LOCATION_TIME_INTERVAL_MS = 30_000;
export const LOCATION_DISTANCE_INTERVAL_M = 50;

export const STORAGE_KEYS = {
  pairing: 'logistics.driver.pairing',
  queue: 'logistics.driver.queue',
  status: 'logistics.driver.status',
} as const;
