package com.logistics.driver.net

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException

/** The backend's response envelope: `{ success, message, data }`. */
@Serializable
data class ApiEnvelope<T>(
    val success: Boolean,
    val message: String,
    val data: T? = null,
)

/** `GET /api/driver/me`: who this phone is paired as. */
@Serializable
data class DriverMe(
    @SerialName("driver_id") val driverId: String,
    val name: String,
    @SerialName("org_id") val orgId: String,
    @SerialName("org_name") val orgName: String,
    /** `false`: location reports are refused until a dispatcher reactivates the driver. */
    @SerialName("is_active") val isActive: Boolean,
    /** `null`: no vehicle assigned, so location reports are refused. */
    val vehicle: DriverVehicle? = null,
)

@Serializable
data class DriverVehicle(
    @SerialName("registration_number") val registrationNumber: String,
    val location: VehicleLocation? = null,
)

@Serializable
data class VehicleLocation(
    val latitude: Double,
    val longitude: Double,
    /** Unix seconds. */
    val timestamp: Long,
    val address: String? = null,
)

/** One fix as `POST /api/driver/location` takes it. */
@Serializable
data class LocationFixDto(
    val latitude: Double,
    val longitude: Double,
    /** Unix seconds. */
    @SerialName("recorded_at") val recordedAt: Long,
    @SerialName("accuracy_m") val accuracyM: Double? = null,
    @SerialName("speed_mps") val speedMps: Double? = null,
)

@Serializable
data class LocationReport(val fixes: List<LocationFixDto>)

/** `POST /api/driver/location`: what the server did with a batch. */
@Serializable
data class LocationReportResult(
    val accepted: Int,
    /** `false`: the server already had a newer position, which is not an error. */
    @SerialName("location_updated") val locationUpdated: Boolean,
    @SerialName("vehicle_registration_number") val vehicleRegistrationNumber: String,
    val location: VehicleLocation? = null,
)

/**
 * A failed API call. [status] is the HTTP status, or [NETWORK_ERROR] when no
 * response arrived at all (offline, DNS, TLS, timeout).
 */
class DriverApiException(val status: Int, message: String) : Exception(message) {
    companion object {
        const val NETWORK_ERROR = 0
    }
}

/** The backend calls the app makes, all authenticated by the device token. */
interface DriverApi {
    suspend fun me(baseUrl: String, deviceToken: String): DriverMe

    /** Upload 1-[com.logistics.driver.Config.MAX_FIXES_PER_REQUEST] fixes, oldest first. */
    suspend fun reportLocation(baseUrl: String, deviceToken: String, fixes: List<LocationFixDto>): LocationReportResult
}

class OkHttpDriverApi(
    private val client: OkHttpClient,
    private val json: Json,
    private val io: CoroutineDispatcher = Dispatchers.IO,
) : DriverApi {

    override suspend fun me(baseUrl: String, deviceToken: String): DriverMe =
        call(Request.Builder().url("$baseUrl/api/driver/me").get(), deviceToken)

    override suspend fun reportLocation(
        baseUrl: String,
        deviceToken: String,
        fixes: List<LocationFixDto>,
    ): LocationReportResult {
        val body = json.encodeToString(LocationReport(fixes)).toRequestBody(JSON_MEDIA_TYPE)
        return call(Request.Builder().url("$baseUrl/api/driver/location").post(body), deviceToken)
    }

    /** Send [builder] with the device token and unwrap the `{ success, message, data }` envelope. */
    private suspend inline fun <reified T> call(builder: Request.Builder, deviceToken: String): T = withContext(io) {
        val request = builder.header("Authorization", "Bearer $deviceToken").build()

        val (status, body) = try {
            client.newCall(request).execute().use { it.code to it.body.string() }
        } catch (e: IOException) {
            throw DriverApiException(DriverApiException.NETWORK_ERROR, e.message ?: "Network request failed")
        } catch (e: IllegalArgumentException) {
            // OkHttp rejects a malformed URL before sending anything.
            throw DriverApiException(DriverApiException.NETWORK_ERROR, e.message ?: "Invalid server address")
        }

        val envelope = runCatching { json.decodeFromString<ApiEnvelope<T>>(body) }.getOrNull()
        if (status !in 200..299) {
            throw DriverApiException(status, envelope?.message ?: "HTTP $status")
        }
        envelope?.data ?: throw DriverApiException(status, envelope?.message ?: "Unexpected response from the server")
    }

    private companion object {
        val JSON_MEDIA_TYPE = "application/json".toMediaType()
    }
}

/** What to tell the driver when a call fails. */
fun describeApiError(e: Throwable): String = when {
    e !is DriverApiException -> e.message ?: "Something went wrong"
    e.status == DriverApiException.NETWORK_ERROR ->
        "Can't reach the server. Check the address and the phone's internet connection."
    e.status == 401 ->
        "This device token wasn't recognised. Ask your dispatcher for a new one."
    e.status == 404 ->
        "That address doesn't look like the logistics server. Check it with your dispatcher."
    else -> "The server returned an error (${e.status}): ${e.message}"
}
