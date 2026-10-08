package com.logistics.driver

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.logistics.driver.ui.DriverApp
import com.logistics.driver.ui.theme.DriverTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val container = (application as DriverApplication).container
        setContent {
            DriverTheme {
                DriverApp(container.pairingRepository)
            }
        }
    }
}
