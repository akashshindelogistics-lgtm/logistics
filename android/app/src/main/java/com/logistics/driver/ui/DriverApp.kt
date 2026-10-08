package com.logistics.driver.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Scaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.foundation.layout.padding
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import com.logistics.driver.data.Pairing
import com.logistics.driver.data.PairingRepository
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
fun DriverApp(repository: PairingRepository) {
    // Widened to Any? so the not-yet-loaded state can be the Loading marker.
    // Built once: a new Flow on every recomposition would reset collection.
    val pairingFlow = remember(repository) { repository.pairing.map<Pairing?, Any?> { it } }
    val state by pairingFlow.collectAsStateWithLifecycle(initialValue = Loading)

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
                    PairScreen(vm)
                }
                is Pairing -> key(current) {
                    val vm: StatusViewModel = viewModel(
                        key = current.serverUrl + current.deviceToken.hashCode(),
                        factory = viewModelFactory { initializer { StatusViewModel(repository, current) } },
                    )
                    StatusScreen(vm)
                }
            }
        }
    }
}
