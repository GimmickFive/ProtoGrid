package app.protogrid.client.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import app.protogrid.client.R
import app.protogrid.client.vpn.VpnStatus

/**
 * Palette copied from apps/protogrid-desktop/src/theme.rs so the Android
 * client reads as the same product: navy surfaces, a blue primary accent,
 * thin borders, muted labels.
 */
object Pg {
    val bg = Color(0xFF0F1115)
    val bg2 = Color(0xFF0C0E12)
    val sidebar = Color(0xFF14161B)
    val panel = Color(0xFF191C22)
    val panel2 = Color(0xFF1F232B)
    val panel3 = Color(0xFF252A34)
    val line = Color(0xFF272C35)
    val lineStrong = Color(0xFF353C48)
    val text = Color(0xFFE8EDF5)
    val muted = Color(0xFFA0A9B7)
    val faint = Color(0xFF6F7988)

    val accent = Color(0xFF3B82F6)
    val accent2 = Color(0xFF60A5FA)
    val accentSoft = Color(0x243B82F6)
    val accentLine = Color(0x6B3B82F6)

    val cyan = Color(0xFF22D3EE)
    val green = Color(0xFF10B981)
    val amber = Color(0xFFF59E0B)
    val red = Color(0xFFEF4444)
    val violet = Color(0xFFA78BFA)
}

val Inter = FontFamily(
    Font(R.font.inter_regular, FontWeight.Normal),
    Font(R.font.inter_medium, FontWeight.Medium),
    Font(R.font.inter_semibold, FontWeight.SemiBold),
    Font(R.font.inter_bold, FontWeight.Bold),
)

val Mono = FontFamily(
    Font(R.font.jetbrains_mono_regular, FontWeight.Normal),
    Font(R.font.jetbrains_mono_medium, FontWeight.Medium),
    Font(R.font.jetbrains_mono_bold, FontWeight.Bold),
)

private val PgTypography = Typography(
    displaySmall = TextStyle(
        fontFamily = Inter,
        fontWeight = FontWeight.Bold,
        fontSize = 30.sp,
        lineHeight = 36.sp,
        letterSpacing = (-0.8).sp,
    ),
    headlineMedium = TextStyle(
        fontFamily = Inter,
        fontWeight = FontWeight.Bold,
        fontSize = 25.sp,
        lineHeight = 31.sp,
        letterSpacing = (-0.5).sp,
    ),
    headlineSmall = TextStyle(
        fontFamily = Inter,
        fontWeight = FontWeight.SemiBold,
        fontSize = 20.sp,
        lineHeight = 26.sp,
        letterSpacing = (-0.2).sp,
    ),
    titleLarge = TextStyle(
        fontFamily = Inter,
        fontWeight = FontWeight.SemiBold,
        fontSize = 17.sp,
        lineHeight = 23.sp,
    ),
    titleMedium = TextStyle(
        fontFamily = Inter,
        fontWeight = FontWeight.SemiBold,
        fontSize = 15.sp,
        lineHeight = 20.sp,
    ),
    titleSmall = TextStyle(
        fontFamily = Inter,
        fontWeight = FontWeight.Medium,
        fontSize = 13.sp,
        lineHeight = 18.sp,
    ),
    bodyLarge = TextStyle(
        fontFamily = Inter,
        fontWeight = FontWeight.Normal,
        fontSize = 14.sp,
        lineHeight = 20.sp,
    ),
    bodyMedium = TextStyle(
        fontFamily = Inter,
        fontWeight = FontWeight.Normal,
        fontSize = 13.sp,
        lineHeight = 18.sp,
    ),
    bodySmall = TextStyle(
        fontFamily = Inter,
        fontWeight = FontWeight.Normal,
        fontSize = 11.sp,
        lineHeight = 16.sp,
    ),
    labelLarge = TextStyle(
        fontFamily = Inter,
        fontWeight = FontWeight.Medium,
        fontSize = 13.sp,
        lineHeight = 18.sp,
    ),
    labelMedium = TextStyle(
        fontFamily = Mono,
        fontWeight = FontWeight.Medium,
        fontSize = 10.sp,
        lineHeight = 14.sp,
        letterSpacing = 1.0.sp,
    ),
    labelSmall = TextStyle(
        fontFamily = Mono,
        fontWeight = FontWeight.Normal,
        fontSize = 9.sp,
        lineHeight = 12.sp,
        letterSpacing = 1.1.sp,
    ),
)

private val PgColorScheme = darkColorScheme(
    primary = Pg.accent,
    onPrimary = Color.White,
    primaryContainer = Pg.accentSoft,
    onPrimaryContainer = Pg.accent2,
    secondary = Pg.cyan,
    onSecondary = Pg.bg,
    background = Pg.bg,
    onBackground = Pg.text,
    surface = Pg.panel,
    onSurface = Pg.text,
    surfaceVariant = Pg.panel2,
    onSurfaceVariant = Pg.muted,
    outline = Pg.line,
    outlineVariant = Pg.line,
    error = Pg.red,
    onError = Color.White,
)

@Composable
fun ProtoGridTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = PgColorScheme,
        typography = PgTypography,
        content = content,
    )
}

fun statusColor(status: VpnStatus): Color = when (status) {
    VpnStatus.Stopped -> Pg.faint
    VpnStatus.Connecting -> Pg.amber
    VpnStatus.Connected -> Pg.green
    VpnStatus.Error -> Pg.red
}

fun statusLabel(status: VpnStatus): String = when (status) {
    VpnStatus.Stopped -> "Отключено"
    VpnStatus.Connecting -> "Подключение…"
    VpnStatus.Connected -> "Подключено"
    VpnStatus.Error -> "Ошибка"
}

fun protocolColor(protocol: String): Color = when (protocol.lowercase()) {
    "vless" -> Pg.accent2
    "vmess" -> Pg.cyan
    "trojan" -> Pg.amber
    "hysteria2", "hy2" -> Pg.red
    "tuic" -> Pg.violet
    "shadowsocks", "ss" -> Pg.green
    "openvpn" -> Pg.green
    else -> Pg.muted
}

fun protocolLabel(protocol: String): String = when (protocol.lowercase()) {
    "vless" -> "VLESS"
    "vmess" -> "VMess"
    "trojan" -> "Trojan"
    "hysteria2", "hy2" -> "Hysteria2"
    "tuic" -> "TUIC"
    "shadowsocks", "ss" -> "Shadowsocks"
    "openvpn" -> "OpenVPN"
    else -> protocol.uppercase()
}
