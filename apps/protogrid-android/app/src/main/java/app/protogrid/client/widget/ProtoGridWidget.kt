package app.protogrid.client.widget

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews
import app.protogrid.client.MainActivity
import app.protogrid.client.R
import app.protogrid.client.vpn.TrafficMonitor
import app.protogrid.client.vpn.VpnState
import app.protogrid.client.vpn.VpnStatus
import java.util.Locale

/**
 * 4x1 home-screen widget: tunnel state, proxy traffic totals and a
 * start/stop button. The body opens the app, the button toggles the tunnel.
 */
class ProtoGridWidget : AppWidgetProvider() {
    override fun onUpdate(
        context: Context,
        appWidgetManager: AppWidgetManager,
        appWidgetIds: IntArray,
    ) {
        appWidgetIds.forEach { update(context, appWidgetManager, it) }
    }

    companion object {
        fun updateAll(context: Context) {
            val manager = AppWidgetManager.getInstance(context) ?: return
            val ids = manager.getAppWidgetIds(ComponentName(context, ProtoGridWidget::class.java))
            ids.forEach { update(context, manager, it) }
        }

        private fun update(context: Context, manager: AppWidgetManager, id: Int) {
            val views = RemoteViews(context.packageName, R.layout.widget_protogrid)
            val status = VpnState.status.value
            val active = status == VpnStatus.Connected || status == VpnStatus.Connecting

            views.setTextViewText(R.id.widget_status, label(context, status))
            views.setTextColor(R.id.widget_status, color(status))
            views.setInt(R.id.widget_logo, "setColorFilter", color(status))

            val stats = if (status == VpnStatus.Connected) {
                context.getString(
                    R.string.widget_stats,
                    formatBytes(TrafficMonitor.proxyDownTotal.value),
                    formatBytes(TrafficMonitor.proxyUpTotal.value),
                )
            } else {
                context.getString(R.string.widget_stats_idle)
            }
            views.setTextViewText(R.id.widget_stats, stats)

            views.setTextViewText(
                R.id.widget_power,
                context.getString(if (active) R.string.widget_stop else R.string.widget_start),
            )
            views.setTextColor(R.id.widget_power, 0xFFFFFFFF.toInt())
            views.setInt(
                R.id.widget_power,
                "setBackgroundResource",
                if (active) R.drawable.widget_button_stop else R.drawable.widget_button_start,
            )
            views.setContentDescription(
                R.id.widget_power,
                context.getString(
                    if (active) R.string.widget_turn_off else R.string.widget_turn_on,
                ),
            )

            val openIntent = Intent(context, MainActivity::class.java)
            val openPending = PendingIntent.getActivity(
                context,
                0,
                openIntent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            views.setOnClickPendingIntent(R.id.widget_root, openPending)

            val toggleIntent = Intent(context, WidgetActionReceiver::class.java)
                .setAction(WidgetActionReceiver.ACTION_TOGGLE)
            val togglePending = PendingIntent.getBroadcast(
                context,
                1,
                toggleIntent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            views.setOnClickPendingIntent(R.id.widget_power, togglePending)

            manager.updateAppWidget(id, views)
        }

        private fun label(context: Context, status: VpnStatus): String = when (status) {
            VpnStatus.Connected -> context.getString(R.string.notification_connected)
            VpnStatus.Connecting -> context.getString(R.string.notification_connecting)
            VpnStatus.Error -> context.getString(R.string.widget_error)
            VpnStatus.Stopped -> context.getString(R.string.widget_stopped)
        }

        private fun color(status: VpnStatus): Int = when (status) {
            VpnStatus.Connected -> 0xFF10B981.toInt()
            VpnStatus.Connecting -> 0xFFF59E0B.toInt()
            VpnStatus.Error -> 0xFFEF4444.toInt()
            VpnStatus.Stopped -> 0xFF6F7988.toInt()
        }

        private fun formatBytes(bytes: Long): String {
            if (bytes < 1024) return "$bytes Б"
            var value = bytes.toDouble()
            var unit = 0
            while (value >= 1024 && unit < BYTE_UNITS.lastIndex) {
                value /= 1024
                unit++
            }
            val format = if (value >= 100) "%.0f %s" else "%.1f %s"
            return String.format(Locale.US, format, value, BYTE_UNITS[unit])
        }

        private val BYTE_UNITS = arrayOf("Б", "КБ", "МБ", "ГБ", "ТБ")
    }
}
