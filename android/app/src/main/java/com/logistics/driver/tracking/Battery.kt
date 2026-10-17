package com.logistics.driver.tracking

import android.content.Context
import android.content.Intent
import android.os.PowerManager
import android.provider.Settings

/** Whether Android's battery optimisation may stop the service while the screen is off. */
fun isBatteryOptimised(context: Context): Boolean {
    val power = context.getSystemService(PowerManager::class.java) ?: return false
    return !power.isIgnoringBatteryOptimizations(context.packageName)
}

/**
 * The battery-optimisation list, where the driver picks the app and chooses
 * "Don't optimise". Not ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS: that
 * direct prompt is meant for a short list of app types.
 */
fun batterySettingsIntent(): Intent = Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)
