package com.logistics.driver.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.foundation.layout.padding
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.logistics.driver.AppContainer
import com.logistics.driver.data.Pairing
import kotlinx.coroutines.flow.map

/** Not yet known whether the phone is paired (DataStore hasn't loaded). */
private object Loading

/**
 * The whole app: the pairing screen until the phone is paired, the status
 * screen after. Two screens don't need a navigation library; the pairing
 * state decides.
 */
@OptIn(ExperimentalComposeUiApi::class)
@Composable
fun DriverApp(container: AppContainer) {
    val repository = container.pairingRepository
    // Widened to Any? so the not-yet-loaded state can be the Loading marker.
    // Built once: a new Flow on every recomposition would reset collection.
    val pairingFlow = remember(repository) { repository.pairing.map<Pairing?, Any?> { it } }
    val state by pairingFlow.collectAsStateWithLifecycle(initialValue = Loading)
    // A pairing link the app was opened with (MainActivity).
    val pendingLink by container.pendingPairingLink.collectAsStateWithLifecycle()

    // testTagsAsResourceId lets UI Automator (and the adb-driven demo) find
    // fields by their test tags.
    Scaffold(modifier = Modifier.semantics { testTagsAsResourceId = true }) { padding ->
        Box(Modifier.fillMaxSize().padding(padding)) {
            when (val current = state) {
                Loading -> CircularProgressIndicator(Modifier.align(Alignment.Center))
                null -> {
                    val vm: PairViewModel = viewModel(factory = viewModelFactory {
                        initializer { PairViewModel(repository) }
                    })
                    LaunchedEffect(pendingLink) {
                        pendingLink?.let {
                            vm.applyPairingCode(it)
                            container.pendingPairingLink.value = null
                        }
                    }
                    PairScreen(vm)
                }
                is Pairing -> key(current) {
                    val vm: StatusViewModel = viewModel(
                        key = current.serverUrl + current.deviceToken.hashCode(),
                        factory = viewModelFactory {
                            initializer {
                                StatusViewModel(
                                    repository,
                                    current,
                                    container.locationReporter,
                                    container.statusStore,
                                    container.tracking,
                                )
                            }
                        },
                    )
                    StatusScreen(vm)
                }
            }
            if (state is Pairing && pendingLink != null) {
                // Pairing again needs an unpair first, so a link can't swap
                // the pairing behind the driver's back.
                AlertDialog(
                    onDismissRequest = { container.pendingPairingLink.value = null },
                    title = { Text("This phone is already paired") },
                    text = { Text("To pair it with the new code, tap Unpair first, then scan or open the code again.") },
                    confirmButton = {
                        TextButton(
                            onClick = { container.pendingPairingLink.value = null },
                            modifier = Modifier.testTag("already-paired-ok"),
                        ) { Text("OK") }
                    },
                )
            }
        }
    }
}
