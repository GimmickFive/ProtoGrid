package app.protogrid.client.widget

import android.app.PendingIntent
import android.content.Intent
import android.graphics.drawable.Icon
import android.net.VpnService
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import app.protogrid.client.R
import app.protogrid.client.vpn.ProtoGridVpnService
import app.protogrid.client.vpn.VpnState
import app.protogrid.client.vpn.VpnStatus

/** Quick Settings tile that toggles the tunnel from the notification shade. */
class ProtoGridTileService : TileService() {
    override fun onStartListening() {
        super.onStartListening()
        updateTile()
    }

    override fun onClick() {
        super.onClick()
        when (VpnState.status.value) {
            VpnStatus.Connected, VpnStatus.Connecting -> ProtoGridVpnService.stop(this)
            else -> {
                val prepare = VpnService.prepare(this)
                if (prepare != null) {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                        startActivityAndCollapse(
                            PendingIntent.getActivity(
                                this,
                                0,
                                prepare,
                                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
                            ),
                        )
                    } else {
                        @Suppress("DEPRECATION")
                        startActivityAndCollapse(prepare)
                    }
                } else {
                    ProtoGridVpnService.start(this)
                }
            }
        }
        updateTile()
    }

    private fun updateTile() {
        val tile = qsTile ?: return
        tile.state = when (VpnState.status.value) {
            VpnStatus.Connected, VpnStatus.Connecting -> Tile.STATE_ACTIVE
            else -> Tile.STATE_INACTIVE
        }
        tile.label = "ProtoGrid"
        tile.icon = Icon.createWithResource(this, R.drawable.ic_stat_vpn)
        tile.updateTile()
    }
}
