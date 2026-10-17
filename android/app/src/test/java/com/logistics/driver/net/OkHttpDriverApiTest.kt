package com.logistics.driver.net

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test

class OkHttpDriverApiTest {
    private lateinit var server: MockWebServer
    private val api = OkHttpDriverApi(OkHttpClient(), Json { ignoreUnknownKeys = true }, Dispatchers.Unconfined)
    private val token = "1b4e28ba-2fa1-11d2-883f-0016d3cca427"

    @Before
    fun start() {
        server = MockWebServer()
        server.start()
    }

    @After
    fun stop() {
        server.shutdown()
    }

    private fun baseUrl() = server.url("/").toString().trimEnd('/')

    private suspend fun expectError(): DriverApiException {
        try {
            api.me(baseUrl(), token)
        } catch (e: DriverApiException) {
            return e
        }
        fail("expected DriverApiException")
        throw AssertionError()
    }

    @Test
    fun `me sends the device token and parses the driver, org and vehicle`() = runTest {
        server.enqueue(
            MockResponse().setResponseCode(200).setBody(
                """
                {"success":true,"message":"Driver found","data":{
                  "driver_id":"d1","name":"Ramesh","org_id":"o1","org_name":"Mauli Brickvision",
                  "is_active":true,
                  "vehicle":{"registration_number":"MH12AB1234",
                    "location":{"latitude":18.52,"longitude":73.85,"timestamp":1791400000,"address":null}},
                  "some_future_field":1}}
                """.trimIndent(),
            ),
        )

        val me = api.me(baseUrl(), token)

        val request = server.takeRequest()
        assertEquals("GET", request.method)
        assertEquals("/api/driver/me", request.path)
        assertEquals("Bearer $token", request.getHeader("Authorization"))
        assertEquals("Ramesh", me.name)
        assertEquals("Mauli Brickvision", me.orgName)
        assertTrue(me.isActive)
        assertEquals("MH12AB1234", me.vehicle?.registrationNumber)
        assertEquals(1791400000L, me.vehicle?.location?.timestamp)
    }

    @Test
    fun `me handles an inactive driver with no vehicle`() = runTest {
        server.enqueue(
            MockResponse().setResponseCode(200).setBody(
                """{"success":true,"message":"Driver found","data":{"driver_id":"d1","name":"R","org_id":"o1","org_name":"O","is_active":false,"vehicle":null}}""",
            ),
        )

        val me = api.me(baseUrl(), token)

        assertFalse(me.isActive)
        assertNull(me.vehicle)
    }

    @Test
    fun `a 401 carries the status and the server's message`() = runTest {
        server.enqueue(
            MockResponse().setResponseCode(401)
                .setBody("""{"success":false,"message":"Unknown driver device token","data":null}"""),
        )

        val e = expectError()

        assertEquals(401, e.status)
        assertEquals("Unknown driver device token", e.message)
    }

    @Test
    fun `a non-JSON error page still reports its status`() = runTest {
        server.enqueue(MockResponse().setResponseCode(502).setBody("<html>Bad gateway</html>"))

        val e = expectError()

        assertEquals(502, e.status)
        assertEquals("HTTP 502", e.message)
    }

    @Test
    fun `no server at the address is a network error`() = runTest {
        val url = baseUrl()
        server.shutdown()

        try {
            api.me(url, token)
            fail("expected DriverApiException")
        } catch (e: DriverApiException) {
            assertEquals(DriverApiException.NETWORK_ERROR, e.status)
        }
    }

    @Test
    fun `reportLocation posts the fixes with recorded_at in seconds and parses the result`() = runTest {
        server.enqueue(
            MockResponse().setResponseCode(200).setBody(
                """
                {"success":true,"message":"Location reported","data":{
                  "accepted":2,"location_updated":true,"vehicle_registration_number":"MH12AB1234",
                  "location":{"latitude":18.53,"longitude":73.86,"timestamp":1791400030,"address":null}}}
                """.trimIndent(),
            ),
        )

        val result = api.reportLocation(
            baseUrl(),
            token,
            listOf(
                LocationFixDto(18.52, 73.85, 1791400000, accuracyM = 8.0, speedMps = 5.1),
                LocationFixDto(18.53, 73.86, 1791400030),
            ),
        )

        val request = server.takeRequest()
        assertEquals("POST", request.method)
        assertEquals("/api/driver/location", request.path)
        assertEquals("Bearer $token", request.getHeader("Authorization"))
        assertTrue(request.getHeader("Content-Type")!!.startsWith("application/json"))
        val body = Json.parseToJsonElement(request.body.readUtf8()).jsonObject
        val fixes = body["fixes"]!!.jsonArray
        assertEquals(2, fixes.size)
        assertEquals("1791400000", fixes[0].jsonObject["recorded_at"].toString())
        assertEquals("8.0", fixes[0].jsonObject["accuracy_m"].toString())
        assertEquals("5.1", fixes[0].jsonObject["speed_mps"].toString())
        assertEquals(2, result.accepted)
        assertTrue(result.locationUpdated)
        assertEquals("MH12AB1234", result.vehicleRegistrationNumber)
        assertEquals(1791400030L, result.location?.timestamp)
    }

    @Test
    fun `reportLocation passes on 409 no vehicle`() = runTest {
        server.enqueue(
            MockResponse().setResponseCode(409)
                .setBody("""{"success":false,"message":"No vehicle is assigned to this driver","data":null}"""),
        )

        try {
            api.reportLocation(baseUrl(), token, listOf(LocationFixDto(18.52, 73.85, 1791400000)))
            fail("expected DriverApiException")
        } catch (e: DriverApiException) {
            assertEquals(409, e.status)
            assertEquals("No vehicle is assigned to this driver", e.message)
        }
    }

    @Test
    fun `describeApiError gives the driver something actionable`() {
        assertTrue(describeApiError(DriverApiException(0, "x")).startsWith("Can't reach the server"))
        assertTrue(describeApiError(DriverApiException(401, "x")).contains("Ask your dispatcher for a new one"))
        assertTrue(describeApiError(DriverApiException(404, "x")).contains("doesn't look like the logistics server"))
        assertEquals("The server returned an error (500): boom", describeApiError(DriverApiException(500, "boom")))
    }
}
