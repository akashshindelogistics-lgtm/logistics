package com.logistics.driver.data

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import com.logistics.driver.domain.FlushOutcome
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

/** What the status screen shows about reporting, kept across restarts. */
data class SyncStatus(
    /** The driver turned "Share my location" on, and hasn't turned it off. */
    val sharingOn: Boolean = false,
    /** Unix seconds of the newest fix taken. */
    val lastFixAt: Long? = null,
    /** Unix millis of the last upload attempt. */
    val lastSyncAt: Long? = null,
    val lastOutcome: FlushOutcome? = null,
    val lastMessage: String? = null,
    /** Vehicle the last successful upload moved. */
    val vehicle: String? = null,
    /** The battery-optimisation explanation has been shown once already. */
    val batteryPromptShown: Boolean = false,
)

interface StatusStore {
    val status: Flow<SyncStatus>
    suspend fun setSharing(on: Boolean)
    suspend fun recordFix(recordedAt: Long)
    suspend fun recordSync(at: Long, outcome: FlushOutcome, message: String, vehicle: String? = null)
    suspend fun setBatteryPromptShown()
    /** Forget everything except [SyncStatus.batteryPromptShown] (a phone setting, not the driver's). */
    suspend fun clear()
}

class DataStoreStatusStore(private val dataStore: DataStore<Preferences>) : StatusStore {

    override val status: Flow<SyncStatus> = dataStore.data.map { prefs ->
        SyncStatus(
            sharingOn = prefs[SHARING_ON] ?: false,
            lastFixAt = prefs[LAST_FIX_AT],
            lastSyncAt = prefs[LAST_SYNC_AT],
            lastOutcome = prefs[LAST_OUTCOME]?.let { name -> FlushOutcome.entries.find { it.name == name } },
            lastMessage = prefs[LAST_MESSAGE],
            vehicle = prefs[VEHICLE],
            batteryPromptShown = prefs[BATTERY_PROMPT_SHOWN] ?: false,
        )
    }

    override suspend fun setSharing(on: Boolean) {
        dataStore.edit { it[SHARING_ON] = on }
    }

    override suspend fun recordFix(recordedAt: Long) {
        dataStore.edit { it[LAST_FIX_AT] = recordedAt }
    }

    override suspend fun recordSync(at: Long, outcome: FlushOutcome, message: String, vehicle: String?) {
        dataStore.edit { prefs ->
            prefs[LAST_SYNC_AT] = at
            prefs[LAST_OUTCOME] = outcome.name
            prefs[LAST_MESSAGE] = message
            vehicle?.let { prefs[VEHICLE] = it }
        }
    }

    override suspend fun setBatteryPromptShown() {
        dataStore.edit { it[BATTERY_PROMPT_SHOWN] = true }
    }

    override suspend fun clear() {
        dataStore.edit { prefs ->
            listOf(SHARING_ON, LAST_FIX_AT, LAST_SYNC_AT, LAST_OUTCOME, LAST_MESSAGE, VEHICLE).forEach { prefs.remove(it) }
        }
    }

    private companion object {
        val SHARING_ON = booleanPreferencesKey("sharing_on")
        val LAST_FIX_AT = longPreferencesKey("last_fix_at")
        val LAST_SYNC_AT = longPreferencesKey("last_sync_at")
        val LAST_OUTCOME = stringPreferencesKey("last_outcome")
        val LAST_MESSAGE = stringPreferencesKey("last_message")
        val VEHICLE = stringPreferencesKey("vehicle")
        val BATTERY_PROMPT_SHOWN = booleanPreferencesKey("battery_prompt_shown")
    }
}
