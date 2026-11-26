package app.protogrid.client

import android.Manifest
import android.app.Activity
import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.pm.PackageManager
import android.net.VpnService
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.content.ContextCompat
import java.util.Locale
import java.text.SimpleDateFormat
import java.util.Date
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import app.protogrid.client.data.AndroidSettings
import app.protogrid.client.data.ProfileStore
import app.protogrid.client.data.ProfileSummary
import app.protogrid.client.data.SubscriptionInfo
import app.protogrid.client.ui.BrandLogo
import app.protogrid.client.ui.GhostButton
import app.protogrid.client.ui.IconAction
import app.protogrid.client.ui.Inter
import app.protogrid.client.ui.Mono
import app.protogrid.client.ui.Panel
import app.protogrid.client.ui.Pg
import app.protogrid.client.ui.PrimaryButton
import app.protogrid.client.ui.ProtocolBadge
import app.protogrid.client.ui.ProtoGridTheme
import app.protogrid.client.ui.RoutingScreen
import app.protogrid.client.ui.SectionLabel
import app.protogrid.client.ui.ServerRow
import app.protogrid.client.ui.StatTile
import app.protogrid.client.ui.StatusDot
import app.protogrid.client.ui.StatusPill
import app.protogrid.client.ui.statusColor
import app.protogrid.client.ui.statusLabel
import app.protogrid.client.update.UpdateManager
import app.protogrid.client.update.UpdateUiState
import app.protogrid.client.vpn.ProtoGridVpnService
import app.protogrid.client.vpn.TrafficMonitor
import app.protogrid.client.vpn.TrafficPoint
import app.protogrid.client.vpn.VpnState
import app.protogrid.client.vpn.VpnStatus
import app.protogrid.client.widget.ProtoGridWidget

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(0xFF0F1115.toInt()),
            navigationBarStyle = SystemBarStyle.dark(0xFF0F1115.toInt()),
        )
        setContent {
            ProtoGridTheme {
                ProtoGridApp()
            }
        }
    }
}

private enum class Tab {
    Overview,
    Servers,
    Routing,
    Monitor,
    Settings,
}

/** Version of the sing-box fork bundled in libbox.aar. The core is updated
 * together with the APK, so there is no separate core download here. */
private const val CORE_VERSION = "1.14.4"

@Composable
private fun ProtoGridApp() {
    val context = LocalContext.current
    val store = remember { ProfileStore(context) }
    val routingSettings = remember { AndroidSettings(context) }

    var tab by remember { mutableStateOf(Tab.Overview) }
    var profiles by remember { mutableStateOf(store.profiles()) }
    var activeId by remember { mutableStateOf(store.activeId()) }
    var subscriptions by remember { mutableStateOf(store.subscriptions()) }
    var showImport by remember { mutableStateOf(false) }
    var busy by remember { mutableStateOf<String?>(null) }
    var notice by remember { mutableStateOf<String?>(null) }
    var silentNotification by remember { mutableStateOf(routingSettings.silentNotification) }
    var notificationSpeed by remember { mutableStateOf(routingSettings.notificationSpeed) }

    val status by VpnState.status.collectAsState()
    val vpnMessage by VpnState.message.collectAsState()
    val update by UpdateManager.state.collectAsState()
    val activeProfile = profiles.firstOrNull { it.id == activeId } ?: profiles.firstOrNull()
    val reconnectScope = rememberCoroutineScope()

    LaunchedEffect(Unit) {
        if (!UpdateManager.state.value.checked) {
            UpdateManager.check()
        }
    }

    val vpnPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.StartActivityForResult(),
    ) { result ->
        if (result.resultCode == Activity.RESULT_OK) {
            ProtoGridVpnService.start(context)
        } else {
            notice = "Разрешение на VPN не выдано"
        }
    }

    val notificationPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { }

    LaunchedEffect(Unit) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.POST_NOTIFICATIONS,
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }

    fun connect() {
        if (profiles.isEmpty()) {
            notice = "Сначала добавьте сервер"
            return
        }
        notice = null
        val intent = VpnService.prepare(context)
        if (intent != null) {
            vpnPermission.launch(intent)
        } else {
            ProtoGridVpnService.start(context)
        }
    }

    fun select(profile: ProfileSummary) {
        store.setActive(profile.id)
        activeId = profile.id
        notice = null
    }

    fun reload() {
        profiles = store.profiles()
        subscriptions = store.subscriptions()
        activeId = store.activeId()
    }

    fun handleServersAfterDelete(previousActive: String?) {
        val connected = status == VpnStatus.Connected || status == VpnStatus.Connecting
        if (!connected) return
        if (profiles.isEmpty()) {
            ProtoGridVpnService.stop(context)
            notice = "Серверов не осталось — VPN отключён"
        } else if (activeId != previousActive) {
            // The tunnel was running the deleted server; bring it up again
            // with the newly selected one instead of keeping a dead config.
            ProtoGridVpnService.stop(context)
            reconnectScope.launch {
                delay(1200)
                ProtoGridVpnService.start(context)
            }
        }
    }

    fun delete(profile: ProfileSummary) {
        val previousActive = activeId
        store.delete(profile.id)
        reload()
        notice = "Сервер удалён"
        handleServersAfterDelete(previousActive)
    }

    fun applyRouting() {
        if (status == VpnStatus.Connected || status == VpnStatus.Connecting) {
            ProtoGridVpnService.stop(context)
            reconnectScope.launch {
                delay(1500)
                ProtoGridVpnService.start(context)
            }
        }
    }

    fun importText(text: String) {
        val result = store.importText(text)
        reload()
        showImport = false
        notice = when {
            result.errors.isNotEmpty() -> result.errors.joinToString("\n")
            result.added == 0 -> "Новых серверов не найдено"
            else -> "Добавлено: ${result.added}"
        }
        if (result.added > 0) {
            tab = Tab.Servers
        }
    }

    fun addSubscription(url: String, name: String) {
        if (busy != null) return
        busy = "Загружаем подписку…"
        reconnectScope.launch {
            val result = withContext(Dispatchers.IO) { store.addSubscription(url, name) }
            reload()
            busy = null
            if (result.added > 0) {
                showImport = false
                tab = Tab.Servers
                notice = "Подписка добавлена: ${result.added} ${profilesWord(result.added)}" +
                    if (result.errors.isNotEmpty()) "\n${result.errors.first()}" else ""
            } else {
                notice = result.errors.firstOrNull() ?: "В подписке не найдено серверов"
            }
        }
    }

    fun refreshSubscription(sub: SubscriptionInfo) {
        if (busy != null) return
        busy = "Обновляем «${sub.name}»…"
        reconnectScope.launch {
            val result = withContext(Dispatchers.IO) { store.refreshSubscription(sub.id) }
            reload()
            busy = null
            notice = when {
                result.errors.isNotEmpty() -> result.errors.joinToString("\n")
                else -> "Подписка обновлена: ${result.added} ${profilesWord(result.added)}"
            }
        }
    }

    fun deleteSubscription(sub: SubscriptionInfo) {
        val previousActive = activeId
        store.deleteSubscription(sub.id)
        reload()
        notice = "Подписка «${sub.name}» удалена"
        handleServersAfterDelete(previousActive)
    }

    fun setAutoUpdate(sub: SubscriptionInfo, enabled: Boolean) {
        store.setSubscriptionAutoUpdate(sub.id, enabled)
        reload()
    }

    fun pinWidget() {
        val manager = AppWidgetManager.getInstance(context)
        if (!manager.isRequestPinAppWidgetSupported) return
        manager.requestPinAppWidget(
            ComponentName(context, ProtoGridWidget::class.java),
            null,
            null,
        )
    }

    LaunchedEffect(Unit) {
        val stale = store.staleSubscriptionIds()
        if (stale.isNotEmpty()) {
            reconnectScope.launch {
                var failed = false
                for (id in stale) {
                    val result = withContext(Dispatchers.IO) { store.refreshSubscription(id) }
                    if (result.errors.isNotEmpty()) failed = true
                }
                reload()
                if (failed) notice = "Не удалось обновить часть подписок"
            }
        }
    }

    Scaffold(
        containerColor = Pg.bg,
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
        topBar = { TopBar(status) },
        bottomBar = { BottomBar(tab) { tab = it } },
    ) { padding ->
        Box(
            Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            key(tab) {
                when (tab) {
                    Tab.Overview -> OverviewScreen(
                        status = status,
                        message = vpnMessage ?: notice,
                        profiles = profiles,
                        activeProfile = activeProfile,
                        onToggle = {
                            if (status == VpnStatus.Connected || status == VpnStatus.Connecting) {
                                ProtoGridVpnService.stop(context)
                            } else {
                                connect()
                            }
                        },
                        onAdd = { showImport = true },
                        onOpenServers = { tab = Tab.Servers },
                        onSelect = ::select,
                        onDelete = ::delete,
                        routingSummary = "${routingSettings.trafficPreset.label} · ${routingSettings.perAppSummary()}",
                    )

                    Tab.Servers -> ServersScreen(
                        profiles = profiles,
                        subscriptions = subscriptions,
                        activeId = activeProfile?.id,
                        busy = busy,
                        onAdd = { showImport = true },
                        onSelect = ::select,
                        onDelete = ::delete,
                        onRefreshSubscription = ::refreshSubscription,
                        onDeleteSubscription = ::deleteSubscription,
                        onAutoUpdate = ::setAutoUpdate,
                    )

                    Tab.Routing -> RoutingScreen(
                        status = status,
                        onApply = ::applyRouting,
                    )

                    Tab.Monitor -> MonitorScreen(status = status)

                    Tab.Settings -> SettingsScreen(
                        update = update,
                        silentNotification = silentNotification,
                        notificationSpeed = notificationSpeed,
                        onSilentNotification = {
                            silentNotification = it
                            routingSettings.silentNotification = it
                        },
                        onNotificationSpeed = {
                            notificationSpeed = it
                            routingSettings.notificationSpeed = it
                        },
                        onPinWidget = ::pinWidget,
                    )
                }
            }
        }
    }

    if (showImport) {
        AddDialog(
            busy = busy,
            onDismiss = { showImport = false },
            onImport = ::importText,
            onAddSubscription = ::addSubscription,
        )
    }
}

@Composable
private fun TopBar(status: VpnStatus) {
    Column(
        Modifier
            .fillMaxWidth()
            .background(Pg.bg)
            .statusBarsPadding(),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(58.dp)
                .padding(horizontal = 18.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            BrandLogo()
            Spacer(Modifier.width(10.dp))
            Text(
                text = "ProtoGrid",
                color = Pg.text,
                fontFamily = Inter,
                fontWeight = FontWeight.Bold,
                fontSize = 19.sp,
                letterSpacing = (-0.3).sp,
            )
            Spacer(Modifier.weight(1f))
            StatusPill(status)
        }
        Box(
            Modifier
                .fillMaxWidth()
                .height(1.dp)
                .background(Pg.line),
        )
    }
}

@Composable
private fun BottomBar(tab: Tab, onSelect: (Tab) -> Unit) {
    Column(
        Modifier
            .fillMaxWidth()
            .background(Pg.sidebar)
            .navigationBarsPadding(),
    ) {
        Box(
            Modifier
                .fillMaxWidth()
                .height(1.dp)
                .background(Pg.line),
        )
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(58.dp)
                .padding(horizontal = 12.dp, vertical = 7.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            NavItem(
                label = "Обзор",
                icon = R.drawable.ic_grid,
                selected = tab == Tab.Overview,
                modifier = Modifier.weight(1f),
                onClick = { onSelect(Tab.Overview) },
            )
            NavItem(
                label = "Серверы",
                icon = R.drawable.ic_servers,
                selected = tab == Tab.Servers,
                modifier = Modifier.weight(1f),
                onClick = { onSelect(Tab.Servers) },
            )
            NavItem(
                label = "Маршрут",
                icon = R.drawable.ic_route,
                selected = tab == Tab.Routing,
                modifier = Modifier.weight(1f),
                onClick = { onSelect(Tab.Routing) },
            )
            NavItem(
                label = "Монитор",
                icon = R.drawable.ic_chart,
                selected = tab == Tab.Monitor,
                modifier = Modifier.weight(1f),
                onClick = { onSelect(Tab.Monitor) },
            )
            NavItem(
                label = "Настройки",
                icon = R.drawable.ic_settings,
                selected = tab == Tab.Settings,
                modifier = Modifier.weight(1f),
                onClick = { onSelect(Tab.Settings) },
            )
        }
    }
}

@Composable
private fun NavItem(
    label: String,
    icon: Int,
    selected: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    val color = if (selected) Pg.accent2 else Pg.faint
    Column(
        modifier = modifier
            .height(48.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(if (selected) Pg.accentSoft else Color.Transparent)
            .clickable(onClick = onClick),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Icon(
            painter = painterResource(icon),
            contentDescription = null,
            tint = color,
            modifier = Modifier.size(17.dp),
        )
        Spacer(Modifier.height(2.dp))
        Text(
            text = label,
            color = color,
            fontFamily = Inter,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
            fontSize = 9.sp,
            maxLines = 1,
        )
    }
}

@Composable
private fun OverviewScreen(
    status: VpnStatus,
    message: String?,
    profiles: List<ProfileSummary>,
    activeProfile: ProfileSummary?,
    onToggle: () -> Unit,
    onAdd: () -> Unit,
    onOpenServers: () -> Unit,
    onSelect: (ProfileSummary) -> Unit,
    onDelete: (ProfileSummary) -> Unit,
    routingSummary: String,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 18.dp),
    ) {
        Row(verticalAlignment = Alignment.Bottom) {
            Column(Modifier.weight(1f)) {
                Text(
                    text = "Обзор",
                    color = Pg.text,
                    fontFamily = Inter,
                    fontWeight = FontWeight.Bold,
                    fontSize = 26.sp,
                    letterSpacing = (-0.5).sp,
                )
                Spacer(Modifier.height(2.dp))
                Text(
                    text = "Состояние подключения",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 12.sp,
                )
            }
            Text(
                text = "v${BuildConfig.VERSION_NAME}",
                color = Pg.faint,
                fontFamily = Mono,
                fontSize = 10.sp,
            )
        }

        Spacer(Modifier.height(18.dp))

        Panel(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(18.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    StatusDot(statusColor(status), 9.dp)
                    Spacer(Modifier.width(9.dp))
                    Text(
                        text = statusLabel(status),
                        color = Pg.text,
                        fontFamily = Inter,
                        fontWeight = FontWeight.SemiBold,
                        fontSize = 18.sp,
                    )
                    Spacer(Modifier.weight(1f))
                    Text(
                        text = "ProtoGrid-core $CORE_VERSION",
                        color = Pg.faint,
                        fontFamily = Mono,
                        fontSize = 9.sp,
                    )
                }

                if (!message.isNullOrBlank()) {
                    Spacer(Modifier.height(8.dp))
                    Text(
                        text = message,
                        color = if (status == VpnStatus.Error) Pg.red else Pg.muted,
                        fontFamily = Inter,
                        fontSize = 11.sp,
                        lineHeight = 15.sp,
                    )
                }

                Spacer(Modifier.height(14.dp))
                Box(
                    Modifier
                        .fillMaxWidth()
                        .height(1.dp)
                        .background(Pg.line),
                )
                Spacer(Modifier.height(14.dp))

                SectionLabel(
                    if (activeProfile != null) "Активный сервер" else "Сервер не выбран",
                )
                Spacer(Modifier.height(6.dp))

                if (activeProfile != null) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            text = activeProfile.name,
                            modifier = Modifier.weight(1f),
                            color = Pg.text,
                            fontFamily = Inter,
                            fontWeight = FontWeight.SemiBold,
                            fontSize = 16.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Spacer(Modifier.width(8.dp))
                        ProtocolBadge(activeProfile.protocol)
                    }
                    Spacer(Modifier.height(3.dp))
                    Text(
                        text = activeProfile.endpoint,
                        color = Pg.faint,
                        fontFamily = Mono,
                        fontSize = 11.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                } else {
                    Text(
                        text = "Добавьте share-ссылку или подписку, чтобы выбрать сервер",
                        color = Pg.muted,
                        fontFamily = Inter,
                        fontSize = 12.sp,
                    )
                }

                Spacer(Modifier.height(10.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    SectionLabel("Маршрут")
                    Spacer(Modifier.width(8.dp))
                    Text(
                        text = routingSummary,
                        color = Pg.muted,
                        fontFamily = Inter,
                        fontSize = 10.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }

                Spacer(Modifier.height(18.dp))

                PrimaryButton(
                    text = if (status == VpnStatus.Connected || status == VpnStatus.Connecting) {
                        "Отключить"
                    } else {
                        "Подключить"
                    },
                    onClick = onToggle,
                    modifier = Modifier.fillMaxWidth(),
                    icon = painterResource(R.drawable.ic_power),
                )

                if (activeProfile == null) {
                    Spacer(Modifier.height(10.dp))
                    GhostButton(
                        text = "Добавить сервер",
                        onClick = onAdd,
                        modifier = Modifier.fillMaxWidth(),
                        icon = painterResource(R.drawable.ic_add),
                    )
                }
            }
        }

        Spacer(Modifier.height(14.dp))

        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            StatTile(
                label = "Серверы",
                value = profiles.size.toString(),
                hint = "в конфигурации",
                modifier = Modifier.weight(1f),
            )
            StatTile(
                label = "Ядро",
                value = CORE_VERSION,
                hint = "ProtoGrid-core",
                modifier = Modifier.weight(1f),
                valueMono = true,
            )
        }

        Spacer(Modifier.height(20.dp))

        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = "Серверы",
                modifier = Modifier.weight(1f),
                color = Pg.text,
                fontFamily = Inter,
                fontWeight = FontWeight.SemiBold,
                fontSize = 17.sp,
            )
            Text(
                text = "Все →",
                modifier = Modifier
                    .clip(RoundedCornerShape(8.dp))
                    .clickable(onClick = onOpenServers)
                    .padding(horizontal = 6.dp, vertical = 4.dp),
                color = Pg.accent2,
                fontFamily = Inter,
                fontWeight = FontWeight.Medium,
                fontSize = 12.sp,
            )
        }

        Spacer(Modifier.height(10.dp))

        if (profiles.isEmpty()) {
            Panel(Modifier.fillMaxWidth()) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(20.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Text(
                        text = "Пока нет серверов",
                        color = Pg.text,
                        fontFamily = Inter,
                        fontWeight = FontWeight.SemiBold,
                        fontSize = 14.sp,
                    )
                    Spacer(Modifier.height(4.dp))
                    Text(
                        text = "Вставьте одну или несколько share-ссылок",
                        color = Pg.faint,
                        fontFamily = Inter,
                        fontSize = 11.sp,
                        textAlign = TextAlign.Center,
                    )
                    Spacer(Modifier.height(14.dp))
                    PrimaryButton(
                        text = "Добавить сервер",
                        onClick = onAdd,
                        icon = painterResource(R.drawable.ic_add),
                    )
                }
            }
        } else {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                profiles.take(3).forEach { profile ->
                    ServerRow(
                        profile = profile,
                        selected = profile.id == activeProfile?.id,
                        onSelect = { onSelect(profile) },
                        onDelete = { onDelete(profile) },
                    )
                }
            }
        }

        Spacer(Modifier.height(16.dp))
    }
}

@Composable
private fun UpdateCard(state: UpdateUiState) {
    val context = LocalContext.current
    val release = state.release
    Panel(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    SectionLabel("Обновление")
                    Spacer(Modifier.height(4.dp))
                    Text(
                        text = "Версия ${state.currentVersion}",
                        color = Pg.text,
                        fontFamily = Inter,
                        fontWeight = FontWeight.SemiBold,
                        fontSize = 14.sp,
                    )
                    Text(
                        text = "Ядро ProtoGrid-core $CORE_VERSION входит в APK",
                        color = Pg.faint,
                        fontFamily = Inter,
                        fontSize = 10.sp,
                    )
                }
                Text(
                    text = when {
                        state.downloading -> "${(state.progress * 100).toInt()}%"
                        release != null -> "v${release.version}"
                        state.checked && state.error == null -> "актуально"
                        else -> ""
                    },
                    color = if (release != null) Pg.amber else Pg.faint,
                    fontFamily = Mono,
                    fontSize = 10.sp,
                )
            }

            Spacer(Modifier.height(10.dp))

            when {
                state.downloading -> {
                    Text(
                        text = "Скачивание APK с GitHub…",
                        color = Pg.muted,
                        fontFamily = Inter,
                        fontSize = 11.sp,
                    )
                    Spacer(Modifier.height(8.dp))
                    LinearProgressIndicator(
                        progress = { state.progress },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(6.dp)
                            .clip(RoundedCornerShape(3.dp)),
                        color = Pg.accent2,
                        trackColor = Pg.panel2,
                    )
                }

                state.downloadedPath != null -> {
                    Text(
                        text = "APK скачан. Установите обновление поверх текущей версии.",
                        color = Pg.muted,
                        fontFamily = Inter,
                        fontSize = 11.sp,
                    )
                    state.error?.let { error ->
                        Spacer(Modifier.height(6.dp))
                        Text(
                            text = error,
                            color = Pg.red,
                            fontFamily = Inter,
                            fontSize = 10.sp,
                        )
                    }
                    Spacer(Modifier.height(10.dp))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        GhostButton(
                            text = "Проверить",
                            onClick = { UpdateManager.check() },
                        )
                        Spacer(Modifier.width(8.dp))
                        PrimaryButton(
                            text = "Установить",
                            onClick = { UpdateManager.installDownloaded(context) },
                            modifier = Modifier.weight(1f),
                        )
                    }
                }

                release != null -> {
                    Text(
                        text = "Доступна версия ${release.version}.",
                        color = Pg.amber,
                        fontFamily = Inter,
                        fontWeight = FontWeight.Medium,
                        fontSize = 12.sp,
                    )
                    release.notes
                        .lineSequence()
                        .firstOrNull { it.isNotBlank() }
                        ?.trim()
                        ?.trimStart('#')
                        ?.trim()
                        ?.take(140)
                        ?.takeIf { it.isNotBlank() }
                        ?.let { note ->
                            Spacer(Modifier.height(4.dp))
                            Text(
                                text = note,
                                color = Pg.faint,
                                fontFamily = Inter,
                                fontSize = 10.sp,
                                maxLines = 2,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    state.error?.let { error ->
                        Spacer(Modifier.height(6.dp))
                        Text(
                            text = error,
                            color = Pg.red,
                            fontFamily = Inter,
                            fontSize = 10.sp,
                        )
                    }
                    Spacer(Modifier.height(10.dp))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        PrimaryButton(
                            text = "Скачать и установить",
                            onClick = { UpdateManager.downloadAndInstall(context) },
                            modifier = Modifier.weight(1f),
                        )
                        Spacer(Modifier.width(6.dp))
                        TextButton(
                            onClick = { UpdateManager.openReleasePage(context) },
                            colors = ButtonDefaults.textButtonColors(
                                contentColor = Pg.accent2,
                            ),
                        ) {
                            Text(
                                text = "Что нового",
                                fontFamily = Inter,
                                fontWeight = FontWeight.Medium,
                                fontSize = 11.sp,
                            )
                        }
                    }
                }

                state.checking -> {
                    Text(
                        text = "Проверяем обновления на GitHub…",
                        color = Pg.muted,
                        fontFamily = Inter,
                        fontSize = 11.sp,
                    )
                }

                state.error != null -> {
                    Text(
                        text = "Не удалось проверить обновления: ${state.error}",
                        color = Pg.red,
                        fontFamily = Inter,
                        fontSize = 10.sp,
                    )
                    Spacer(Modifier.height(8.dp))
                    GhostButton(
                        text = "Повторить",
                        onClick = { UpdateManager.check() },
                    )
                }

                state.checked -> {
                    Text(
                        text = "Установлена последняя версия.",
                        color = Pg.muted,
                        fontFamily = Inter,
                        fontSize = 11.sp,
                    )
                    Spacer(Modifier.height(8.dp))
                    GhostButton(
                        text = "Проверить снова",
                        onClick = { UpdateManager.check() },
                    )
                }

                else -> {
                    GhostButton(
                        text = "Проверить обновления",
                        onClick = { UpdateManager.check() },
                    )
                }
            }
        }
    }
}

@Composable
private fun SettingsScreen(
    update: UpdateUiState,
    silentNotification: Boolean,
    notificationSpeed: Boolean,
    onSilentNotification: (Boolean) -> Unit,
    onNotificationSpeed: (Boolean) -> Unit,
    onPinWidget: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 18.dp),
    ) {
        Row(verticalAlignment = Alignment.Bottom) {
            Column(Modifier.weight(1f)) {
                Text(
                    text = "Настройки",
                    color = Pg.text,
                    fontFamily = Inter,
                    fontWeight = FontWeight.Bold,
                    fontSize = 26.sp,
                    letterSpacing = (-0.5).sp,
                )
                Spacer(Modifier.height(2.dp))
                Text(
                    text = "Уведомления, виджет и приложение",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 12.sp,
                )
            }
            Text(
                text = "v${BuildConfig.VERSION_NAME}",
                color = Pg.faint,
                fontFamily = Mono,
                fontSize = 10.sp,
            )
        }

        Spacer(Modifier.height(18.dp))

        Panel(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(16.dp)) {
                SectionLabel("Уведомление")
                Spacer(Modifier.height(12.dp))
                SettingSwitch(
                    title = "Тихая шторка",
                    subtitle = "Уведомление VPN без звука и всплывания",
                    checked = silentNotification,
                    onChange = onSilentNotification,
                )
                Spacer(Modifier.height(12.dp))
                SettingSwitch(
                    title = "Показывать скорость",
                    subtitle = "Скачивание и отправка в тексте уведомления",
                    checked = notificationSpeed,
                    onChange = onNotificationSpeed,
                )
                Spacer(Modifier.height(8.dp))
                Text(
                    text = "Изменения применятся при следующем подключении.",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 10.sp,
                )
            }
        }

        Spacer(Modifier.height(14.dp))

        Panel(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(16.dp)) {
                SectionLabel("Виджет")
                Spacer(Modifier.height(8.dp))
                Text(
                    text = "Виджет на главном экране показывает статус VPN и переключает его по нажатию.",
                    color = Pg.muted,
                    fontFamily = Inter,
                    fontSize = 11.sp,
                    lineHeight = 16.sp,
                )
                Spacer(Modifier.height(12.dp))
                GhostButton(
                    text = "Закрепить виджет",
                    onClick = onPinWidget,
                    modifier = Modifier.fillMaxWidth(),
                    icon = painterResource(R.drawable.ic_add),
                )
                Spacer(Modifier.height(10.dp))
                Text(
                    text = "Плитка ProtoGrid в шторке: добавьте её через «Изменить плитки» в быстрых настройках.",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 10.sp,
                    lineHeight = 14.sp,
                )
            }
        }

        Spacer(Modifier.height(14.dp))

        UpdateCard(update)

        Spacer(Modifier.height(14.dp))

        Panel(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(16.dp)) {
                SectionLabel("О приложении")
                Spacer(Modifier.height(10.dp))
                InfoRow("Версия", BuildConfig.VERSION_NAME)
                Spacer(Modifier.height(6.dp))
                InfoRow("Ядро", "ProtoGrid-core $CORE_VERSION")
            }
        }

        Spacer(Modifier.height(20.dp))
    }
}

@Composable
private fun SettingSwitch(
    title: String,
    subtitle: String,
    checked: Boolean,
    onChange: (Boolean) -> Unit,
) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(
                text = title,
                color = Pg.text,
                fontFamily = Inter,
                fontWeight = FontWeight.SemiBold,
                fontSize = 14.sp,
            )
            Spacer(Modifier.height(2.dp))
            Text(
                text = subtitle,
                color = Pg.faint,
                fontFamily = Inter,
                fontSize = 11.sp,
                lineHeight = 15.sp,
            )
        }
        Spacer(Modifier.width(8.dp))
        Switch(
            checked = checked,
            onCheckedChange = onChange,
            modifier = Modifier.height(28.dp),
            colors = SwitchDefaults.colors(
                checkedThumbColor = Color.White,
                checkedTrackColor = Pg.accent,
                uncheckedThumbColor = Pg.muted,
                uncheckedTrackColor = Pg.panel2,
            ),
        )
    }
}

@Composable
private fun InfoRow(label: String, value: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(
            text = label,
            modifier = Modifier.weight(1f),
            color = Pg.muted,
            fontFamily = Inter,
            fontSize = 12.sp,
        )
        Text(
            text = value,
            color = Pg.text,
            fontFamily = Mono,
            fontSize = 11.sp,
        )
    }
}

@Composable
private fun ServersScreen(
    profiles: List<ProfileSummary>,
    subscriptions: List<SubscriptionInfo>,
    activeId: String?,
    busy: String?,
    onAdd: () -> Unit,
    onSelect: (ProfileSummary) -> Unit,
    onDelete: (ProfileSummary) -> Unit,
    onRefreshSubscription: (SubscriptionInfo) -> Unit,
    onDeleteSubscription: (SubscriptionInfo) -> Unit,
    onAutoUpdate: (SubscriptionInfo, Boolean) -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 16.dp, vertical = 18.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(
                    text = "Серверы",
                    color = Pg.text,
                    fontFamily = Inter,
                    fontWeight = FontWeight.Bold,
                    fontSize = 26.sp,
                    letterSpacing = (-0.5).sp,
                )
                Spacer(Modifier.height(2.dp))
                Text(
                    text = "${profiles.size} ${profilesWord(profiles.size)}" +
                        if (subscriptions.isNotEmpty()) {
                            " · ${subscriptions.size} ${subscriptionWord(subscriptions.size)}"
                        } else {
                            ""
                        },
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 12.sp,
                )
            }
            PrimaryButton(
                text = "Добавить",
                onClick = onAdd,
                icon = painterResource(R.drawable.ic_add),
            )
        }

        Spacer(Modifier.height(16.dp))

        LazyColumn(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            contentPadding = PaddingValues(bottom = 16.dp),
        ) {
            if (subscriptions.isNotEmpty()) {
                item(key = "subscriptions-label") {
                    SectionLabel("Подписки")
                }
                items(subscriptions, key = { "sub-${it.id}" }) { sub ->
                    SubscriptionCard(
                        sub = sub,
                        busy = busy != null,
                        onRefresh = { onRefreshSubscription(sub) },
                        onDelete = { onDeleteSubscription(sub) },
                        onAutoUpdate = { onAutoUpdate(sub, it) },
                    )
                }
                item(key = "servers-label") {
                    Spacer(Modifier.height(6.dp))
                    SectionLabel("Серверы")
                }
            }

            if (profiles.isEmpty()) {
                item(key = "empty-servers") {
                    EmptyServers(onAdd)
                }
            } else {
                items(profiles, key = { it.id }) { profile ->
                    ServerRow(
                        profile = profile,
                        selected = profile.id == activeId,
                        onSelect = { onSelect(profile) },
                        onDelete = { onDelete(profile) },
                    )
                }
            }
        }
    }
}

@Composable
private fun SubscriptionCard(
    sub: SubscriptionInfo,
    busy: Boolean,
    onRefresh: () -> Unit,
    onDelete: () -> Unit,
    onAutoUpdate: (Boolean) -> Unit,
) {
    val now = System.currentTimeMillis() / 1000
    val fraction = if (sub.total > 0) {
        (sub.used.toFloat() / sub.total.toFloat()).coerceIn(0f, 1f)
    } else {
        0f
    }

    Panel(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(14.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(
                        text = sub.name,
                        color = Pg.text,
                        fontFamily = Inter,
                        fontWeight = FontWeight.SemiBold,
                        fontSize = 15.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Spacer(Modifier.height(2.dp))
                    Text(
                        text = sub.displayUrl,
                        color = Pg.faint,
                        fontFamily = Mono,
                        fontSize = 10.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Spacer(Modifier.width(8.dp))
                Switch(
                    checked = sub.autoUpdate,
                    onCheckedChange = onAutoUpdate,
                    modifier = Modifier.height(28.dp),
                    colors = SwitchDefaults.colors(
                        checkedThumbColor = Color.White,
                        checkedTrackColor = Pg.accent,
                        uncheckedThumbColor = Pg.muted,
                        uncheckedTrackColor = Pg.panel2,
                    ),
                )
            }

            Spacer(Modifier.height(10.dp))

            when {
                sub.total > 0 -> {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            text = "${formatBytes(sub.used)} из ${formatBytes(sub.total)}",
                            color = Pg.muted,
                            fontFamily = Inter,
                            fontSize = 11.sp,
                        )
                        Spacer(Modifier.weight(1f))
                        Text(
                            text = "${(fraction * 100).toInt()}%",
                            color = Pg.faint,
                            fontFamily = Mono,
                            fontSize = 10.sp,
                        )
                    }
                    Spacer(Modifier.height(5.dp))
                    LinearProgressIndicator(
                        progress = { fraction },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(5.dp)
                            .clip(RoundedCornerShape(3.dp)),
                        color = Pg.accent2,
                        trackColor = Pg.panel2,
                    )
                }

                sub.used > 0 -> Text(
                    text = "Использовано ${formatBytes(sub.used)}",
                    color = Pg.muted,
                    fontFamily = Inter,
                    fontSize = 11.sp,
                )
            }

            if (sub.expire > 0) {
                Spacer(Modifier.height(6.dp))
                val days = ((sub.expire - now) / 86_400).toInt()
                Text(
                    text = "до ${formatDate(sub.expire)}" +
                        when {
                            days < 0 -> " · истекла"
                            else -> " · осталось $days дн."
                        },
                    color = if (days in 0..3) Pg.amber else Pg.faint,
                    fontFamily = Inter,
                    fontSize = 10.sp,
                )
            }

            Spacer(Modifier.height(6.dp))
            Text(
                text = "${sub.serverCount} ${profilesWord(sub.serverCount)}" +
                    if (sub.lastUpdated != null) {
                        " · обновлено ${formatDateTime(sub.lastUpdated)}"
                    } else {
                        ""
                    },
                color = Pg.faint,
                fontFamily = Inter,
                fontSize = 10.sp,
            )

            sub.lastError?.let { error ->
                Spacer(Modifier.height(6.dp))
                Text(
                    text = error,
                    color = Pg.red,
                    fontFamily = Inter,
                    fontSize = 10.sp,
                    lineHeight = 14.sp,
                )
            }

            Spacer(Modifier.height(4.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                TextButton(
                    onClick = onRefresh,
                    enabled = !busy,
                    colors = ButtonDefaults.textButtonColors(
                        contentColor = Pg.accent2,
                        disabledContentColor = Pg.faint,
                    ),
                    contentPadding = PaddingValues(horizontal = 0.dp, vertical = 4.dp),
                ) {
                    Text(
                        text = "Обновить",
                        fontFamily = Inter,
                        fontWeight = FontWeight.Medium,
                        fontSize = 12.sp,
                    )
                }
                Spacer(Modifier.weight(1f))
                TextButton(
                    onClick = onDelete,
                    enabled = !busy,
                    colors = ButtonDefaults.textButtonColors(
                        contentColor = Pg.red,
                        disabledContentColor = Pg.faint,
                    ),
                    contentPadding = PaddingValues(horizontal = 0.dp, vertical = 4.dp),
                ) {
                    Text(
                        text = "Удалить",
                        fontFamily = Inter,
                        fontWeight = FontWeight.Medium,
                        fontSize = 12.sp,
                    )
                }
            }
        }
    }
}

@Composable
private fun EmptyServers(onAdd: () -> Unit) {
    Panel(Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp, vertical = 36.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Box(
                modifier = Modifier
                    .size(54.dp)
                    .clip(CircleShape)
                    .background(Pg.panel2),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    painter = painterResource(R.drawable.ic_servers),
                    contentDescription = null,
                    tint = Pg.faint,
                    modifier = Modifier.size(24.dp),
                )
            }
            Spacer(Modifier.height(14.dp))
            Text(
                text = "Пока нет серверов",
                color = Pg.text,
                fontFamily = Inter,
                fontWeight = FontWeight.SemiBold,
                fontSize = 15.sp,
            )
            Spacer(Modifier.height(4.dp))
            Text(
                text = "Добавьте share-ссылки, подписку или .ovpn: vless, vmess, trojan, ss, hy2",
                color = Pg.faint,
                fontFamily = Inter,
                fontSize = 11.sp,
                lineHeight = 16.sp,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(16.dp))
            PrimaryButton(
                text = "Добавить сервер",
                onClick = onAdd,
                icon = painterResource(R.drawable.ic_add),
            )
        }
    }
}

@Composable
private fun AddDialog(
    busy: String?,
    onDismiss: () -> Unit,
    onImport: (String) -> Unit,
    onAddSubscription: (String, String) -> Unit,
) {
    var mode by remember { mutableStateOf(AddMode.Links) }
    var text by remember { mutableStateOf("") }
    var name by remember { mutableStateOf("") }
    var url by remember { mutableStateOf("") }
    var fileError by remember { mutableStateOf<String?>(null) }
    val clipboard = LocalClipboardManager.current
    val context = LocalContext.current
    val locked = busy != null

    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            val content = runCatching {
                context.contentResolver.openInputStream(uri)
                    ?.bufferedReader()
                    ?.use { it.readText() }
            }.getOrNull()
            if (content.isNullOrBlank()) {
                fileError = "Не удалось прочитать выбранный файл"
            } else {
                fileError = null
                onImport(content)
            }
        }
    }

    Dialog(
        onDismissRequest = { if (!locked) onDismiss() },
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth(0.92f)
                .clip(RoundedCornerShape(16.dp))
                .background(Pg.panel)
                .padding(20.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(
                        text = "Добавить",
                        color = Pg.text,
                        fontFamily = Inter,
                        fontWeight = FontWeight.Bold,
                        fontSize = 19.sp,
                    )
                    Spacer(Modifier.height(2.dp))
                    Text(
                        text = when (mode) {
                            AddMode.Links -> "Одна или несколько share-ссылок"
                            AddMode.Subscription -> "Серверы загрузятся из подписки"
                            AddMode.OpenVpn -> "Импорт файла .ovpn"
                        },
                        color = Pg.faint,
                        fontFamily = Inter,
                        fontSize = 11.sp,
                    )
                }
                IconAction(
                    painter = painterResource(R.drawable.ic_close),
                    contentDescription = "Закрыть",
                    onClick = { if (!locked) onDismiss() },
                )
            }

            Spacer(Modifier.height(14.dp))

            AddModeTabs(mode) { mode = it }

            Spacer(Modifier.height(14.dp))

            when (mode) {
                AddMode.Links -> {
                    PgField(
                        value = text,
                        onValueChange = { text = it },
                        placeholder = "vless://…\nvmess://…\ntrojan://…\nss://…  hy2://…",
                        minLines = 5,
                        maxLines = 8,
                    )
                }

                AddMode.Subscription -> {
                    PgField(
                        value = name,
                        onValueChange = { name = it },
                        placeholder = "Название (необязательно)",
                        singleLine = true,
                    )
                    Spacer(Modifier.height(8.dp))
                    PgField(
                        value = url,
                        onValueChange = { url = it },
                        placeholder = "https://…/sub/…",
                        singleLine = true,
                    )
                    Spacer(Modifier.height(6.dp))
                    Text(
                        text = "Только https. Серверы и трафик подписки обновляются автоматически.",
                        color = Pg.faint,
                        fontFamily = Inter,
                        fontSize = 10.sp,
                        lineHeight = 14.sp,
                    )
                }

                AddMode.OpenVpn -> {
                    Text(
                        text = "Выберите файл .ovpn. Поддерживаются профили со встроенными сертификатами и ключами (inline), без внешних файлов.",
                        color = Pg.muted,
                        fontFamily = Inter,
                        fontSize = 11.sp,
                        lineHeight = 16.sp,
                    )
                    Spacer(Modifier.height(12.dp))
                    GhostButton(
                        text = "Выбрать .ovpn",
                        onClick = {
                            fileError = null
                            picker.launch(arrayOf("*/*"))
                        },
                        modifier = Modifier.fillMaxWidth(),
                        icon = painterResource(R.drawable.ic_add),
                    )
                    fileError?.let { error ->
                        Spacer(Modifier.height(8.dp))
                        Text(
                            text = error,
                            color = Pg.red,
                            fontFamily = Inter,
                            fontSize = 10.sp,
                        )
                    }
                }
            }

            Spacer(Modifier.height(4.dp))

            if (mode != AddMode.OpenVpn) {
                TextButton(
                    onClick = {
                        val pasted = clipboard.getText()?.text?.trim().orEmpty()
                        if (pasted.isNotBlank()) {
                            if (pasted.startsWith("http://", true) || pasted.startsWith("https://", true)) {
                                mode = AddMode.Subscription
                                url = pasted
                            } else {
                                text = pasted
                            }
                        }
                    },
                    enabled = !locked,
                    colors = ButtonDefaults.textButtonColors(contentColor = Pg.accent2),
                    contentPadding = PaddingValues(horizontal = 0.dp, vertical = 4.dp),
                ) {
                    Text(
                        text = "Вставить из буфера",
                        fontFamily = Inter,
                        fontWeight = FontWeight.Medium,
                        fontSize = 12.sp,
                    )
                }
            }

            Spacer(Modifier.height(10.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                TextButton(
                    onClick = onDismiss,
                    enabled = !locked,
                    colors = ButtonDefaults.textButtonColors(contentColor = Pg.muted),
                ) {
                    Text(
                        text = "Отмена",
                        fontFamily = Inter,
                        fontWeight = FontWeight.Medium,
                        fontSize = 12.sp,
                    )
                }
                Spacer(Modifier.weight(1f))
                PrimaryButton(
                    text = busy ?: when (mode) {
                        AddMode.Links -> "Импортировать"
                        AddMode.Subscription -> "Добавить подписку"
                        AddMode.OpenVpn -> "Выбрать файл"
                    },
                    onClick = {
                        when (mode) {
                            AddMode.Links -> onImport(text)
                            AddMode.Subscription -> onAddSubscription(url, name)
                            AddMode.OpenVpn -> {
                                fileError = null
                                picker.launch(arrayOf("*/*"))
                            }
                        }
                    },
                    enabled = !locked && when (mode) {
                        AddMode.Links -> text.isNotBlank()
                        AddMode.Subscription -> url.isNotBlank()
                        AddMode.OpenVpn -> true
                    },
                )
            }
        }
    }
}

private enum class AddMode {
    Links,
    Subscription,
    OpenVpn,
}

@Composable
private fun AddModeTabs(mode: AddMode, onSelect: (AddMode) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .background(Pg.bg2)
            .padding(4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        listOf(
            AddMode.Links to "Ссылки",
            AddMode.Subscription to "Подписка",
            AddMode.OpenVpn to "OpenVPN",
        ).forEach { (id, label) ->
            val selected = mode == id
            Box(
                modifier = Modifier
                    .weight(1f)
                    .clip(RoundedCornerShape(8.dp))
                    .background(if (selected) Pg.accentSoft else Color.Transparent)
                    .clickable { onSelect(id) }
                    .padding(vertical = 8.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = label,
                    color = if (selected) Pg.accent2 else Pg.muted,
                    fontFamily = Inter,
                    fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
                    fontSize = 11.sp,
                )
            }
        }
    }
}

@Composable
private fun PgField(
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    minLines: Int = 1,
    maxLines: Int = 1,
    singleLine: Boolean = false,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = Modifier.fillMaxWidth(),
        singleLine = singleLine,
        minLines = minLines,
        maxLines = maxLines,
        textStyle = TextStyle(
            fontFamily = Mono,
            fontSize = 12.sp,
            color = Pg.text,
            lineHeight = 18.sp,
        ),
        placeholder = {
            Text(
                text = placeholder,
                color = Pg.faint,
                fontFamily = Mono,
                fontSize = 12.sp,
                lineHeight = 18.sp,
            )
        },
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
        shape = RoundedCornerShape(10.dp),
        colors = OutlinedTextFieldDefaults.colors(
            focusedTextColor = Pg.text,
            unfocusedTextColor = Pg.text,
            focusedContainerColor = Pg.bg2,
            unfocusedContainerColor = Pg.bg2,
            cursorColor = Pg.accent2,
            focusedBorderColor = Pg.accent,
            unfocusedBorderColor = Pg.lineStrong,
            focusedPlaceholderColor = Pg.faint,
            unfocusedPlaceholderColor = Pg.faint,
        ),
    )
}

private enum class MonitorMode(val label: String) {
    Total("Всего"),
    Proxy("Прокси"),
    Direct("Напрямую"),
}

@Composable
private fun MonitorTabs(mode: MonitorMode, onSelect: (MonitorMode) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(9.dp))
            .background(Pg.bg2)
            .padding(3.dp),
        horizontalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        MonitorMode.entries.forEach { item ->
            val selected = item == mode
            Box(
                modifier = Modifier
                    .weight(1f)
                    .clip(RoundedCornerShape(7.dp))
                    .background(if (selected) Pg.accentSoft else Color.Transparent)
                    .clickable { onSelect(item) }
                    .padding(vertical = 7.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = item.label,
                    color = if (selected) Pg.accent2 else Pg.muted,
                    fontFamily = Inter,
                    fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
                    fontSize = 11.sp,
                )
            }
        }
    }
}

@Composable
private fun MonitorScreen(status: VpnStatus) {
    var mode by remember { mutableStateOf(MonitorMode.Total) }

    val totalPoints by TrafficMonitor.points.collectAsState()
    val proxyPoints by TrafficMonitor.proxyPoints.collectAsState()
    val directPoints by TrafficMonitor.directPoints.collectAsState()
    val totalDown by TrafficMonitor.downTotal.collectAsState()
    val totalUp by TrafficMonitor.upTotal.collectAsState()
    val proxyDown by TrafficMonitor.proxyDownTotal.collectAsState()
    val proxyUp by TrafficMonitor.proxyUpTotal.collectAsState()
    val connectionCount by TrafficMonitor.connections.collectAsState()
    val startedAt by TrafficMonitor.startedAt.collectAsState()
    val available by TrafficMonitor.available.collectAsState()

    val points = when (mode) {
        MonitorMode.Total -> totalPoints
        MonitorMode.Proxy -> proxyPoints
        MonitorMode.Direct -> directPoints
    }
    val down = when (mode) {
        MonitorMode.Total -> totalDown
        MonitorMode.Proxy -> proxyDown
        MonitorMode.Direct -> (totalDown - proxyDown).coerceAtLeast(0L)
    }
    val up = when (mode) {
        MonitorMode.Total -> totalUp
        MonitorMode.Proxy -> proxyUp
        MonitorMode.Direct -> (totalUp - proxyUp).coerceAtLeast(0L)
    }

    val last = points.lastOrNull() ?: TrafficPoint(0, 0)
    val elapsed = startedAt?.let { (System.currentTimeMillis() - it).coerceAtLeast(0L) } ?: 0L

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 16.dp, vertical = 18.dp),
    ) {
        Row(verticalAlignment = Alignment.Bottom) {
            Column(Modifier.weight(1f)) {
                Text(
                    text = "Мониторинг",
                    color = Pg.text,
                    fontFamily = Inter,
                    fontWeight = FontWeight.Bold,
                    fontSize = 26.sp,
                    letterSpacing = (-0.5).sp,
                )
                Spacer(Modifier.height(2.dp))
                Text(
                    text = "Трафик через ProtoGrid-core",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 12.sp,
                )
            }
            Text(
                text = "v${BuildConfig.VERSION_NAME}",
                color = Pg.faint,
                fontFamily = Mono,
                fontSize = 10.sp,
            )
        }

        Spacer(Modifier.height(18.dp))

        Panel(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(16.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    SectionLabel("Скорость")
                    Spacer(Modifier.weight(1f))
                    Text(
                        text = "60 секунд",
                        color = Pg.faint,
                        fontFamily = Mono,
                        fontSize = 9.sp,
                    )
                }

                Spacer(Modifier.height(9.dp))

                MonitorTabs(mode) { mode = it }

                Spacer(Modifier.height(10.dp))

                Row {
                    Column(Modifier.weight(1f)) {
                        SectionLabel("Скачивание", color = Pg.cyan)
                        Spacer(Modifier.height(4.dp))
                        Text(
                            text = formatRate(last.down),
                            color = Pg.cyan,
                            fontFamily = Mono,
                            fontWeight = FontWeight.Bold,
                            fontSize = 20.sp,
                            maxLines = 1,
                        )
                    }
                    Column(
                        modifier = Modifier.weight(1f),
                        horizontalAlignment = Alignment.End,
                    ) {
                        SectionLabel("Отправка", color = Pg.accent2)
                        Spacer(Modifier.height(4.dp))
                        Text(
                            text = formatRate(last.up),
                            color = Pg.accent2,
                            fontFamily = Mono,
                            fontWeight = FontWeight.Bold,
                            fontSize = 20.sp,
                            maxLines = 1,
                        )
                    }
                }

                Spacer(Modifier.height(12.dp))

                TrafficChart(
                    points = points,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(170.dp),
                )

                Spacer(Modifier.height(8.dp))

                Row(verticalAlignment = Alignment.CenterVertically) {
                    Legend(Pg.cyan, "Скачивание")
                    Spacer(Modifier.width(14.dp))
                    Legend(Pg.accent, "Отправка")
                    Spacer(Modifier.weight(1f))
                    Text(
                        text = when {
                            status != VpnStatus.Connected -> "не подключено"
                            available -> "live"
                            else -> "нет данных"
                        },
                        color = Pg.faint,
                        fontFamily = Mono,
                        fontSize = 9.sp,
                    )
                }
            }
        }

        Spacer(Modifier.height(14.dp))

        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            StatTile(
                label = "Получено",
                value = formatBytes(down),
                hint = "за сессию",
                modifier = Modifier.weight(1f),
                valueMono = true,
                valueColor = Pg.cyan,
            )
            StatTile(
                label = "Отправлено",
                value = formatBytes(up),
                hint = "за сессию",
                modifier = Modifier.weight(1f),
                valueMono = true,
                valueColor = Pg.accent2,
            )
        }

        Spacer(Modifier.height(10.dp))

        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            StatTile(
                label = "Соединения",
                value = connectionCount.toString(),
                hint = "активных",
                modifier = Modifier.weight(1f),
            )
            StatTile(
                label = "Сессия",
                value = formatDuration(elapsed),
                hint = "с подключения",
                modifier = Modifier.weight(1f),
                valueMono = true,
            )
        }

        Spacer(Modifier.height(16.dp))

        if (status != VpnStatus.Connected) {
            Panel(Modifier.fillMaxWidth()) {
                Row(
                    modifier = Modifier.padding(16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    StatusDot(Pg.faint, 8.dp)
                    Spacer(Modifier.width(8.dp))
                    Text(
                        text = "Подключитесь, чтобы видеть трафик",
                        color = Pg.muted,
                        fontFamily = Inter,
                        fontSize = 11.sp,
                    )
                }
            }
        }
    }
}

@Composable
private fun Legend(color: Color, label: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        StatusDot(color, 6.dp)
        Spacer(Modifier.width(5.dp))
        Text(
            text = label,
            color = Pg.faint,
            fontFamily = Mono,
            fontSize = 9.sp,
        )
    }
}

@Composable
private fun TrafficChart(points: List<TrafficPoint>, modifier: Modifier = Modifier) {
    Canvas(modifier) {
        val width = size.width
        val height = size.height
        val top = 4.dp.toPx()
        val bottom = height - 4.dp.toPx()
        val chartHeight = (bottom - top).coerceAtLeast(1f)
        val grid = Pg.line.copy(alpha = 0.55f)

        for (index in 0..4) {
            val y = top + chartHeight * index / 4f
            drawLine(grid, Offset(0f, y), Offset(width, y), strokeWidth = 1f)
        }
        for (index in 0..6) {
            val x = width * index / 6f
            drawLine(grid, Offset(x, top), Offset(x, bottom), strokeWidth = 1f)
        }

        val data = if (points.isEmpty()) List(60) { TrafficPoint(0, 0) } else points
        val peak = maxOf(1L, data.maxOfOrNull { maxOf(it.down, it.up) } ?: 0L).toFloat()

        fun pathFor(value: (TrafficPoint) -> Long): Path {
            val path = Path()
            data.forEachIndexed { index, point ->
                val x = if (data.size <= 1) 0f else width * index / (data.size - 1).toFloat()
                val y = bottom - (value(point) / peak) * chartHeight
                if (index == 0) path.moveTo(x, y) else path.lineTo(x, y)
            }
            return path
        }

        val downPath = pathFor { it.down }
        val fillPath = Path().apply {
            addPath(downPath)
            lineTo(width, bottom)
            lineTo(0f, bottom)
            close()
        }
        drawPath(
            path = fillPath,
            brush = Brush.verticalGradient(
                colors = listOf(Pg.cyan.copy(alpha = 0.20f), Color.Transparent),
                startY = top,
                endY = bottom,
            ),
        )
        drawPath(
            path = downPath,
            color = Pg.cyan,
            style = Stroke(width = 2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round),
        )
        drawPath(
            path = pathFor { it.up },
            color = Pg.accent,
            style = Stroke(width = 2.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round),
        )
    }
}

private fun formatRate(bytes: Long): String = "${formatBytes(bytes)}/с"

private fun formatBytes(bytes: Long): String {
    if (bytes < 1024) return "$bytes Б"
    var value = bytes.toDouble()
    var unit = 0
    while (value >= 1024 && unit < BYTE_UNITS.lastIndex) {
        value /= 1024
        unit++
    }
    return String.format(Locale.US, "%.1f %s", value, BYTE_UNITS[unit])
}

private fun formatDuration(ms: Long): String {
    val total = (ms / 1000).coerceAtLeast(0)
    val hours = total / 3600
    val minutes = (total % 3600) / 60
    val seconds = total % 60
    return if (hours > 0) {
        String.format(Locale.US, "%d:%02d:%02d", hours, minutes, seconds)
    } else {
        String.format(Locale.US, "%02d:%02d", minutes, seconds)
    }
}

private val BYTE_UNITS = arrayOf("Б", "КБ", "МБ", "ГБ")

private fun profilesWord(count: Int): String {
    val mod100 = count % 100
    val mod10 = count % 10
    return when {
        mod100 in 11..14 -> "профилей"
        mod10 == 1 -> "профиль"
        mod10 in 2..4 -> "профиля"
        else -> "профилей"
    }
}

private fun subscriptionWord(count: Int): String {
    val mod100 = count % 100
    val mod10 = count % 10
    return when {
        mod100 in 11..14 -> "подписок"
        mod10 == 1 -> "подписка"
        mod10 in 2..4 -> "подписки"
        else -> "подписок"
    }
}

private fun formatDate(seconds: Long): String =
    SimpleDateFormat("dd.MM.yyyy", Locale.getDefault()).format(Date(seconds * 1000))

private fun formatDateTime(seconds: Long): String =
    SimpleDateFormat("dd.MM.yyyy HH:mm", Locale.getDefault()).format(Date(seconds * 1000))
