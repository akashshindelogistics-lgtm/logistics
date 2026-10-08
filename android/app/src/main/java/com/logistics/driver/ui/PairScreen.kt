package com.logistics.driver.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle

@Composable
fun PairScreen(vm: PairViewModel) {
    val state by vm.state.collectAsStateWithLifecycle()

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("Pair this phone", style = MaterialTheme.typography.headlineMedium)
        Text(
            "Your dispatcher gives you a device token and the server address. " +
                "Enter both to link this phone to you. While you share your location, " +
                "your assigned vehicle's position on the dispatch map follows this phone.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        OutlinedTextField(
            value = state.serverUrl,
            onValueChange = vm::onServerUrlChange,
            label = { Text("Server address") },
            placeholder = { Text("https://logistics.example.com") },
            isError = state.serverUrlError != null,
            supportingText = state.serverUrlError?.let { { Text(it) } },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Next),
            modifier = Modifier.fillMaxWidth().testTag("server-url"),
        )

        OutlinedTextField(
            value = state.deviceToken,
            onValueChange = vm::onDeviceTokenChange,
            label = { Text("Device token") },
            placeholder = { Text("1b4e28ba-2fa1-11d2-883f-0016d3cca427") },
            isError = state.deviceTokenError != null,
            supportingText = state.deviceTokenError?.let { { Text(it) } },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii, imeAction = ImeAction.Done),
            modifier = Modifier.fillMaxWidth().testTag("device-token"),
        )

        state.error?.let { message ->
            Card(
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer),
                modifier = Modifier.fillMaxWidth().testTag("pair-error"),
            ) {
                Text(
                    message,
                    color = MaterialTheme.colorScheme.onErrorContainer,
                    modifier = Modifier.padding(16.dp),
                )
            }
        }

        Button(
            onClick = vm::pair,
            enabled = !state.busy,
            modifier = Modifier.fillMaxWidth().testTag("pair-button"),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                if (state.busy) {
                    CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                }
                Text(if (state.busy) "Checking…" else "Pair")
            }
        }
    }
}
