package com.logistics.driver.domain

import com.logistics.driver.net.AppRelease

/** A newer release of the app than the one installed, ready to download. */
data class AppUpdate(val versionName: String, val sizeBytes: Long, val downloadUrl: String)

/**
 * The update to offer, or `null` when [release] isn't newer than the
 * installed [installedVersionCode] (Android won't install an older or equal
 * `versionCode` over it anyway). [serverUrl] has no trailing slash.
 */
fun appUpdate(release: AppRelease?, installedVersionCode: Int, serverUrl: String): AppUpdate? {
    if (release == null || release.versionCode <= installedVersionCode) return null
    return AppUpdate(release.versionName, release.sizeBytes, serverUrl + release.downloadPath)
}

/** "12.3 MB": the download size, for drivers on mobile data. */
fun formatMegabytes(bytes: Long): String = "%.1f MB".format(java.util.Locale.ROOT, bytes / 1_000_000.0)
