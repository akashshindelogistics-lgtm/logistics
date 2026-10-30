plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
    // Generates the Room database code.
    alias(libs.plugins.ksp)
}

android {
    namespace = "com.logistics.driver"
    compileSdk {
        version = release(37)
    }

    defaultConfig {
        // Same id the Expo app used; it was never published, so nothing is lost.
        applicationId = "com.logistics.driver"
        minSdk = 26
        // 36 = Android 16, what Google Play requires from 31 Aug 2026. We
        // sideload, but there's no reason to target lower.
        targetSdk = 36
        // Must go up on every release: Android only installs an APK over the
        // old one if versionCode is higher (see docs/android-driver-app.md).
        versionCode = 1
        versionName = "0.1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    // Release signing comes from the environment, so the keystore and its
    // passwords never enter git (scripts/build-release.sh sets these). Without
    // DRIVER_KEYSTORE, assembleRelease still builds, unsigned, as CI does.
    val keystore = providers.environmentVariable("DRIVER_KEYSTORE").orNull
    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = providers.environmentVariable("DRIVER_KEYSTORE_PASSWORD").get()
                keyAlias = providers.environmentVariable("DRIVER_KEY_ALIAS").getOrElse("driver")
                keyPassword = providers.environmentVariable("DRIVER_KEY_PASSWORD")
                    .orElse(providers.environmentVariable("DRIVER_KEYSTORE_PASSWORD")).get()
            }
        }
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.findByName("release")
            optimization {
                enable = true
                packageScope = setOf("androidx.**", "kotlin.**", "kotlinx.**")
            }
        }
    }

    buildFeatures {
        compose = true
        // BuildConfig.DEBUG decides whether a plain http:// server address is
        // allowed (debug builds only, for testing against a local backend).
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    testOptions {
        unitTests.isReturnDefaultValues = true
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.datastore.preferences)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.material3)
    implementation(libs.okhttp)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.androidx.room.runtime)
    implementation(libs.androidx.room.ktx)
    ksp(libs.androidx.room.compiler)
    implementation(libs.androidx.work.runtime.ktx)
    implementation(libs.play.services.location)
    debugImplementation(libs.androidx.compose.ui.tooling)

    testImplementation(libs.junit)
    testImplementation(libs.okhttp.mockwebserver)
    testImplementation(libs.kotlinx.coroutines.test)

    // Instrumented tests (src/androidTest), run on an emulator or phone with
    // ./gradlew connectedDebugAndroidTest.
    androidTestImplementation(libs.androidx.test.ext.junit)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.espresso.core)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.compose.ui.test.junit4)
    androidTestImplementation(libs.okhttp.mockwebserver)
    androidTestImplementation(libs.kotlinx.coroutines.test)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
}
