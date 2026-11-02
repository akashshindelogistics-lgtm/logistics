package com.logistics.driver.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PairingLinkTest {
    private val token = "1b4e28ba-2fa1-11d2-883f-0016d3cca427"

    @Test
    fun `reads the server and token from a dashboard pairing link`() {
        // As frontend/src/lib/pairingLink.ts builds it: the server is URL-encoded.
        val link = "logisticsdriver://pair?server=https%3A%2F%2Flogi-api.example.org&token=$token"
        assertEquals(PairingLink("https://logi-api.example.org", token), parsePairingLink(link))
    }

    @Test
    fun `tolerates spaces, case and parameter order`() {
        assertEquals(
            PairingLink("http://10.0.2.2:8080", token),
            parsePairingLink("  LogisticsDriver://PAIR?token=$token&server=http%3A%2F%2F10.0.2.2%3A8080  "),
        )
    }

    @Test
    fun `anything else is not a pairing link`() {
        listOf(
            "",
            "hello",
            token,
            "https://logi-api.example.org/api/driver-app/download",
            "logisticsdriver://other?server=x&token=$token",
            "logisticsdriver://pair?server=https%3A%2F%2Fx.org",
            "logisticsdriver://pair?token=$token",
            "logisticsdriver://pair?server=&token=$token",
        ).forEach { assertNull(it, parsePairingLink(it)) }
    }
}
