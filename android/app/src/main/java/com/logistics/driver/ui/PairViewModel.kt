package com.logistics.driver.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.logistics.driver.BuildConfig
import com.logistics.driver.Config
import com.logistics.driver.data.PairingRepository
import com.logistics.driver.domain.InputResult
import com.logistics.driver.domain.parseDeviceToken
import com.logistics.driver.domain.parsePairingLink
import com.logistics.driver.domain.parseServerAddress
import com.logistics.driver.net.describeApiError
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class PairUiState(
    val serverUrl: String = if (BuildConfig.DEBUG) Config.DEBUG_DEFAULT_SERVER else "",
    val deviceToken: String = "",
    val serverUrlError: String? = null,
    val deviceTokenError: String? = null,
    /** Why the server refused the pairing (bad token, unreachable, ...). */
    val error: String? = null,
    val busy: Boolean = false,
    /** The fields were filled in from a pairing code, waiting for the driver to check them and tap Pair. */
    val fromCode: Boolean = false,
)

class PairViewModel(
    private val repository: PairingRepository,
    private val allowCleartext: Boolean = BuildConfig.DEBUG,
) : ViewModel() {
    private val _state = MutableStateFlow(PairUiState())
    val state: StateFlow<PairUiState> = _state.asStateFlow()

    fun onServerUrlChange(value: String) =
        _state.update { it.copy(serverUrl = value, serverUrlError = null, error = null) }

    /**
     * Fill in both fields from a scanned or opened pairing code. Never pairs
     * by itself: the driver sees which server the code points at and taps
     * Pair, so a stray link can't send the phone's location elsewhere.
     */
    fun applyPairingCode(text: String) {
        val link = parsePairingLink(text)
        if (link == null) {
            _state.update { it.copy(error = "That code isn't a pairing code. Scan the code on your driver page in the dashboard.", fromCode = false) }
            return
        }
        _state.update {
            it.copy(
                serverUrl = link.serverUrl,
                deviceToken = link.deviceToken,
                serverUrlError = null,
                deviceTokenError = null,
                error = null,
                fromCode = true,
            )
        }
    }

    /** The scanner couldn't run (no Google Play services, or it failed). */
    fun onScannerUnavailable() = _state.update {
        it.copy(error = "The QR scanner isn't available on this phone. Type the server address and device token instead.")
    }

    fun onDeviceTokenChange(value: String) =
        _state.update { it.copy(deviceToken = value, deviceTokenError = null, error = null) }

    /**
     * Validate both fields, then check the token with the server. On success
     * the repository saves the pairing and the app switches to the status
     * screen by itself; on failure the reason is shown and nothing is saved.
     */
    fun pair() {
        val current = _state.value
        if (current.busy) return
        val url = parseServerAddress(current.serverUrl, allowCleartext)
        val token = parseDeviceToken(current.deviceToken)
        if (url is InputResult.Invalid || token is InputResult.Invalid) {
            _state.update {
                it.copy(
                    serverUrlError = (url as? InputResult.Invalid)?.reason,
                    deviceTokenError = (token as? InputResult.Invalid)?.reason,
                )
            }
            return
        }
        url as InputResult.Valid
        token as InputResult.Valid

        _state.update { it.copy(busy = true, error = null) }
        viewModelScope.launch {
            try {
                repository.pair(url.value, token.value)
            } catch (e: Exception) {
                _state.update { it.copy(error = describeApiError(e)) }
            } finally {
                _state.update { it.copy(busy = false) }
            }
        }
    }
}
