package app.protogrid.client.data

import android.content.Context
import app.protogrid.client.core.CoreBridge
import org.json.JSONArray
import org.json.JSONObject

data class ProfileSummary(
    val id: String,
    val name: String,
    val protocol: String,
    val address: String,
    val port: Int,
) {
    val endpoint: String
        get() = if (address.contains(':')) "[$address]:$port" else "$address:$port"
}

data class ImportResult(
    val added: Int,
    val errors: List<String>,
)

class ProfileStore(context: Context) {
    private val prefs = context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun serversJson(): String = prefs.getString(KEY_SERVERS, "[]") ?: "[]"

    private fun storedServers(): JSONArray =
        runCatching { JSONArray(serversJson()) }.getOrDefault(JSONArray())

    fun profiles(): List<ProfileSummary> = runCatching {
        val array = storedServers()
        buildList {
            for (index in 0 until array.length()) {
                val item = array.optJSONObject(index) ?: continue
                val id = item.optString("id")
                if (id.isBlank()) continue
                add(
                    ProfileSummary(
                        id = id,
                        name = item.optString("name").ifBlank { item.optString("address") },
                        protocol = item.optString("protocol"),
                        address = item.optString("address"),
                        port = item.optInt("port"),
                    ),
                )
            }
        }
    }.getOrDefault(emptyList())

    fun activeId(): String? = prefs.getString(KEY_ACTIVE, null)?.takeIf { it.isNotBlank() }

    fun setActive(id: String) {
        prefs.edit().putString(KEY_ACTIVE, id).apply()
    }

    fun delete(id: String) {
        val array = storedServers()
        val remaining = JSONArray()
        for (index in 0 until array.length()) {
            val item = array.optJSONObject(index) ?: continue
            if (item.optString("id") != id) {
                remaining.put(item)
            }
        }
        val editor = prefs.edit().putString(KEY_SERVERS, remaining.toString())
        if (activeId() == id) {
            val next = remaining.optJSONObject(0)?.optString("id").orEmpty()
            if (next.isNotBlank()) editor.putString(KEY_ACTIVE, next) else editor.remove(KEY_ACTIVE)
        }
        editor.apply()
    }

    fun importText(text: String): ImportResult {
        val parsed = runCatching { JSONObject(CoreBridge.parseLinks(text)) }.getOrElse {
            return ImportResult(0, listOf(it.message ?: "не удалось разобрать ссылки"))
        }
        parsed.optString("error").takeIf { it.isNotBlank() }?.let {
            return ImportResult(0, listOf(it))
        }

        val incoming = parsed.optJSONArray("servers") ?: JSONArray()
        val errors = buildList {
            parsed.optJSONArray("errors")?.let { array ->
                for (index in 0 until array.length()) {
                    array.optString(index).takeIf { it.isNotBlank() }?.let(::add)
                }
            }
        }

        val existing = storedServers()
        val known = mutableSetOf<String>()
        for (index in 0 until existing.length()) {
            existing.optJSONObject(index)?.let { known += keyOf(it) }
        }

        var added = 0
        for (index in 0 until incoming.length()) {
            val item = incoming.optJSONObject(index) ?: continue
            if (known.add(keyOf(item))) {
                existing.put(item)
                added++
            }
        }

        if (added > 0) {
            val editor = prefs.edit().putString(KEY_SERVERS, existing.toString())
            if (activeId() == null) {
                existing.optJSONObject(0)?.optString("id")?.takeIf { it.isNotBlank() }?.let {
                    editor.putString(KEY_ACTIVE, it)
                }
            }
            editor.apply()
        }
        return ImportResult(added, errors)
    }

    private fun keyOf(item: JSONObject): String = listOf(
        "protocol",
        "address",
        "port",
        "uuid",
        "password",
        "method",
        "service_name",
    ).joinToString("|") { item.optString(it) }

    private companion object {
        const val PREFS = "protogrid"
        const val KEY_SERVERS = "servers"
        const val KEY_ACTIVE = "active"
    }
}
