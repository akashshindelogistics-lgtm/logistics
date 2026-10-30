package com.logistics.driver.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

// Black and white, matching the dashboard's monochrome theme.
private val Light = lightColorScheme(
    primary = Color(0xFF111111),
    onPrimary = Color.White,
    primaryContainer = Color(0xFFE5E5E5),
    onPrimaryContainer = Color(0xFF0A0A0A),
    secondary = Color(0xFF525252),
    onSecondary = Color.White,
    background = Color(0xFFF5F5F5),
    onBackground = Color(0xFF0A0A0A),
    surface = Color.White,
    onSurface = Color(0xFF0A0A0A),
    surfaceVariant = Color(0xFFF0F0F0),
    onSurfaceVariant = Color(0xFF525252),
    outline = Color(0xFFA3A3A3),
    error = Color(0xFF000000),
    onError = Color.White,
    errorContainer = Color(0xFFD4D4D4),
    onErrorContainer = Color(0xFF000000),
)

private val Dark = darkColorScheme(
    primary = Color(0xFFFAFAFA),
    onPrimary = Color.Black,
    primaryContainer = Color(0xFF262626),
    onPrimaryContainer = Color(0xFFF5F5F5),
    secondary = Color(0xFFA3A3A3),
    onSecondary = Color.Black,
    background = Color(0xFF0A0A0A),
    onBackground = Color(0xFFF5F5F5),
    surface = Color(0xFF141414),
    onSurface = Color(0xFFF5F5F5),
    surfaceVariant = Color(0xFF1F1F1F),
    onSurfaceVariant = Color(0xFFD4D4D4),
    outline = Color(0xFF737373),
    error = Color.White,
    onError = Color.Black,
    errorContainer = Color(0xFF404040),
    onErrorContainer = Color.White,
)

@Composable
fun DriverTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = if (isSystemInDarkTheme()) Dark else Light, content = content)
}
