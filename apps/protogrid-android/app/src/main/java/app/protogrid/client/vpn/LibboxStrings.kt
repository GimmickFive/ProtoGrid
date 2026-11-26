package app.protogrid.client.vpn

import io.nekohasekai.libbox.StringIterator

/** Kotlin string list exposed to libbox (per-app package filters). */
class LibboxStrings(private val iterator: Iterator<String>) : StringIterator {
    override fun len(): Int = 0

    override fun hasNext(): Boolean = iterator.hasNext()

    override fun next(): String = iterator.next()
}
