package com.logistics.driver

import android.app.Application
import com.logistics.driver.tracking.Notifications

class DriverApplication : Application() {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        container = AppContainer(this)
        Notifications.createChannels(this)
    }
}
