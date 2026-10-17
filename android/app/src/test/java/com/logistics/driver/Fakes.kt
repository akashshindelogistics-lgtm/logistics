package com.logistics.driver

import com.logistics.driver.data.FixQueue
import com.logistics.driver.data.Pairing
import com.logistics.driver.data.PairingStore
import com.logistics.driver.data.QueuedFix
import com.logistics.driver.data.StatusStore
import com.logistics.driver.data.SyncStatus
import com.logistics.driver.domain.Fix
import com.logistics.driver.domain.FlushOutcome
import com.logistics.driver.net.DriverApi
import com.logistics.driver.net.DriverMe
import com.logistics.driver.net.LocationFixDto
import com.logistics.driver.net.LocationReportResult
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update

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

/**
 * Answers `me` with [result], or throws [error] if set; records each call.
 * `reportLocation` takes its answers from [reportResults], one per call, and
 * records each batch in [reported].
 */
class FakeDriverApi(
    var result: DriverMe = driverMe(),
    var error: Exception? = null,
) : DriverApi {
    val calls = mutableListOf<Pair<String, String>>()
    val reported = mutableListOf<List<LocationFixDto>>()
    val reportResults = ArrayDeque<Result<LocationReportResult>>()

    override suspend fun me(baseUrl: String, deviceToken: String): DriverMe {
        calls += baseUrl to deviceToken
        error?.let { throw it }
        return result
    }

    override suspend fun reportLocation(baseUrl: String, deviceToken: String, fixes: List<LocationFixDto>): LocationReportResult {
        reported += fixes
        return reportResults.removeFirstOrNull()?.getOrThrow() ?: reportOk()
    }
}

fun reportOk(updated: Boolean = true) = LocationReportResult(
    accepted = 1,
    locationUpdated = updated,
    vehicleRegistrationNumber = "MH12AB1234",
)

fun driverMe(name: String = "Ramesh") = DriverMe(
    driverId = "d1",
    name = name,
    orgId = "o1",
    orgName = "Mauli Brickvision",
    isActive = true,
    vehicle = null,
)

const val TOKEN = "1b4e28ba-2fa1-11d2-883f-0016d3cca427"

/** In-memory [FixQueue] with the same cap and ordering rules as the Room one. */
class FakeFixQueue : FixQueue {
    private var nextId = 1L
    val items = MutableStateFlow<List<QueuedFix>>(emptyList())
    override val size: Flow<Int> = items.map { it.size }

    override suspend fun enqueue(fix: Fix, cap: Int) {
        items.value = (items.value + QueuedFix.of(fix).copy(id = nextId++)).takeLast(cap)
    }
    override suspend fun oldest(limit: Int) = items.value.take(limit)
    override suspend fun remove(ids: List<Long>) {
        items.value = items.value.filterNot { it.id in ids }
    }
    override suspend fun clear() {
        items.value = emptyList()
    }
}

class FakeStatusStore : StatusStore {
    val current = MutableStateFlow(SyncStatus())
    override val status: Flow<SyncStatus> = current

    override suspend fun setSharing(on: Boolean) = current.update { it.copy(sharingOn = on) }
    override suspend fun recordFix(recordedAt: Long) = current.update { it.copy(lastFixAt = recordedAt) }
    override suspend fun recordSync(at: Long, outcome: FlushOutcome, message: String, vehicle: String?) =
        current.update { it.copy(lastSyncAt = at, lastOutcome = outcome, lastMessage = message, vehicle = vehicle ?: it.vehicle) }
    override suspend fun setBatteryPromptShown() = current.update { it.copy(batteryPromptShown = true) }
    override suspend fun clear() = current.update { SyncStatus(batteryPromptShown = it.batteryPromptShown) }
}
