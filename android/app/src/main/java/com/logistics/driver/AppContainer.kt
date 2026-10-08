package com.logistics.driver

import android.content.Context
import androidx.datastore.preferences.preferencesDataStore
import com.logistics.driver.data.DataStorePairingStore
import com.logistics.driver.data.KeystoreTokenCipher
import com.logistics.driver.data.PairingRepository
import com.logistics.driver.net.OkHttpDriverApi
import kotlinx.serialization.json.Json
import okhttp3.OkHttpClient
import java.util.concurrent.TimeUnit

private val Context.driverPrefs by preferencesDataStore(name = "driver")

/** Manual dependency wiring; the app is too small for a DI framework. */
class AppContainer(context: Context) {
    private val json = Json { ignoreUnknownKeys = true }

    // No logging interceptor: the Authorization header carries the device token.
    private val httpClient = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(20, TimeUnit.SECONDS)
        .build()

    val pairingRepository = PairingRepository(
        store = DataStorePairingStore(context.applicationContext.driverPrefs, KeystoreTokenCipher()),
        api = OkHttpDriverApi(httpClient, json),
    )
}
