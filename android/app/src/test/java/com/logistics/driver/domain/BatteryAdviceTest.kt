package com.logistics.driver.domain

import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class BatteryAdviceTest {
    @Test
    fun `brands are matched case-insensitively, including sub-brands`() {
        assertTrue(brandBatterySteps("Xiaomi")!!.contains("Autostart"))
        assertTrue(brandBatterySteps("POCO")!!.contains("Autostart"))
        assertTrue(brandBatterySteps("samsung")!!.contains("Never sleeping apps"))
        assertTrue(brandBatterySteps("realme")!!.contains("Allow background activity"))
        assertTrue(brandBatterySteps("vivo")!!.contains("Background power consumption"))
    }

    @Test
    fun `stock Android needs only the standard setting`() {
        assertNull(brandBatterySteps("Google"))
        assertNull(brandBatterySteps("unknown"))
    }
}
