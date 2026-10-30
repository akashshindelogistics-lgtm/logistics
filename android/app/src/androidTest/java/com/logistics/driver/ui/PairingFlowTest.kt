package com.logistics.driver.ui

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.logistics.driver.DriverApplication
import com.logistics.driver.MainActivity
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.TimeUnit

/**
 * The pairing screen in the real app: real view models, DataStore, Keystore
 * and OkHttp. The backend is a MockWebServer on the phone itself, reached at
 * http://127.0.0.1, which debug builds allow in cleartext.
 */
@RunWith(AndroidJUnit4::class)
class PairingFlowTest {
    @get:Rule
    val compose = createAndroidComposeRule<MainActivity>()

    private val container get() = ApplicationProvider.getApplicationContext<DriverApplication>().container
    private val server = MockWebServer()
    private val token = "1b4e28ba-2fa1-11d2-883f-0016d3cca427"

    /** What `GET /api/driver/me` answers; set per test. */
    @Volatile
    private var meResponse = MockResponse().setResponseCode(200).setBody(
        """
        {"success":true,"message":"Driver found","data":{
          "driver_id":"d1","name":"Ramesh Patil","org_id":"o1","org_name":"Mauli Brickvision",
          "is_active":true,
          "vehicle":{"registration_number":"MH12AB1234","location":null}}}
        """.trimIndent(),
    )

    @Before
    fun setUp() {
        server.dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest) =
                if (request.path == "/api/driver/me") meResponse else MockResponse().setResponseCode(404)
        }
        server.start()
        // Start unpaired whatever an earlier run (or the demo) left behind.
        runBlocking { container.pairingRepository.unpair() }
        // Generous: the first test after the emulator boots also pays for the
        // app's first launch (Keystore key, DataStore, Compose warm-up), which
        // took over 5 s on the API 37 emulator.
        compose.waitUntil(20_000) { hasTag("pair-button") }
    }

    @After
    fun tearDown() {
        runBlocking { container.pairingRepository.unpair() }
        server.shutdown()
    }

    private fun hasTag(tag: String) =
        compose.onAllNodesWithTag(tag).fetchSemanticsNodes().isNotEmpty()

    private fun serverUrl() = "http://127.0.0.1:${server.port}"

    private fun enter(url: String, token: String) {
        compose.onNodeWithTag("server-url").performTextReplacement(url)
        compose.onNodeWithTag("device-token").performTextReplacement(token)
        compose.onNodeWithTag("pair-button").performClick()
    }

    @Test
    fun emptyTokenAndBadAddressAreRejectedWithoutCallingTheServer() {
        enter("ftp://example.com", "")

        compose.onNodeWithText("Enter the full address, starting with https://").assertIsDisplayed()
        compose.onNodeWithText("Enter the device token from your dispatcher").assertIsDisplayed()

        enter(serverUrl(), "not-a-token")

        compose.onNodeWithText("That doesn't look like a device token", substring = true).assertIsDisplayed()
        assertEquals(0, server.requestCount)
    }

    @Test
    fun validPairingOpensTheStatusScreen() {
        enter(serverUrl(), "  Bearer ${token.uppercase()} ")

        compose.waitUntil(10_000) { hasTag("driver-name") }
        compose.onNodeWithTag("driver-name").assertTextContains("Ramesh Patil")
        compose.onNodeWithTag("org-name").assertTextContains("Mauli Brickvision")
        compose.onNodeWithText("MH12AB1234").assertIsDisplayed()

        val request = server.takeRequest(5, TimeUnit.SECONDS)!!
        assertEquals("/api/driver/me", request.path)
        // The pasted "Bearer " prefix and upper case are normalised away.
        assertEquals("Bearer $token", request.getHeader("Authorization"))
    }

    @Test
    fun rejectedTokenStaysOnThePairingScreen() {
        meResponse = MockResponse().setResponseCode(401)
            .setBody("""{"success":false,"message":"Unknown driver device token","data":null}""")

        enter(serverUrl(), token)

        compose.waitUntil(10_000) { hasTag("pair-error") }
        compose.onNodeWithText("This device token wasn't recognised", substring = true).assertIsDisplayed()
        compose.onNodeWithTag("pair-button").assertIsDisplayed()
        assertEquals(null, runBlocking { container.pairingRepository.pairing.first() })
    }
}
