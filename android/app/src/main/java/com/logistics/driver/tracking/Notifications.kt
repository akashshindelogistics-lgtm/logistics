package com.logistics.driver.tracking

import android.Manifest
import android.app.Notification
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import com.logistics.driver.MainActivity
import com.logistics.driver.R

/** The two notifications the app posts: "sharing location" and "tap to resume". */
object Notifications {
    private const val SHARING_CHANNEL = "sharing"
    private const val RESUME_CHANNEL = "resume"
    const val SHARING_ID = 1
    private const val RESUME_ID = 2

    fun createChannels(context: Context) {
        val manager = NotificationManagerCompat.from(context)
        manager.createNotificationChannel(
            // Low: always on screen while sharing, so it mustn't make a sound.
            NotificationChannelCompat.Builder(SHARING_CHANNEL, NotificationManagerCompat.IMPORTANCE_LOW)
                .setName("Location sharing")
                .setDescription("Shown while your location is being shared")
                .build(),
        )
        manager.createNotificationChannel(
            NotificationChannelCompat.Builder(RESUME_CHANNEL, NotificationManagerCompat.IMPORTANCE_HIGH)
                .setName("Sharing stopped")
                .setDescription("When location sharing stopped and needs you to resume it")
                .build(),
        )
    }

    /** The foreground service's notification, with a Stop action. */
    fun sharing(context: Context): Notification {
        val stop = PendingIntent.getService(
            context,
            0,
            Intent(context, LocationService::class.java).setAction(LocationService.ACTION_STOP),
            PendingIntent.FLAG_IMMUTABLE,
        )
        return NotificationCompat.Builder(context, SHARING_CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("Sharing your location")
            .setContentText("Your vehicle's position on the dispatch map follows this phone.")
            .setContentIntent(openApp(context, resume = false))
            .addAction(0, "Stop", stop)
            .setOngoing(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .build()
    }

    /**
     * Sharing was on but the service isn't running (after a reboot, an app
     * update, or the OS killing it). Android doesn't let the app restart a
     * location service from the background, so the driver taps to resume.
     */
    fun showResume(context: Context) {
        if (Build.VERSION.SDK_INT >= 33 &&
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            return
        }
        val notification = NotificationCompat.Builder(context, RESUME_CHANNEL)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("Location sharing stopped")
            .setContentText("Tap to resume sharing your location.")
            .setContentIntent(openApp(context, resume = true))
            .setAutoCancel(true)
            .setCategory(NotificationCompat.CATEGORY_REMINDER)
            .build()
        NotificationManagerCompat.from(context).notify(RESUME_ID, notification)
    }

    fun cancelResume(context: Context) = NotificationManagerCompat.from(context).cancel(RESUME_ID)

    private fun openApp(context: Context, resume: Boolean): PendingIntent {
        val intent = Intent(context, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP)
            .putExtra(MainActivity.EXTRA_RESUME_SHARING, resume)
        // Distinct request codes, or the two PendingIntents would share extras.
        return PendingIntent.getActivity(context, if (resume) 1 else 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
    }
}
