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
 */
object TrafficMonitor {
    private const val WINDOW = 60
    private const val CLASH_API = "http://127.0.0.1:9090/connections"
    private const val CONNECT_TIMEOUT_MS = 900
    private const val READ_TIMEOUT_MS = 1500

    val points = MutableStateFlow(List(WINDOW) { TrafficPoint(0, 0) })
    val downTotal = MutableStateFlow(0L)
    val upTotal = MutableStateFlow(0L)
    val connections = MutableStateFlow(0)
    val startedAt = MutableStateFlow<Long?>(null)
    val available = MutableStateFlow(false)

    private var job: Job? = null

    fun start(scope: CoroutineScope) {
        stop()
        startedAt.value = System.currentTimeMillis()
        job = scope.launch(Dispatchers.IO) {
            var previousDown = 0L
            var previousUp = 0L
            var hasBaseline = false
            while (currentCoroutineContext().isActive) {
                val snapshot = fetch()
                if (snapshot == null) {
                    available.value = false
                } else {
                    val down = snapshot.optLong("downloadTotal")
                    val up = snapshot.optLong("uploadTotal")
                    val active = snapshot.optJSONArray("connections")?.length() ?: 0

                    val downSpeed = if (hasBaseline) {
                        (down - previousDown).coerceAtLeast(0L)
                    } else {
                        0L
                    }
                    val upSpeed = if (hasBaseline) {
                        (up - previousUp).coerceAtLeast(0L)
                    } else {
                        0L
                    }

                    points.value = points.value.drop(1) + TrafficPoint(downSpeed, upSpeed)
                    downTotal.value = down
                    upTotal.value = up
                    connections.value = active
                    available.value = true

                    previousDown = down
                    previousUp = up
                    hasBaseline = true
                }
                delay(1000)
            }
        }
    }

    fun stop() {
        job?.cancel()
        job = null
        points.value = List(WINDOW) { TrafficPoint(0, 0) }
        downTotal.value = 0
        upTotal.value = 0
        connections.value = 0
        startedAt.value = null
        available.value = false
    }

    private fun fetch(): JSONObject? = try {
        val connection = URL(CLASH_API).openConnection() as HttpURLConnection
        connection.connectTimeout = CONNECT_TIMEOUT_MS
        connection.readTimeout = READ_TIMEOUT_MS
        connection.requestMethod = "GET"
        connection.useCaches = false
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
