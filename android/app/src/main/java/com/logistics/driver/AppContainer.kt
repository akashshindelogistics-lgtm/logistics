package com.logistics.driver

import android.content.Context
import androidx.datastore.preferences.preferencesDataStore
import com.logistics.driver.data.AppDb
import com.logistics.driver.data.DataStorePairingStore
import com.logistics.driver.data.DataStoreStatusStore
import com.logistics.driver.data.KeystoreTokenCipher
import com.logistics.driver.data.LocationReporter
import com.logistics.driver.data.PairingRepository
import com.logistics.driver.data.RoomFixQueue
import com.logistics.driver.net.OkHttpDriverApi
import com.logistics.driver.tracking.ServiceTrackingControl
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.serialization.json.Json
import okhttp3.OkHttpClient
import java.util.concurrent.TimeUnit

private val Context.driverPrefs by preferencesDataStore(name = "driver")
private val Context.statusPrefs by preferencesDataStore(name = "status")

/** Manual dependency wiring; the app is too small for a DI framework. */
class AppContainer(context: Context) {
    private val appContext = context.applicationContext
    private val json = Json { ignoreUnknownKeys = true }

    // No logging interceptor: the Authorization header carries the device token.
    private val httpClient = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(20, TimeUnit.SECONDS)
        .build()

    private val api = OkHttpDriverApi(httpClient, json)
    private val pairingStore = DataStorePairingStore(appContext.driverPrefs, KeystoreTokenCipher())

    /** Lives as long as the process; for work that must finish even if the service stops. */
    val appScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    val pairingRepository = PairingRepository(store = pairingStore, api = api)

    val statusStore = DataStoreStatusStore(appContext.statusPrefs)

    val locationReporter = LocationReporter(
        pairingStore = pairingStore,
        queue = RoomFixQueue(AppDb.open(appContext).fixDao()),
        status = statusStore,
        api = api,
    )

    val tracking = ServiceTrackingControl(appContext)
}
