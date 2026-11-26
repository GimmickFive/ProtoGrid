package app.protogrid.client.data

import android.content.Context
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.graphics.drawable.Drawable

data class AppEntry(
    val packageName: String,
    val label: String,
    val system: Boolean,
    val icon: Drawable?,
)

object AppCatalog {
    fun load(context: Context, includeSystem: Boolean): List<AppEntry> {
        val manager = context.packageManager
        val ownPackage = context.packageName
        val result = mutableListOf<AppEntry>()

        val packages = runCatching {
            manager.getInstalledApplications(PackageManager.GET_META_DATA)
        }.getOrDefault(emptyList())

        for (info in packages) {
            if (info.packageName == ownPackage) continue
            val system = info.flags and (
                ApplicationInfo.FLAG_SYSTEM or ApplicationInfo.FLAG_UPDATED_SYSTEM_APP
                ) != 0
            if (system && !includeSystem) continue
            val label = runCatching { manager.getApplicationLabel(info).toString() }
                .getOrDefault(info.packageName)
            val icon = runCatching { manager.getApplicationIcon(info) }.getOrNull()
            result += AppEntry(
                packageName = info.packageName,
                label = label,
                system = system,
                icon = icon,
            )
        }

        return result.sortedBy { it.label.lowercase() }
    }
}
