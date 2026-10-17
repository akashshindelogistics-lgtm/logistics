package com.logistics.driver.tracking

import android.Manifest
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.location.Location
import android.os.Build
import android.os.IBinder
import android.os.Looper
import android.util.Log
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import com.google.android.gms.location.LocationCallback
import com.google.android.gms.location.LocationRequest
import com.google.android.gms.location.LocationResult
import com.google.android.gms.location.LocationServices
import com.google.android.gms.location.Priority
import com.logistics.driver.Config
import com.logistics.driver.DriverApplication
import com.logistics.driver.domain.Fix
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/**
 * Foreground service (type `location`) that requests a fix every
 * [Config.LOCATION_INTERVAL_MS] or [Config.LOCATION_MIN_DISTANCE_M], queues
 * it and schedules an upload. Only started from the status screen while the
 * app is visible, which is what lets it keep receiving fixes with the screen
 * off on just the "while in use" location permission.
 */
class LocationService : Service() {
    private val container get() = (application as DriverApplication).container
    private val fused by lazy { LocationServices.getFusedLocationProviderClient(this) }
    private var updatesRequested = false

    private val callback = object : LocationCallback() {
        override fun onLocationResult(result: LocationResult) {
            result.locations.forEach(::onLocation)
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            // The notification's Stop action: same as switching sharing off.
            container.appScope.launch { container.statusStore.setSharing(false) }
            stopSelf()
            return START_NOT_STICKY
        }
        if (!hasLocationPermission(this)) {
            stopSelf()
            return START_NOT_STICKY
        }

        try {
            ServiceCompat.startForeground(
                this,
                Notifications.SHARING_ID,
                Notifications.sharing(this),
                if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION else 0,
            )
        } catch (e: RuntimeException) {
            // Android 12+ refuses a location foreground service started from
            // the background, which is what a START_STICKY restart is.
            Log.w(TAG, "Couldn't start in the foreground; asking the driver to resume", e)
            Notifications.showResume(this)
            stopSelf()
            return START_NOT_STICKY
        }
        Notifications.cancelResume(this)

        if (!updatesRequested) {
            // High accuracy, not balanced: balanced never turns GPS on and
            // relies on Wi-Fi and cell positioning, which on a highway is off
            // by hundreds of metres or more. One fix per 30 s / 50 m keeps
            // the battery cost modest.
            val request = LocationRequest.Builder(Priority.PRIORITY_HIGH_ACCURACY, Config.LOCATION_INTERVAL_MS)
                .setMinUpdateDistanceMeters(Config.LOCATION_MIN_DISTANCE_M)
                .build()
            try {
                fused.requestLocationUpdates(request, callback, Looper.getMainLooper())
                updatesRequested = true
            } catch (e: SecurityException) {
                Log.w(TAG, "Location permission was revoked", e)
                stopSelf()
                return START_NOT_STICKY
            }
        }
        _running.value = true

        if (intent == null) {
            // A sticky restart. If the driver switched sharing off in the
            // meantime, don't carry on.
            container.appScope.launch {
                if (!container.statusStore.status.first().sharingOn) stopSelf()
            }
        }
        return START_STICKY
    }

    private fun onLocation(location: Location) {
        val fix = Fix(
            latitude = location.latitude,
            longitude = location.longitude,
            recordedAt = location.time / 1000,
            accuracyM = if (location.hasAccuracy()) location.accuracy.toDouble() else null,
            speedMps = if (location.hasSpeed()) location.speed.toDouble() else null,
        )
        // The app scope, not the service's lifetime: a fix that arrives just
        // before Stop is still queued.
        container.appScope.launch {
            if (container.locationReporter.ingest(fix)) {
                FlushWorker.schedule(applicationContext)
            }
        }
    }

    override fun onDestroy() {
        if (updatesRequested) fused.removeLocationUpdates(callback)
        _running.value = false
        super.onDestroy()
    }

    companion object {
        const val ACTION_STOP = "com.logistics.driver.action.STOP_SHARING"
        private const val TAG = "LocationService"

        private val _running = MutableStateFlow(false)

        /** Whether the service is running in this process right now. */
        val running: StateFlow<Boolean> = _running.asStateFlow()

        fun hasLocationPermission(context: Context): Boolean =
            listOf(Manifest.permission.ACCESS_FINE_LOCATION, Manifest.permission.ACCESS_COARSE_LOCATION).any {
                ContextCompat.checkSelfPermission(context, it) == PackageManager.PERMISSION_GRANTED
            }
    }
}
