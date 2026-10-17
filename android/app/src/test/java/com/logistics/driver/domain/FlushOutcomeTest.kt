package com.logistics.driver.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class FlushOutcomeTest {
    @Test
    fun `each status maps to its outcome`() {
        assertEquals(FlushOutcome.NETWORK_ERROR, classifyFlushFailure(0, "").outcome)
        assertEquals(FlushOutcome.UNAUTHORIZED, classifyFlushFailure(401, "").outcome)
        assertEquals(FlushOutcome.INACTIVE, classifyFlushFailure(403, "").outcome)
        assertEquals(FlushOutcome.NO_VEHICLE, classifyFlushFailure(409, "").outcome)
        assertEquals(FlushOutcome.INVALID_FIX, classifyFlushFailure(400, "bad").outcome)
        assertEquals(FlushOutcome.SERVER_ERROR, classifyFlushFailure(500, "").outcome)
        assertEquals(FlushOutcome.SERVER_ERROR, classifyFlushFailure(502, "").outcome)
    }

    @Test
    fun `only network and server faults are retried`() {
        assertTrue(FlushOutcome.NETWORK_ERROR.retry)
        assertTrue(FlushOutcome.SERVER_ERROR.retry)
        listOf(FlushOutcome.UNAUTHORIZED, FlushOutcome.INACTIVE, FlushOutcome.NO_VEHICLE, FlushOutcome.INVALID_FIX, FlushOutcome.OK, FlushOutcome.STALE)
            .forEach { assertFalse(it.name, it.retry) }
    }

    @Test
    fun `stale is a success, not an error`() {
        assertTrue(FlushOutcome.STALE.isSuccess)
        assertTrue(FlushOutcome.OK.isSuccess)
        assertFalse(FlushOutcome.NETWORK_ERROR.isSuccess)
    }

    @Test
    fun `the server's message is passed on where the app has no wording of its own`() {
        assertEquals("The server refused the fixes: latitude out of range", classifyFlushFailure(400, "latitude out of range").message)
        assertEquals("The server returned an error (500): boom", classifyFlushFailure(500, "boom").message)
    }
}
