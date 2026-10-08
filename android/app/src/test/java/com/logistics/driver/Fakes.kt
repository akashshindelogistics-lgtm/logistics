package com.logistics.driver

import com.logistics.driver.data.Pairing
import com.logistics.driver.data.PairingStore
import com.logistics.driver.net.DriverApi
import com.logistics.driver.net.DriverMe
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

class FakePairingStore : PairingStore {
    private val current = MutableStateFlow<Pairing?>(null)
    override val pairing: StateFlow<Pairing?> = current
    override suspend fun save(pairing: Pairing) {
        current.value = pairing
    }
    override suspend fun clear() {
        current.value = null
    }
}

/** Answers `me` with [result], or throws [error] if set; records each call. */
class FakeDriverApi(
    var result: DriverMe = driverMe(),
    var error: Exception? = null,
) : DriverApi {
    val calls = mutableListOf<Pair<String, String>>()
    override suspend fun me(baseUrl: String, deviceToken: String): DriverMe {
        calls += baseUrl to deviceToken
        error?.let { throw it }
        return result
    }
}

fun driverMe(name: String = "Ramesh") = DriverMe(
    driverId = "d1",
    name = name,
    orgId = "o1",
    orgName = "Mauli Brickvision",
    isActive = true,
    vehicle = null,
)

const val TOKEN = "1b4e28ba-2fa1-11d2-883f-0016d3cca427"
