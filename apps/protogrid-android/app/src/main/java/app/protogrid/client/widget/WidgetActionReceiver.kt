package app.protogrid.client.widget

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.net.VpnService
import app.protogrid.client.vpn.ProtoGridVpnService
import app.protogrid.client.vpn.VpnState
import app.protogrid.client.vpn.VpnStatus

/** Handles taps on the home-screen widget. */
class WidgetActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent?) {
        when (VpnState.status.value) {
            VpnStatus.Connected, VpnStatus.Connecting -> ProtoGridVpnService.stop(context)
            else -> {
                val prepare = VpnService.prepare(context)
                if (prepare != null) {
                    context.startActivity(prepare.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                } else {
                    ProtoGridVpnService.start(context)
                }
            }
        }
    }

    companion object {
        const val ACTION_TOGGLE = "app.protogrid.client.action.WIDGET_TOGGLE"
    }
}
