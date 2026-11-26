package app.protogrid.client.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CheckboxDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.graphics.drawable.toBitmap
import app.protogrid.client.R
import app.protogrid.client.core.CoreBridge
import app.protogrid.client.data.AndroidSettings
import app.protogrid.client.data.AppCatalog
import app.protogrid.client.data.AppEntry
import app.protogrid.client.data.PerAppMode
import app.protogrid.client.data.TrafficAction
import app.protogrid.client.data.TrafficPreset
import app.protogrid.client.data.TrafficRule
import app.protogrid.client.data.TrafficRuleKind
import app.protogrid.client.vpn.VpnStatus
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.util.Locale

private enum class RouteMode {
    Apps,
    Traffic,
}

@Composable
fun RoutingScreen(
    status: VpnStatus,
    onApply: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val settings = remember { AndroidSettings(context) }
    var mode by remember { mutableStateOf(RouteMode.Apps) }
    var notice by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(status) {
        if (status == VpnStatus.Connected) notice = null
        if (status == VpnStatus.Error) notice = "Ошибка подключения"
    }

    Column(
        modifier = modifier
            .fillMaxSize()
            .padding(horizontal = 16.dp, vertical = 18.dp),
    ) {
        Row(verticalAlignment = Alignment.Bottom) {
            Column(Modifier.weight(1f)) {
                Text(
                    text = "Маршрутизация",
                    color = Pg.text,
                    fontFamily = Inter,
                    fontWeight = FontWeight.Bold,
                    fontSize = 26.sp,
                    letterSpacing = (-0.5).sp,
                )
                Spacer(Modifier.height(2.dp))
                Text(
                    text = "Приложения и правила трафика",
                    color = Pg.faint,
                    fontFamily = Inter,
                    fontSize = 12.sp,
                )
            }
            Text(
                text = "1.14.4",
                color = Pg.faint,
                fontFamily = Mono,
                fontSize = 10.sp,
            )
        }

        Spacer(Modifier.height(16.dp))

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(10.dp))
                .background(Pg.panel2)
                .border(1.dp, Pg.line, RoundedCornerShape(10.dp))
                .padding(3.dp),
        ) {
            Segment(
                label = "По приложениям",
                selected = mode == RouteMode.Apps,
                modifier = Modifier.weight(1f),
                onClick = {
                    mode = RouteMode.Apps
                    notice = null
                },
            )
            Segment(
                label = "По трафику",
                selected = mode == RouteMode.Traffic,
                modifier = Modifier.weight(1f),
                onClick = {
                    mode = RouteMode.Traffic
                    notice = null
                },
            )
        }

        Spacer(Modifier.height(14.dp))

        when (mode) {
            RouteMode.Apps -> AppsRouting(
                settings = settings,
                modifier = Modifier.weight(1f),
            )

            RouteMode.Traffic -> TrafficRouting(
                settings = settings,
                modifier = Modifier.weight(1f),
            )
        }

        Spacer(Modifier.height(10.dp))

        if (notice != null) {
            Text(
                text = notice!!,
                color = Pg.green,
                fontFamily = Inter,
                fontSize = 11.sp,
            )
            Spacer(Modifier.height(6.dp))
        }

        PrimaryButton(
            text = if (status == VpnStatus.Connected || status == VpnStatus.Connecting) {
                "Применить и переподключить"
            } else {
                "Сохранить"
            },
            onClick = {
                notice = if (status == VpnStatus.Connected || status == VpnStatus.Connecting) {
                    "Переподключаю…"
                } else {
                    "Настройки сохранены"
                }
                onApply()
            },
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(Modifier.height(6.dp))
        Text(
            text = "Настройки сохраняются сразу. Для активного туннеля нажмите «Применить».",
            color = Pg.faint,
            fontFamily = Inter,
            fontSize = 10.sp,
            lineHeight = 14.sp,
        )
    }
}

@Composable
private fun Segment(
    label: String,
    selected: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    Box(
        modifier = modifier
            .height(36.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(if (selected) Pg.accentSoft else Color.Transparent)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color = if (selected) Pg.accent2 else Pg.muted,
            fontFamily = Inter,
            fontWeight = FontWeight.Medium,
            fontSize = 12.sp,
            maxLines = 1,
        )
    }
}

@Composable
private fun AppsRouting(settings: AndroidSettings, modifier: Modifier) {
    val context = LocalContext.current
    var mode by remember { mutableStateOf(settings.perAppMode) }
    var selected by remember { mutableStateOf(settings.perAppPackages) }
    var showSystem by remember { mutableStateOf(settings.showSystemApps) }
    var query by remember { mutableStateOf("") }
    var apps by remember { mutableStateOf<List<AppEntry>?>(null) }

    LaunchedEffect(showSystem) {
        apps = null
        apps = withContext(Dispatchers.IO) {
            AppCatalog.load(context, showSystem)
        }
    }

    Column(modifier) {
        SectionLabel("Режим VPN")
        Spacer(Modifier.height(8.dp))

        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            ChoiceChip(
                label = "Все",
                selected = mode == PerAppMode.All,
                modifier = Modifier.weight(1f),
                onClick = {
                    mode = PerAppMode.All
                    settings.perAppMode = mode
                },
            )
            ChoiceChip(
                label = "Только выбранные",
                selected = mode == PerAppMode.Include,
                modifier = Modifier.weight(1f),
                onClick = {
                    mode = PerAppMode.Include
                    settings.perAppMode = mode
                },
            )
            ChoiceChip(
                label = "Все кроме",
                selected = mode == PerAppMode.Exclude,
                modifier = Modifier.weight(1f),
                onClick = {
                    mode = PerAppMode.Exclude
                    settings.perAppMode = mode
                },
            )
        }

        Spacer(Modifier.height(5.dp))
        Text(
            text = when (mode) {
                PerAppMode.All -> "Весь трафик телефона идёт через ProtoGrid-core"
                PerAppMode.Include -> "VPN только для отмеченных приложений"
                PerAppMode.Exclude -> "VPN для всех, кроме отмеченных приложений"
            },
            color = Pg.faint,
            fontFamily = Inter,
            fontSize = 10.sp,
            lineHeight = 14.sp,
        )

        Spacer(Modifier.height(10.dp))

        if (mode == PerAppMode.All) {
            Panel(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp)) {
                    Text(
                        text = "Per-app фильтр выключен",
                        color = Pg.text,
                        fontFamily = Inter,
                        fontWeight = FontWeight.Medium,
                        fontSize = 13.sp,
                    )
                    Spacer(Modifier.height(3.dp))
                    Text(
                        text = "Выберите «Только выбранные» или «Все кроме выбранных», чтобы управлять списком приложений.",
                        color = Pg.faint,
                        fontFamily = Inter,
                        fontSize = 11.sp,
                        lineHeight = 15.sp,
                    )
                }
            }
            Spacer(Modifier.weight(1f))
        } else {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                placeholder = {
                    Text(
                        text = "Поиск приложения",
                        color = Pg.faint,
                        fontFamily = Inter,
                        fontSize = 12.sp,
                    )
                },
                textStyle = TextStyle(
                    fontFamily = Inter,
                    fontSize = 13.sp,
                    color = Pg.text,
                ),
                shape = RoundedCornerShape(10.dp),
                colors = brandFieldColors(),
            )

            Spacer(Modifier.height(6.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = "Выбрано: ${selected.size}",
                    color = Pg.muted,
                    fontFamily = Mono,
                    fontSize = 10.sp,
                )
                Spacer(Modifier.weight(1f))
                TextButton(
                    onClick = {
                        showSystem = !showSystem
                        settings.showSystemApps = showSystem
                    },
                    contentPadding = PaddingValues(horizontal = 6.dp),
                ) {
                    Text(
                        text = if (showSystem) "Скрыть системные" else "Системные",
                        color = Pg.accent2,
                        fontFamily = Inter,
                        fontWeight = FontWeight.Medium,
                        fontSize = 11.sp,
                    )
                }
            }

            Spacer(Modifier.height(2.dp))

            when {
                apps == null -> {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .weight(1f),
                        contentAlignment = Alignment.Center,
                    ) {
                        CircularProgressIndicator(color = Pg.accent, strokeWidth = 2.dp)
                    }
                }

                else -> {
                    val filtered = apps!!.filter {
                        query.isBlank() ||
                            it.label.contains(query, ignoreCase = true) ||
                            it.packageName.contains(query, ignoreCase = true)
                    }
                    if (filtered.isEmpty()) {
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .weight(1f),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                text = "Ничего не найдено",
                                color = Pg.faint,
                                fontFamily = Inter,
                                fontSize = 12.sp,
                            )
                        }
                    } else {
                        LazyColumn(
                            modifier = Modifier.weight(1f),
                            verticalArrangement = Arrangement.spacedBy(6.dp),
                            contentPadding = PaddingValues(bottom = 8.dp),
                        ) {
                            items(filtered, key = { it.packageName }) { app ->
                                AppRow(
                                    app = app,
                                    selected = app.packageName in selected,
                                    onToggle = { checked ->
                                        selected = if (checked) {
                                            selected + app.packageName
                                        } else {
                                            selected - app.packageName
                                        }
                                        settings.perAppPackages = selected
                                    },
                                )
                            }
                        }
                    }
                }
            }

            if (mode == PerAppMode.Include && selected.isEmpty()) {
                Text(
                    text = "Ничего не выбрано — VPN будет для всех приложений.",
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
private fun AppRow(
    app: AppEntry,
    selected: Boolean,
    onToggle: (Boolean) -> Unit,
) {
    val shape = RoundedCornerShape(10.dp)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(if (selected) Pg.accent.copy(alpha = 0.08f) else Pg.panel)
            .border(
                1.dp,
                if (selected) Pg.accent.copy(alpha = 0.30f) else Pg.line,
                shape,
            )
            .clickable { onToggle(!selected) }
            .padding(horizontal = 10.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        AppIcon(app)
        Spacer(Modifier.width(10.dp))
        Column(Modifier.weight(1f)) {
            Text(
                text = app.label,
                color = Pg.text,
                fontFamily = Inter,
                fontWeight = FontWeight.Medium,
                fontSize = 13.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = app.packageName,
                color = Pg.faint,
                fontFamily = Mono,
                fontSize = 9.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Checkbox(
            checked = selected,
            onCheckedChange = null,
            colors = CheckboxDefaults.colors(
                checkedColor = Pg.accent,
                uncheckedColor = Pg.lineStrong,
                checkmarkColor = Color.White,
            ),
        )
    }
}

@Composable
private fun AppIcon(app: AppEntry) {
    val bitmap = remember(app.packageName) {
        runCatching { app.icon?.toBitmap(64, 64)?.asImageBitmap() }.getOrNull()
    }
    if (bitmap != null) {
        Image(
            bitmap = bitmap,
            contentDescription = null,
            modifier = Modifier
                .size(36.dp)
                .clip(RoundedCornerShape(9.dp)),
        )
    } else {
        Box(
            modifier = Modifier
                .size(36.dp)
                .clip(RoundedCornerShape(9.dp))
                .background(Pg.panel3),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = app.label.take(1).uppercase(),
                color = Pg.muted,
                fontFamily = Inter,
                fontWeight = FontWeight.SemiBold,
                fontSize = 14.sp,
            )
        }
    }
}

@Composable
private fun TrafficRouting(settings: AndroidSettings, modifier: Modifier) {
    var preset by remember { mutableStateOf(settings.trafficPreset) }
    var finalAction by remember { mutableStateOf(settings.finalAction) }
    var rules by remember { mutableStateOf(settings.customRules) }
    var showAdd by remember { mutableStateOf(false) }

    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val geoDir = remember { File(context.filesDir, "rulesets") }
    var geoCache by remember { mutableStateOf(readGeoCache(geoDir)) }
    var geoStatus by remember { mutableStateOf<String?>(null) }
    var downloading by remember { mutableStateOf(false) }
    val geoRules = rules.filter {
        it.kind == TrafficRuleKind.Geoip || it.kind == TrafficRuleKind.Geosite
    }

    Column(
        modifier = modifier
            .verticalScroll(rememberScrollState())
            .padding(bottom = 8.dp),
    ) {
        SectionLabel("Пресет")
        Spacer(Modifier.height(8.dp))

        TrafficPreset.entries.forEach { item ->
            SelectCard(
                title = item.label,
                description = item.description,
                selected = preset == item,
                onClick = {
                    preset = item
                    settings.trafficPreset = item
                },
            )
            Spacer(Modifier.height(6.dp))
        }

        Spacer(Modifier.height(8.dp))

        if (preset == TrafficPreset.Custom) {
            SectionLabel("Финальное действие")
            Spacer(Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                TrafficAction.entries.forEach { action ->
                    ChoiceChip(
                        label = action.label,
                        selected = finalAction == action,
                        modifier = Modifier.weight(1f),
                        onClick = {
                            finalAction = action
                            settings.finalAction = action
                        },
                    )
                }
            }

            Spacer(Modifier.height(14.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                SectionLabel("Правила", Modifier.weight(1f))
                TextButton(
                    onClick = { showAdd = true },
                    contentPadding = PaddingValues(horizontal = 6.dp),
                ) {
                    Text(
                        text = "+ Добавить",
                        color = Pg.accent2,
                        fontFamily = Inter,
                        fontWeight = FontWeight.Medium,
                        fontSize = 11.sp,
                    )
                }
            }

            if (rules.isEmpty()) {
                Panel(Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(16.dp)) {
                        Text(
                            text = "Пока нет правил",
                            color = Pg.text,
                            fontFamily = Inter,
                            fontWeight = FontWeight.Medium,
                            fontSize = 13.sp,
                        )
                        Spacer(Modifier.height(3.dp))
                        Text(
                            text = "Добавьте домен, суффикс или IP/CIDR и выберите действие.",
                            color = Pg.faint,
                            fontFamily = Inter,
                            fontSize = 11.sp,
                            lineHeight = 15.sp,
                        )
                    }
                }
            } else {
                rules.forEach { rule ->
                    RuleCard(
                        rule = rule,
                        onDelete = {
                            rules = rules - rule
                            settings.customRules = rules
                        },
                    )
                    Spacer(Modifier.height(6.dp))
                }
            }

            if (geoRules.isNotEmpty()) {
                Spacer(Modifier.height(14.dp))
                SectionLabel("GeoIP / GeoSite")
                Spacer(Modifier.height(8.dp))
                Panel(Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(16.dp)) {
                        Text(
                            text = "Базы .srs кэшируются в приложении. Источник: GitHub SagerNet.",
                            color = Pg.faint,
                            fontFamily = Inter,
                            fontSize = 11.sp,
                            lineHeight = 15.sp,
                        )
                        Spacer(Modifier.height(5.dp))
                        Text(
                            text = "В кэше: ${geoCache.files} файлов · ${formatCacheSize(geoCache.bytes)}",
                            color = Pg.muted,
                            fontFamily = Mono,
                            fontSize = 10.sp,
                        )
                        if (!geoStatus.isNullOrBlank()) {
                            Spacer(Modifier.height(6.dp))
                            Text(
                                text = geoStatus!!,
                                color = if (geoStatus!!.startsWith("Ошибка")) Pg.red else Pg.green,
                                fontFamily = Inter,
                                fontSize = 10.sp,
                                lineHeight = 14.sp,
                                maxLines = 4,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        Spacer(Modifier.height(10.dp))
                        GhostButton(
                            text = if (downloading) "Скачивание…" else "Скачать / обновить базы",
                            onClick = {
                                if (downloading) return@GhostButton
                                downloading = true
                                geoStatus = null
                                scope.launch {
                                    val raw = withContext(Dispatchers.IO) {
                                        runCatching {
                                            CoreBridge.prepareRuleSets(
                                                settings.customRulesJson(),
                                                geoDir.absolutePath,
                                                7L * 24 * 60 * 60,
                                            )
                                        }.getOrElse {
                                            """{"error":${JSONObject.quote(it.message ?: "download failed")}}"""
                                        }
                                    }
                                    val result = runCatching { JSONObject(raw) }.getOrNull()
                                    val error = result?.optString("error").orEmpty()
                                    geoStatus = if (error.isNotBlank()) {
                                        "Ошибка: $error"
                                    } else {
                                        val available = result?.optJSONArray("available")?.length() ?: 0
                                        val downloaded = result?.optInt("downloaded") ?: 0
                                        val errors = result?.optJSONArray("errors")
                                        val errorText = buildList {
                                            if (errors != null) {
                                                for (index in 0 until errors.length()) {
                                                    errors.optString(index)
                                                        .takeIf { it.isNotBlank() }
                                                        ?.let(::add)
                                                }
                                            }
                                        }.joinToString("; ")
                                        buildString {
                                            append("Готово: доступно ")
                                            append(available)
                                            append(", скачано ")
                                            append(downloaded)
                                            if (errorText.isNotBlank()) {
                                                append("; ")
                                                append(errorText)
                                            }
                                        }
                                    }
                                    geoCache = readGeoCache(geoDir)
                                    downloading = false
                                }
                            },
                            modifier = Modifier.fillMaxWidth(),
                        )
                        Spacer(Modifier.height(6.dp))
                        Text(
                            text = "После загрузки нажмите «Применить и переподключить».",
                            color = Pg.faint,
                            fontFamily = Inter,
                            fontSize = 10.sp,
                        )
                    }
                }
            }
        } else {
            Panel(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp)) {
                    Text(
                        text = preset.description,
                        color = Pg.text,
                        fontFamily = Inter,
                        fontWeight = FontWeight.Medium,
                        fontSize = 12.sp,
                    )
                    Spacer(Modifier.height(3.dp))
                    Text(
                        text = "Правила пресета задаёт ProtoGrid-core. Для своих правил выберите «Свой профиль».",
                        color = Pg.faint,
                        fontFamily = Inter,
                        fontSize = 11.sp,
                        lineHeight = 15.sp,
                    )
                }
            }
        }
    }

    if (showAdd) {
        AddRuleDialog(
            onDismiss = { showAdd = false },
            onAdd = { rule ->
                rules = rules + rule
                settings.customRules = rules
                showAdd = false
            },
        )
    }
}

@Composable
private fun SelectCard(
    title: String,
    description: String,
    selected: Boolean,
    onClick: () -> Unit,
) {
    val shape = RoundedCornerShape(12.dp)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(if (selected) Pg.accentSoft else Pg.panel)
            .border(
                1.dp,
                if (selected) Pg.accentLine else Pg.line,
                shape,
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioMark(selected)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Text(
                text = title,
                color = Pg.text,
                fontFamily = Inter,
                fontWeight = FontWeight.Medium,
                fontSize = 13.sp,
            )
            Spacer(Modifier.height(2.dp))
            Text(
                text = description,
                color = Pg.faint,
                fontFamily = Inter,
                fontSize = 10.sp,
                lineHeight = 14.sp,
            )
        }
    }
}

@Composable
private fun RadioMark(selected: Boolean) {
    Box(
        modifier = Modifier
            .size(18.dp)
            .clip(CircleShape)
            .border(1.5.dp, if (selected) Pg.accent else Pg.lineStrong, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        if (selected) {
            Box(
                modifier = Modifier
                    .size(8.dp)
                    .clip(CircleShape)
                    .background(Pg.accent),
            )
        }
    }
}

@Composable
private fun ChoiceChip(
    label: String,
    selected: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    Box(
        modifier = modifier
            .height(36.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(if (selected) Pg.accentSoft else Pg.panel2)
            .border(
                1.dp,
                if (selected) Pg.accentLine else Pg.line,
                RoundedCornerShape(8.dp),
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 6.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color = if (selected) Pg.accent2 else Pg.muted,
            fontFamily = Inter,
            fontWeight = FontWeight.Medium,
            fontSize = 11.sp,
            maxLines = 1,
            textAlign = TextAlign.Center,
        )
    }
}

@Composable
private fun RuleCard(rule: TrafficRule, onDelete: () -> Unit) {
    val shape = RoundedCornerShape(10.dp)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(shape)
            .background(Pg.panel2)
            .border(1.dp, Pg.line, shape)
            .padding(start = 12.dp, end = 8.dp, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = rule.kind.label,
                    color = Pg.faint,
                    fontFamily = Mono,
                    fontSize = 9.sp,
                    fontWeight = FontWeight.Medium,
                )
                Spacer(Modifier.width(8.dp))
                ActionBadge(rule.action)
            }
            Spacer(Modifier.height(4.dp))
            Text(
                text = rule.value,
                color = Pg.text,
                fontFamily = Mono,
                fontSize = 12.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        IconAction(
            painter = painterResource(R.drawable.ic_trash),
            contentDescription = "Удалить правило",
            onClick = onDelete,
            tint = Pg.faint,
        )
    }
}

@Composable
private fun ActionBadge(action: TrafficAction) {
    val color = when (action) {
        TrafficAction.Proxy -> Pg.accent2
        TrafficAction.Direct -> Pg.green
        TrafficAction.Block -> Pg.red
    }
    Text(
        text = action.label,
        modifier = Modifier
            .clip(RoundedCornerShape(5.dp))
            .background(color.copy(alpha = 0.08f))
            .border(1.dp, color.copy(alpha = 0.40f), RoundedCornerShape(5.dp))
            .padding(horizontal = 6.dp, vertical = 2.dp),
        color = color,
        fontFamily = Mono,
        fontSize = 8.sp,
        fontWeight = FontWeight.Medium,
        maxLines = 1,
    )
}

@Composable
private fun AddRuleDialog(onDismiss: () -> Unit, onAdd: (TrafficRule) -> Unit) {
    var kind by remember { mutableStateOf(TrafficRuleKind.DomainSuffix) }
    var action by remember { mutableStateOf(TrafficAction.Proxy) }
    var value by remember { mutableStateOf("") }

    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth(0.92f)
                .clip(RoundedCornerShape(16.dp))
                .background(Pg.panel)
                .border(1.dp, Pg.lineStrong, RoundedCornerShape(16.dp))
                .padding(20.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(
                        text = "Новое правило",
                        color = Pg.text,
                        fontFamily = Inter,
                        fontWeight = FontWeight.Bold,
                        fontSize = 18.sp,
                    )
                    Spacer(Modifier.height(2.dp))
                    Text(
                        text = "Домен, суффикс или IP/CIDR",
                        color = Pg.faint,
                        fontFamily = Inter,
                        fontSize = 11.sp,
                    )
                }
                IconAction(
                    painter = painterResource(R.drawable.ic_close),
                    contentDescription = "Закрыть",
                    onClick = onDismiss,
                )
            }

            Spacer(Modifier.height(16.dp))

            SectionLabel("Тип")
            Spacer(Modifier.height(6.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                ChoiceChip(
                    label = "Домен",
                    selected = kind == TrafficRuleKind.Domain,
                    modifier = Modifier.weight(1f),
                    onClick = { kind = TrafficRuleKind.Domain },
                )
                ChoiceChip(
                    label = "Суффикс",
                    selected = kind == TrafficRuleKind.DomainSuffix,
                    modifier = Modifier.weight(1f),
                    onClick = { kind = TrafficRuleKind.DomainSuffix },
                )
            }
            Spacer(Modifier.height(6.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                ChoiceChip(
                    label = "Ключевое слово",
                    selected = kind == TrafficRuleKind.DomainKeyword,
                    modifier = Modifier.weight(1f),
                    onClick = { kind = TrafficRuleKind.DomainKeyword },
                )
                ChoiceChip(
                    label = "IP / CIDR",
                    selected = kind == TrafficRuleKind.IpCidr,
                    modifier = Modifier.weight(1f),
                    onClick = { kind = TrafficRuleKind.IpCidr },
                )
            }
            Spacer(Modifier.height(6.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                ChoiceChip(
                    label = "GeoIP",
                    selected = kind == TrafficRuleKind.Geoip,
                    modifier = Modifier.weight(1f),
                    onClick = { kind = TrafficRuleKind.Geoip },
                )
                ChoiceChip(
                    label = "GeoSite",
                    selected = kind == TrafficRuleKind.Geosite,
                    modifier = Modifier.weight(1f),
                    onClick = { kind = TrafficRuleKind.Geosite },
                )
            }

            if (kind == TrafficRuleKind.Geoip || kind == TrafficRuleKind.Geosite) {
                Spacer(Modifier.height(6.dp))
                Text(
                    text = "Нужны базы .srs: после добавления нажмите «Скачать / обновить базы».",
                    color = Pg.amber,
                    fontFamily = Inter,
                    fontSize = 10.sp,
                    lineHeight = 14.sp,
                )
            }

            Spacer(Modifier.height(12.dp))

            SectionLabel("Действие")
            Spacer(Modifier.height(6.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                TrafficAction.entries.forEach { item ->
                    ChoiceChip(
                        label = item.label,
                        selected = action == item,
                        modifier = Modifier.weight(1f),
                        onClick = { action = item },
                    )
                }
            }

            Spacer(Modifier.height(12.dp))

            OutlinedTextField(
                value = value,
                onValueChange = { value = it },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                placeholder = {
                    Text(
                        text = when (kind) {
                            TrafficRuleKind.Geoip -> "ru, us, de"
                            TrafficRuleKind.Geosite -> "category-ru, category-ads-all"
                            TrafficRuleKind.IpCidr -> "10.0.0.0/8"
                            TrafficRuleKind.Domain -> "example.com"
                            TrafficRuleKind.DomainSuffix -> ".ru"
                            TrafficRuleKind.DomainKeyword -> "ads"
                        },
                        color = Pg.faint,
                        fontFamily = Mono,
                        fontSize = 11.sp,
                    )
                },
                textStyle = TextStyle(
                    fontFamily = Mono,
                    fontSize = 12.sp,
                    color = Pg.text,
                ),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                shape = RoundedCornerShape(10.dp),
                colors = brandFieldColors(),
            )

            Spacer(Modifier.height(16.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                TextButton(
                    onClick = onDismiss,
                    colors = androidx.compose.material3.ButtonDefaults.textButtonColors(
                        contentColor = Pg.muted,
                    ),
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
                    text = "Добавить",
                    onClick = {
                        if (value.isNotBlank()) {
                            onAdd(TrafficRule.create(kind, value, action))
                        }
                    },
                    enabled = value.isNotBlank(),
                )
            }
        }
    }
}

private data class GeoCache(val files: Int, val bytes: Long)

private fun readGeoCache(dir: File): GeoCache {
    val files = dir.listFiles { file -> file.isFile && file.extension == "srs" }.orEmpty()
    return GeoCache(files.size, files.sumOf { it.length() })
}

private fun formatCacheSize(bytes: Long): String = when {
    bytes >= 1024L * 1024L -> String.format(Locale.US, "%.1f МБ", bytes / (1024.0 * 1024.0))
    bytes >= 1024L -> String.format(Locale.US, "%.0f КБ", bytes / 1024.0)
    else -> "$bytes Б"
}

@Composable
private fun brandFieldColors() = OutlinedTextFieldDefaults.colors(
    focusedTextColor = Pg.text,
    unfocusedTextColor = Pg.text,
    focusedContainerColor = Pg.bg2,
    unfocusedContainerColor = Pg.bg2,
    cursorColor = Pg.accent2,
    focusedBorderColor = Pg.accent,
    unfocusedBorderColor = Pg.lineStrong,
    focusedPlaceholderColor = Pg.faint,
    unfocusedPlaceholderColor = Pg.faint,
)
