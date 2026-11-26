package app.protogrid.client.widget

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.widget.RemoteViews
import app.protogrid.client.R
import app.protogrid.client.vpn.VpnState
import app.protogrid.client.vpn.VpnStatus

/**
 * Home-screen widget with the current VPN state; tapping it starts or stops
 * the tunnel (or asks for the VPN permission when it is missing).
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
            views.setTextViewText(R.id.widget_status, label(status))
            views.setTextColor(R.id.widget_status, color(status))
            val intent = Intent(context, WidgetActionReceiver::class.java)
                .setAction(WidgetActionReceiver.ACTION_TOGGLE)
            val pending = PendingIntent.getBroadcast(
                context,
                0,
                intent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            views.setOnClickPendingIntent(R.id.widget_root, pending)
            manager.updateAppWidget(id, views)
        }

        private fun label(status: VpnStatus): String = when (status) {
            VpnStatus.Connected -> "Подключено"
            VpnStatus.Connecting -> "Подключение…"
            VpnStatus.Error -> "Ошибка"
            VpnStatus.Stopped -> "Отключено"
        }

        private fun color(status: VpnStatus): Int = when (status) {
            VpnStatus.Connected -> 0xFF10B981.toInt()
            VpnStatus.Connecting -> 0xFFF59E0B.toInt()
            VpnStatus.Error -> 0xFFEF4444.toInt()
            VpnStatus.Stopped -> 0xFF6F7988.toInt()
        }
    }
}
