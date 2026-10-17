package com.logistics.driver.data

import com.logistics.driver.Config
import com.logistics.driver.domain.Fix
import com.logistics.driver.domain.FlushOutcome
import com.logistics.driver.domain.classifyFlushFailure
import com.logistics.driver.domain.validateFix
import com.logistics.driver.net.DriverApi
import com.logistics.driver.net.DriverApiException
import com.logistics.driver.net.LocationFixDto
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Queues fixes from the location service and uploads them. Shared by the
 * service, [com.logistics.driver.tracking.FlushWorker] and the status
 * screen's Sync now, all in one process, so one [Mutex] keeps two uploads
 * from sending the same batch.
 */
class LocationReporter(
    private val pairingStore: PairingStore,
    private val queue: FixQueue,
    private val status: StatusStore,
    private val api: DriverApi,
    private val nowMillis: () -> Long = System::currentTimeMillis,
) {
    private val flushLock = Mutex()

    val queueSize = queue.size

    /** Validate and queue one fix. Returns `false` (and records why) when it's dropped. */
    suspend fun ingest(fix: Fix): Boolean {
        val reason = validateFix(fix, nowMillis() / 1000)
        if (reason != null) {
            status.recordSync(nowMillis(), FlushOutcome.INVALID_FIX, "Dropped a fix from the phone: $reason")
            return false
        }
        queue.enqueue(fix, Config.MAX_QUEUED_FIXES)
        status.recordFix(fix.recordedAt)
        return true
    }

    /**
     * Upload the queue in batches, oldest first, until it's empty or a batch
     * fails. Returns the last batch's outcome, or `null` when there was
     * nothing to send (empty queue, or not paired).
     *
     * A failed batch stays queued whatever the reason; see [FlushOutcome].
     */
    suspend fun flush(): FlushOutcome? = flushLock.withLock {
        val pairing = pairingStore.pairing.first() ?: return null
        var last: FlushOutcome? = null
        var batch = queue.oldest(Config.MAX_FIXES_PER_REQUEST)
        while (batch.isNotEmpty()) {
            try {
                val result = api.reportLocation(pairing.serverUrl, pairing.deviceToken, batch.map { it.toDto() })
                queue.remove(batch.map { it.id })
                last = if (result.locationUpdated) FlushOutcome.OK else FlushOutcome.STALE
                val message = if (result.locationUpdated) {
                    "Reported to ${result.vehicleRegistrationNumber}"
                } else {
                    "The server already had a newer position"
                }
                status.recordSync(nowMillis(), last, message, result.vehicleRegistrationNumber)
            } catch (e: DriverApiException) {
                val failure = classifyFlushFailure(e.status, e.message ?: "")
                status.recordSync(nowMillis(), failure.outcome, failure.message)
                return failure.outcome
            }
            batch = queue.oldest(Config.MAX_FIXES_PER_REQUEST)
        }
        last
    }

    /** Drop every queued fix and the sync history, for unpairing. */
    suspend fun reset() = flushLock.withLock {
        queue.clear()
        status.clear()
    }
}

private fun QueuedFix.toDto() = LocationFixDto(latitude, longitude, recordedAt, accuracyM, speedMps)
