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
    /** null — Android не даёт публичного способа узнать состояние. */
    val alwaysOn = MutableStateFlow<Boolean?>(null)

    fun update(status: VpnStatus, message: String? = null) {
        this.status.value = status
        this.message.value = message
    }

    fun setAlwaysOn(value: Boolean?) {
        alwaysOn.value = value
    }
}
