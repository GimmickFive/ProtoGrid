package app.protogrid.client.vpn

import android.content.Context
import android.net.ConnectivityManager
import android.net.LinkProperties
import android.net.Network
import android.net.NetworkCapabilities
import android.os.Handler
import android.os.Looper
import android.util.Log
import io.nekohasekai.libbox.InterfaceUpdateListener
import java.net.NetworkInterface

/**
 * Minimal default-network monitor for libbox. sing-box uses it to bind
 * outbound sockets to the real interface while the VPN is up.
 */
object DefaultNetworkMonitor {
    private const val TAG = "ProtoGridNetwork"

    private var callback: ConnectivityManager.NetworkCallback? = null
    private var listener: InterfaceUpdateListener? = null

    fun start(context: Context, updateListener: InterfaceUpdateListener) {
        listener = updateListener
        if (callback != null) return

        val manager = context.applicationContext.getSystemService(ConnectivityManager::class.java)
            ?: return
        val networkCallback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                update(manager, network)
            }

            override fun onLinkPropertiesChanged(network: Network, linkProperties: LinkProperties) {
                update(manager, network)
            }

            override fun onCapabilitiesChanged(
                network: Network,
                networkCapabilities: NetworkCapabilities,
            ) {
                update(manager, network)
            }
        }
        callback = networkCallback
        runCatching {
            manager.registerDefaultNetworkCallback(networkCallback, Handler(Looper.getMainLooper()))
        }.onFailure { Log.w(TAG, "registerDefaultNetworkCallback failed", it) }
        manager.activeNetwork?.let { update(manager, it) }
    }

    fun stop(context: Context) {
        val manager = context.applicationContext.getSystemService(ConnectivityManager::class.java)
        val networkCallback = callback
        callback = null
        listener = null
        if (manager != null && networkCallback != null) {
            runCatching { manager.unregisterNetworkCallback(networkCallback) }
        }
    }

    private fun update(manager: ConnectivityManager, network: Network) {
        val updateListener = listener ?: return
        val properties = manager.getLinkProperties(network) ?: return
        val name = properties.interfaceName ?: return
        val index = runCatching { NetworkInterface.getByName(name)?.index ?: -1 }
            .getOrDefault(-1)
        if (index < 0) return
        updateListener.updateDefaultInterface(name, index, false, false)
    }
}
