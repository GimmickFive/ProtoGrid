package app.protogrid.client.vpn

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import org.json.JSONArray
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
 *
 * The samples are written to SharedPreferences every couple of seconds and
 * restored on process start: an OEM can kill the app process while the VPN
 * service is being restarted (boot / always-on VPN), and the chart must not
 * start from zero again. Core counters restart at zero on reconnect, so the
 * persisted totals are kept as an offset and added to the new counter delta.
 */
object TrafficMonitor {
    private const val WINDOW = 60
    private const val CLASH_API = "http://127.0.0.1:9090/connections"
    private const val CONNECT_TIMEOUT_MS = 900
    private const val READ_TIMEOUT_MS = 1500
    private const val POLL_MS = 500L
    private const val SAVE_MS = 2000L

    private const val PREFS = "traffic_history"
    private const val KEY_SESSION = "session"
    private const val KEY_STARTED_AT = "started_at"
    private const val KEY_POINTS = "points"
    private const val KEY_PROXY_POINTS = "proxy_points"
    private const val KEY_DIRECT_POINTS = "direct_points"
    private const val KEY_DOWN_TOTAL = "down_total"
    private const val KEY_UP_TOTAL = "up_total"
    private const val KEY_PROXY_DOWN_TOTAL = "proxy_down_total"
    private const val KEY_PROXY_UP_TOTAL = "proxy_up_total"
    private const val KEY_CONNECTIONS = "connections"

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
    private var appContext: Context? = null

    // Totals earned before the current core instance started. The fresh core
    // begins its counters at zero; adding this offset keeps the UI continuous.
    private var offsetDown = 0L
    private var offsetUp = 0L
    private var offsetProxyDown = 0L
    private var offsetProxyUp = 0L

    /** Called once per process start, before the UI or widgets read the flows. */
    fun restore(context: Context) {
        appContext = context.applicationContext
        val raw = appContext
            ?.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            ?.getString(KEY_SESSION, null)
            ?: return
        try {
            val session = JSONObject(raw)
            startedAt.value = session.optLong(KEY_STARTED_AT, System.currentTimeMillis())
            points.value = decodePoints(session.optJSONArray(KEY_POINTS))
            proxyPoints.value = decodePoints(session.optJSONArray(KEY_PROXY_POINTS))
            directPoints.value = decodePoints(session.optJSONArray(KEY_DIRECT_POINTS))
            downTotal.value = session.optLong(KEY_DOWN_TOTAL)
            upTotal.value = session.optLong(KEY_UP_TOTAL)
            proxyDownTotal.value = session.optLong(KEY_PROXY_DOWN_TOTAL)
            proxyUpTotal.value = session.optLong(KEY_PROXY_UP_TOTAL)
            connections.value = session.optInt(KEY_CONNECTIONS)
            offsetDown = downTotal.value
            offsetUp = upTotal.value
            offsetProxyDown = proxyDownTotal.value
            offsetProxyUp = proxyUpTotal.value
            available.value = true
        } catch (_: Exception) {
            // A truncated/legacy session is not worth crashing the app for.
        }
    }

    fun start(scope: CoroutineScope, apiSecret: String) {
        // Keep whatever was restored for this process: the service may have been
        // recreated by the system after the UI was killed.
        job?.cancel()
        job = null
        startedAt.value = startedAt.value ?: System.currentTimeMillis()
        available.value = false
        job = scope.launch(Dispatchers.IO) {
            var previousDown = 0L
            var previousUp = 0L
            var previousProxyDown = 0L
            var previousProxyUp = 0L
            var baseDown = 0L
            var baseUp = 0L
            var baseProxyDown = 0L
            var baseProxyUp = 0L
            var hasBaseline = false
            var lastSave = 0L

            while (currentCoroutineContext().isActive) {
                val snapshot = fetch(apiSecret)
                if (snapshot == null) {
                    available.value = false
                } else {
                    val down = snapshot.optLong("downloadTotal")
                    val up = snapshot.optLong("uploadTotal")
                    val proxyDown = sumProxied(snapshot.optJSONObject("outboundDownloadTotal"))
                    val proxyUp = sumProxied(snapshot.optJSONObject("outboundUploadTotal"))

                    // The core's counters start at zero on every reconnect. Take
                    // the first reading as the baseline instead of diffing it
                    // against the previous core's much larger totals.
                    if (!hasBaseline) {
                        baseDown = down
                        baseUp = up
                        baseProxyDown = proxyDown
                        baseProxyUp = proxyUp
                        previousDown = down
                        previousUp = up
                        previousProxyDown = proxyDown
                        previousProxyUp = proxyUp
                        hasBaseline = true
                    }

                    val totalDownDelta = (down - previousDown).coerceAtLeast(0L)
                    val totalUpDelta = (up - previousUp).coerceAtLeast(0L)
                    val proxyDownDelta = (proxyDown - previousProxyDown).coerceAtLeast(0L)
                    val proxyUpDelta = (proxyUp - previousProxyUp).coerceAtLeast(0L)

                    points.value = points.value.drop(1) + TrafficPoint(totalDownDelta, totalUpDelta)
                    proxyPoints.value =
                        proxyPoints.value.drop(1) + TrafficPoint(proxyDownDelta, proxyUpDelta)
                    directPoints.value = directPoints.value.drop(1) + TrafficPoint(
                        (totalDownDelta - proxyDownDelta).coerceAtLeast(0L),
                        (totalUpDelta - proxyUpDelta).coerceAtLeast(0L),
                    )

                    downTotal.value = offsetDown + (down - baseDown).coerceAtLeast(0L)
                    upTotal.value = offsetUp + (up - baseUp).coerceAtLeast(0L)
                    proxyDownTotal.value =
                        offsetProxyDown + (proxyDown - baseProxyDown).coerceAtLeast(0L)
                    proxyUpTotal.value =
                        offsetProxyUp + (proxyUp - baseProxyUp).coerceAtLeast(0L)
                    connections.value = snapshot.optJSONArray("connections")?.length() ?: 0
                    available.value = true

                    previousDown = down
                    previousUp = up
                    previousProxyDown = proxyDown
                    previousProxyUp = proxyUp

                    val now = System.currentTimeMillis()
                    if (now - lastSave >= SAVE_MS) {
                        save(now)
                        lastSave = now
                    }
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
        offsetDown = 0
        offsetUp = 0
        offsetProxyDown = 0
        offsetProxyUp = 0
        appContext?.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            ?.edit()
            ?.remove(KEY_SESSION)
            ?.apply()
    }

    private fun save(now: Long) {
        val context = appContext ?: return
        val session = JSONObject()
            .put(KEY_STARTED_AT, startedAt.value ?: now)
            .put(KEY_POINTS, encodePoints(points.value))
            .put(KEY_PROXY_POINTS, encodePoints(proxyPoints.value))
            .put(KEY_DIRECT_POINTS, encodePoints(directPoints.value))
            .put(KEY_DOWN_TOTAL, downTotal.value)
            .put(KEY_UP_TOTAL, upTotal.value)
            .put(KEY_PROXY_DOWN_TOTAL, proxyDownTotal.value)
            .put(KEY_PROXY_UP_TOTAL, proxyUpTotal.value)
            .put(KEY_CONNECTIONS, connections.value)
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_SESSION, session.toString())
            .apply()
    }

    private fun emptyPoints(): List<TrafficPoint> = List(WINDOW) { TrafficPoint(0, 0) }

    private fun encodePoints(points: List<TrafficPoint>): JSONArray = JSONArray().apply {
        points.forEach { put(JSONArray().put(it.down).put(it.up)) }
    }

    private fun decodePoints(array: JSONArray?): List<TrafficPoint> {
        if (array == null) return emptyPoints()
        val decoded = ArrayList<TrafficPoint>(WINDOW)
        for (index in 0 until array.length()) {
            val pair = array.optJSONArray(index) ?: continue
            decoded.add(TrafficPoint(pair.optLong(0), pair.optLong(1)))
        }
        return if (decoded.isEmpty()) emptyPoints() else decoded
    }

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
