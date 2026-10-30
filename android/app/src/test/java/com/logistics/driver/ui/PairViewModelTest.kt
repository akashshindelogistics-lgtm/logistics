package com.logistics.driver.ui

import com.logistics.driver.FakeDriverApi
import com.logistics.driver.FakePairingStore
import com.logistics.driver.TOKEN
import com.logistics.driver.data.Pairing
import com.logistics.driver.data.PairingRepository
import com.logistics.driver.net.DriverApiException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class PairViewModelTest {
    private val store = FakePairingStore()
    private val api = FakeDriverApi()
    private val repository = PairingRepository(store, api)

    @Before
    fun setUp() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After
    fun tearDown() = Dispatchers.resetMain()

    private fun viewModel(allowCleartext: Boolean = false) = PairViewModel(repository, allowCleartext).apply {
        onServerUrlChange("")
    }

    @Test
    fun `invalid fields show their errors and never call the server`() {
        val vm = viewModel()
        vm.onServerUrlChange("http://api.example.com")
        vm.onDeviceTokenChange("nope")

        vm.pair()

        val state = vm.state.value
        assertNotNull(state.serverUrlError)
        assertNotNull(state.deviceTokenError)
        assertTrue(api.calls.isEmpty())
    }

    @Test
    fun `valid fields pair with the normalised values`() = runTest {
        val vm = viewModel()
        vm.onServerUrlChange(" https://api.example.com/ ")
        vm.onDeviceTokenChange(TOKEN.uppercase())

        vm.pair()

        assertEquals(Pairing("https://api.example.com", TOKEN), store.pairing.first())
        assertFalse(vm.state.value.busy)
        assertNull(vm.state.value.error)
    }

    @Test
    fun `a server rejection is shown and nothing is saved`() = runTest {
        api.error = DriverApiException(401, "Unknown driver device token")
        val vm = viewModel()
        vm.onServerUrlChange("https://api.example.com")
        vm.onDeviceTokenChange(TOKEN)

        vm.pair()

        assertTrue(vm.state.value.error!!.contains("wasn't recognised"))
        assertFalse(vm.state.value.busy)
        assertNull(store.pairing.first())
    }

    @Test
    fun `editing a field clears the previous error`() {
        api.error = DriverApiException(0, "offline")
        val vm = viewModel()
        vm.onServerUrlChange("https://api.example.com")
        vm.onDeviceTokenChange(TOKEN)
        vm.pair()
        assertNotNull(vm.state.value.error)

        vm.onDeviceTokenChange(TOKEN)

        assertNull(vm.state.value.error)
    }
}
