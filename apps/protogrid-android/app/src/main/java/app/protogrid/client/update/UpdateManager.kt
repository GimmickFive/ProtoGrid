package app.protogrid.client.update

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import app.protogrid.client.BuildConfig
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import org.json.JSONArray

data class UpdateAsset(
    val name: String,
    val url: String,
    val size: Long,
    val digest: String?,
)

data class AppRelease(
    val tag: String,
    val version: String,
    val notes: String,
    val pageUrl: String,
    val publishedAt: String,
    val apk: UpdateAsset,
)

/** ABI of the APK this build can actually replace. */
private fun targetAbi(): String {
    val known = { abi: String ->
        abi.equals("arm64-v8a", ignoreCase = true) ||
            abi.equals("x86_64", ignoreCase = true)
    }
    return BuildConfig.BUILD_ABIS.split(',')
        .firstOrNull(known)
        ?: Build.SUPPORTED_ABIS.firstOrNull(known)
        ?: "arm64-v8a"
}

private fun variantLabel(): String =
    "${if (BuildConfig.LITE_CORE) "Lite" else "Standard"} · ${targetAbi()}"

data class UpdateUiState(
    val currentVersion: String = BuildConfig.VERSION_NAME,
    val variant: String = variantLabel(),
    val checking: Boolean = false,
    val checked: Boolean = false,
    val release: AppRelease? = null,
    /** Release that carries an APK for this exact build variant and ABI. */
    val currentRelease: AppRelease? = null,
    val error: String? = null,
) {
    val updateAvailable: Boolean get() = release != null
}

/**
 * Checks the public GitHub releases for a newer APK. Installation is left to
 * the browser/system installer: the app deliberately does not request
 * REQUEST_INSTALL_PACKAGES, which Play Protect treats as dropper behaviour.
 */
object UpdateManager {
    private const val REPO = "GimmickFive/ProtoGrid"
    private const val RELEASES_API =
        "https://api.github.com/repos/$REPO/releases?per_page=20"
    private const val RELEASES_URL = "https://github.com/$REPO/releases"

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _state = MutableStateFlow(UpdateUiState())
    val state: StateFlow<UpdateUiState> = _state.asStateFlow()

    fun check() {
        if (_state.value.checking) return
        _state.update { it.copy(checking = true, error = null) }
        scope.launch {
            runCatching { fetchReleaseForBuild() }
                .onSuccess { release ->
                    val newer = release != null &&
                        isNewer(release.version, BuildConfig.VERSION_NAME)
                    _state.update {
                        it.copy(
                            checking = false,
                            checked = true,
                            release = if (newer) release else null,
                            currentRelease = release,
                            error = null,
                        )
                    }
                }
                .onFailure { error ->
                    _state.update {
                        it.copy(
                            checking = false,
                            checked = true,
                            error = error.message ?: "не удалось проверить обновления",
                        )
                    }
                }
        }
    }

    fun openReleasePage(context: Context) {
        val release = _state.value.release ?: _state.value.currentRelease
        val url = when {
            release == null -> RELEASES_URL
            release.pageUrl.isNotBlank() -> release.pageUrl
            else -> "https://github.com/$REPO/releases/tag/${release.tag}"
        }
        runCatching {
            context.startActivity(
                Intent(Intent.ACTION_VIEW, Uri.parse(url))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        }.onFailure { error ->
            _state.update {
                it.copy(error = "Не удалось открыть браузер: ${error.message}")
            }
        }
    }

    /** Newest release that publishes an APK for this flavor and ABI. */
    private fun fetchReleaseForBuild(): AppRelease? {
        val connection = (URL(RELEASES_API).openConnection() as HttpURLConnection).apply {
            connectTimeout = 20_000
            readTimeout = 30_000
            instanceFollowRedirects = true
            setRequestProperty("Accept", "application/vnd.github+json")
            setRequestProperty("X-GitHub-Api-Version", "2022-11-28")
            setRequestProperty("User-Agent", "ProtoGrid/${BuildConfig.VERSION_NAME}")
        }
        val text = try {
            val code = connection.responseCode
            if (code !in 200..299) {
                throw IllegalStateException("GitHub API вернул HTTP $code")
            }
            connection.inputStream.bufferedReader().use { it.readText() }
        } finally {
            connection.disconnect()
        }
        val array = JSONArray(text)
        var best: AppRelease? = null
        var bestKey: List<Int> = emptyList()
        for (index in 0 until array.length()) {
            val item = array.optJSONObject(index) ?: continue
            if (item.optBoolean("draft")) continue
            val tag = item.optString("tag_name")
            val version = versionOf(tag)
            val key = versionKey(version)
            if (key.isEmpty()) continue
            val assets = item.optJSONArray("assets") ?: continue
            val apk = pickApk(assets) ?: continue
            if (best == null || compareKeys(key, bestKey) > 0) {
                best = AppRelease(
                    tag = tag,
                    version = version,
                    notes = item.optString("body"),
                    pageUrl = item.optString("html_url"),
                    publishedAt = item.optString("published_at"),
                    apk = apk,
                )
                bestKey = key
            }
        }
        return best
    }

    private fun pickApk(assets: JSONArray): UpdateAsset? {
        val abi = targetAbi()
        for (index in 0 until assets.length()) {
            val item = assets.optJSONObject(index) ?: continue
            val name = item.optString("name")
            val url = item.optString("browser_download_url")
            if (!name.endsWith(".apk", ignoreCase = true) || url.isBlank()) continue
            // A release carries `ProtoGrid-<ver>-<abi>.apk` and
            // `ProtoGrid-<ver>-lite-<abi>.apk`; check only our own core/ABI so
            // a full or x86 build never reports an update for a Lite/arm APK.
            val liteAsset = name.contains("-lite-", ignoreCase = true)
            if (liteAsset != BuildConfig.LITE_CORE) continue
            if (!name.contains(abi, ignoreCase = true)) continue
            return UpdateAsset(
                name = name,
                url = url,
                size = item.optLong("size"),
                digest = item.optString("digest").takeIf { it.isNotBlank() },
            )
        }
        return null
    }

    private fun versionOf(tag: String): String =
        tag.trim().removePrefix("android-v").removePrefix("v")

    private fun versionKey(version: String): List<Int> {
        val main = versionOf(version).substringBefore('-').substringBefore('+')
        return main.split('.').mapNotNull { it.trim().toIntOrNull() }
    }

    private fun compareKeys(left: List<Int>, right: List<Int>): Int {
        for (index in 0 until maxOf(left.size, right.size)) {
            val a = left.getOrElse(index) { 0 }
            val b = right.getOrElse(index) { 0 }
            if (a != b) return a.compareTo(b)
        }
        return 0
    }

    private fun isNewer(latest: String, current: String): Boolean {
        val left = versionKey(latest)
        val right = versionKey(current)
        if (left.isEmpty() || right.isEmpty()) return false
        return compareKeys(left, right) > 0
    }
}
