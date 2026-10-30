package com.logistics.driver.data

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

/** What the phone is paired with. Only ever held decrypted in memory. */
data class Pairing(val serverUrl: String, val deviceToken: String)

interface PairingStore {
    /** The current pairing, or `null` when the phone isn't paired. */
    val pairing: Flow<Pairing?>
    suspend fun save(pairing: Pairing)
    suspend fun clear()
}

/** Keeps the server address in DataStore and the token encrypted by [cipher]. */
class DataStorePairingStore(
    private val dataStore: DataStore<Preferences>,
    private val cipher: TokenCipher,
) : PairingStore {

    override val pairing: Flow<Pairing?> = dataStore.data.map { prefs ->
        val url = prefs[SERVER_URL] ?: return@map null
        val encrypted = prefs[DEVICE_TOKEN] ?: return@map null
        // A token that no longer decrypts (the Keystore key was wiped, e.g.
        // by a security reset) is treated as unpaired: the driver pairs again.
        runCatching { Pairing(url, cipher.decrypt(encrypted)) }.getOrNull()
    }

    override suspend fun save(pairing: Pairing) {
        val encrypted = cipher.encrypt(pairing.deviceToken)
        dataStore.edit { prefs ->
            prefs[SERVER_URL] = pairing.serverUrl
            prefs[DEVICE_TOKEN] = encrypted
        }
    }

    override suspend fun clear() {
        dataStore.edit { prefs ->
            prefs.remove(SERVER_URL)
            prefs.remove(DEVICE_TOKEN)
        }
    }

    private companion object {
        val SERVER_URL = stringPreferencesKey("server_url")
        val DEVICE_TOKEN = stringPreferencesKey("device_token_encrypted")
    }
}
