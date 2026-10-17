package com.logistics.driver.ui

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.logistics.driver.BuildConfig
import com.logistics.driver.domain.brandBatterySteps
import com.logistics.driver.net.DriverMe
import com.logistics.driver.tracking.LocationService
import com.logistics.driver.tracking.batterySettingsIntent
import com.logistics.driver.tracking.isBatteryOptimised
import java.text.DateFormat
import java.util.Date

/** Location, plus notifications on Android 13+ so the "sharing" notification shows. */
private val SHARING_PERMISSIONS = buildList {
    add(Manifest.permission.ACCESS_FINE_LOCATION)
    add(Manifest.permission.ACCESS_COARSE_LOCATION)
    if (Build.VERSION.SDK_INT >= 33) add(Manifest.permission.POST_NOTIFICATIONS)
}.toTypedArray()

@Composable
fun StatusScreen(vm: StatusViewModel) {
    val state by vm.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    var confirmUnpair by remember { mutableStateOf(false) }
    var showBatteryDialog by remember { mutableStateOf(false) }

    // Re-checked on every return to the app, e.g. from the battery settings page.
    var batteryOptimised by remember { mutableStateOf(isBatteryOptimised(context)) }
    LifecycleResumeEffect(Unit) {
        batteryOptimised = isBatteryOptimised(context)
        onPauseOrDispose { }
    }

    fun beginSharing() {
        vm.startSharing()
        if (batteryOptimised && !state.sync.batteryPromptShown) {
            showBatteryDialog = true
            vm.onBatteryPromptShown()
        }
    }

    val permissionLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
        // Notifications are optional; only location decides.
        if (LocationService.hasLocationPermission(context)) beginSharing() else vm.onLocationPermissionDenied()
    }

    fun requestSharing() {
        val missing = SHARING_PERMISSIONS.any { context.checkSelfPermission(it) != PackageManager.PERMISSION_GRANTED }
        if (missing) permissionLauncher.launch(SHARING_PERMISSIONS) else beginSharing()
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        if (state.loading) {
            LinearProgressIndicator(Modifier.fillMaxWidth())
        }

        val me = state.me
        Text(
            me?.name ?: "Paired",
            style = MaterialTheme.typography.headlineMedium,
            modifier = Modifier.testTag("driver-name"),
        )
        me?.let {
            Text(it.orgName, style = MaterialTheme.typography.titleMedium, modifier = Modifier.testTag("org-name"))
        }

        state.error?.let { ErrorCard(it, Modifier.testTag("status-error")) }

        me?.let { DriverCard(it) }

        SharingCard(
            state = state,
            // Turning sharing off is always allowed, even if the driver has
            // since been made inactive or lost the vehicle.
            canShare = (me?.isActive == true && me.vehicle != null) || state.sync.sharingOn,
            onChange = { on -> if (on) requestSharing() else vm.stopSharing() },
            onResume = ::requestSharing,
        )

        if (state.sync.sharingOn && batteryOptimised) {
            BatteryCard(onOpenSettings = { context.startActivity(batterySettingsIntent()) })
        }

        SyncCard(state, onSyncNow = vm::syncNow)

        Card(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                InfoRow("Server", state.serverUrl)
                InfoRow("App version", BuildConfig.VERSION_NAME)
            }
        }

        Row(horizontalArrangement = Arrangement.spacedBy(12.dp), modifier = Modifier.fillMaxWidth()) {
            OutlinedButton(
                onClick = vm::refresh,
                enabled = !state.loading,
                modifier = Modifier.weight(1f).testTag("refresh-button"),
            ) { Text("Refresh") }
            OutlinedButton(
                onClick = { confirmUnpair = true },
                modifier = Modifier.weight(1f).testTag("unpair-button"),
            ) { Text("Unpair") }
        }
    }

    if (confirmUnpair) {
        AlertDialog(
            onDismissRequest = { confirmUnpair = false },
            title = { Text("Unpair this phone?") },
            text = {
                Text(
                    "The phone stops sharing your location and is no longer linked to you. " +
                        "Fixes not yet uploaded are discarded. To pair again you need a new device token from your dispatcher.",
                )
            },
            confirmButton = {
                TextButton(
                    onClick = { confirmUnpair = false; vm.unpair() },
                    modifier = Modifier.testTag("confirm-unpair"),
                ) { Text("Unpair") }
            },
            dismissButton = { TextButton(onClick = { confirmUnpair = false }) { Text("Cancel") } },
        )
    }

    if (showBatteryDialog) {
        AlertDialog(
            onDismissRequest = { showBatteryDialog = false },
            title = { Text("Keep sharing with the screen off") },
            text = { Text(batteryAdvice()) },
            confirmButton = {
                TextButton(
                    onClick = { showBatteryDialog = false; context.startActivity(batterySettingsIntent()) },
                    modifier = Modifier.testTag("battery-open-settings"),
                ) { Text("Open settings") }
            },
            dismissButton = { TextButton(onClick = { showBatteryDialog = false }) { Text("Later") } },
        )
    }
}

private fun batteryAdvice(): String {
    val standard = "Your phone may stop location sharing to save battery. In the list that opens, " +
        "choose All apps, then Logistics Driver, then \"Don't optimise\"."
    return brandBatterySteps(Build.MANUFACTURER)?.let { "$standard\n\n$it" } ?: standard
}

@Composable
private fun DriverCard(me: DriverMe) {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            InfoRow("Status", if (me.isActive) "Active" else "Inactive", Modifier.testTag("driver-status"))
            if (!me.isActive) {
                Hint("Your dispatcher has marked you inactive, so your location can't be shared.")
            }
            val vehicle = me.vehicle
            InfoRow("Vehicle", vehicle?.registrationNumber ?: "None assigned", Modifier.testTag("vehicle"))
            if (vehicle == null) {
                Hint("Ask your dispatcher to assign you a vehicle before you share your location.")
            } else {
                InfoRow("Last position", vehicle.location?.let { formatTime(it.timestamp * 1000) } ?: "No position yet")
            }
        }
    }
}

@Composable
private fun SharingCard(state: StatusUiState, canShare: Boolean, onChange: (Boolean) -> Unit, onResume: () -> Unit) {
    val on = state.sync.sharingOn
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Share my location", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                Switch(
                    checked = on,
                    onCheckedChange = onChange,
                    enabled = canShare,
                    modifier = Modifier.testTag("sharing-switch"),
                )
            }
            when {
                on && !state.serviceRunning -> {
                    Text(
                        "Sharing is switched on but stopped, for example after the phone restarted.",
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.testTag("sharing-stopped"),
                    )
                    Button(onClick = onResume, modifier = Modifier.testTag("resume-sharing")) { Text("Resume sharing") }
                }
                on -> Hint("Your position is sent about every 30 seconds while you move, also with the screen off.")
                else -> Hint("Switch on at the start of your shift, and off at the end.")
            }
            state.sharingError?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.testTag("sharing-error")) }
        }
    }
}

@Composable
private fun BatteryCard(onOpenSettings: () -> Unit) {
    Card(
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.secondaryContainer),
        modifier = Modifier.fillMaxWidth().testTag("battery-card"),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Battery saver may stop sharing", style = MaterialTheme.typography.titleSmall)
            Text(batteryAdvice(), style = MaterialTheme.typography.bodySmall)
            OutlinedButton(onClick = onOpenSettings) { Text("Open battery settings") }
        }
    }
}

@Composable
private fun SyncCard(state: StatusUiState, onSyncNow: () -> Unit) {
    val sync = state.sync
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            InfoRow("Queued fixes", state.queuedFixes.toString(), Modifier.testTag("queued-fixes"))
            InfoRow("Last fix", sync.lastFixAt?.let { formatTime(it * 1000) } ?: "Not yet", Modifier.testTag("last-fix"))
            InfoRow("Last sync", sync.lastSyncAt?.let(::formatTime) ?: "Not yet", Modifier.testTag("last-sync"))
            sync.lastMessage?.let { message ->
                val failed = sync.lastOutcome?.isSuccess == false
                Text(
                    message,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.testTag("sync-message"),
                )
            }
            OutlinedButton(
                onClick = onSyncNow,
                enabled = !state.syncing && state.queuedFixes > 0,
                modifier = Modifier.testTag("sync-now"),
            ) { Text(if (state.syncing) "Syncing…" else "Sync now") }
        }
    }
}

@Composable
private fun ErrorCard(message: String, modifier: Modifier = Modifier) {
    Card(
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer),
        modifier = modifier.fillMaxWidth(),
    ) {
        Text(message, color = MaterialTheme.colorScheme.onErrorContainer, modifier = Modifier.padding(16.dp))
    }
}

@Composable
private fun InfoRow(label: String, value: String, modifier: Modifier = Modifier) {
    Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Text(label, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f))
        Text(value, fontWeight = FontWeight.Medium)
    }
}

@Composable
private fun Hint(text: String) {
    Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

private fun formatTime(millis: Long): String =
    DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(millis))
