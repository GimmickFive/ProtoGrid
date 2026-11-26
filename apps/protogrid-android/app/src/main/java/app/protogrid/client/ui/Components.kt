package app.protogrid.client.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.protogrid.client.R
import app.protogrid.client.data.ProfileSummary
import app.protogrid.client.vpn.VpnStatus

@Composable
fun Panel(
    modifier: Modifier = Modifier,
    padding: Dp = 0.dp,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(
        modifier = modifier
            .clip(RoundedCornerShape(14.dp))
            .background(Pg.panel)
            .border(1.dp, Pg.line, RoundedCornerShape(14.dp))
            .padding(padding),
        content = content,
    )
}

/** The ProtoGrid brand mark used across the app. */
@Composable
fun BrandLogo(modifier: Modifier = Modifier, size: Dp = 28.dp) {
    Image(
        painter = painterResource(R.drawable.ic_launcher),
        contentDescription = null,
        modifier = modifier
            .size(size)
            .clip(RoundedCornerShape(size * 0.22f)),
        contentScale = ContentScale.Fit,
    )
}

@Composable
fun SectionLabel(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = Pg.faint,
) {
    Text(
        text = text.uppercase(),
        modifier = modifier,
        color = color,
        fontFamily = Mono,
        fontSize = 10.sp,
        fontWeight = FontWeight.Medium,
        letterSpacing = 1.1.sp,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
    )
}

@Composable
fun StatusDot(color: Color, size: Dp = 8.dp, modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .size(size)
            .clip(CircleShape)
            .background(color),
    )
}

@Composable
fun StatusPill(status: VpnStatus, modifier: Modifier = Modifier) {
    Row(
        modifier = modifier
            .clip(RoundedCornerShape(50))
            .background(Pg.panel2)
            .border(1.dp, Pg.line, RoundedCornerShape(50))
            .padding(horizontal = 10.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        StatusDot(statusColor(status), 7.dp)
        Spacer(Modifier.width(7.dp))
        Text(
            text = statusLabel(status),
            color = Pg.text,
            fontFamily = Inter,
            fontSize = 12.sp,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
        )
    }
}

@Composable
fun PrimaryButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    icon: Painter? = null,
) {
    Button(
        onClick = onClick,
        modifier = modifier.height(48.dp),
        enabled = enabled,
        shape = RoundedCornerShape(10.dp),
        colors = ButtonDefaults.buttonColors(
            containerColor = Pg.accent,
            contentColor = Color.White,
            disabledContainerColor = Pg.panel3,
            disabledContentColor = Pg.faint,
        ),
        elevation = ButtonDefaults.buttonElevation(
            defaultElevation = 0.dp,
            pressedElevation = 0.dp,
            disabledElevation = 0.dp,
        ),
        contentPadding = PaddingValues(horizontal = 16.dp),
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
        }
        Text(
            text = text,
            fontFamily = Inter,
            fontWeight = FontWeight.SemiBold,
            fontSize = 14.sp,
            maxLines = 1,
        )
    }
}

@Composable
fun GhostButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: Painter? = null,
) {
    OutlinedButton(
        onClick = onClick,
        modifier = modifier.height(40.dp),
        shape = RoundedCornerShape(9.dp),
        border = BorderStroke(1.dp, Pg.lineStrong),
        colors = ButtonDefaults.outlinedButtonColors(contentColor = Pg.muted),
        contentPadding = PaddingValues(horizontal = 14.dp),
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, modifier = Modifier.size(16.dp))
            Spacer(Modifier.width(7.dp))
        }
        Text(
            text = text,
            fontFamily = Inter,
            fontWeight = FontWeight.Medium,
            fontSize = 12.sp,
            maxLines = 1,
        )
    }
}

@Composable
fun IconAction(
    painter: Painter,
    contentDescription: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tint: Color = Pg.muted,
) {
    Box(
        modifier = modifier
            .size(34.dp)
            .clip(RoundedCornerShape(9.dp))
            .background(Pg.panel2)
            .border(1.dp, Pg.line, RoundedCornerShape(9.dp))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            painter = painter,
            contentDescription = contentDescription,
            tint = tint,
            modifier = Modifier.size(17.dp),
        )
    }
}

@Composable
fun ProtocolBadge(protocol: String, modifier: Modifier = Modifier) {
    val color = protocolColor(protocol)
    Text(
        text = protocolLabel(protocol),
        modifier = modifier
            .clip(RoundedCornerShape(6.dp))
            .background(color.copy(alpha = 0.08f))
            .border(1.dp, color.copy(alpha = 0.45f), RoundedCornerShape(6.dp))
            .padding(horizontal = 7.dp, vertical = 3.dp),
        color = color,
        fontFamily = Mono,
        fontSize = 9.sp,
        fontWeight = FontWeight.Medium,
        letterSpacing = 0.3.sp,
        maxLines = 1,
    )
}

@Composable
private fun SelectionMark(selected: Boolean) {
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
private fun SelectedPill() {
    Row(
        modifier = Modifier
            .clip(RoundedCornerShape(50))
            .background(Pg.accentSoft)
            .border(1.dp, Pg.accentLine, RoundedCornerShape(50))
            .padding(horizontal = 8.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        StatusDot(Pg.accent, 5.dp)
        Spacer(Modifier.width(5.dp))
        Text(
            text = "Выбран",
            color = Pg.accent2,
            fontFamily = Mono,
            fontSize = 9.sp,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
        )
    }
}

@Composable
fun ServerRow(
    profile: ProfileSummary,
    selected: Boolean,
    onSelect: () -> Unit,
    onDelete: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val shape = RoundedCornerShape(12.dp)
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(if (selected) Pg.accent.copy(alpha = 0.09f) else Pg.panel)
            .border(
                1.dp,
                if (selected) Pg.accent.copy(alpha = 0.35f) else Pg.line,
                shape,
            )
            .clickable(onClick = onSelect)
            .padding(start = 14.dp, end = 10.dp, top = 12.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        SelectionMark(selected)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = profile.name,
                    modifier = Modifier.weight(1f, fill = false),
                    color = Pg.text,
                    fontFamily = Inter,
                    fontWeight = FontWeight.Medium,
                    fontSize = 14.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.width(8.dp))
                ProtocolBadge(profile.protocol)
            }
            Spacer(Modifier.height(3.dp))
            Text(
                text = profile.endpoint,
                color = Pg.faint,
                fontFamily = Mono,
                fontSize = 11.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        if (selected) {
            Spacer(Modifier.width(8.dp))
            SelectedPill()
        }
        Spacer(Modifier.width(6.dp))
        IconAction(
            painter = painterResource(R.drawable.ic_trash),
            contentDescription = "Удалить",
            onClick = onDelete,
            tint = Pg.faint,
        )
    }
}

@Composable
fun StatTile(
    label: String,
    value: String,
    hint: String,
    modifier: Modifier = Modifier,
    valueMono: Boolean = false,
    valueColor: Color = Pg.text,
) {
    Panel(modifier) {
        Column(Modifier.padding(14.dp)) {
            SectionLabel(label)
            Spacer(Modifier.height(6.dp))
            Text(
                text = value,
                color = valueColor,
                fontFamily = if (valueMono) Mono else Inter,
                fontWeight = FontWeight.Bold,
                fontSize = if (valueMono) 16.sp else 24.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(Modifier.height(2.dp))
            Text(
                text = hint,
                color = Pg.faint,
                fontFamily = Inter,
                fontSize = 10.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}
