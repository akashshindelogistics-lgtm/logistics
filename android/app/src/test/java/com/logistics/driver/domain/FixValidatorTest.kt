package com.logistics.driver.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

/** The cases from mobile/src/lib/__tests__/validation.test.ts, plus the exact messages. */
class FixValidatorTest {
    private val now = 1_700_000_000L
    private fun fix(
        latitude: Double = 18.52,
        longitude: Double = 73.85,
        recordedAt: Long = now - 10,
        accuracyM: Double? = null,
        speedMps: Double? = null,
    ) = Fix(latitude, longitude, recordedAt, accuracyM, speedMps)

    @Test
    fun `accepts a normal fix, with or without accuracy and speed`() {
        assertNull(validateFix(fix(), now))
        assertNull(validateFix(fix(accuracyM = 8.0, speedMps = 5.5), now))
        assertNull(validateFix(fix(accuracyM = 0.0, speedMps = 0.0), now))
    }

    @Test
    fun `accepts the edges of the coordinate ranges`() {
        assertNull(validateFix(fix(latitude = 90.0, longitude = -180.0), now))
        assertNull(validateFix(fix(latitude = -90.0, longitude = 180.0), now))
    }

    @Test
    fun `rejects coordinates off the Earth`() {
        assertEquals("latitude must be between -90 and 90", validateFix(fix(latitude = 91.0), now))
        assertNotNull(validateFix(fix(latitude = -91.0), now))
        assertEquals("longitude must be between -180 and 180", validateFix(fix(longitude = 181.0), now))
        assertNotNull(validateFix(fix(longitude = -181.0), now))
    }

    @Test
    fun `rejects a zero or negative timestamp`() {
        assertEquals("recorded_at must be a positive unix timestamp in seconds", validateFix(fix(recordedAt = 0), now))
        assertNotNull(validateFix(fix(recordedAt = -5), now))
    }

    @Test
    fun `allows clock skew up to 300 s, and no more`() {
        assertNull(validateFix(fix(recordedAt = now + 100), now))
        assertNull(validateFix(fix(recordedAt = now + 300), now))
        assertEquals(
            "recorded_at is more than 300 seconds in the future — check the phone's clock",
            validateFix(fix(recordedAt = now + 301), now),
        )
        assertNotNull(validateFix(fix(recordedAt = now + 3_600), now))
    }

    @Test
    fun `rejects negative accuracy or speed`() {
        assertEquals("accuracy_m must be a non-negative number", validateFix(fix(accuracyM = -1.0), now))
        assertEquals("speed_mps must be a non-negative number", validateFix(fix(speedMps = -1.0), now))
    }

    @Test
    fun `rejects non-finite values`() {
        assertNotNull(validateFix(fix(latitude = Double.NaN), now))
        assertNotNull(validateFix(fix(longitude = Double.POSITIVE_INFINITY), now))
        assertNotNull(validateFix(fix(accuracyM = Double.NaN), now))
        assertNotNull(validateFix(fix(speedMps = Double.POSITIVE_INFINITY), now))
    }
}
