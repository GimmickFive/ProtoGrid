package app.protogrid.client

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.util.Log
import app.protogrid.client.data.AndroidSettings
import app.protogrid.client.data.ProfileStore
import app.protogrid.client.vpn.ProtoGridVpnService

/**
 * Restores the tunnel after a reboot when autostart is enabled in Settings.
 * The VPN permission and at least one profile must already exist, so nothing
 * is requested while the device boots.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action !in BOOT_ACTIONS) return
        Log.i(TAG, "onReceive: ${intent.action}")
        if (!AndroidSettings(context).autoStart) {
            Log.i(TAG, "autostart skipped: disabled")
            return
        }
        if (VpnService.prepare(context) != null) {
            Log.w(TAG, "autostart skipped: VPN permission is missing")
            return
        }
        val profiles = ProfileStore(context).profiles()
        if (profiles.isEmpty()) {
            Log.w(TAG, "autostart skipped: no servers")
            return
        }
        Log.i(TAG, "starting tunnel with ${profiles.size} server(s)")
        runCatching { ProtoGridVpnService.start(context) }
            .onFailure { Log.w(TAG, "autostart failed", it) }
    }

    private companion object {
        const val TAG = "ProtoGridBoot"
        val BOOT_ACTIONS = setOf(
            Intent.ACTION_BOOT_COMPLETED,
            "android.intent.action.QUICKBOOT_POWERON",
            "com.htc.intent.action.QUICKBOOT_POWERON",
        )
    }
}
