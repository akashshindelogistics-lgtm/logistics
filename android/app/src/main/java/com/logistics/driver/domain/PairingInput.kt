package com.logistics.driver.domain

import java.net.URI
import java.net.URISyntaxException

/** Result of checking what the driver typed on the pairing screen. */
sealed interface InputResult<out T> {
    data class Valid<T>(val value: T) : InputResult<T>
    data class Invalid(val reason: String) : InputResult<Nothing>
}

/**
 * Normalise a server address: trim it, lower-case the scheme and drop
 * trailing slashes, so `"HTTPS://api.example.com/ "` becomes
 * `"https://api.example.com"` and the app can append `/api/...` to it.
 *
 * `http://` is only accepted when [allowCleartext] is true (debug builds),
 * because the device token must never cross the network unencrypted.
 */
fun parseServerAddress(input: String, allowCleartext: Boolean): InputResult<String> {
    val trimmed = input.trim().trimEnd('/')
    if (trimmed.isEmpty()) {
        return InputResult.Invalid("Enter the server address")
    }
    val uri = try {
        URI(trimmed)
    } catch (_: URISyntaxException) {
        return InputResult.Invalid("That isn't a valid web address")
    }
    val scheme = uri.scheme?.lowercase()
    if (scheme != "http" && scheme != "https") {
        return InputResult.Invalid("Enter the full address, starting with https://")
    }
    if (uri.host.isNullOrEmpty()) {
        return InputResult.Invalid("The address is missing the server name")
    }
    if (scheme == "http" && !allowCleartext) {
        return InputResult.Invalid("Use an https:// address; http:// isn't secure enough for the device token")
    }
    if (uri.query != null || uri.fragment != null) {
        return InputResult.Invalid("Enter just the server address, without ? or #")
    }
    return InputResult.Valid(scheme + trimmed.substring(scheme.length))
}

private val UUID_PATTERN =
    Regex("^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")

/**
 * A device token is the UUID returned by
 * `POST /api/drivers/{id}/device-token/rotate`. Accepts surrounding spaces
 * and a pasted `Bearer ` prefix; returns it lower-cased.
 */
fun parseDeviceToken(input: String): InputResult<String> {
    val token = input.trim().removePrefix("Bearer ").trim()
    if (token.isEmpty()) {
        return InputResult.Invalid("Enter the device token from your dispatcher")
    }
    if (!UUID_PATTERN.matches(token)) {
        return InputResult.Invalid("That doesn't look like a device token. It has 36 characters, like 1b4e28ba-2fa1-11d2-883f-0016d3cca427")
    }
    return InputResult.Valid(token.lowercase())
}
