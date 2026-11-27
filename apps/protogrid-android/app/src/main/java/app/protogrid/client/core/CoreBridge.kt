package app.protogrid.client.core

import app.protogrid.client.BuildConfig

/**
 * JNI bridge into `crates/protogrid-mobile`.
 *
 * Both functions return JSON. Errors are reported as `{ "error": "..." }`
 * instead of throwing across the JNI boundary.
 */
object CoreBridge {
    init {
        System.loadLibrary("protogrid_mobile")
        setLiteMode(BuildConfig.LITE_CORE)
    }

    /**
     * Tells the shared Rust core whether this APK links the Lite libbox.
     * Lite only registers VLESS over TCP, so the config builder must reject
     * every other protocol and transport before the core tries to run them.
     */
    @JvmStatic
    external fun setLiteMode(enabled: Boolean)

    /** Returns `{ "servers": [...], "errors": [...] }`. */
    @JvmStatic
    external fun parseLinks(input: String): String

    /**
     * Downloads a subscription URL through ProtoGrid-core.
     *
     * Returns `{ "servers": [...], "errors": [...], "userinfo": {...} | null,
     * "display_url": "..." }` or `{ "error": "..." }`. Blocks for up to 30 s,
     * so call it from a background dispatcher.
     */
    @JvmStatic
    external fun fetchSubscription(url: String): String

    /** Returns a ProtoGrid-core configuration or `{ "error": "..." }`. */
    @JvmStatic
    external fun buildConfig(
        serversJson: String,
        activeId: String,
        routingJson: String,
        apiSecret: String,
    ): String

    /** Downloads/caches GeoIP/GeoSite `.srs` files; returns a JSON status. */
    @JvmStatic
    external fun prepareRuleSets(rulesJson: String, dir: String, maxAgeSeconds: Long): String
}
