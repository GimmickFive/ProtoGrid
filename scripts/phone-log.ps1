param(
    [string]$Device = "",
    [ValidateSet("auto", "lite", "standard")]
    [string]$Flavor = "auto",
    [int]$Lines = 200,
    [switch]$Raw
)

$ErrorActionPreference = "Stop"
try {
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
} catch {
}

# Write-Host uses LF-only newlines in some Windows PowerShell hosts, which makes
# the output look like a staircase. [Console]::WriteLine always ends with CRLF.
function Write-Line {
    param(
        [Parameter(Position = 0)]
        [string]$Text = "",
        [string]$ForegroundColor = ""
    )

    if ($ForegroundColor -and -not [Console]::IsOutputRedirected) {
        try {
            $color = [ConsoleColor]$ForegroundColor
            $oldColor = [Console]::ForegroundColor
            try {
                [Console]::ForegroundColor = $color
                [Console]::WriteLine($Text)
            } finally {
                [Console]::ForegroundColor = $oldColor
            }
            return
        } catch {
            # If colors are unavailable, fall back to plain output below.
        }
    }
    [Console]::WriteLine($Text)
}

function Find-Adb {
    $candidates = @()
    if ($env:ADB) { $candidates += $env:ADB }
    $command = Get-Command adb -ErrorAction SilentlyContinue
    if ($command) { $candidates += $command.Source }
    $candidates += @(
        "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe",
        "$env:USERPROFILE\AppData\Local\Android\Sdk\platform-tools\adb.exe",
        "C:\platform-tools\adb.exe",
        "$PSScriptRoot\..\tools\platform-tools\adb.exe",
        "$PSScriptRoot\..\dist\platform-tools\adb.exe"
    )
    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path $candidate)) {
            return @{ Exe = $candidate; Prefix = @() }
        }
    }
    $wsl = Get-Command wsl.exe -ErrorAction SilentlyContinue
    if ($wsl) {
        $preferred = "/tmp/opencode/pt/platform-tools/adb"
        & wsl.exe test -x $preferred
        if ($LASTEXITCODE -eq 0) {
            return @{ Exe = "wsl.exe"; Prefix = @($preferred) }
        }
        $linuxAdb = ((& wsl.exe bash -lc "command -v adb 2>/dev/null | head -n1") -join "`n").Trim()
        if ($linuxAdb) {
            return @{ Exe = "wsl.exe"; Prefix = @($linuxAdb) }
        }
    }
    return $null
}

$adbInfo = Find-Adb
if (-not $adbInfo) {
    Write-Line "adb не найден." -ForegroundColor Red
    Write-Line "Поставьте platform-tools (https://developer.android.com/tools/releases/platform-tools)"
    Write-Line "или положите adb.exe в tools\platform-tools\ и запустите снова."
    exit 1
}

function Invoke-Adb {
    param([string[]]$Arguments)
    $all = @($adbInfo.Prefix) + $Arguments
    & $adbInfo.Exe @all
}

$litePackage = "app.protogrid.client.lite"
$standardPackage = "app.protogrid.client"

function Get-LogPath {
    param([string]$Package)
    return "/sdcard/Android/data/$Package/files/core.log"
}

function Get-LogMtime {
    param([string]$Serial, [string]$Package)
    $path = Get-LogPath $Package
    $value = (Invoke-Adb -Arguments @("-s", $Serial, "shell", "stat", "-c", "%Y", $path)) -join ""
    if ($LASTEXITCODE -eq 0 -and $value -match '^\d+$') {
        return [long]$value
    }
    return -1
}

if ($Device) {
    Write-Line "Подключаюсь к $Device ..." -ForegroundColor DarkGray
    $connectOutput = (Invoke-Adb -Arguments @("connect", $Device)) -join "`n"
    if ($connectOutput.Trim()) {
        Write-Line $connectOutput.Trim() -ForegroundColor DarkGray
    }
    if ($connectOutput -notmatch '(?i)connected to|already connected') {
        Write-Line "Не удалось подключиться к $Device." -ForegroundColor Red
        Write-Line "Проверьте IP:порт беспроводной отладки и сопряжение (adb pair)."
        exit 1
    }
}

$deviceLines = @(Invoke-Adb -Arguments @("devices"))
$serial = $null
$state = $null
foreach ($line in $deviceLines) {
    if ($line -match '^(\S+)\s+(device|offline|unauthorized|authorizing)$') {
        $candidate = $Matches[1]
        $candidateState = $Matches[2]
        if ($Device -and $candidate -ne $Device) { continue }
        if ($candidateState -eq 'device') {
            if (-not $serial) {
                $serial = $candidate
                $state = $candidateState
            }
        } elseif (-not $serial) {
            $serial = $candidate
            $state = $candidateState
        }
    }
}
if (-not $serial) {
    Write-Line "Телефон не найден в adb." -ForegroundColor Red
    Write-Line "Включите «Отладка по Wi-Fi» и запустите:"
    Write-Line "  run-phone-log.bat 192.168.1.42:ПОРТ" -ForegroundColor Yellow
    exit 1
}
if ($state -ne 'device') {
    Write-Line "Устройство $serial в состоянии '$state'." -ForegroundColor Red
    switch ($state) {
        'unauthorized' { Write-Line "Разблокируйте телефон и подтвердите отладку по этому компьютеру." }
        'offline' { Write-Line "Переподключите отладку по Wi-Fi или выполните: adb reconnect $serial" }
        default { Write-Line "Проверьте подключение телефона." }
    }
    exit 1
}

$installed = (Invoke-Adb -Arguments @("-s", $serial, "shell", "pm", "list", "packages")) -join "`n"
$packageSet = @{}
foreach ($line in ($installed -split "`r?`n")) {
    if ($line -match '^package:(.+?)\s*$') {
        $packageSet[$Matches[1]] = $true
    }
}

$installedPackages = @()
if ($packageSet.ContainsKey($litePackage)) { $installedPackages += $litePackage }
if ($packageSet.ContainsKey($standardPackage)) { $installedPackages += $standardPackage }

if ($Flavor -ne "auto") {
    $wanted = $standardPackage
    if ($Flavor -eq "lite") { $wanted = $litePackage }
    if (-not $packageSet.ContainsKey($wanted)) {
        Write-Line "Пакет $wanted не установлен на телефоне." -ForegroundColor Red
        exit 1
    }
    $installedPackages = @($wanted)
}

if ($installedPackages.Count -eq 0) {
    Write-Line "ProtoGrid не установлен на телефоне ($serial)." -ForegroundColor Red
    Write-Line "Ожидался пакет $litePackage или $standardPackage."
    exit 1
}

# If both flavors are installed, follow the log that already exists / is fresher.
$package = $installedPackages[0]
if ($installedPackages.Count -gt 1) {
    $bestPackage = $null
    $bestTime = -1
    foreach ($candidate in $installedPackages) {
        $candidateTime = Get-LogMtime -Serial $serial -Package $candidate
        if ($candidateTime -gt $bestTime) {
            $bestTime = $candidateTime
            $bestPackage = $candidate
        }
    }
    if ($bestPackage) { $package = $bestPackage }
}
$logPath = Get-LogPath $package

function Get-ConsoleWidth {
    $width = 0
    try {
        $width = [Console]::WindowWidth
    } catch {
        $width = 0
    }
    if ($width -lt 40) { $width = 120 }
    return $width - 1
}

function Shorten-LogMessage {
    param([string]$Message)
    # vless[<uuid>] / tun[tun-in] -> vless / tun
    return [regex]::Replace($Message, '(outbound|inbound)/([A-Za-z0-9_.-]+)\[[^\]]*\]', '$1/$2')
}

$logPattern = '^(?<tz>[+-]\d{4}) (?<date>\d{4}-\d{2}-\d{2}) (?<time>\d{2}:\d{2}:\d{2}) (?<level>[A-Z]+) (?:\[(?<id>\d+) (?<ms>[^\]]+)\] )?(?<message>.*)$'

function Format-LogLine {
    param([string]$Line)
    if ($Line -match $logPattern) {
        $time = $Matches['time']
        $level = $Matches['level']
        $ms = $Matches['ms']
        $message = Shorten-LogMessage $Matches['message']
        $prefix = '{0} {1}' -f $time, $level.PadRight(5)
        if ($ms) { $prefix = $prefix + ' [' + $ms + ']' }
        return ($prefix + ' ' + $message)
    }
    $Line = [regex]::Replace($Line, '^[+-]\d{4} \d{4}-\d{2}-\d{2} ', '')
    return Shorten-LogMessage $Line
}

function Limit-LogLine {
    param([string]$Text, [int]$Width)
    if ($Width -lt 20) { $Width = 20 }
    if ($Text.Length -le $Width) { return $Text }
    return $Text.Substring(0, $Width - 3) + '...'
}

function Get-LogColor {
    param([string]$Line)
    if ($Line -match '\b(ERROR|FATAL|PANIC)\b') { return 'Red' }
    if ($Line -match 'outbound/(block|reject)') { return 'Red' }
    if ($Line -match 'outbound/direct') { return 'Green' }
    if ($Line -match 'dns[:/]') { return 'DarkCyan' }
    if ($Line -match 'outbound/') { return 'Cyan' }
    if ($Line -match 'inbound/') { return 'DarkGray' }
    return ''
}

Write-Line "ProtoGrid: $package ($serial)" -ForegroundColor Cyan
Write-Line "Лог: $logPath" -ForegroundColor DarkGray
if (-not $Raw) {
    Write-Line "Формат: время уровень [задержка] событие. Полные строки: -Raw." -ForegroundColor DarkGray
}
Write-Line "Подключитесь в приложении, если ещё не подключено. Ctrl+C — выход." -ForegroundColor DarkGray
Write-Line ""

$waited = 0
while ($true) {
    Invoke-Adb -Arguments @("-s", $serial, "shell", "test", "-f", $logPath) | Out-Null
    if ($LASTEXITCODE -eq 0) { break }
    if ($waited -eq 0) {
        Write-Line "Ждём файл лога..." -ForegroundColor Yellow
    }
    if ($waited -ge 120) {
        Write-Line "Файл лога так и не появился: $logPath" -ForegroundColor Red
        Write-Line "Откройте ProtoGrid на телефоне и нажмите «Подключиться», затем запустите снова."
        exit 1
    }
    Start-Sleep -Seconds 2
    $waited += 2
}

Invoke-Adb -Arguments @("-s", $serial, "shell", "tail", "-n", "$Lines", "-f", $logPath) |
    ForEach-Object {
        $rawLine = [string]$_
        if ($rawLine -match 'failed to search process') { return }

        $displayLine = $rawLine
        if (-not $Raw) {
            $displayLine = Format-LogLine $rawLine
            $displayLine = Limit-LogLine -Text $displayLine -Width (Get-ConsoleWidth)
        }

        $color = Get-LogColor $rawLine
        if ($color) {
            Write-Line $displayLine -ForegroundColor $color
        } else {
            Write-Line $displayLine
        }
    }

Write-Line ""
Write-Line "Поток лога завершился. Телефон отключился или приложение остановилось." -ForegroundColor Yellow
