package app.protogrid.client.update

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.content.FileProvider
import app.protogrid.client.BuildConfig
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.util.Locale
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
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

data class UpdateUiState(
    val currentVersion: String = BuildConfig.VERSION_NAME,
    val checking: Boolean = false,
    val checked: Boolean = false,
    val release: AppRelease? = null,
    val downloading: Boolean = false,
    val progress: Float = 0f,
    val downloadedPath: String? = null,
    val error: String? = null,
) {
    val updateAvailable: Boolean get() = release != null
}

/**
 * Checks the public GitHub releases for a newer APK and downloads it on an
 * explicit click. The core (ProtoGrid-core/libbox) is packaged inside the APK,
 * so it moves to the new version together with the application.
 */
object UpdateManager {
    private const val REPO = "GimmickFive/ProtoGrid"
    private const val RELEASES_API =
        "https://api.github.com/repos/$REPO/releases?per_page=20"

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _state = MutableStateFlow(UpdateUiState())
    val state: StateFlow<UpdateUiState> = _state.asStateFlow()

    fun check() {
        if (_state.value.checking || _state.value.downloading) return
        _state.update { it.copy(checking = true, error = null) }
        scope.launch {
            runCatching { fetchLatestRelease() }
                .onSuccess { release ->
                    val newer = release != null &&
                        isNewer(release.version, BuildConfig.VERSION_NAME)
                    val next = if (newer) release else null
                    _state.update { current ->
                        val sameRelease = current.release?.version == next?.version
                        current.copy(
                            checking = false,
                            checked = true,
                            release = next,
                            downloadedPath = if (sameRelease) {
                                current.downloadedPath
                            } else {
                                null
                            },
                            progress = if (sameRelease) current.progress else 0f,
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

    fun downloadAndInstall(context: Context) {
        val appContext = context.applicationContext
        val release = _state.value.release ?: return
        if (_state.value.downloading) return
        _state.update {
            it.copy(
                downloading = true,
                progress = 0f,
                downloadedPath = null,
                error = null,
            )
        }
        scope.launch {
            try {
                val file = download(appContext, release.apk)
                _state.update {
                    it.copy(
                        downloading = false,
                        progress = 1f,
                        downloadedPath = file.absolutePath,
                    )
                }
                withContext(Dispatchers.Main) { install(context, file) }
            } catch (error: Throwable) {
                _state.update {
                    it.copy(
                        downloading = false,
                        error = error.message ?: "не удалось скачать APK",
                    )
                }
            }
        }
    }

    fun installDownloaded(context: Context) {
        val path = _state.value.downloadedPath ?: return
        val file = File(path)
        scope.launch {
            withContext(Dispatchers.Main) { install(context, file) }
        }
    }

    fun openReleasePage(context: Context) {
        val release = _state.value.release ?: return
        val url = release.pageUrl.ifBlank {
            "https://github.com/$REPO/releases/tag/${release.tag}"
        }
        runCatching {
            context.startActivity(
                Intent(Intent.ACTION_VIEW, Uri.parse(url))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        }
    }

    private fun install(context: Context, file: File) {
        if (!file.isFile) {
            _state.update { it.copy(error = "Скачанный APK не найден") }
            return
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            !context.packageManager.canRequestPackageInstalls()
        ) {
            runCatching {
                context.startActivity(
                    Intent(
                        Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                        Uri.parse("package:${context.packageName}"),
                    ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                )
            }
            _state.update {
                it.copy(
                    error = "Разрешите установку из этого источника, затем нажмите «Установить».",
                )
            }
            return
        }
        _state.update { it.copy(error = null) }
        val uri = FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            file,
        )
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, "application/vnd.android.package-archive")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        runCatching { context.startActivity(intent) }
            .onFailure { error ->
                _state.update {
                    it.copy(
                        error = "Не удалось открыть установщик: ${error.message}",
                    )
                }
            }
    }

    private fun download(context: Context, asset: UpdateAsset): File {
        val dir = File(context.filesDir, "updates").apply { mkdirs() }
        val name = File(asset.name).name.ifBlank { "protogrid-update.apk" }
        val target = File(dir, name)
        val temp = File(dir, "$name.part")
        val connection = (URL(asset.url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 20_000
            readTimeout = 90_000
            instanceFollowRedirects = true
            setRequestProperty("User-Agent", "ProtoGrid/${BuildConfig.VERSION_NAME}")
            setRequestProperty("Accept", "application/octet-stream")
        }
        try {
            val code = connection.responseCode
            if (code !in 200..299) {
                throw IllegalStateException("GitHub вернул HTTP $code")
            }
            val total = connection.contentLengthLong.takeIf { it > 0 } ?: asset.size
            connection.inputStream.use { input ->
                temp.outputStream().use { output ->
                    val buffer = ByteArray(64 * 1024)
                    var copied = 0L
                    var lastPercent = -1
                    while (true) {
                        val read = input.read(buffer)
                        if (read < 0) break
                        output.write(buffer, 0, read)
                        copied += read
                        if (total > 0) {
                            val percent = ((copied * 100) / total).toInt()
                            if (percent != lastPercent) {
                                lastPercent = percent
                                val progress = (copied.toFloat() / total).coerceIn(0f, 1f)
                                _state.update { it.copy(progress = progress) }
                            }
                        }
                    }
                }
            }
        } finally {
            connection.disconnect()
        }
        if (asset.size > 0 && temp.length() != asset.size) {
            temp.delete()
            throw IllegalStateException(
                "Размер APK не совпал: ожидалось ${asset.size} Б, получено ${temp.length()} Б",
            )
        }
        verifyDigest(temp, asset.digest)
        if (target.exists()) target.delete()
        if (!temp.renameTo(target)) {
            temp.copyTo(target, overwrite = true)
            temp.delete()
        }
        return target
    }

    private fun verifyDigest(file: File, digest: String?) {
        val expected = digest?.trim()
            ?.takeIf { it.isNotEmpty() }
            ?.removePrefix("sha256:")
            ?.lowercase(Locale.US)
            ?: return
        val md = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                md.update(buffer, 0, read)
            }
        }
        val actual = md.digest().joinToString("") { "%02x".format(it) }
        if (actual != expected) {
            file.delete()
            throw IllegalStateException("Контрольная сумма APK не совпала")
        }
    }

    private fun fetchLatestRelease(): AppRelease? {
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
        val wanted = when {
            Build.SUPPORTED_ABIS.any { it.contains("arm64", ignoreCase = true) } ->
                "arm64-v8a"
            Build.SUPPORTED_ABIS.any { it.contains("x86_64", ignoreCase = true) } ->
                "x86_64"
            else -> "arm64-v8a"
        }
        var fallback: UpdateAsset? = null
        for (index in 0 until assets.length()) {
            val item = assets.optJSONObject(index) ?: continue
            val name = item.optString("name")
            val url = item.optString("browser_download_url")
            if (!name.endsWith(".apk", ignoreCase = true) || url.isBlank()) continue
            // The same release carries `ProtoGrid-<ver>-<abi>.apk` and
            // `ProtoGrid-<ver>-lite-<abi>.apk`; never offer the other core.
            val liteAsset = name.contains("-lite-", ignoreCase = true)
            if (liteAsset != BuildConfig.LITE_CORE) continue
            val asset = UpdateAsset(
                name = name,
                url = url,
                size = item.optLong("size"),
                digest = item.optString("digest").takeIf { it.isNotBlank() },
            )
            if (name.contains(wanted, ignoreCase = true)) return asset
            if (fallback == null) fallback = asset
        }
        return fallback
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
