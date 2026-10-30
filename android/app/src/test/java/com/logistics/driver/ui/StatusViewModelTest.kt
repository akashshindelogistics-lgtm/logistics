package com.logistics.driver.ui

import com.logistics.driver.FakeDriverApi
import com.logistics.driver.FakeFixQueue
import com.logistics.driver.FakePairingStore
import com.logistics.driver.FakeStatusStore
import com.logistics.driver.TOKEN
import com.logistics.driver.data.LocationReporter
import com.logistics.driver.data.Pairing
import com.logistics.driver.data.PairingRepository
import com.logistics.driver.data.SyncStatus
import com.logistics.driver.domain.FlushOutcome
import com.logistics.driver.tracking.TrackingControl
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class StatusViewModelTest {
    private val pairing = Pairing("https://api.example.com", TOKEN)
    private val pairingStore = FakePairingStore()
    private val api = FakeDriverApi()
    private val status = FakeStatusStore()
    private val tracking = object : TrackingControl {
        override val running = MutableStateFlow(false)
        override fun start() = Unit
        override fun stop() = Unit
        override fun scheduleUpload() = Unit
    }

    @Before
    fun setUp() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After
    fun tearDown() = Dispatchers.resetMain()

    private fun viewModel() = StatusViewModel(
        PairingRepository(pairingStore, api),
        pairing,
        LocationReporter(pairingStore, FakeFixQueue(), status, api),
        status,
        tracking,
    )

    @Test
    fun `driver info is fetched again after each upload that moved the vehicle`() = runTest {
        // An upload from before the screen opened doesn't count.
        status.current.value = SyncStatus(lastSyncAt = 1, lastOutcome = FlushOutcome.OK)
        viewModel()
        assertEquals(1, api.calls.size)

        status.recordSync(2, FlushOutcome.OK, "Reported")
        assertEquals(2, api.calls.size)

        status.recordSync(3, FlushOutcome.NETWORK_ERROR, "Offline")
        status.recordSync(4, FlushOutcome.STALE, "Already newer")
        assertEquals(2, api.calls.size)

        status.recordSync(5, FlushOutcome.OK, "Reported")
        assertEquals(3, api.calls.size)
    }

    @Test
    fun `the first upload after pairing refreshes too`() = runTest {
        viewModel()
        assertEquals(1, api.calls.size)

        status.recordSync(1, FlushOutcome.OK, "Reported")

        assertEquals(2, api.calls.size)
    }
}
