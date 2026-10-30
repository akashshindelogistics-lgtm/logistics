package com.logistics.driver.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.logistics.driver.data.LocationReporter
import com.logistics.driver.data.Pairing
import com.logistics.driver.data.PairingRepository
import com.logistics.driver.data.StatusStore
import com.logistics.driver.data.SyncStatus
import com.logistics.driver.domain.FlushOutcome
import com.logistics.driver.net.DriverMe
import com.logistics.driver.net.describeApiError
import com.logistics.driver.tracking.TrackingControl
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class StatusUiState(
    val serverUrl: String,
    /** Latest `GET /api/driver/me`; kept when a later refresh fails. */
    val me: DriverMe? = null,
    val loading: Boolean = false,
    val error: String? = null,
    /** Reporting state saved by the service and uploads; `sync.sharingOn` is the switch. */
    val sync: SyncStatus = SyncStatus(),
    /** The location service is running in this process. */
    val serviceRunning: Boolean = false,
    val queuedFixes: Int = 0,
    val syncing: Boolean = false,
    /** Why sharing couldn't start (location permission refused). */
    val sharingError: String? = null,
)

class StatusViewModel(
    private val repository: PairingRepository,
    private val pairing: Pairing,
    private val reporter: LocationReporter,
    private val statusStore: StatusStore,
    private val tracking: TrackingControl,
) : ViewModel() {
    private val local = MutableStateFlow(StatusUiState(serverUrl = pairing.serverUrl))

    val state: StateFlow<StatusUiState> =
        combine(local, statusStore.status, reporter.queueSize, tracking.running) { s, sync, queued, running ->
            s.copy(sync = sync, queuedFixes = queued, serviceRunning = running)
        }.stateIn(viewModelScope, SharingStarted.Eagerly, local.value)

    init {
        refresh()
        // "Last position" comes from GET /api/driver/me, so ask again after
        // each upload that moved the vehicle; otherwise it says "No position
        // yet" while the sync row says the position was reported. drop(1)
        // skips the state stored from before this screen opened.
        statusStore.status
            .map { if (it.lastOutcome == FlushOutcome.OK) it.lastSyncAt else null }
            .distinctUntilChanged()
            .drop(1)
            .filterNotNull()
            .onEach { refresh() }
            .launchIn(viewModelScope)
    }

    fun refresh() {
        if (local.value.loading) return
        local.update { it.copy(loading = true, error = null) }
        viewModelScope.launch {
            try {
                val me = repository.whoAmI(pairing)
                local.update { it.copy(me = me) }
            } catch (e: Exception) {
                local.update { it.copy(error = describeApiError(e)) }
            } finally {
                local.update { it.copy(loading = false) }
            }
        }
    }

    /**
     * Turn sharing on and start the service. Call only once location
     * permission is granted, and from a tap: Android only lets a location
     * service start while the app is on screen.
     */
    fun startSharing() {
        local.update { it.copy(sharingError = null) }
        tracking.start()
        viewModelScope.launch { statusStore.setSharing(true) }
    }

    fun stopSharing() {
        tracking.stop()
        viewModelScope.launch { statusStore.setSharing(false) }
    }

    fun onLocationPermissionDenied() = local.update {
        it.copy(sharingError = "Location permission is needed to share your location. Allow it when asked, or in the phone's Settings > Apps > Logistics Driver > Permissions.")
    }

    fun onBatteryPromptShown() {
        viewModelScope.launch { statusStore.setBatteryPromptShown() }
    }

    /** Upload the queue now. If the network is the problem, WorkManager carries on when it's back. */
    fun syncNow() {
        if (local.value.syncing) return
        local.update { it.copy(syncing = true) }
        viewModelScope.launch {
            try {
                if (reporter.flush()?.retry == true) tracking.scheduleUpload()
            } finally {
                local.update { it.copy(syncing = false) }
            }
        }
    }

    /** Stop sharing, drop the queue (it belongs to this driver) and forget the pairing. */
    fun unpair() {
        tracking.stop()
        viewModelScope.launch {
            reporter.reset()
            repository.unpair()
        }
    }
}
