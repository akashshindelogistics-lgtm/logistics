package com.logistics.driver.tracking

import android.content.Context
import android.content.Intent
import androidx.core.content.ContextCompat
import kotlinx.coroutines.flow.StateFlow

/** Starting and stopping the location service, behind an interface so view models can be tested. */
interface TrackingControl {
    val running: StateFlow<Boolean>
    fun start()
    fun stop()
    /** Upload the queue as soon as there's a network connection. */
    fun scheduleUpload()
}

class ServiceTrackingControl(private val context: Context) : TrackingControl {
    override val running: StateFlow<Boolean> = LocationService.running

    override fun start() {
        ContextCompat.startForegroundService(context, Intent(context, LocationService::class.java))
    }

    override fun stop() {
        context.stopService(Intent(context, LocationService::class.java))
    }

    override fun scheduleUpload() = FlushWorker.schedule(context, replace = true)
}
