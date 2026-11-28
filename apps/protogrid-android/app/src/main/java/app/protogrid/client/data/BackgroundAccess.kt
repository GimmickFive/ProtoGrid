package app.protogrid.client.data

import android.app.AppOpsManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings

/**
 * System screens behind the "background work" settings.
 *
 * Android has no runtime permission for autostart: BOOT_COMPLETED is a normal
 * permission granted at install. The foreground VPN service is already exempt
 * from Doze, so no battery optimization dialog is needed. Some vendor ROMs
 * additionally hide an autostart whitelist in their own security apps, so a
 * fallback list of known activities is provided for those.
 */
object BackgroundAccess {
    /**
     * System VPN screen where the user turns on "Always-on VPN". Android then
     * keeps the tunnel running and restores it after a reboot on its own.
     */
    fun vpnSettingsIntent(context: Context): Intent =
        Intent(Settings.ACTION_VPN_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

    /**
     * Package selected as always-on VPN. Readable on Android 10 and below;
     * from Android 12 the key is hidden and a normal app gets SecurityException,
     * so callers must treat null as "unknown".
     */
    fun alwaysOnPackage(context: Context): String? = runCatching {
        Settings.Secure.getString(context.contentResolver, "always_on_vpn_app")
    }.getOrNull()

    /**
     * MIUI/HyperOS exposes autostart as vendor app-op 10008. Returns null on
     * ROMs that do not have this op; there is no public Android API for it.
     */
    @Suppress("DEPRECATION")
    fun autoStartAllowed(context: Context): Boolean? {
        val manufacturer = Build.MANUFACTURER.lowercase()
        if (!manufacturer.contains("xiaomi") &&
            !manufacturer.contains("redmi") &&
            !manufacturer.contains("poco")
        ) {
            return null
        }
        return runCatching {
            val ops = context.getSystemService(AppOpsManager::class.java) ?: return null
            val uid = android.os.Process.myUid()
            val pkg = context.packageName
            val intMode = runCatching {
                AppOpsManager::class.java
                    .getDeclaredMethod(
                        "checkOpNoThrow",
                        Int::class.javaPrimitiveType,
                        Int::class.javaPrimitiveType,
                        String::class.java,
                    )
                    .apply { isAccessible = true }
                    .invoke(ops, 10008, uid, pkg) as Int
            }.getOrNull()
            val rawMode = runCatching {
                ops.unsafeCheckOpRawNoThrow("MIUIOP(10008)", uid, pkg)
            }.getOrNull()
            when (intMode ?: rawMode) {
                AppOpsManager.MODE_ALLOWED -> true
                AppOpsManager.MODE_IGNORED, AppOpsManager.MODE_ERRORED -> false
                else -> null
            }
        }.getOrNull()
    }

    /**
     * System notification settings. A second POST_NOTIFICATIONS prompt is
     * suppressed by Android after the user denied it once, so the only
     * reliable way back is this screen.
     */
    fun notificationSettingsIntent(context: Context): Intent =
        Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
            .putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
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
