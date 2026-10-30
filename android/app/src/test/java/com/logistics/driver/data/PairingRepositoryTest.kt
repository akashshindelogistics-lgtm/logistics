package com.logistics.driver.data

import com.logistics.driver.FakeDriverApi
import com.logistics.driver.FakePairingStore
import com.logistics.driver.TOKEN
import com.logistics.driver.net.DriverApiException
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.fail
import org.junit.Test

class PairingRepositoryTest {
    private val store = FakePairingStore()
    private val api = FakeDriverApi()
    private val repository = PairingRepository(store, api)

    @Test
    fun `pair checks the token with the server, then saves it`() = runTest {
        val me = repository.pair("https://api.example.com", TOKEN)

        assertEquals("Ramesh", me.name)
        assertEquals(listOf("https://api.example.com" to TOKEN), api.calls)
        assertEquals(Pairing("https://api.example.com", TOKEN), repository.pairing.first())
    }

    @Test
    fun `a rejected token is not saved`() = runTest {
        api.error = DriverApiException(401, "Unknown driver device token")

        try {
            repository.pair("https://api.example.com", TOKEN)
            fail("expected DriverApiException")
        } catch (e: DriverApiException) {
            assertEquals(401, e.status)
        }
        assertNull(repository.pairing.first())
    }

    @Test
    fun `whoAmI asks the server with the saved pairing, and unpair forgets it`() = runTest {
        repository.pair("https://api.example.com", TOKEN)
        api.result = com.logistics.driver.driverMe(name = "Suresh")

        assertEquals("Suresh", repository.whoAmI(repository.pairing.first()!!).name)

        repository.unpair()
        assertNull(repository.pairing.first())
    }
}
