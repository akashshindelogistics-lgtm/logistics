package com.logistics.driver.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.logistics.driver.BuildConfig
import com.logistics.driver.net.DriverMe
import java.text.DateFormat
import java.util.Date

@Composable
fun StatusScreen(vm: StatusViewModel) {
    val state by vm.state.collectAsStateWithLifecycle()
    var confirmUnpair by remember { mutableStateOf(false) }

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

        state.error?.let { message ->
            Card(
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer),
                modifier = Modifier.fillMaxWidth().testTag("status-error"),
            ) {
                Text(message, color = MaterialTheme.colorScheme.onErrorContainer, modifier = Modifier.padding(16.dp))
            }
        }

        me?.let { DriverCard(it) }

        SharingCard(
            sharing = state.sharingRequested,
            canShare = me?.isActive == true && me.vehicle != null,
            onChange = vm::onSharingChange,
        )

        Card(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                InfoRow("Queued fixes", "0")
                InfoRow("Last sync", "Not yet")
                HorizontalDivider()
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
            text = { Text("The phone stops being linked to you. To pair again you need a new device token from your dispatcher.") },
            confirmButton = {
                TextButton(
                    onClick = { confirmUnpair = false; vm.unpair() },
                    modifier = Modifier.testTag("confirm-unpair"),
                ) { Text("Unpair") }
            },
            dismissButton = { TextButton(onClick = { confirmUnpair = false }) { Text("Cancel") } },
        )
    }
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
                val located = vehicle.location?.let {
                    DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(it.timestamp * 1000))
                } ?: "No position yet"
                InfoRow("Last position", located)
            }
        }
    }
}

@Composable
private fun SharingCard(sharing: Boolean, canShare: Boolean, onChange: (Boolean) -> Unit) {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Share my location", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                Switch(
                    checked = sharing,
                    onCheckedChange = onChange,
                    enabled = canShare,
                    modifier = Modifier.testTag("sharing-switch"),
                )
            }
            Hint("Location reporting arrives in the next version of the app. This switch doesn't send anything yet.")
        }
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
