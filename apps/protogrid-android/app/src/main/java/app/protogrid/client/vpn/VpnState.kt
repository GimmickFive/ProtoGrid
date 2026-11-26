package app.protogrid.client.vpn

import kotlinx.coroutines.flow.MutableStateFlow

enum class VpnStatus {
    Stopped,
    Connecting,
    Connected,
    Error,
}

object VpnState {
    val status = MutableStateFlow(VpnStatus.Stopped)
    val message = MutableStateFlow<String?>(null)

    fun update(status: VpnStatus, message: String? = null) {
        this.status.value = status
        this.message.value = message
    }
}
