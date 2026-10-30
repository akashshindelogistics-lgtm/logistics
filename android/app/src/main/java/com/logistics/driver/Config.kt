package com.logistics.driver

/**
 * Constants shared across the app. The ones marked "mirrors" must match the
 * backend (src/logistics/server/routes/drivers.rs); change them together.
 */
object Config {
    /** Mirrors `MAX_DRIVER_FIXES_PER_REQUEST`. A longer queue is sent in several requests. */
    const val MAX_FIXES_PER_REQUEST = 100

    /** Mirrors `MAX_FIX_CLOCK_SKEW_SECS`. A fix further in the future is rejected by the server. */
    const val MAX_FIX_CLOCK_SKEW_SECS = 300L

    /** Offline queue cap, about a day of driving at one fix per 30 s. Oldest fixes are dropped past it. */
    const val MAX_QUEUED_FIXES = 2000

    /** How often to ask for a location fix, and the minimum distance between fixes. */
    const val LOCATION_INTERVAL_MS = 30_000L
    const val LOCATION_MIN_DISTANCE_M = 50f

    /** Pre-filled server address in debug builds: the development machine as seen from the emulator. */
    const val DEBUG_DEFAULT_SERVER = "http://10.0.2.2:8080"
}
