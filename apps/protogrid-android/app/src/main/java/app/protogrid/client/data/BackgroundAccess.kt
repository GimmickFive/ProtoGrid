package app.protogrid.client.data

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.PowerManager
import android.provider.Settings

/**
 * System screens behind the "background work" settings.
 *
 * Android has no runtime permission for autostart: BOOT_COMPLETED is a normal
 * permission granted at install, while battery optimization exemptions are
 * requested through a system dialog. Some vendor ROMs additionally hide an
 * autostart whitelist in their own security apps, so a fallback list of known
 * activities is provided for those.
 */
object BackgroundAccess {
    /** True when Android is allowed to keep the tunnel alive in Doze. */
    fun isBatteryUnrestricted(context: Context): Boolean {
        val power = context.getSystemService(PowerManager::class.java) ?: return false
        return power.isIgnoringBatteryOptimizations(context.packageName)
    }

    /** Opens the direct "allow background work" prompt when the OS supports it. */
    fun batteryRequestIntent(context: Context): Intent =
        Intent(
            Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS,
            Uri.parse("package:${context.packageName}"),
        ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

    /** Battery optimization list, used to revoke the exemption manually. */
    fun batterySettingsIntent(): Intent =
        Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

    /** App info page: the fallback when the OEM hides the optimization list. */
    fun appDetailsIntent(context: Context): Intent =
        Intent(
            Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
            Uri.parse("package:${context.packageName}"),
        ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

    fun openAppDetails(context: Context): Boolean =
        runCatching { context.startActivity(appDetailsIntent(context)) }.isSuccess

    /**
     * Tries the vendor autostart manager and falls back to the app's system
     * page. Returns false only if even the fallback could not be opened.
     */
    fun openAutoStartSettings(context: Context): Boolean {
        for (component in AUTO_START_COMPONENTS) {
            val intent = Intent()
                .setComponent(component)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            if (runCatching { context.startActivity(intent) }.isSuccess) return true
        }
        return openAppDetails(context)
    }

    private val AUTO_START_COMPONENTS = listOf(
        // Xiaomi / Redmi / POCO
        ComponentName(
            "com.miui.securitycenter",
            "com.miui.permcenter.autostart.AutoStartManagementActivity",
        ),
        // Huawei / Honor
        ComponentName(
            "com.huawei.systemmanager",
            "com.huawei.systemmanager.startupmgr.ui.StartupNormalAppListActivity",
        ),
        // Oppo / Realme
        ComponentName(
            "com.coloros.safecenter",
            "com.coloros.safecenter.permission.startup.StartupAppListActivity",
        ),
        ComponentName(
            "com.coloros.safecenter",
            "com.coloros.safecenter.startupapp.StartupAppListActivity",
        ),
        // Vivo / iQOO
        ComponentName(
            "com.vivo.permissionmanager",
            "com.vivo.permissionmanager.activity.BgStartUpManagerActivity",
        ),
        // OnePlus
        ComponentName(
            "com.oneplus.security",
            "com.oneplus.security.chainlaunch.view.ChainLaunchAppListActivity",
        ),
        // Samsung
        ComponentName(
            "com.samsung.android.lool",
            "com.samsung.android.sm.ui.battery.BatteryActivity",
        ),
    )
}
