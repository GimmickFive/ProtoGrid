package app.protogrid.client.vpn

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL

data class TrafficPoint(val down: Long, val up: Long)

/**
 * Reads the Clash API exposed by ProtoGrid-core and turns the cumulative
 * counters into per-second samples for the Monitoring tab.
 *
 * ProtoGrid-core reports totals per outbound (`outboundUploadTotal` /
 * `outboundDownloadTotal`), so the monitor can show how much went through
 * the proxy and how much went out directly. Closed connections are included
 * by the core, which keeps the numbers exact for the whole session.
 */
object TrafficMonitor {
    private const val WINDOW = 60
    private const val CLASH_API = "http://127.0.0.1:9090/connections"
    private const val CONNECT_TIMEOUT_MS = 900
    private const val READ_TIMEOUT_MS = 1500
    private const val POLL_MS = 500L

    val points = MutableStateFlow(emptyPoints())
    val proxyPoints = MutableStateFlow(emptyPoints())
    val directPoints = MutableStateFlow(emptyPoints())

    val downTotal = MutableStateFlow(0L)
    val upTotal = MutableStateFlow(0L)
    val proxyDownTotal = MutableStateFlow(0L)
    val proxyUpTotal = MutableStateFlow(0L)

    val connections = MutableStateFlow(0)
    val startedAt = MutableStateFlow<Long?>(null)
    val available = MutableStateFlow(false)

    private var job: Job? = null

    fun start(scope: CoroutineScope, apiSecret: String) {
        stop()
        startedAt.value = System.currentTimeMillis()
        job = scope.launch(Dispatchers.IO) {
            var previousDown = 0L
            var previousUp = 0L
            var previousProxyDown = 0L
            var previousProxyUp = 0L
            var hasBaseline = false

            while (currentCoroutineContext().isActive) {
                val snapshot = fetch(apiSecret)
                if (snapshot == null) {
                    available.value = false
                } else {
                    val down = snapshot.optLong("downloadTotal")
                    val up = snapshot.optLong("uploadTotal")
                    val proxyDown = sumProxied(snapshot.optJSONObject("outboundDownloadTotal"))
                    val proxyUp = sumProxied(snapshot.optJSONObject("outboundUploadTotal"))

                    val totalDownDelta = if (hasBaseline) {
                        (down - previousDown).coerceAtLeast(0L)
                    } else {
                        0L
                    }
                    val totalUpDelta = if (hasBaseline) {
                        (up - previousUp).coerceAtLeast(0L)
                    } else {
                        0L
                    }
                    val proxyDownDelta = if (hasBaseline) {
                        (proxyDown - previousProxyDown).coerceAtLeast(0L)
                    } else {
                        0L
                    }
                    val proxyUpDelta = if (hasBaseline) {
                        (proxyUp - previousProxyUp).coerceAtLeast(0L)
                    } else {
                        0L
                    }

                    points.value = points.value.drop(1) + TrafficPoint(totalDownDelta, totalUpDelta)
                    proxyPoints.value =
                        proxyPoints.value.drop(1) + TrafficPoint(proxyDownDelta, proxyUpDelta)
                    directPoints.value = directPoints.value.drop(1) + TrafficPoint(
                        (totalDownDelta - proxyDownDelta).coerceAtLeast(0L),
                        (totalUpDelta - proxyUpDelta).coerceAtLeast(0L),
                    )

                    downTotal.value = down
                    upTotal.value = up
                    proxyDownTotal.value = proxyDown.coerceAtMost(down)
                    proxyUpTotal.value = proxyUp.coerceAtMost(up)
                    connections.value = snapshot.optJSONArray("connections")?.length() ?: 0
                    available.value = true

                    previousDown = down
                    previousUp = up
                    previousProxyDown = proxyDown
                    previousProxyUp = proxyUp
                    hasBaseline = true
                }
                delay(POLL_MS)
            }
        }
    }

    fun stop() {
        job?.cancel()
        job = null
        points.value = emptyPoints()
        proxyPoints.value = emptyPoints()
        directPoints.value = emptyPoints()
        downTotal.value = 0
        upTotal.value = 0
        proxyDownTotal.value = 0
        proxyUpTotal.value = 0
        connections.value = 0
        startedAt.value = null
        available.value = false
    }

    private fun emptyPoints(): List<TrafficPoint> = List(WINDOW) { TrafficPoint(0, 0) }

    /** Sums every outbound that is not a local bypass (direct/block/dns). */
    private fun sumProxied(totals: JSONObject?): Long {
        if (totals == null) return 0
        var sum = 0L
        val keys = totals.keys()
        while (keys.hasNext()) {
            val tag = keys.next()
            if (isDirectTag(tag)) continue
            sum += totals.optLong(tag)
        }
        return sum
    }

    private fun isDirectTag(tag: String): Boolean =
        tag.equals("direct", true) ||
            tag.equals("block", true) ||
            tag.equals("reject", true) ||
            tag.equals("dns-out", true)

    private fun fetch(apiSecret: String): JSONObject? = try {
        val connection = URL(CLASH_API).openConnection() as HttpURLConnection
        connection.connectTimeout = CONNECT_TIMEOUT_MS
        connection.readTimeout = READ_TIMEOUT_MS
        connection.requestMethod = "GET"
        connection.useCaches = false
        if (apiSecret.isNotEmpty()) {
            connection.setRequestProperty("Authorization", "Bearer $apiSecret")
        }
        try {
            if (connection.responseCode == HttpURLConnection.HTTP_OK) {
                val body = connection.inputStream.bufferedReader().use { it.readText() }
                JSONObject(body)
            } else {
                null
            }
        } finally {
            connection.disconnect()
        }
    } catch (_: Exception) {
        null
    }
}
