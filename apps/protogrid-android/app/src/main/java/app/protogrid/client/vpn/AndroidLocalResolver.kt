package app.protogrid.client.vpn

import android.util.Log
import io.nekohasekai.libbox.ExchangeContext
import io.nekohasekai.libbox.LocalDNSTransport
import java.net.Inet4Address
import java.net.Inet6Address
import java.net.InetAddress

/**
 * Resolves outbound domains through Android's system resolver.
 *
 * The Go core reads `/etc/resolv.conf` on its own, which does not exist on
 * Android, so domain-based servers (e.g. `de1.example.com`) would fail with
 * `dial udp [::1]:53: network is unreachable`. The VPN package is excluded
 * from its own tunnel, so `InetAddress` resolves over the real network.
 */
object AndroidLocalResolver : LocalDNSTransport {
    private const val TAG = "ProtoGridDns"
    private const val NXDOMAIN = 3
    private const val SERVFAIL = 2

    override fun raw(): Boolean = false

    override fun exchange(ctx: ExchangeContext, message: ByteArray) {
        // raw() is false, so the core uses lookup() instead of raw exchanges.
        ctx.errorCode(SERVFAIL)
    }

    override fun lookup(ctx: ExchangeContext, network: String, domain: String) {
        try {
            val addresses = InetAddress.getAllByName(domain)
            val preferV6 = network.contains('6')
            val ordered = if (preferV6) {
                addresses.filterIsInstance<Inet6Address>() + addresses.filterIsInstance<Inet4Address>()
            } else {
                addresses.filterIsInstance<Inet4Address>() + addresses.filterIsInstance<Inet6Address>()
            }
            val address = ordered.firstOrNull()
            if (address == null) {
                ctx.errorCode(NXDOMAIN)
            } else {
                ctx.success(address.hostAddress)
            }
        } catch (error: Exception) {
            Log.w(TAG, "lookup $domain failed", error)
            ctx.errorCode(NXDOMAIN)
        }
    }
}
