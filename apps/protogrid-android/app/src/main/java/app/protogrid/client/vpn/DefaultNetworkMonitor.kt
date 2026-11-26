package app.protogrid.client.vpn

import android.content.Context
import android.net.ConnectivityManager
import android.net.LinkProperties
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import io.nekohasekai.libbox.InterfaceUpdateListener
import java.net.NetworkInterface

/**
 * Minimal default-network monitor for libbox. sing-box uses it to bind
 * outbound sockets to the real interface while the VPN is up.
 *
 * `registerDefaultNetworkCallback` reports the VPN itself as the default
 * network while our tunnel is running, which makes sing-box try to dial the
 * proxy through its own tun ("no available network interface"). We therefore
 * request a network that is explicitly not a VPN.
 */
object DefaultNetworkMonitor {
    private const val TAG = "ProtoGridNetwork"

    private var callback: ConnectivityManager.NetworkCallback? = null
    private var listener: InterfaceUpdateListener? = null

    fun start(context: Context, updateListener: InterfaceUpdateListener) {
        listener = updateListener
        val manager = context.applicationContext.getSystemService(ConnectivityManager::class.java)
            ?: return
        val existing = callback
        if (existing != null) {
            manager.activeNetwork?.let { update(manager, it) }
            return
        }

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
            when {
                Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
                    manager.registerBestMatchingNetworkCallback(
                        underlyingRequest(),
                        networkCallback,
                        Handler(Looper.getMainLooper()),
                    )
                }

                Build.VERSION.SDK_INT >= Build.VERSION_CODES.P -> {
                    manager.requestNetwork(
                        underlyingRequest(),
                        networkCallback,
                        Handler(Looper.getMainLooper()),
                    )
                }

                else -> manager.registerDefaultNetworkCallback(
                    networkCallback,
                    Handler(Looper.getMainLooper()),
                )
            }
        }.onFailure { Log.w(TAG, "register network callback failed", it) }
        // activeNetwork is the VPN while the tunnel is up, so the callback
        // above is the only reliable source for the underlying interface.
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

    private fun underlyingRequest(): NetworkRequest =
        NetworkRequest.Builder()
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_RESTRICTED)
            .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
            .build()

    private fun update(manager: ConnectivityManager, network: Network) {
        val updateListener = listener ?: return
        val capabilities = manager.getNetworkCapabilities(network)
        if (capabilities?.hasTransport(NetworkCapabilities.TRANSPORT_VPN) == true) return
        val properties = manager.getLinkProperties(network) ?: return
        val name = properties.interfaceName ?: return
        val index = runCatching { NetworkInterface.getByName(name)?.index ?: -1 }
            .getOrDefault(-1)
        if (index < 0) return
        updateListener.updateDefaultInterface(name, index, false, false)
    }
}
