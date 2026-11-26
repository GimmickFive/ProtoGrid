package app.protogrid.client.data

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.UUID

enum class PerAppMode(val id: String) {
    All("all"),
    Include("include"),
    Exclude("exclude");

    companion object {
        fun fromId(id: String?): PerAppMode =
            entries.firstOrNull { it.id == id } ?: All
    }
}

enum class TrafficPreset(val id: String, val label: String, val description: String) {
    Global("global", "Глобальный прокси", "Весь трафик через сервер"),
    Bypass("bypass", "Жизнь в РФ", "RU-домены и локальная сеть напрямую"),
    Direct("direct", "Прямой", "Без прокси"),
    Custom("custom", "Свой профиль", "Свои правила маршрутизации");

    companion object {
        fun fromId(id: String?): TrafficPreset =
            entries.firstOrNull { it.id == id } ?: Global
    }
}

enum class TrafficRuleKind(val id: String, val label: String) {
    Domain("domain", "Домен"),
    DomainSuffix("domain_suffix", "Суффикс"),
    DomainKeyword("domain_keyword", "Ключевое слово"),
    IpCidr("ip_cidr", "IP / CIDR"),
    Geoip("geoip", "GeoIP"),
    Geosite("geosite", "GeoSite");

    companion object {
        fun fromId(id: String?): TrafficRuleKind =
            entries.firstOrNull { it.id == id } ?: DomainSuffix
    }
}

enum class TrafficAction(val id: String, val label: String) {
    Proxy("proxy", "Прокси"),
    Direct("direct", "Напрямую"),
    Block("block", "Блок");

    companion object {
        fun fromId(id: String?): TrafficAction =
            entries.firstOrNull { it.id == id } ?: Proxy
    }
}

data class TrafficRule(
    val id: String,
    val kind: TrafficRuleKind,
    val value: String,
    val action: TrafficAction,
) {
    fun toJson(): JSONObject = JSONObject().apply {
        put("id", id)
        put("name", value)
        put("enabled", true)
        put("action", action.id)
        put("kind", kind.id)
        put("value", value)
    }

    companion object {
        fun fromJson(item: JSONObject): TrafficRule = TrafficRule(
            id = item.optString("id").ifBlank { UUID.randomUUID().toString() },
            kind = TrafficRuleKind.fromId(item.optString("kind")),
            value = item.optString("value"),
            action = TrafficAction.fromId(item.optString("action")),
        )

        fun create(kind: TrafficRuleKind, value: String, action: TrafficAction): TrafficRule =
            TrafficRule(
                id = UUID.randomUUID().toString(),
                kind = kind,
                value = value.trim(),
                action = action,
            )
    }
}

/**
 * Android-only settings that sit next to the shared ProtoGrid-core config:
 * VpnService per-app lists and the traffic routing preset.
 */
class AndroidSettings(context: Context) {
    private val appContext = context.applicationContext
    private val prefs = appContext
        .getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    var perAppMode: PerAppMode
        get() = PerAppMode.fromId(prefs.getString(KEY_PER_APP_MODE, null))
        set(value) {
            prefs.edit().putString(KEY_PER_APP_MODE, value.id).apply()
        }

    var perAppPackages: Set<String>
        get() = prefs.getStringSet(KEY_PER_APP_PACKAGES, emptySet())?.toSet().orEmpty()
        set(value) {
            prefs.edit().putStringSet(KEY_PER_APP_PACKAGES, value).apply()
        }

    var trafficPreset: TrafficPreset
        get() = TrafficPreset.fromId(prefs.getString(KEY_TRAFFIC_PRESET, null))
        set(value) {
            prefs.edit().putString(KEY_TRAFFIC_PRESET, value.id).apply()
        }

    var finalAction: TrafficAction
        get() = TrafficAction.fromId(prefs.getString(KEY_FINAL_ACTION, null))
        set(value) {
            prefs.edit().putString(KEY_FINAL_ACTION, value.id).apply()
        }

    var customRules: List<TrafficRule>
        get() {
            val raw = prefs.getString(KEY_RULES, null) ?: return emptyList()
            return runCatching {
                val array = JSONArray(raw)
                buildList {
                    for (index in 0 until array.length()) {
                        array.optJSONObject(index)?.let { add(TrafficRule.fromJson(it)) }
                    }
                }
            }.getOrDefault(emptyList())
        }
        set(value) {
            val array = JSONArray()
            value.forEach { array.put(it.toJson()) }
            prefs.edit().putString(KEY_RULES, array.toString()).apply()
        }

    var showSystemApps: Boolean
        get() = prefs.getBoolean(KEY_SHOW_SYSTEM, false)
        set(value) {
            prefs.edit().putBoolean(KEY_SHOW_SYSTEM, value).apply()
        }

    val rulesetDir: File
        get() = File(appContext.filesDir, "rulesets")

    fun customRulesJson(): String = JSONArray().apply {
        customRules.forEach { put(it.toJson()) }
    }.toString()

    fun routingJson(): String = JSONObject().apply {
        put("preset", trafficPreset.id)
        put("final_action", finalAction.id)
        put("rule_set_dir", rulesetDir.absolutePath)
        put("rules", JSONArray().apply { customRules.forEach { put(it.toJson()) } })
    }.toString()

    fun perAppSummary(): String = when (perAppMode) {
        PerAppMode.All -> "Все приложения"
        PerAppMode.Include -> "Только ${perAppPackages.size} прил."
        PerAppMode.Exclude -> "Все, кроме ${perAppPackages.size}"
    }

    private companion object {
        const val PREFS = "protogrid_routing"
        const val KEY_PER_APP_MODE = "per_app_mode"
        const val KEY_PER_APP_PACKAGES = "per_app_packages"
        const val KEY_TRAFFIC_PRESET = "traffic_preset"
        const val KEY_FINAL_ACTION = "final_action"
        const val KEY_RULES = "custom_rules"
        const val KEY_SHOW_SYSTEM = "show_system_apps"
    }
}
