package app.protogrid.client

import android.app.Application
import android.os.Build
import android.util.Log
import app.protogrid.client.vpn.TrafficMonitor
import app.protogrid.client.widget.ProtoGridWidget
import io.nekohasekai.libbox.Libbox
import io.nekohasekai.libbox.SetupOptions
import org.json.JSONObject
import java.io.File

class ProtoGridApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        instance = this
        // The VPN service may be restarted by the system after the process was
        // killed; restore chart history before any UI or widget reads it.
        TrafficMonitor.restore(this)
        setupLibbox()
        // Existing widgets keep the old RemoteViews after an APK update until
        // the next tunnel state change; refresh them once on app start.
        runCatching { ProtoGridWidget.updateAll(this) }
    }

    private fun setupLibbox() {
        val working = File(filesDir, "libbox").apply { mkdirs() }
        val temp = File(cacheDir, "libbox").apply { mkdirs() }
        val options = SetupOptions().apply {
            basePath = filesDir.path
            workingPath = working.path
            tempPath = temp.path
            fixAndroidStack = false
            logMaxLines = 1000
            debug = BuildConfig.DEBUG
            crashReportSource = "ProtoGrid"
            appVersion = BuildConfig.VERSION_CODE.toString()
            appMarketingVersion = BuildConfig.VERSION_NAME
            platformMetadata = JSONObject().apply {
                put("os", "Android ${Build.VERSION.RELEASE}")
                put("sdk", Build.VERSION.SDK_INT)
                put("manufacturer", Build.MANUFACTURER)
                put("model", Build.MODEL)
                put("app", "ProtoGrid")
            }.toString()
        }
        runCatching { Libbox.setup(options) }
            .onFailure { Log.e(TAG, "libbox setup failed", it) }
    }

    companion object {
        private const val TAG = "ProtoGrid"

        lateinit var instance: ProtoGridApplication
            private set
    }
}
