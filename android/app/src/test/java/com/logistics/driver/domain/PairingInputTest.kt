package com.logistics.driver.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PairingInputTest {

    private fun valid(result: InputResult<String>): String {
        assertTrue("expected valid, got $result", result is InputResult.Valid)
        return (result as InputResult.Valid).value
    }

    private fun invalid(result: InputResult<String>): String {
        assertTrue("expected invalid, got $result", result is InputResult.Invalid)
        return (result as InputResult.Invalid).reason
    }

    @Test
    fun `server address is trimmed, scheme lower-cased and trailing slashes dropped`() {
        assertEquals("https://api.example.com", valid(parseServerAddress("  HTTPS://api.example.com// ", allowCleartext = false)))
    }

    @Test
    fun `server address keeps a port and a path prefix`() {
        assertEquals("https://example.com:8443/logistics", valid(parseServerAddress("https://example.com:8443/logistics/", false)))
    }

    @Test
    fun `http is allowed only when cleartext is`() {
        assertEquals("http://10.0.2.2:8080", valid(parseServerAddress("http://10.0.2.2:8080", allowCleartext = true)))
        assertTrue(invalid(parseServerAddress("http://10.0.2.2:8080", allowCleartext = false)).contains("https://"))
    }

    @Test
    fun `server address without a scheme, host or with junk is rejected`() {
        assertEquals("Enter the server address", invalid(parseServerAddress("   ", true)))
        assertTrue(invalid(parseServerAddress("api.example.com", true)).contains("https://"))
        assertTrue(invalid(parseServerAddress("ftp://api.example.com", true)).contains("https://"))
        invalid(parseServerAddress("https://", true))
        invalid(parseServerAddress("https://exa mple.com", true))
        invalid(parseServerAddress("https://example.com/?x=1", true))
    }

    @Test
    fun `device token accepts a UUID with spaces or a Bearer prefix and lower-cases it`() {
        val token = "1B4E28BA-2FA1-11D2-883F-0016D3CCA427"
        assertEquals(token.lowercase(), valid(parseDeviceToken("  $token ")))
        assertEquals(token.lowercase(), valid(parseDeviceToken("Bearer $token")))
    }

    @Test
    fun `device token that is empty or not a UUID is rejected`() {
        assertEquals("Enter the device token from your dispatcher", invalid(parseDeviceToken(" ")))
        invalid(parseDeviceToken("not-a-token"))
        invalid(parseDeviceToken("1b4e28ba2fa111d2883f0016d3cca427"))
        invalid(parseDeviceToken("1b4e28ba-2fa1-11d2-883f-0016d3cca42"))
    }
}
