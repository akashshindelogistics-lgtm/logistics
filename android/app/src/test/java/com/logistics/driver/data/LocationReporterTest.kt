package com.logistics.driver.data

import com.logistics.driver.Config
import com.logistics.driver.FakeDriverApi
import com.logistics.driver.FakeFixQueue
import com.logistics.driver.FakePairingStore
import com.logistics.driver.FakeStatusStore
import com.logistics.driver.TOKEN
import com.logistics.driver.domain.Fix
import com.logistics.driver.domain.FlushOutcome
import com.logistics.driver.net.DriverApiException
import com.logistics.driver.reportOk
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class LocationReporterTest {
    private val nowMillis = 1_700_000_000_000L
    private val nowSecs = nowMillis / 1000
    private val pairing = FakePairingStore()
    private val queue = FakeFixQueue()
    private val status = FakeStatusStore()
    private val api = FakeDriverApi()
    private val reporter = LocationReporter(pairing, queue, status, api, nowMillis = { nowMillis })

    private fun fix(n: Int = 0) = Fix(18.52, 73.85, nowSecs - 10_000 + n, accuracyM = 8.0)

    private suspend fun paired() = pairing.save(Pairing("https://api.example.com", TOKEN))

    @Test
    fun `a valid fix is queued and recorded as the last fix`() = runTest {
        assertTrue(reporter.ingest(fix()))

        assertEquals(1, queue.items.value.size)
        assertEquals(nowSecs - 10_000, status.current.value.lastFixAt)
    }

    @Test
    fun `an invalid fix is dropped, with the reason shown`() = runTest {
        assertFalse(reporter.ingest(fix().copy(recordedAt = nowSecs + 3_600)))

        assertTrue(queue.items.value.isEmpty())
        assertEquals(FlushOutcome.INVALID_FIX, status.current.value.lastOutcome)
        assertTrue(status.current.value.lastMessage!!.startsWith("Dropped a fix from the phone: recorded_at is more than 300"))
    }

    @Test
    fun `past the cap the oldest fixes are dropped`() = runTest {
        repeat(Config.MAX_QUEUED_FIXES + 5) { reporter.ingest(fix(it)) }

        assertEquals(Config.MAX_QUEUED_FIXES, queue.items.value.size)
        assertEquals(nowSecs - 10_000 + 5, queue.items.value.first().recordedAt)
    }

    @Test
    fun `nothing is sent when not paired or the queue is empty`() = runTest {
        reporter.ingest(fix())
        assertNull(reporter.flush())

        queue.clear()
        paired()
        assertNull(reporter.flush())
        assertTrue(api.reported.isEmpty())
    }

    @Test
    fun `the queue is sent in batches of 100, oldest first, and emptied`() = runTest {
        paired()
        repeat(250) { reporter.ingest(fix(it)) }

        assertEquals(FlushOutcome.OK, reporter.flush())

        assertEquals(listOf(100, 100, 50), api.reported.map { it.size })
        assertEquals(nowSecs - 10_000, api.reported[0][0].recordedAt)
        assertEquals(8.0, api.reported[0][0].accuracyM)
        assertTrue(queue.items.value.isEmpty())
        assertEquals("Reported to MH12AB1234", status.current.value.lastMessage)
        assertEquals("MH12AB1234", status.current.value.vehicle)
        assertEquals(nowMillis, status.current.value.lastSyncAt)
    }

    @Test
    fun `an already newer position on the server is reported as stale, and the batch removed`() = runTest {
        paired()
        reporter.ingest(fix())
        api.reportResults += Result.success(reportOk(updated = false))

        assertEquals(FlushOutcome.STALE, reporter.flush())
        assertTrue(queue.items.value.isEmpty())
    }

    @Test
    fun `a failed batch stays queued and stops the flush`() = runTest {
        paired()
        repeat(150) { reporter.ingest(fix(it)) }
        api.reportResults += Result.success(reportOk())
        api.reportResults += Result.failure(DriverApiException(DriverApiException.NETWORK_ERROR, "offline"))

        assertEquals(FlushOutcome.NETWORK_ERROR, reporter.flush())

        // The first batch went through; the second is kept for the retry.
        assertEquals(50, queue.items.value.size)
        assertEquals(FlushOutcome.NETWORK_ERROR, status.current.value.lastOutcome)
    }

    @Test
    fun `a rejected token keeps the queue and is not retried`() = runTest {
        paired()
        reporter.ingest(fix())
        api.reportResults += Result.failure(DriverApiException(401, "Unknown driver device token"))

        val outcome = reporter.flush()

        assertEquals(FlushOutcome.UNAUTHORIZED, outcome)
        assertFalse(outcome!!.retry)
        assertEquals(1, queue.items.value.size)
    }

    @Test
    fun `reset drops the queue and the history but not the battery prompt`() = runTest {
        reporter.ingest(fix())
        status.setSharing(true)
        status.setBatteryPromptShown()

        reporter.reset()

        assertTrue(queue.items.value.isEmpty())
        assertEquals(SyncStatus(batteryPromptShown = true), status.current.value)
    }
}
