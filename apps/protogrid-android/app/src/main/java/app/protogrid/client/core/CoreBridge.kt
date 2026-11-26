package app.protogrid.client.core

/**
 * JNI bridge into `crates/protogrid-mobile`.
 *
 * Both functions return JSON. Errors are reported as `{ "error": "..." }`
 * instead of throwing across the JNI boundary.
 */
object CoreBridge {
    init {
        System.loadLibrary("protogrid_mobile")
    }

    /** Returns `{ "servers": [...], "errors": [...] }`. */
    @JvmStatic
    external fun parseLinks(input: String): String

    /** Returns a ProtoGrid-core configuration or `{ "error": "..." }`. */
    @JvmStatic
    external fun buildConfig(serversJson: String, activeId: String, routingJson: String): String

    /** Downloads/caches GeoIP/GeoSite `.srs` files; returns a JSON status. */
    @JvmStatic
    external fun prepareRuleSets(rulesJson: String, dir: String, maxAgeSeconds: Long): String
}
