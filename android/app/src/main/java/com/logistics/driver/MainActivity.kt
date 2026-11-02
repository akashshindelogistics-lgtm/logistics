package com.logistics.driver

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.logistics.driver.domain.PAIRING_LINK_SCHEME
import com.logistics.driver.tracking.LocationService
import com.logistics.driver.tracking.Notifications
import com.logistics.driver.ui.DriverApp
import com.logistics.driver.ui.theme.DriverTheme

class MainActivity : ComponentActivity() {
    private val container get() = (application as DriverApplication).container

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            DriverTheme {
                DriverApp(container)
            }
        }
        if (savedInstanceState == null) {
            handleResume(intent)
            handlePairingLink(intent)
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleResume(intent)
        handlePairingLink(intent)
    }

    /** Opened from a pairing link (the dashboard's QR code read by the camera, or a shared link). */
    private fun handlePairingLink(intent: Intent?) {
        val data = intent?.data ?: return
        if (intent.action == Intent.ACTION_VIEW && data.scheme == PAIRING_LINK_SCHEME) {
            container.pendingPairingLink.value = data.toString()
        }
    }

    /**
     * The "Tap to resume sharing" notification opened the app. The app is on
     * screen now, so Android allows the location service to start.
     */
    private fun handleResume(intent: Intent?) {
        if (intent?.getBooleanExtra(EXTRA_RESUME_SHARING, false) != true) return
        Notifications.cancelResume(this)
        if (LocationService.hasLocationPermission(this)) container.tracking.start()
    }

    companion object {
        const val EXTRA_RESUME_SHARING = "resume_sharing"
    }
}
