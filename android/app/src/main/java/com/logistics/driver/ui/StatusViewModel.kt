package com.logistics.driver.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.logistics.driver.data.Pairing
import com.logistics.driver.data.PairingRepository
import com.logistics.driver.net.DriverMe
import com.logistics.driver.net.describeApiError
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class StatusUiState(
    val serverUrl: String,
    /** Latest `GET /api/driver/me`; kept when a later refresh fails. */
    val me: DriverMe? = null,
    val loading: Boolean = false,
    val error: String? = null,
    /**
     * Phase 2 stub: the switch only records the driver's choice. Phase 3
     * starts and stops the location foreground service from it.
     */
    val sharingRequested: Boolean = false,
)

class StatusViewModel(
    private val repository: PairingRepository,
    private val pairing: Pairing,
) : ViewModel() {
    private val _state = MutableStateFlow(StatusUiState(serverUrl = pairing.serverUrl))
    val state: StateFlow<StatusUiState> = _state.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        if (_state.value.loading) return
        _state.update { it.copy(loading = true, error = null) }
        viewModelScope.launch {
            try {
                val me = repository.whoAmI(pairing)
                _state.update { it.copy(me = me) }
            } catch (e: Exception) {
                _state.update { it.copy(error = describeApiError(e)) }
            } finally {
                _state.update { it.copy(loading = false) }
            }
        }
    }

    fun onSharingChange(on: Boolean) = _state.update { it.copy(sharingRequested = on) }

    /** Forget the pairing; the app returns to the pairing screen. */
    fun unpair() {
        viewModelScope.launch { repository.unpair() }
    }
}
