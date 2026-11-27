# Removes stale Wintun tunnel devices (SWD\WINTUN...) that block ProtoGrid's
# TUN start after a crash or a too-fast restart:
#   configure tun interface:
#     (create adapter: Cannot create a file when that file already exists.
#      | open existing adapter: Element not found.)
#
# Only Wintun devices are touched: TAP / OpenVPN / WireGuard / Hyper-V adapters
# are never removed. Devices that are currently attached are skipped unless
# -Force is given and no VPN/core process is running.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File reset-tun.ps1
#   ... -List      only show what would be removed
#   ... -Force     also try attached Wintun adapters (no VPN process running)
#   ... -Pause     wait for Enter at the end (double-click mode)

[CmdletBinding()]
param(
    [switch]$List,
    [switch]$Force,
    [switch]$Pause
)

$ErrorActionPreference = 'Stop'
$netClass = '{4d36e972-e325-11ce-bfc1-08002be10318}'

function Test-Admin {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Stop-Here {
    param([int]$Code)
    if ($Pause) { [void](Read-Host 'Press Enter to close') }
    exit $Code
}

function Get-TunAlias {
    param([string]$InstanceId)
    $guid = ($InstanceId -split '\\')[-1]
    $key = 'HKLM:\SYSTEM\CurrentControlSet\Control\Network\' + $netClass + '\' + $guid + '\Connection'
    if (-not (Test-Path -LiteralPath $key)) { return $null }
    $props = Get-ItemProperty -LiteralPath $key -ErrorAction SilentlyContinue
    if ($props -and $props.PSObject.Properties['Name'] -and $props.Name) {
        return [string]$props.Name
    }
    return $null
}

try {
    $devices = @(Get-PnpDevice -Class Net -ErrorAction Stop |
        Where-Object { $_.InstanceId -like 'SWD\WINTUN*' })
} catch {
    Write-Host ('Get-PnpDevice failed: ' + $_.Exception.Message)
    Write-Host 'Fallback: Device Manager -> View -> Show hidden devices ->'
    Write-Host 'uninstall the greyed out "sing-tun Tunnel" entries.'
    Stop-Here 1
}

$vpnNames = @(
    'protogrid-core', 'sing-box', 'xray', 'v2ray', 'v2rayN',
    'clash', 'clash-meta', 'mihomo', 'hiddify', 'nekoray'
)
$busy = @(Get-Process -Name $vpnNames -ErrorAction SilentlyContinue)
if ($busy.Count -gt 0) {
    $names = ($busy | ForEach-Object { $_.ProcessName } | Sort-Object -Unique) -join ', '
    Write-Host ('Running VPN/core processes: ' + $names)
    Write-Host 'Close them first: an adapter that is in use must not be removed.'
}

$items = @(foreach ($device in $devices) {
    $alias = Get-TunAlias $device.InstanceId
    if (-not $alias) { $alias = [string]$device.FriendlyName }
    [pscustomobject]@{
        Alias      = $alias
        Present    = [bool]$device.Present
        Status     = [string]$device.Status
        Problem    = [string]$device.Problem
        InstanceId = [string]$device.InstanceId
    }
})

if ($items.Count -eq 0) {
    Write-Host 'No Wintun tunnel devices found - nothing to clean.'
    Stop-Here 0
}

Write-Host ''
Write-Host 'Wintun tunnel devices:'
$items |
    Format-Table -AutoSize -Property Alias, Present, Status, Problem, InstanceId |
    Out-String -Width 4096 |
    Write-Host

$targets = @($items | Where-Object { -not $_.Present })
$kept = @($items | Where-Object { $_.Present })

if ($Force -and $busy.Count -eq 0) {
    $targets = $items
    $kept = @()
}

if ($kept.Count -gt 0) {
    Write-Host ('Attached (in use) devices are kept: ' + (($kept | ForEach-Object { $_.Alias }) -join ', '))
}

if ($List) {
    Write-Host ''
    Write-Host ('Would remove: ' + $targets.Count + ' device(s).')
    Stop-Here 0
}

if ($targets.Count -eq 0) {
    Write-Host ''
    Write-Host 'Nothing to remove. Reboot if ProtoGrid still fails to start TUN.'
    Stop-Here 0
}

if (-not (Test-Admin)) {
    Write-Host 'Administrator rights are required, asking for elevation...'
    $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', ('"' + $PSCommandPath + '"'))
    if ($List) { $arguments += '-List' }
    if ($Force) { $arguments += '-Force' }
    if ($Pause) { $arguments += '-Pause' }
    Start-Process -FilePath (Get-Process -Id $PID).Path -Verb RunAs -ArgumentList $arguments
    Stop-Here 0
}

$failed = 0
foreach ($item in $targets) {
    Write-Host ('Removing ' + $item.Alias + ' (' + $item.InstanceId + ') ...')
    $output = & pnputil.exe /remove-device $item.InstanceId 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host '  removed'
        continue
    }
    $removed = $false
    try {
        Remove-PnpDevice -InstanceId $item.InstanceId -Confirm:$false -ErrorAction Stop
        $removed = $true
    } catch {
        $removed = $false
    }
    if ($removed) {
        Write-Host '  removed'
    } else {
        $failed++
        Write-Host ('  failed: ' + (($output | Out-String).Trim()))
    }
}

Write-Host ''
if ($failed -eq 0) {
    Write-Host ('Done: removed ' + $targets.Count + ' device(s). You can start ProtoGrid now.')
} else {
    Write-Host ('Removed ' + ($targets.Count - $failed) + ', failed ' + $failed + '. Reboot the PC.')
}
Stop-Here ([int]($failed -gt 0))
