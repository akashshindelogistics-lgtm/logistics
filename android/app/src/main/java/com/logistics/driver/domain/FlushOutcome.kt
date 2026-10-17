package com.logistics.driver.domain

/**
 * How the last upload of queued fixes went. [retry] says whether trying again
 * soon can help: a network error or a server fault can clear up by itself,
 * but a rejected token, an inactive driver or a missing vehicle needs the
 * dispatcher, so those wait for the next fix or Sync now instead.
 *
 * Whatever the outcome, a rejected batch stays queued; only a `200` removes it.
 */
enum class FlushOutcome(val retry: Boolean) {
    /** Uploaded, and the vehicle moved. */
    OK(retry = false),
    /** Uploaded, but the server already had a newer position. Not an error. */
    STALE(retry = false),
    NETWORK_ERROR(retry = true),
    SERVER_ERROR(retry = true),
    UNAUTHORIZED(retry = false),
    INACTIVE(retry = false),
    NO_VEHICLE(retry = false),
    /** `400`. Shouldn't happen, since fixes are validated before queueing. */
    INVALID_FIX(retry = false),
    ;

    val isSuccess: Boolean get() = this == OK || this == STALE
}

data class FlushFailure(val outcome: FlushOutcome, val message: String)

/**
 * Classify a failed upload by its HTTP [status] (`0` when no response
 * arrived). [serverMessage] is the backend's `message`, shown for the cases
 * the app has no wording of its own for.
 */
fun classifyFlushFailure(status: Int, serverMessage: String): FlushFailure = when (status) {
    0 -> FlushFailure(FlushOutcome.NETWORK_ERROR, "No internet connection. Fixes stay queued and upload when it's back.")
    401 -> FlushFailure(FlushOutcome.UNAUTHORIZED, "The device token was rejected. Unpair and pair again with a new token.")
    403 -> FlushFailure(FlushOutcome.INACTIVE, "You're marked inactive. Ask your dispatcher.")
    409 -> FlushFailure(FlushOutcome.NO_VEHICLE, "No vehicle is assigned to you yet. Ask your dispatcher.")
    400 -> FlushFailure(FlushOutcome.INVALID_FIX, "The server refused the fixes: $serverMessage")
    else -> FlushFailure(FlushOutcome.SERVER_ERROR, "The server returned an error ($status): $serverMessage")
}
