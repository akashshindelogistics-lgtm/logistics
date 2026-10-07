import { MAX_FIX_CLOCK_SKEW_SECS } from './config';
import type { DriverLocationFix } from './types';

/**
 * Mirrors `validate_driver_fix` in `src/logistics/server/routes.rs`. Running
 * the same checks client-side means a bad fix is dropped before it is
 * queued, instead of being queued forever and repeatedly rejected by the
 * server (which fails the *whole* batch it is uploaded in, per the backend's
 * "one bad fix rejects the whole batch" rule).
 *
 * Returns the reason the fix is invalid, or `null` if it is fine to queue.
 */
export function validateFix(fix: DriverLocationFix, nowSeconds: number): string | null {
  if (!isFinite(fix.latitude) || fix.latitude < -90 || fix.latitude > 90) {
    return 'latitude must be between -90 and 90';
  }
  if (!isFinite(fix.longitude) || fix.longitude < -180 || fix.longitude > 180) {
    return 'longitude must be between -180 and 180';
  }
  if (!Number.isFinite(fix.recordedAt) || fix.recordedAt <= 0) {
    return 'recordedAt must be a positive unix timestamp in seconds';
  }
  if (fix.recordedAt > nowSeconds + MAX_FIX_CLOCK_SKEW_SECS) {
    return `recordedAt is more than ${MAX_FIX_CLOCK_SKEW_SECS} seconds in the future — check the phone's clock`;
  }
  if (fix.accuracyM !== undefined && (!isFinite(fix.accuracyM) || fix.accuracyM < 0)) {
    return 'accuracyM must be a non-negative number';
  }
  if (fix.speedMps !== undefined && (!isFinite(fix.speedMps) || fix.speedMps < 0)) {
    return 'speedMps must be a non-negative number';
  }
  return null;
}
