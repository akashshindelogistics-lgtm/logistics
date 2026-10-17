package com.logistics.driver.domain

/**
 * Extra steps, beyond Android's own battery-optimisation setting, that a
 * brand's phones need before they leave a background app alone. From
 * dontkillmyapp.com; menu names vary a little between OS versions.
 *
 * [manufacturer] is `Build.MANUFACTURER`. Returns `null` for brands that
 * only need the standard setting.
 */
fun brandBatterySteps(manufacturer: String): String? = when (manufacturer.trim().lowercase()) {
    "xiaomi", "redmi", "poco" ->
        "On Xiaomi phones also open Settings > Apps > Manage apps > Logistics Driver, turn on Autostart, " +
            "and set Battery saver to \"No restrictions\"."
    "vivo", "iqoo" ->
        "On Vivo phones also open Settings > Battery > Background power consumption management, " +
            "and allow Logistics Driver high background power use."
    "oppo", "realme", "oneplus" ->
        "Also open Settings > Apps > Logistics Driver > Battery usage, and turn on \"Allow background activity\" " +
            "and \"Allow auto launch\"."
    "samsung" ->
        "On Samsung phones also open Settings > Battery > Background usage limits, " +
            "and add Logistics Driver to \"Never sleeping apps\"."
    "huawei", "honor" ->
        "Also open Settings > Battery > App launch, find Logistics Driver, switch it to manage manually " +
            "and allow all three options."
    else -> null
}
