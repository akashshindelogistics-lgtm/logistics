package com.logistics.driver.domain

import com.logistics.driver.Config

/**
 * One position captured by the phone. [recordedAt] is unix **seconds** of
 * when the fix was taken, not when it is uploaded; the backend stores
 * `Location.timestamp` the same way.
 */
data class Fix(
    val latitude: Double,
    val longitude: Double,
    val recordedAt: Long,
    val accuracyM: Double? = null,
    val speedMps: Double? = null,
)

/**
 * Mirrors `validate_driver_fix` in src/logistics/server/routes/drivers.rs.
 * The server rejects a whole batch if one fix in it is invalid, so a bad fix
 * is dropped here instead of being queued and blocking every good fix
 * uploaded alongside it.
 *
 * Returns why the fix is invalid, or `null` if it can be queued.
 */
fun validateFix(fix: Fix, nowSeconds: Long): String? {
    if (!fix.latitude.isFinite() || fix.latitude !in -90.0..90.0) {
        return "latitude must be between -90 and 90"
    }
    if (!fix.longitude.isFinite() || fix.longitude !in -180.0..180.0) {
        return "longitude must be between -180 and 180"
    }
    if (fix.recordedAt <= 0) {
        return "recorded_at must be a positive unix timestamp in seconds"
    }
    if (fix.recordedAt > nowSeconds + Config.MAX_FIX_CLOCK_SKEW_SECS) {
        return "recorded_at is more than ${Config.MAX_FIX_CLOCK_SKEW_SECS} seconds in the future — check the phone's clock"
    }
    if (fix.accuracyM != null && (!fix.accuracyM.isFinite() || fix.accuracyM < 0)) {
        return "accuracy_m must be a non-negative number"
    }
    if (fix.speedMps != null && (!fix.speedMps.isFinite() || fix.speedMps < 0)) {
        return "speed_mps must be a non-negative number"
    }
    return null
}
