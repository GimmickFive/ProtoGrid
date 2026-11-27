package app.protogrid.client.data

import android.content.Context
import app.protogrid.client.BuildConfig
import app.protogrid.client.core.CoreBridge
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

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

/** A subscription saved on the device, with the provider's traffic counters. */
data class SubscriptionInfo(
    val id: String,
    val name: String,
    val url: String,
    val displayUrl: String,
    val autoUpdate: Boolean,
    val lastUpdated: Long?,
    val upload: Long,
    val download: Long,
    val total: Long,
    val expire: Long,
    val lastError: String?,
    val serverCount: Int,
) {
    val used: Long
        get() = upload + download
}

class ProfileStore(context: Context) {
    private val prefs = context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    private val secure = SecureStore(context)

    fun serversJson(): String = secure.read(KEY_SERVERS) ?: "[]"

    private fun storedServers(): JSONArray =
        runCatching { JSONArray(serversJson()) }.getOrDefault(JSONArray())

    fun profiles(): List<ProfileSummary> = runCatching {
        val array = storedServers()
        buildList {
            for (index in 0 until array.length()) {
                val item = array.optJSONObject(index) ?: continue
                if (!supportedByCore(item)) continue
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
        secure.write(KEY_SERVERS, remaining.toString())
        if (activeId() == id) {
            val next = remaining.optJSONObject(0)?.optString("id").orEmpty()
            val editor = prefs.edit()
            if (next.isNotBlank()) editor.putString(KEY_ACTIVE, next) else editor.remove(KEY_ACTIVE)
            editor.apply()
        }
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
        }.toMutableList()

        val existing = storedServers()
        val known = mutableSetOf<String>()
        for (index in 0 until existing.length()) {
            existing.optJSONObject(index)?.let { known += keyOf(it) }
        }

        var added = 0
        var skipped = 0
        for (index in 0 until incoming.length()) {
            val item = incoming.optJSONObject(index) ?: continue
            if (!supportedByCore(item)) {
                skipped++
                continue
            }
            if (known.add(keyOf(item))) {
                existing.put(item)
                added++
            }
        }
        if (skipped > 0) {
            errors += "Пропущено серверов: $skipped — Lite поддерживает только VLESS/Reality по TCP"
        }

        if (added > 0) {
            secure.write(KEY_SERVERS, existing.toString())
            if (activeId() == null) {
                existing.optJSONObject(0)?.optString("id")?.takeIf { it.isNotBlank() }?.let {
                    prefs.edit().putString(KEY_ACTIVE, it).apply()
                }
            }
        }
        return ImportResult(added, errors)
    }

    // --- Subscriptions -------------------------------------------------------

    fun subscriptions(): List<SubscriptionInfo> = runCatching {
        val array = storedSubscriptions()
        val counts = serverCountsBySubscription()
        buildList {
            for (index in 0 until array.length()) {
                val item = array.optJSONObject(index) ?: continue
                val id = item.optString("id")
                if (id.isBlank()) continue
                val userinfo = item.optJSONObject("userinfo")
                add(
                    SubscriptionInfo(
                        id = id,
                        name = item.optString("name").ifBlank {
                            item.optString("display_url").ifBlank { "Подписка" }
                        },
                        url = item.optString("url"),
                        displayUrl = item.optString("display_url").ifBlank {
                            displayUrl(item.optString("url"))
                        },
                        autoUpdate = item.optBoolean("auto_update", true),
                        lastUpdated = item.optLong("last_updated", 0).takeIf { it > 0 },
                        upload = userinfo?.optLong("upload", 0) ?: 0,
                        download = userinfo?.optLong("download", 0) ?: 0,
                        total = userinfo?.optLong("total", 0) ?: 0,
                        expire = userinfo?.optLong("expire", 0) ?: 0,
                        lastError = item.optString("last_error").takeIf { it.isNotBlank() },
                        serverCount = counts[id] ?: 0,
                    ),
                )
            }
        }
    }.getOrDefault(emptyList())

    /** Creates a subscription and adds its servers in one pass. */
    fun addSubscription(rawUrl: String, name: String): ImportResult {
        val url = rawUrl.trim()
        if (url.isBlank()) return ImportResult(0, listOf("пустой адрес подписки"))
        val fetched = fetchSubscription(url)
            ?: return ImportResult(0, listOf(lastFetchError ?: "не удалось загрузить подписку"))
        if (fetched.servers.length() == 0) {
            val reason = fetched.errors.firstOrNull() ?: "в подписке не найдено серверов"
            return ImportResult(0, listOf(reason))
        }

        val id = UUID.randomUUID().toString()
        val added = insertSubscriptionServers(fetched.servers, id, previous = null)
        val display = fetched.displayUrl.ifBlank { displayUrl(url) }
        val subscriptions = storedSubscriptions()
        subscriptions.put(
            JSONObject().apply {
                put("id", id)
                put("name", name.trim().ifBlank { defaultName(display) })
                put("url", url)
                put("display_url", display)
                put("auto_update", true)
                put("last_updated", nowSeconds())
                fetched.userinfo?.let { put("userinfo", it) }
            },
        )
        saveSubscriptions(subscriptions)
        ensureActive()
        return ImportResult(added, fetched.errors)
    }

    /** Re-downloads a subscription and replaces its servers in place. */
    fun refreshSubscription(id: String): ImportResult {
        val subscriptions = storedSubscriptions()
        val index = indexOfSubscription(subscriptions, id)
        if (index < 0) return ImportResult(0, listOf("подписка не найдена"))
        val record = subscriptions.optJSONObject(index) ?: return ImportResult(0, emptyList())
        val url = record.optString("url")

        val fetched = fetchSubscription(url)
        if (fetched == null) {
            val error = lastFetchError ?: "не удалось загрузить подписку"
            record.put("last_error", error)
            subscriptions.put(index, record)
            saveSubscriptions(subscriptions)
            return ImportResult(0, listOf(error))
        }

        val previous = storedServers()
        val previousIds = mutableMapOf<String, String>()
        for (i in 0 until previous.length()) {
            val item = previous.optJSONObject(i) ?: continue
            if (item.optString("subscription_id") == id) {
                previousIds[keyOf(item)] = item.optString("id")
            }
        }
        val added = insertSubscriptionServers(fetched.servers, id, previousIds)
        record.put("last_updated", nowSeconds())
        record.remove("last_error")
        if (fetched.userinfo != null) record.put("userinfo", fetched.userinfo)
        subscriptions.put(index, record)
        saveSubscriptions(subscriptions)
        ensureActive()
        return ImportResult(added, fetched.errors)
    }

    fun deleteSubscription(id: String) {
        val subscriptions = storedSubscriptions()
        val remainingSubscriptions = JSONArray()
        for (index in 0 until subscriptions.length()) {
            val item = subscriptions.optJSONObject(index) ?: continue
            if (item.optString("id") != id) remainingSubscriptions.put(item)
        }
        saveSubscriptions(remainingSubscriptions)

        val servers = storedServers()
        val remaining = JSONArray()
        for (index in 0 until servers.length()) {
            val item = servers.optJSONObject(index) ?: continue
            if (item.optString("subscription_id") != id) remaining.put(item)
        }
        secure.write(KEY_SERVERS, remaining.toString())
        ensureActive()
    }

    fun setSubscriptionAutoUpdate(id: String, enabled: Boolean) {
        val subscriptions = storedSubscriptions()
        val index = indexOfSubscription(subscriptions, id)
        if (index < 0) return
        subscriptions.optJSONObject(index)?.put("auto_update", enabled)
        saveSubscriptions(subscriptions)
    }

    /** Subscriptions whose auto-update is due (older than 12 hours). */
    fun staleSubscriptionIds(): List<String> {
        val now = nowSeconds()
        return subscriptions()
            .filter { it.autoUpdate && it.lastUpdated != null && now - it.lastUpdated > STALE_SECONDS }
            .map { it.id }
    }

    // --- Internals -----------------------------------------------------------

    private data class Fetched(
        val servers: JSONArray,
        val errors: List<String>,
        val userinfo: JSONObject?,
        val displayUrl: String,
    )

    private var lastFetchError: String? = null

    private fun fetchSubscription(url: String): Fetched? {
        lastFetchError = null
        val raw = runCatching { CoreBridge.fetchSubscription(url) }.getOrElse {
            lastFetchError = it.message ?: "не удалось загрузить подписку"
            return null
        }
        val parsed = runCatching { JSONObject(raw) }.getOrElse {
            lastFetchError = it.message ?: "некорректный ответ подписки"
            return null
        }
        parsed.optString("error").takeIf { it.isNotBlank() }?.let {
            lastFetchError = it
            return null
        }
        val rawServers = parsed.optJSONArray("servers") ?: JSONArray()
        val errors = buildList {
            parsed.optJSONArray("errors")?.let { array ->
                for (index in 0 until array.length()) {
                    array.optString(index).takeIf { it.isNotBlank() }?.let(::add)
                }
            }
        }.toMutableList()
        val servers = JSONArray()
        var skipped = 0
        for (index in 0 until rawServers.length()) {
            val item = rawServers.optJSONObject(index) ?: continue
            if (!supportedByCore(item)) {
                skipped++
                continue
            }
            servers.put(item)
        }
        if (skipped > 0) {
            errors += "Пропущено серверов: $skipped — Lite поддерживает только VLESS/Reality по TCP"
        }
        return Fetched(
            servers = servers,
            errors = errors,
            userinfo = parsed.optJSONObject("userinfo"),
            displayUrl = parsed.optString("display_url"),
        )
    }

    /** The Lite libbox only registers VLESS over plain TCP. */
    private fun supportedByCore(item: JSONObject): Boolean {
        if (!BuildConfig.LITE_CORE) return true
        val protocol = item.optString("protocol").lowercase()
        val transport = item.optString("transport").ifBlank { "tcp" }.lowercase()
        return protocol == "vless" && transport == "tcp"
    }

    /**
     * Appends `incoming` servers, tagging them with `subscriptionId`. Servers
     * that belonged to the same subscription before keep their old id so the
     * active choice survives a refresh.
     */
    private fun insertSubscriptionServers(
        incoming: JSONArray,
        subscriptionId: String,
        previous: Map<String, String>?,
    ): Int {
        val existing = storedServers()
        val kept = JSONArray()
        val indexByKey = mutableMapOf<String, Int>()
        for (index in 0 until existing.length()) {
            val item = existing.optJSONObject(index) ?: continue
            if (subscriptionIdOf(item) == subscriptionId) continue
            indexByKey[keyOf(item)] = kept.length()
            kept.put(item)
        }

        val reused = previous?.toMutableMap() ?: mutableMapOf()
        var added = 0
        for (index in 0 until incoming.length()) {
            val item = incoming.optJSONObject(index)?.let { JSONObject(it.toString()) } ?: continue
            item.put("subscription_id", subscriptionId)
            val key = keyOf(item)
            val oldId = reused.remove(key)
            if (oldId != null) {
                item.put("id", oldId)
                kept.put(item)
                added++
                continue
            }
            val existingIndex = indexByKey[key]
            if (existingIndex != null) {
                // The same server was added by hand before: link it to the
                // subscription instead of creating a duplicate.
                val existingItem = kept.optJSONObject(existingIndex)
                if (existingItem != null && subscriptionIdOf(existingItem).isBlank()) {
                    existingItem.put("subscription_id", subscriptionId)
                }
                continue
            }
            indexByKey[key] = kept.length()
            kept.put(item)
            added++
        }
        secure.write(KEY_SERVERS, kept.toString())
        return added
    }

    private fun ensureActive() {
        val servers = storedServers()
        val ids = mutableSetOf<String>()
        for (index in 0 until servers.length()) {
            servers.optJSONObject(index)?.optString("id")?.takeIf { it.isNotBlank() }?.let(ids::add)
        }
        val current = activeId()
        if (current != null && current in ids) return
        val editor = prefs.edit()
        val next = servers.optJSONObject(0)?.optString("id").orEmpty()
        if (next.isNotBlank()) editor.putString(KEY_ACTIVE, next) else editor.remove(KEY_ACTIVE)
        editor.apply()
    }

    private fun storedSubscriptions(): JSONArray =
        runCatching { JSONArray(secure.read(KEY_SUBSCRIPTIONS) ?: "[]") }
            .getOrDefault(JSONArray())

    private fun saveSubscriptions(array: JSONArray) {
        secure.write(KEY_SUBSCRIPTIONS, array.toString())
    }

    private fun indexOfSubscription(array: JSONArray, id: String): Int {
        for (index in 0 until array.length()) {
            if (array.optJSONObject(index)?.optString("id") == id) return index
        }
        return -1
    }

    private fun serverCountsBySubscription(): Map<String, Int> {
        val counts = mutableMapOf<String, Int>()
        val servers = storedServers()
        for (index in 0 until servers.length()) {
            val item = servers.optJSONObject(index) ?: continue
            if (!supportedByCore(item)) continue
            val id = subscriptionIdOf(item)
            if (id.isNotBlank()) counts[id] = (counts[id] ?: 0) + 1
        }
        return counts
    }

    /**
     * `subscription_id` is serialized as `null` by the Rust core and Android's
     * `optString` turns that into the literal `"null"`, so both spellings are
     * treated as "not linked".
     */
    private fun subscriptionIdOf(item: JSONObject): String =
        item.optString("subscription_id").takeUnless { it.isBlank() || it == "null" }.orEmpty()

    private fun keyOf(item: JSONObject): String = listOf(
        "protocol",
        "address",
        "port",
        "uuid",
        "password",
        "method",
        "service_name",
    ).joinToString("|") { item.optString(it) }

    private fun nowSeconds(): Long = System.currentTimeMillis() / 1000

    private fun defaultName(displayUrl: String): String =
        displayUrl.substringAfter("://").substringBefore('/').ifBlank { "Подписка" }

    /** Same idea as `subscription::display_url`: never show the token in the UI. */
    private fun displayUrl(raw: String): String {
        val trimmed = raw.trim()
        return trimmed.substringBefore('?').substringBefore('#')
    }

    private companion object {
        const val PREFS = "protogrid"
        const val KEY_SERVERS = "servers"
        const val KEY_ACTIVE = "active"
        const val KEY_SUBSCRIPTIONS = "subscriptions"
        const val STALE_SECONDS = 12L * 60 * 60
    }
}
