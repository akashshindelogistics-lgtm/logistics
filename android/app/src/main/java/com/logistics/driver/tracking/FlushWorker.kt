package com.logistics.driver.tracking

import android.content.Context
import androidx.work.BackoffPolicy
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.logistics.driver.DriverApplication
import java.util.concurrent.TimeUnit

/**
 * Uploads the queue once the phone has a network connection. WorkManager
 * keeps the job across the service or the process being killed, and backs
 * off between retries.
 */
class FlushWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val outcome = (applicationContext as DriverApplication).container.locationReporter.flush()
        // 401/403/409/400 need the dispatcher, not a retry; they wait for the
        // next fix or Sync now.
        return if (outcome?.retry == true) Result.retry() else Result.success()
    }

    companion object {
        private const val NAME = "flush-queue"

        /**
         * Queue an upload. An upload already waiting (for the network, or in
         * backoff) is kept, so a fix every 30 s doesn't reset the backoff;
         * [replace] starts a fresh one instead, for Sync now.
         */
        fun schedule(context: Context, replace: Boolean = false) {
            val request = OneTimeWorkRequestBuilder<FlushWorker>()
                .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
                .build()
            WorkManager.getInstance(context).enqueueUniqueWork(
                NAME,
                if (replace) ExistingWorkPolicy.REPLACE else ExistingWorkPolicy.KEEP,
                request,
            )
        }
    }
}
