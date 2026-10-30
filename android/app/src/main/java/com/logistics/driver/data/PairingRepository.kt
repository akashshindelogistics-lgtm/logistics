package com.logistics.driver.data

import com.logistics.driver.net.DriverApi
import com.logistics.driver.net.DriverMe
import kotlinx.coroutines.flow.Flow

/**
 * Pairing the phone with a driver. A pairing is only saved after
 * `GET /api/driver/me` accepts the token, so a mistyped token or server
 * address is caught on the pairing screen instead of at the first upload.
 */
class PairingRepository(
    private val store: PairingStore,
    private val api: DriverApi,
) {
    val pairing: Flow<Pairing?> = store.pairing

    /** Check the token against the server, then save it. Throws on failure; nothing is saved then. */
    suspend fun pair(serverUrl: String, deviceToken: String): DriverMe {
        val me = api.me(serverUrl, deviceToken)
        store.save(Pairing(serverUrl, deviceToken))
        return me
    }

    /** Who the saved pairing belongs to, fresh from the server. */
    suspend fun whoAmI(pairing: Pairing): DriverMe = api.me(pairing.serverUrl, pairing.deviceToken)

    suspend fun unpair() = store.clear()
}
