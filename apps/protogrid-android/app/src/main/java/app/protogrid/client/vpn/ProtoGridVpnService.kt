package app.protogrid.client.vpn

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import android.system.OsConstants
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import app.protogrid.client.MainActivity
import app.protogrid.client.R
import app.protogrid.client.core.CoreBridge
import app.protogrid.client.data.AndroidSettings
import app.protogrid.client.data.PerAppMode
import app.protogrid.client.data.ProfileStore
import io.nekohasekai.libbox.BridgeOptions
import io.nekohasekai.libbox.BridgeSession
import io.nekohasekai.libbox.CommandServer
import io.nekohasekai.libbox.CommandServerHandler
import io.nekohasekai.libbox.ConnectionOwner
import io.nekohasekai.libbox.InterfaceUpdateListener
import io.nekohasekai.libbox.Libbox
import io.nekohasekai.libbox.LocalDNSTransport
import io.nekohasekai.libbox.NeighborUpdateListener
import io.nekohasekai.libbox.NetworkInterfaceIterator
import io.nekohasekai.libbox.Notification as LibboxNotification
import io.nekohasekai.libbox.OverrideOptions
import io.nekohasekai.libbox.PlatformInterface
import io.nekohasekai.libbox.PlatformUser
import io.nekohasekai.libbox.RoutePrefixIterator
import io.nekohasekai.libbox.ShellSession
import io.nekohasekai.libbox.StringIterator
import io.nekohasekai.libbox.SystemProxyStatus
import io.nekohasekai.libbox.TunOptions
import io.nekohasekai.libbox.WIFIState
import io.nekohasekai.libbox.NetworkInterface as LibboxNetworkInterface
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.net.Inet6Address
import java.net.InterfaceAddress
import java.net.NetworkInterface as JavaNetworkInterface
import java.util.UUID

class ProtoGridVpnService :
    VpnService(),
    PlatformInterface,
    CommandServerHandler {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val store by lazy { ProfileStore(this) }

    @Volatile
    private var stopping = false

    private var commandServer: CommandServer? = null
    private var tunFd: ParcelFileDescriptor? = null
    private var clashApiSecret: String = ""

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            stopTunnel()
            return START_NOT_STICKY
        }

        stopping = false
        startForegroundNotification(getString(R.string.notification_connecting))
        try {
            ensureCommandServer()
        } catch (error: Throwable) {
            fail(error.message ?: error.toString())
            return START_NOT_STICKY
        }
        scope.launch { startTunnel() }
        return START_STICKY
    }

    override fun onDestroy() {
        scope.cancel()
        TrafficMonitor.stop()
        closeEngine()
        DefaultNetworkMonitor.stop(this)
        if (VpnState.status.value != VpnStatus.Error) {
            VpnState.update(VpnStatus.Stopped)
        }
        super.onDestroy()
    }

    override fun onRevoke() {
        stopTunnel()
    }

    private fun ensureCommandServer() {
        if (commandServer != null) return
        val server = CommandServer(this, this)
        server.start()
        commandServer = server
    }

    private suspend fun startTunnel() {
        if (stopping) return
        try {
            VpnState.update(VpnStatus.Connecting)
            updateNotification(getString(R.string.notification_connecting))

            val profiles = store.profiles()
            if (profiles.isEmpty()) {
                fail("Сначала добавьте сервер")
                return
            }
            val activeId = store.activeId()
                ?.takeIf { id -> profiles.any { it.id == id } }
                ?: profiles.first().id.also { store.setActive(it) }

            val settings = AndroidSettings(this@ProtoGridVpnService)
            runCatching {
                CoreBridge.prepareRuleSets(
                    settings.customRulesJson(),
                    settings.rulesetDir.absolutePath,
                    RULE_SET_MAX_AGE_SECONDS,
                )
            }.onFailure { Log.w(TAG, "GeoIP/GeoSite cache prepare failed", it) }
            clashApiSecret = UUID.randomUUID().toString().replace("-", "")
            val config = CoreBridge.buildConfig(
                store.serversJson(),
                activeId,
                settings.routingJson(),
                clashApiSecret,
            )
            val error = JSONObject(config).optString("error")
            if (error.isNotBlank()) {
                fail(error)
                return
            }

            val server = commandServer ?: error("libbox не инициализирован")
            server.startOrReloadService(config, perAppOptions(settings))

            VpnState.update(VpnStatus.Connected)
            TrafficMonitor.start(scope, clashApiSecret)
            updateNotification(getString(R.string.notification_connected))
        } catch (error: Throwable) {
            fail(error.message ?: error.toString())
        }
    }

    private fun perAppOptions(settings: AndroidSettings): OverrideOptions {
        val options = OverrideOptions()
        val selected = settings.perAppPackages
        when (settings.perAppMode) {
            PerAppMode.Include -> if (selected.isNotEmpty()) {
                options.setIncludePackage(
                    LibboxStrings((selected + packageName).iterator()),
                )
            }

            PerAppMode.Exclude -> if (selected.isNotEmpty()) {
                options.setExcludePackage(
                    LibboxStrings((selected - packageName).iterator()),
                )
            }

            PerAppMode.All -> Unit
        }
        return options
    }

    private fun fail(message: String) {
        Log.e(TAG, message)
        VpnState.update(VpnStatus.Error, message)
        stopTunnel()
    }

    private fun stopTunnel() {
        if (stopping) return
        stopping = true
        TrafficMonitor.stop()
        closeEngine()
        DefaultNetworkMonitor.stop(this)
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
        if (VpnState.status.value != VpnStatus.Error) {
            VpnState.update(VpnStatus.Stopped)
        }
    }

    private fun closeEngine() {
        runCatching { commandServer?.closeService() }
            .onFailure { Log.w(TAG, "closeService failed", it) }
        runCatching { commandServer?.close() }
            .onFailure { Log.w(TAG, "close command server failed", it) }
        commandServer = null
        runCatching { tunFd?.close() }
        tunFd = null
    }

    // --- CommandServerHandler -------------------------------------------------

    override fun serviceStop() {
        stopTunnel()
    }

    override fun serviceReload() {
        scope.launch { startTunnel() }
    }

    override fun getSystemProxyStatus(): SystemProxyStatus = SystemProxyStatus()

    override fun setSystemProxyEnabled(enabled: Boolean) = Unit

    override fun triggerNativeCrash() = Unit

    override fun writeDebugMessage(message: String?) {
        message?.let { Log.d(TAG, it) }
    }

    override fun connectSSHAgent(): Int = -1

    // --- PlatformInterface ----------------------------------------------------

    override fun localDNSTransport(): LocalDNSTransport? = null

    override fun usePlatformAutoDetectInterfaceControl(): Boolean = true

    override fun autoDetectInterfaceControl(fd: Int) {
        if (!protect(fd)) {
            Log.w(TAG, "protect($fd) failed")
        }
    }

    override fun openTun(options: TunOptions): Int {
        if (prepare(this) != null) {
            error("VPN permission is missing")
        }

        val builder = Builder()
            .setSession("ProtoGrid")
            .setMtu(options.getMTU())
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            builder.setMetered(false)
        }
        builder.allowBypass()

        var hasIpv4 = false
        var hasIpv6 = false
        val inet4Address = options.getInet4Address()
        while (inet4Address.hasNext()) {
            val prefix = inet4Address.next()
            builder.addAddress(prefix.address(), prefix.prefix())
            hasIpv4 = true
        }
        val inet6Address = options.getInet6Address()
        while (inet6Address.hasNext()) {
            val prefix = inet6Address.next()
            builder.addAddress(prefix.address(), prefix.prefix())
            hasIpv6 = true
        }

        val dnsMode = options.getDNSMode().getValue()
        if (!dnsMode.isNullOrEmpty() && dnsMode != Libbox.DNSModeDisabled) {
            val dnsServers = options.getDNSServerAddress()
            while (dnsServers.hasNext()) {
                val address = dnsServers.next()
                builder.addDnsServer(address)
                if (!options.getAutoRoute()) {
                    builder.addRoute(address, if (address.contains(':')) 128 else 32)
                }
            }
        }

        if (options.getAutoRoute()) {
            val hasIpv4Routes = if (hasIpv4) addRoutes(builder, options.getInet4RouteRange()) else false
            val hasIpv6Routes = if (hasIpv6) addRoutes(builder, options.getInet6RouteRange()) else false
            if (!hasIpv4Routes && hasIpv4) builder.addRoute("0.0.0.0", 0)
            if (!hasIpv6Routes && hasIpv6) builder.addRoute("::", 0)
            addPackages(builder, options.getIncludePackage(), allow = true)
            addPackages(builder, options.getExcludePackage(), allow = false)
        }

        val descriptor = builder.establish()
            ?: error("VPN is not prepared or was revoked")
        tunFd = descriptor
        return descriptor.fd
    }

    private fun addRoutes(builder: Builder, ranges: RoutePrefixIterator): Boolean {
        var added = false
        while (ranges.hasNext()) {
            val prefix = ranges.next()
            builder.addRoute(prefix.address(), prefix.prefix())
            added = true
        }
        return added
    }

    private fun addPackages(builder: Builder, packages: StringIterator?, allow: Boolean) {
        if (packages == null) return
        while (packages.hasNext()) {
            val packageName = packages.next()
            try {
                if (allow) {
                    builder.addAllowedApplication(packageName)
                } else {
                    builder.addDisallowedApplication(packageName)
                }
            } catch (_: PackageManager.NameNotFoundException) {
                Log.w(TAG, "unknown package: $packageName")
            }
        }
    }

    override fun findConnectionOwner(
        ipProtocol: Int,
        sourceAddress: String?,
        sourcePort: Int,
        destinationAddress: String?,
        destinationPort: Int,
    ): ConnectionOwner = unsupported("findConnectionOwner")

    override fun startDefaultInterfaceMonitor(listener: InterfaceUpdateListener?) {
        listener?.let { DefaultNetworkMonitor.start(this, it) }
    }

    override fun closeDefaultInterfaceMonitor(listener: InterfaceUpdateListener?) {
        DefaultNetworkMonitor.stop(this)
    }

    override fun getInterfaces(): NetworkInterfaceIterator {
        val manager = getSystemService(ConnectivityManager::class.java)
        val javaInterfaces = runCatching {
            JavaNetworkInterface.getNetworkInterfaces()?.toList().orEmpty()
        }.getOrDefault(emptyList())

        val interfaces = mutableListOf<LibboxNetworkInterface>()
        for (network in manager.allNetworks) {
            val properties = manager.getLinkProperties(network) ?: continue
            val capabilities = manager.getNetworkCapabilities(network) ?: continue
            val name = properties.interfaceName ?: continue
            val javaInterface = javaInterfaces.firstOrNull { it.name == name } ?: continue

            val box = LibboxNetworkInterface()
            box.setName(name)
            box.setIndex(javaInterface.index)
            box.setMTU(runCatching { javaInterface.getMTU() }.getOrDefault(1500))
            box.setDNSServer(
                StringArray(properties.dnsServers.mapNotNull { it.hostAddress }.iterator()),
            )
            box.setGateway(
                StringArray(
                    properties.routes
                        .filter { it.destination.prefixLength == 0 }
                        .mapNotNull { it.gateway?.hostAddress }
                        .iterator(),
                ),
            )
            box.setType(
                when {
                    capabilities.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) ->
                        Libbox.InterfaceTypeWIFI

                    capabilities.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) ->
                        Libbox.InterfaceTypeCellular

                    capabilities.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) ->
                        Libbox.InterfaceTypeEthernet

                    else -> Libbox.InterfaceTypeOther
                },
            )
            box.setAddresses(
                StringArray(javaInterface.interfaceAddresses.map { it.toPrefix() }.iterator()),
            )

            var flags = 0
            if (capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)) {
                flags = OsConstants.IFF_UP or OsConstants.IFF_RUNNING
            }
            if (javaInterface.isLoopback) flags = flags or OsConstants.IFF_LOOPBACK
            if (javaInterface.isPointToPoint) flags = flags or OsConstants.IFF_POINTOPOINT
            if (javaInterface.supportsMulticast()) flags = flags or OsConstants.IFF_MULTICAST
            box.setFlags(flags)
            box.setMetered(
                !capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED),
            )

            interfaces += box
        }
        return InterfaceArray(interfaces.iterator())
    }

    override fun underNetworkExtension(): Boolean = false

    override fun includeAllNetworks(): Boolean = false

    override fun readWIFIState(): WIFIState? = null

    override fun clearDNSCache() = Unit

    override fun sendNotification(notification: LibboxNotification?) = Unit

    override fun cancelNotification(identifier: String?, typeID: Int) = Unit

    override fun startNeighborMonitor(listener: NeighborUpdateListener?) = Unit

    override fun closeNeighborMonitor(listener: NeighborUpdateListener?) = Unit

    override fun registerMyInterface(name: String?) = Unit

    override fun usePlatformShell(): Boolean = false

    override fun checkPlatformShell() {
        throw UnsupportedOperationException("checkPlatformShell is not supported on Android")
    }

    override fun openShellSession(
        user: PlatformUser?,
        command: String?,
        environ: StringIterator?,
        term: String?,
        rows: Int,
        cols: Int,
    ): ShellSession = unsupported("openShellSession")

    override fun lookupUser(username: String?): PlatformUser = unsupported("lookupUser")

    override fun lookupSFTPServer(): String = ""

    override fun readSystemSSHHostKey(): String = ""

    override fun tailscaleHostname(): String = "${Build.MANUFACTURER} ${Build.MODEL}"

    override fun usePlatformBridge(): Boolean = false

    override fun createBridge(options: BridgeOptions?): BridgeSession =
        unsupported("createBridge")

    private fun <T> unsupported(name: String): T =
        throw UnsupportedOperationException("$name is not supported on Android")

    private class StringArray(private val iterator: Iterator<String>) : StringIterator {
        override fun len(): Int = 0

        override fun hasNext(): Boolean = iterator.hasNext()

        override fun next(): String = iterator.next()
    }

    private class InterfaceArray(
        private val iterator: Iterator<LibboxNetworkInterface>,
    ) : NetworkInterfaceIterator {
        override fun hasNext(): Boolean = iterator.hasNext()

        override fun next(): LibboxNetworkInterface = iterator.next()
    }

    private fun InterfaceAddress.toPrefix(): String {
        val host = address ?: return ""
        return if (host is Inet6Address) {
            "${Inet6Address.getByAddress(host.address).hostAddress}/$networkPrefixLength"
        } else {
            "${host.hostAddress}/$networkPrefixLength"
        }
    }

    // --- Notifications --------------------------------------------------------

    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            CHANNEL_ID,
            getString(R.string.notification_channel),
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            setShowBadge(false)
        }
        getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }

    private fun startForegroundNotification(text: String) {
        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED
        } else {
            0
        }
        ServiceCompat.startForeground(this, NOTIFICATION_ID, buildNotification(text), type)
    }

    private fun updateNotification(text: String) {
        getSystemService(NotificationManager::class.java)
            .notify(NOTIFICATION_ID, buildNotification(text))
    }

    private fun buildNotification(text: String): Notification {
        val openIntent = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val stopIntent = PendingIntent.getService(
            this,
            1,
            Intent(this, ProtoGridVpnService::class.java).setAction(ACTION_STOP),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_stat_vpn)
            .setContentTitle(getString(R.string.app_name))
            .setContentText(text)
            .setContentIntent(openIntent)
            .addAction(0, getString(R.string.notification_stop), stopIntent)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }

    companion object {
        private const val TAG = "ProtoGridVpn"
        private const val RULE_SET_MAX_AGE_SECONDS = 7L * 24 * 60 * 60
        private const val CHANNEL_ID = "protogrid_vpn"
        private const val NOTIFICATION_ID = 1

        const val ACTION_STOP = "app.protogrid.client.action.STOP"

        fun start(context: Context) {
            ContextCompat.startForegroundService(
                context,
                Intent(context, ProtoGridVpnService::class.java),
            )
        }

        fun stop(context: Context) {
            context.startService(
                Intent(context, ProtoGridVpnService::class.java).setAction(ACTION_STOP),
            )
        }
    }
}
