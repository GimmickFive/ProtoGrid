package app.protogrid.client

import android.Manifest
import android.app.Activity
import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.content.Intent
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
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
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
import androidx.compose.runtime.DisposableEffect
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
import androidx.compose.ui.input.pointer.pointerInput
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
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import java.util.Locale
import java.text.SimpleDateFormat
import java.util.Date
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import app.protogrid.client.data.AndroidSettings
import app.protogrid.client.data.BackgroundAccess
import app.protogrid.client.data.ProfileStore
import app.protogrid.client.data.ProfileSummary
import app.protogrid.client.data.SubscriptionInfo
import app.protogrid.client.qr.QrImage
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
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions

class MainActivity : ComponentActivity() {
    /** Ссылка из `protogrid://add?url=…`, ждущая обработки в Compose. */
    private var deepLink by mutableStateOf<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(0xFF0F1115.toInt()),
            navigationBarStyle = SystemBarStyle.dark(0xFF0F1115.toInt()),
        )
        deepLink = linkFromIntent(intent)
        setContent {
            ProtoGridTheme {
                ProtoGridApp(deepLink) { deepLink = null }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        linkFromIntent(intent)?.let { deepLink = it }
    }

    /** `protogrid://add?url=https%3A…` и `protogrid://add/<url>`. */
    private fun linkFromIntent(intent: Intent?): String? {
        val data = intent?.data ?: return null
        if (!data.scheme.equals("protogrid", ignoreCase = true)) return null
        val direct = data.getQueryParameter("url")
            ?: data.pathSegments.firstOrNull { it.startsWith("http", ignoreCase = true) }
        return direct?.takeIf { it.startsWith("http://", true) || it.startsWith("https://", true) }
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

/** Display name of the core linked into this flavor's libbox.aar. */
private val CORE_LABEL = if (BuildConfig.LITE_CORE) "ProtoGrid-core Lite" else "ProtoGrid-core"

/** A single http(s) URL is a subscription; everything else is a share link/profile. */
private fun isSubscriptionLink(text: String): Boolean {
    val trimmed = text.trim()
    if (trimmed.isEmpty() || trimmed.contains('\n') || trimmed.contains('\r')) return false
    return trimmed.startsWith("http://", ignoreCase = true) ||
        trimmed.startsWith("https://", ignoreCase = true)
}

@Composable
private fun ProtoGridApp(deepLink: String? = null, onDeepLinkConsumed: () -> Unit = {}) {
    val context = LocalContext.current
    val store = remember { ProfileStore(context) }
    val routingSettings = remember { AndroidSettings(context) }

    var tab by remember { mutableStateOf(Tab.Overview) }
    var profiles by remember { mutableStateOf(store.profiles()) }
    var activeId by remember { mutableStateOf(store.activeId()) }
    var subscriptions by remember { mutableStateOf(store.subscriptions()) }
    var showImport by remember { mutableStateOf(false) }
    var addError by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf<String?>(null) }
    var notice by remember { mutableStateOf<String?>(null) }
    var silentNotification by remember { mutableStateOf(routingSettings.silentNotification) }
    var notificationSpeed by remember { mutableStateOf(routingSettings.notificationSpeed) }
    var autoStart by remember { mutableStateOf(routingSettings.autoStart) }
    var batteryUnrestricted by remember {
        mutableStateOf(BackgroundAccess.isBatteryUnrestricted(context))
    }

    // The exemption can also be changed from the system settings app, so
    // re-read it every time the UI comes back to the foreground.
    val lifecycleOwner = context as? ComponentActivity
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                batteryUnrestricted = BackgroundAccess.isBatteryUnrestricted(context)
            }
        }
        lifecycleOwner?.lifecycle?.addObserver(observer)
        onDispose { lifecycleOwner?.lifecycle?.removeObserver(observer) }
    }

    val status by VpnState.status.collectAsState()
    val vpnMessage by VpnState.message.collectAsState()
    val update by UpdateManager.state.collectAsState()
    val activeProfile = profiles.firstOrNull { it.id == activeId } ?: profiles.firstOrNull()
    LaunchedEffect(profiles, activeId) {
        if (profiles.isNotEmpty() && profiles.none { it.id == activeId }) {
            val first = profiles.first().id
            activeId = first
            store.setActive(first)
        }
    }
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

    fun openAdd() {
        addError = null
        showImport = true
    }

    fun importText(text: String) {
        val result = store.importText(text)
        reload()
        if (result.added == 0 && result.errors.isNotEmpty()) {
            addError = result.errors.joinToString("\n")
            return
        }
        addError = null
        showImport = false
        notice = when {
            result.errors.isNotEmpty() -> result.errors.first()
            result.added == 0 -> "Новых серверов не найдено"
            else -> "Добавлено: ${result.added}"
        }
        if (result.added > 0) {
            tab = Tab.Servers
        }
    }

    fun addSubscription(url: String, name: String) {
        if (busy != null) return
        addError = null
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
                addError = result.errors.firstOrNull() ?: "В подписке не найдено серверов"
            }
        }
    }

    fun submitInput(raw: String) {
        val trimmed = raw.trim()
        if (trimmed.isEmpty()) {
            addError = "Вставьте ссылку или подписку"
            return
        }
        if (isSubscriptionLink(trimmed)) {
            addSubscription(trimmed, "")
        } else {
            importText(trimmed)
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

    fun pinWidget(): Boolean {
        val manager = AppWidgetManager.getInstance(context)
        if (!manager.isRequestPinAppWidgetSupported) return false
        return manager.requestPinAppWidget(
            ComponentName(context, ProtoGridWidget::class.java),
            null,
            null,
        )
    }

    fun openBackgroundAccess() {
        val opened = runCatching {
            context.startActivity(BackgroundAccess.batteryRequestIntent(context))
        }.isSuccess
        if (!opened) {
            runCatching { context.startActivity(BackgroundAccess.batterySettingsIntent()) }
        }
    }

    fun openBatterySettings() {
        runCatching { context.startActivity(BackgroundAccess.batterySettingsIntent()) }
    }

    fun openAutoStartSettings() {
        BackgroundAccess.openAutoStartSettings(context)
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

    LaunchedEffect(deepLink) {
        val link = deepLink?.trim().orEmpty()
        if (link.isNotEmpty()) {
            onDeepLinkConsumed()
            submitInput(link)
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
                .padding(padding)
                .pointerInput(tab) {
                    // Горизонтальный свайп по содержимому переключает вкладки
                    // в том же порядке, что и нижнее меню.
                    val tabs = Tab.entries
                    val threshold = 64.dp.toPx()
                    var dragged = 0f
                    detectHorizontalDragGestures(
                        onDragStart = { dragged = 0f },
                        onDragCancel = { dragged = 0f },
                        onHorizontalDrag = { change, amount ->
                            dragged += amount
                            change.consume()
                        },
                        onDragEnd = {
                            val index = tabs.indexOf(tab)
                            when {
                                dragged <= -threshold && index < tabs.lastIndex ->
                                    tab = tabs[index + 1]
                                dragged >= threshold && index > 0 ->
                                    tab = tabs[index - 1]
                            }
                            dragged = 0f
                        },
                    )
                },
        ) {
            key(tab) {
                when (tab) {
                    Tab.Overview -> OverviewScreen(
                        status = status,
                        message = vpnMessage ?: notice,
                        profiles = profiles,
                        activeProfile = activeProfile,
                        update = update,
                        onToggle = {
                            if (status == VpnStatus.Connected || status == VpnStatus.Connecting) {
                                ProtoGridVpnService.stop(context)
                            } else {
                                connect()
                            }
                        },
                        onAdd = { openAdd() },
                        onOpenServers = { tab = Tab.Servers },
                        onOpenRouting = { tab = Tab.Routing },
                        onOpenTraffic = { tab = Tab.Monitor },
                        onOpenSettings = { tab = Tab.Settings },
                        routingSummary = "${routingSettings.trafficPreset.label} · ${routingSettings.perAppSummary()}",
                    )

                    Tab.Servers -> ServersScreen(
                        profiles = profiles,
                        subscriptions = subscriptions,
                        activeId = activeProfile?.id,
                        busy = busy,
                        onAdd = { openAdd() },
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
                        autoStart = autoStart,
                        batteryUnrestricted = batteryUnrestricted,
                        onSilentNotification = {
                            silentNotification = it
                            routingSettings.silentNotification = it
                        },
                        onNotificationSpeed = {
                            notificationSpeed = it
                            routingSettings.notificationSpeed = it
                        },
                        onAutoStart = {
                            autoStart = it
                            routingSettings.autoStart = it
                        },
                        onRequestBackground = ::openBackgroundAccess,
                        onOpenBatterySettings = ::openBatterySettings,
                        onOpenAutoStartSettings = ::openAutoStartSettings,
                        onPinWidget = ::pinWidget,
                    )
                }
            }
        }
    }

    if (showImport) {
        AddDialog(
            busy = busy,
            lite = BuildConfig.LITE_CORE,
            error = addError,
            onDismiss = { showImport = false },
            onSubmit = { submitInput(it) },
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
                label = "Подключение",
                icon = R.drawable.ic_power,
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
                label = "Маршруты",
                icon = R.drawable.ic_route,
                selected = tab == Tab.Routing,
                modifier = Modifier.weight(1f),
                onClick = { onSelect(Tab.Routing) },
            )
            NavItem(
                label = "Трафик",
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
            fontSize = if (label.length > 9) 8.sp else 9.sp,
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
    update: UpdateUiState,
    onToggle: () -> Unit,
    onAdd: () -> Unit,
    onOpenServers: () -> Unit,
    onOpenRouting: () -> Unit,
    onOpenTraffic: () -> Unit,
    onOpenSettings: () -> Unit,
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
                    text = "Подключение",
                    color = Pg.text,
                    fontFamily = Inter,
                    fontWeight = FontWeight.Bold,
                    fontSize = 26.sp,
                    letterSpacing = (-0.5).sp,
                )
                Spacer(Modifier.height(2.dp))
                Text(
                    text = "Сервер, маршрут и состояние туннеля",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 12.sp,
                )
            }
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

                Row(verticalAlignment = Alignment.CenterVertically) {
                    SectionLabel(
                        if (activeProfile != null) "Активный сервер" else "Сервер не выбран",
                        Modifier.weight(1f),
                    )
                    if (profiles.isNotEmpty()) {
                        Text(
                            text = if (activeProfile != null) "Сменить →" else "Выбрать →",
                            modifier = Modifier
                                .clip(RoundedCornerShape(8.dp))
                                .clickable(onClick = onOpenServers)
                                .padding(horizontal = 6.dp, vertical = 3.dp),
                            color = Pg.accent2,
                            fontFamily = Inter,
                            fontWeight = FontWeight.Medium,
                            fontSize = 11.sp,
                        )
                    }
                }
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
                        text = if (BuildConfig.LITE_CORE) {
                            "Добавьте VLESS-ссылку или подписку, чтобы выбрать сервер"
                        } else {
                            "Добавьте share-ссылку или подписку, чтобы выбрать сервер"
                        },
                        color = Pg.muted,
                        fontFamily = Inter,
                        fontSize = 12.sp,
                    )
                }

                Spacer(Modifier.height(10.dp))
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(8.dp))
                        .clickable(onClick = onOpenRouting)
                        .padding(vertical = 3.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    SectionLabel("Маршрут")
                    Spacer(Modifier.width(8.dp))
                    Text(
                        text = routingSummary,
                        modifier = Modifier.weight(1f),
                        color = Pg.muted,
                        fontFamily = Inter,
                        fontSize = 10.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Spacer(Modifier.width(6.dp))
                    Text(
                        text = "→",
                        color = Pg.accent2,
                        fontFamily = Inter,
                        fontWeight = FontWeight.Medium,
                        fontSize = 12.sp,
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
                        text = if (profiles.isEmpty()) "Добавить сервер" else "Выбрать сервер",
                        onClick = if (profiles.isEmpty()) onAdd else onOpenServers,
                        modifier = Modifier.fillMaxWidth(),
                        icon = painterResource(
                            if (profiles.isEmpty()) R.drawable.ic_add else R.drawable.ic_servers,
                        ),
                    )
                }
            }
        }

        Spacer(Modifier.height(14.dp))

        UpdateBanner(update) { onOpenSettings() }

        SessionSummary(status, onOpenTraffic)

        Spacer(Modifier.height(16.dp))
    }
}

@Composable
private fun UpdateBanner(update: UpdateUiState, onOpen: () -> Unit) {
    val release = update.release
    val visible = release != null || update.downloading || update.downloadedPath != null
    if (!visible) return

    Panel(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .clickable(onClick = onOpen),
    ) {
        Column(Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    SectionLabel("Обновление")
                    Spacer(Modifier.height(3.dp))
                    Text(
                        text = when {
                            update.downloading ->
                                "Скачивание APK · ${(update.progress * 100).toInt()}%"
                            update.downloadedPath != null -> "APK готов к установке"
                            release != null -> "Доступна версия ${release.version}"
                            else -> ""
                        },
                        color = if (release != null) Pg.amber else Pg.text,
                        fontFamily = Inter,
                        fontWeight = FontWeight.Medium,
                        fontSize = 12.sp,
                    )
                }
                Text(
                    text = "Открыть →",
                    color = Pg.accent2,
                    fontFamily = Inter,
                    fontWeight = FontWeight.Medium,
                    fontSize = 11.sp,
                )
            }
            if (update.downloading) {
                Spacer(Modifier.height(8.dp))
                LinearProgressIndicator(
                    progress = { update.progress },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(4.dp)
                        .clip(RoundedCornerShape(2.dp)),
                    color = Pg.accent2,
                    trackColor = Pg.panel2,
                )
            }
        }
    }

    Spacer(Modifier.height(14.dp))
}

@Composable
private fun SessionSummary(status: VpnStatus, onOpen: () -> Unit) {
    val points by TrafficMonitor.points.collectAsState()
    val downTotal by TrafficMonitor.downTotal.collectAsState()
    val upTotal by TrafficMonitor.upTotal.collectAsState()
    val connectionCount by TrafficMonitor.connections.collectAsState()
    val startedAt by TrafficMonitor.startedAt.collectAsState()

    val active = status == VpnStatus.Connected
    val last = points.lastOrNull() ?: TrafficPoint(0, 0)
    val elapsed = startedAt?.let { (System.currentTimeMillis() - it).coerceAtLeast(0L) } ?: 0L

    if (!active && startedAt == null) {
        Panel(
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(14.dp))
                .clickable(onClick = onOpen),
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 13.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                SectionLabel("Трафик сессии")
                Spacer(Modifier.weight(1f))
                Text(
                    text = "нет подключения →",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 11.sp,
                )
            }
        }
        return
    }

    Panel(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .clickable(onClick = onOpen),
    ) {
        Column(Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                SectionLabel("Трафик сессии")
                Spacer(Modifier.weight(1f))
                Text(
                    text = "Открыть →",
                    color = Pg.accent2,
                    fontFamily = Inter,
                    fontWeight = FontWeight.Medium,
                    fontSize = 11.sp,
                )
            }

            Spacer(Modifier.height(10.dp))

            Row {
                Column(Modifier.weight(1f)) {
                    SectionLabel("Скачивание", color = Pg.cyan)
                    Spacer(Modifier.height(4.dp))
                    Text(
                        text = if (active) formatRate(last.down) else "—",
                        color = Pg.cyan,
                        fontFamily = Mono,
                        fontWeight = FontWeight.Bold,
                        fontSize = 18.sp,
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
                        text = if (active) formatRate(last.up) else "—",
                        color = Pg.accent2,
                        fontFamily = Mono,
                        fontWeight = FontWeight.Bold,
                        fontSize = 18.sp,
                        maxLines = 1,
                    )
                }
            }

            Spacer(Modifier.height(12.dp))

            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                SessionMetric(
                    label = "Получено",
                    value = if (active) formatBytes(downTotal) else "—",
                    modifier = Modifier.weight(1f),
                )
                SessionMetric(
                    label = "Отправлено",
                    value = if (active) formatBytes(upTotal) else "—",
                    modifier = Modifier.weight(1f),
                )
                SessionMetric(
                    label = "Сессия",
                    value = if (active) formatDuration(elapsed) else "—",
                    modifier = Modifier.weight(1f),
                )
                SessionMetric(
                    label = "Соединения",
                    value = if (active) connectionCount.toString() else "—",
                    modifier = Modifier.weight(1f),
                )
            }

            if (!active) {
                Spacer(Modifier.height(10.dp))
                Text(
                    text = "Подключитесь, чтобы видеть скорость и объём трафика.",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 10.sp,
                    lineHeight = 14.sp,
                )
            }
        }
    }
}

@Composable
private fun SessionMetric(
    label: String,
    value: String,
    modifier: Modifier = Modifier,
) {
    Column(modifier) {
        Text(
            text = label,
            color = Pg.faint,
            fontFamily = Inter,
            fontSize = 10.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(Modifier.height(2.dp))
        Text(
            text = value,
            color = Pg.text,
            fontFamily = Mono,
            fontSize = 12.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
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
                        text = "Ядро обновляется вместе с приложением",
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
    autoStart: Boolean,
    batteryUnrestricted: Boolean,
    onSilentNotification: (Boolean) -> Unit,
    onNotificationSpeed: (Boolean) -> Unit,
    onAutoStart: (Boolean) -> Unit,
    onRequestBackground: () -> Unit,
    onOpenBatterySettings: () -> Unit,
    onOpenAutoStartSettings: () -> Unit,
    onPinWidget: () -> Boolean,
) {
    var pinHint by remember { mutableStateOf(false) }
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
                    text = "Фоновая работа, уведомления, виджет и обновления",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 12.sp,
                )
            }
        }

        Spacer(Modifier.height(18.dp))

        Panel(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(16.dp)) {
                SectionLabel("Уведомление VPN")
                Spacer(Modifier.height(4.dp))
                Text(
                    text = "Постоянное уведомление показывает, что туннель запущен, " +
                        "и даёт быстрый доступ к отключению.",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 10.sp,
                    lineHeight = 14.sp,
                )
                Spacer(Modifier.height(12.dp))
                SettingSwitch(
                    title = "Тихий режим",
                    subtitle = "Без звука и всплывающих окон — уведомление остаётся только в шторке.",
                    checked = silentNotification,
                    onChange = onSilentNotification,
                )
                Spacer(Modifier.height(14.dp))
                SettingSwitch(
                    title = "Скорость в тексте",
                    subtitle = "Показывать текущие скорости скачивания и отправки в уведомлении.",
                    checked = notificationSpeed,
                    onChange = onNotificationSpeed,
                )
                Spacer(Modifier.height(10.dp))
                Text(
                    text = "Уведомление пересоздаётся при подключении: после изменения " +
                        "этих настроек переподключите VPN.",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 10.sp,
                    lineHeight = 14.sp,
                )
            }
        }

        Spacer(Modifier.height(14.dp))

        Panel(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(16.dp)) {
                SectionLabel("Фоновая работа")
                Spacer(Modifier.height(12.dp))
                SettingSwitch(
                    title = "Не ограничивать в фоне",
                    subtitle = if (batteryUnrestricted) {
                        "Android не будет останавливать VPN для экономии заряда"
                    } else {
                        "Запросить у системы исключение из энергосбережения"
                    },
                    checked = batteryUnrestricted,
                    onChange = { enabled ->
                        if (enabled) onRequestBackground() else onOpenBatterySettings()
                    },
                )
                Spacer(Modifier.height(12.dp))
                SettingSwitch(
                    title = "Автозапуск",
                    subtitle = "Подключать VPN после перезагрузки устройства",
                    checked = autoStart,
                    onChange = onAutoStart,
                )
                if (autoStart && !batteryUnrestricted) {
                    Spacer(Modifier.height(8.dp))
                    Text(
                        text = "Без исключения из энергосбережения система может остановить VPN после перезагрузки.",
                        color = Pg.amber,
                        fontFamily = Inter,
                        fontSize = 10.sp,
                        lineHeight = 14.sp,
                    )
                }
                Spacer(Modifier.height(10.dp))
                Text(
                    text = "На Xiaomi, Huawei, Oppo и Vivo автозапуск нужно дополнительно разрешить в приложении безопасности.",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 10.sp,
                    lineHeight = 14.sp,
                )
                Spacer(Modifier.height(10.dp))
                GhostButton(
                    text = "Настройки автозапуска",
                    onClick = onOpenAutoStartSettings,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }

        Spacer(Modifier.height(14.dp))

        Panel(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(16.dp)) {
                SectionLabel("Виджет")
                Spacer(Modifier.height(8.dp))
                Text(
                    text = "Виджет 4×1 на рабочем столе показывает статус подключения, " +
                        "скорость трафика через прокси и кнопку включения/выключения VPN. " +
                        "Плитка в шторке быстрых настроек дублирует управление.",
                    color = Pg.muted,
                    fontFamily = Inter,
                    fontSize = 11.sp,
                    lineHeight = 16.sp,
                )
                Spacer(Modifier.height(12.dp))
                GhostButton(
                    text = "Добавить виджет",
                    onClick = {
                        pinHint = false
                        if (!onPinWidget()) pinHint = true
                    },
                    modifier = Modifier.fillMaxWidth(),
                    icon = painterResource(R.drawable.ic_add),
                )
                if (pinHint) {
                    Spacer(Modifier.height(10.dp))
                    Text(
                        text = "Лаунчер не умеет закреплять виджеты автоматически. " +
                            "Добавьте вручную: долгое нажатие на рабочем столе → «Виджеты» → ProtoGrid.",
                        color = Pg.muted,
                        fontFamily = Inter,
                        fontSize = 10.sp,
                        lineHeight = 14.sp,
                    )
                }
                Spacer(Modifier.height(10.dp))
                Text(
                    text = "Плитка ProtoGrid в шторке: «Изменить плитки» в быстрых настройках → перетащите ProtoGrid.",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 10.sp,
                    lineHeight = 14.sp,
                )
            }
        }

        Spacer(Modifier.height(14.dp))

        Panel(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(16.dp)) {
                SectionLabel("Навигация")
                Spacer(Modifier.height(8.dp))
                Text(
                    text = "Разделы переключаются нижним меню или свайпом по экрану: " +
                        "влево — следующий раздел, вправо — предыдущий.",
                    color = Pg.muted,
                    fontFamily = Inter,
                    fontSize = 11.sp,
                    lineHeight = 16.sp,
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
                InfoRow("Ядро", "$CORE_LABEL $CORE_VERSION")
                Spacer(Modifier.height(6.dp))
                InfoRow("Обновления", "проверяются при запуске")
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

private enum class ServerSection(val label: String) {
    Servers("Серверы"),
    Subscriptions("Подписки"),
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
    var section by remember {
        mutableStateOf(
            if (profiles.isEmpty() && subscriptions.isNotEmpty()) {
                ServerSection.Subscriptions
            } else {
                ServerSection.Servers
            },
        )
    }

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
                        if (BuildConfig.LITE_CORE) " · только VLESS/TCP" else "",
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

        Spacer(Modifier.height(12.dp))
        ServerSectionTabs(
            section = section,
            serverCount = profiles.size,
            subscriptionCount = subscriptions.size,
            onSelect = { section = it },
        )

        Spacer(Modifier.height(16.dp))

        LazyColumn(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            contentPadding = PaddingValues(bottom = 16.dp),
        ) {
            when (section) {
                ServerSection.Servers -> {
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

                ServerSection.Subscriptions -> {
                    if (subscriptions.isEmpty()) {
                        item(key = "empty-subscriptions") {
                            EmptySubscriptions(onAdd)
                        }
                    } else {
                        items(subscriptions, key = { "sub-${it.id}" }) { sub ->
                            SubscriptionCard(
                                sub = sub,
                                busy = busy != null,
                                onRefresh = { onRefreshSubscription(sub) },
                                onDelete = { onDeleteSubscription(sub) },
                                onAutoUpdate = { onAutoUpdate(sub, it) },
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ServerSectionTabs(
    section: ServerSection,
    serverCount: Int,
    subscriptionCount: Int,
    onSelect: (ServerSection) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(9.dp))
            .background(Pg.bg2)
            .padding(3.dp),
        horizontalArrangement = Arrangement.spacedBy(3.dp),
    ) {
        ServerSection.entries.forEach { item ->
            val selected = item == section
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
                    text = when (item) {
                        ServerSection.Servers -> "${item.label} · $serverCount"
                        ServerSection.Subscriptions -> "${item.label} · $subscriptionCount"
                    },
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
                text = if (BuildConfig.LITE_CORE) {
                    "Добавьте VLESS-ссылку или подписку. Lite работает только с VLESS/Reality по TCP."
                } else {
                    "Добавьте share-ссылки, подписку или .ovpn: vless, vmess, trojan, ss, hy2"
                },
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
private fun EmptySubscriptions(onAdd: () -> Unit) {
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
                    painter = painterResource(R.drawable.ic_add),
                    contentDescription = null,
                    tint = Pg.faint,
                    modifier = Modifier.size(24.dp),
                )
            }
            Spacer(Modifier.height(14.dp))
            Text(
                text = "Пока нет подписок",
                color = Pg.text,
                fontFamily = Inter,
                fontWeight = FontWeight.SemiBold,
                fontSize = 15.sp,
            )
            Spacer(Modifier.height(4.dp))
            Text(
                text = "Добавьте https-ссылку — серверы загрузятся и будут обновляться автоматически.",
                color = Pg.faint,
                fontFamily = Inter,
                fontSize = 11.sp,
                lineHeight = 16.sp,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(16.dp))
            PrimaryButton(
                text = "Добавить подписку",
                onClick = onAdd,
                icon = painterResource(R.drawable.ic_add),
            )
        }
    }
}

@Composable
private fun AddDialog(
    busy: String?,
    lite: Boolean,
    error: String?,
    onDismiss: () -> Unit,
    onSubmit: (String) -> Unit,
) {
    var text by remember { mutableStateOf("") }
    var localError by remember { mutableStateOf<String?>(null) }
    val clipboard = LocalClipboardManager.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val locked = busy != null
    val subscription = isSubscriptionLink(text)
    val hasCamera = remember {
        context.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY)
    }
    val scanOptions = remember {
        ScanOptions()
            .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
            .setPrompt("Наведите камеру на QR-код")
            .setBeepEnabled(false)
            .setOrientationLocked(false)
    }
    val scanLauncher = rememberLauncherForActivityResult(ScanContract()) { result ->
        val contents = result.contents?.trim().orEmpty()
        if (contents.isNotEmpty()) {
            text = contents
            localError = null
        }
    }
    val cameraPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        if (granted) {
            scanLauncher.launch(scanOptions)
        } else {
            localError = "Нет разрешения на камеру"
        }
    }
    fun startScan() {
        localError = null
        if (ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
            PackageManager.PERMISSION_GRANTED
        ) {
            scanLauncher.launch(scanOptions)
        } else {
            cameraPermission.launch(Manifest.permission.CAMERA)
        }
    }
    val imagePicker = rememberLauncherForActivityResult(
        ActivityResultContracts.GetContent(),
    ) { uri ->
        if (uri != null) {
            scope.launch {
                val decoded = withContext(Dispatchers.IO) { QrImage.decode(context, uri) }
                if (decoded.isNullOrBlank()) {
                    localError = "QR-код на изображении не найден"
                } else {
                    text = decoded
                    localError = null
                }
            }
        }
    }
    val configPicker = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument(),
    ) { uri ->
        if (uri != null) {
            val content = runCatching {
                context.contentResolver.openInputStream(uri)
                    ?.bufferedReader()
                    ?.use { it.readText() }
            }.getOrNull()
            if (content.isNullOrBlank()) {
                localError = "Не удалось прочитать выбранный файл"
            } else {
                localError = null
                onSubmit(content)
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
                        text = "Ссылка, подписка или конфиг",
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

            PgField(
                value = text,
                onValueChange = {
                    text = it
                    localError = null
                },
                placeholder = if (lite) {
                    "vless://…\nhttps://…/sub"
                } else {
                    "vless://…  vmess://…  trojan://…  ss://…\nhysteria2://…  tuic://…  openvpn://…\nhttps://…/sub  ·  Clash YAML  ·  .ovpn"
                },
                minLines = 5,
                maxLines = 8,
            )

            Spacer(Modifier.height(6.dp))
            Text(
                text = when {
                    subscription -> "Похоже на подписку: серверы загрузятся по HTTPS и будут обновляться автоматически."
                    text.isNotBlank() -> "Будет добавлен сервер или несколько ссылок."
                    lite -> "Lite принимает только VLESS/Reality по TCP."
                    else -> "Формат определится сам: ссылка, подписка, Clash YAML или .ovpn."
                },
                color = if (subscription) Pg.accent2 else Pg.faint,
                fontFamily = Inter,
                fontSize = 10.sp,
                lineHeight = 14.sp,
            )

            Spacer(Modifier.height(8.dp))

            TextButton(
                onClick = {
                    val pasted = clipboard.getText()?.text?.trim().orEmpty()
                    if (pasted.isNotBlank()) {
                        text = pasted
                        localError = null
                    } else {
                        localError = "Буфер обмена пуст"
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

            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                if (hasCamera) {
                    TextButton(
                        onClick = { startScan() },
                        enabled = !locked,
                        colors = ButtonDefaults.textButtonColors(contentColor = Pg.accent2),
                        contentPadding = PaddingValues(horizontal = 0.dp, vertical = 4.dp),
                    ) {
                        Text(
                            text = "Сканировать QR",
                            fontFamily = Inter,
                            fontWeight = FontWeight.Medium,
                            fontSize = 12.sp,
                        )
                    }
                }
                TextButton(
                    onClick = {
                        localError = null
                        imagePicker.launch("image/*")
                    },
                    enabled = !locked,
                    colors = ButtonDefaults.textButtonColors(contentColor = Pg.accent2),
                    contentPadding = PaddingValues(horizontal = 0.dp, vertical = 4.dp),
                ) {
                    Text(
                        text = "QR из фото",
                        fontFamily = Inter,
                        fontWeight = FontWeight.Medium,
                        fontSize = 12.sp,
                    )
                }
            }

            if (!lite) {
                Spacer(Modifier.height(8.dp))
                GhostButton(
                    text = "Добавить конфиг .ovpn",
                    onClick = {
                        localError = null
                        configPicker.launch(arrayOf("*/*"))
                    },
                    modifier = Modifier.fillMaxWidth(),
                    icon = painterResource(R.drawable.ic_add),
                )
            }

            val shownError = localError ?: error
            if (!shownError.isNullOrBlank()) {
                Spacer(Modifier.height(8.dp))
                Text(
                    text = shownError,
                    color = Pg.red,
                    fontFamily = Inter,
                    fontSize = 10.sp,
                    lineHeight = 14.sp,
                )
            }

            Spacer(Modifier.height(12.dp))

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
                    text = busy ?: if (subscription) "Добавить подписку" else "Импортировать",
                    onClick = { onSubmit(text) },
                    enabled = !locked && text.isNotBlank(),
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
                    text = "Трафик",
                    color = Pg.text,
                    fontFamily = Inter,
                    fontWeight = FontWeight.Bold,
                    fontSize = 26.sp,
                    letterSpacing = (-0.5).sp,
                )
                Spacer(Modifier.height(2.dp))
                Text(
                    text = "Скорость и объём через $CORE_LABEL",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 12.sp,
                )
            }
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

private fun formatDate(seconds: Long): String =
    SimpleDateFormat("dd.MM.yyyy", Locale.getDefault()).format(Date(seconds * 1000))

private fun formatDateTime(seconds: Long): String =
    SimpleDateFormat("dd.MM.yyyy HH:mm", Locale.getDefault()).format(Date(seconds * 1000))
