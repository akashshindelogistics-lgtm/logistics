package com.logistics.driver.domain

import com.logistics.driver.net.AppRelease
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AppUpdateTest {
    private val release = AppRelease(versionCode = 3, versionName = "0.3.0", sizeBytes = 12_345_678, downloadPath = "/api/driver-app/download")

    @Test
    fun `a newer release is offered with the full download address`() {
        assertEquals(
            AppUpdate("0.3.0", 12_345_678, "https://api.example.com/api/driver-app/download"),
            appUpdate(release, installedVersionCode = 2, serverUrl = "https://api.example.com"),
        )
    }

    @Test
    fun `the same or an older release, or none, is not an update`() {
        assertNull(appUpdate(release, installedVersionCode = 3, serverUrl = "https://api.example.com"))
        assertNull(appUpdate(release, installedVersionCode = 4, serverUrl = "https://api.example.com"))
        assertNull(appUpdate(null, installedVersionCode = 1, serverUrl = "https://api.example.com"))
    }

    @Test
    fun `sizes are shown in megabytes with one decimal`() {
        assertEquals("12.3 MB", formatMegabytes(12_345_678))
        assertEquals("0.5 MB", formatMegabytes(500_000))
    }
}
