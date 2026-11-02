package com.logistics.driver.domain

import java.net.URI
import java.net.URLDecoder

/** What a pairing code from the dashboard carries. */
data class PairingLink(val serverUrl: String, val deviceToken: String)

const val PAIRING_LINK_SCHEME = "logisticsdriver"

/**
 * Read a pairing link, `logisticsdriver://pair?server=<address>&token=<token>`,
 * as the dashboard's QR code holds it (frontend/src/lib/pairingLink.ts).
 * Returns `null` for anything else. The values are only checked later, by the
 * same rules as typed ones, when the driver taps Pair.
 */
fun parsePairingLink(text: String): PairingLink? {
    val uri = runCatching { URI(text.trim()) }.getOrNull() ?: return null
    if (!uri.scheme.equals(PAIRING_LINK_SCHEME, ignoreCase = true) || !uri.host.equals("pair", ignoreCase = true)) {
        return null
    }
    val params = uri.rawQuery.orEmpty().split('&').mapNotNull { pair ->
        val parts = pair.split('=', limit = 2)
        if (parts.size != 2) return@mapNotNull null
        // The String overload: the Charset one needs API 33.
        URLDecoder.decode(parts[0], "UTF-8") to URLDecoder.decode(parts[1], "UTF-8")
    }.toMap()
    val server = params["server"]?.trim().takeUnless { it.isNullOrEmpty() } ?: return null
    val token = params["token"]?.trim().takeUnless { it.isNullOrEmpty() } ?: return null
    return PairingLink(server, token)
}
