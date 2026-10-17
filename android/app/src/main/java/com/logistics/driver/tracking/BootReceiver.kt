package com.logistics.driver.tracking

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import com.logistics.driver.DriverApplication
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/**
 * After a reboot or an app update the service is gone. If the driver had
 * sharing on, post "Tap to resume" (the app can't restart a location service
 * from the background), and upload anything still queued.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED && intent.action != Intent.ACTION_MY_PACKAGE_REPLACED) return
        val container = (context.applicationContext as DriverApplication).container
        val pending = goAsync()
        container.appScope.launch {
            try {
                val paired = container.pairingRepository.pairing.first() != null
                if (paired && container.statusStore.status.first().sharingOn) {
                    Notifications.showResume(context)
                }
                if (paired) FlushWorker.schedule(context)
            } finally {
                pending.finish()
            }
        }
    }
}
