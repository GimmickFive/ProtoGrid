param(
    [switch]$Admin,
    [switch]$Portable,
    [switch]$List
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot

if ($Portable) {
    # Newest packaged portable build from dist\ProtoGrid-*-portable.
    $exe = Get-ChildItem -Path (Join-Path $root 'dist') -Directory -Filter 'ProtoGrid-*-portable' -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        ForEach-Object { Join-Path $_.FullName 'ProtoGrid.exe' } |
        Where-Object { Test-Path -LiteralPath $_ } |
        Select-Object -First 1
    if ($exe) {
        $marker = Join-Path (Split-Path -Parent $exe) 'portable.txt'
        if (-not (Test-Path -LiteralPath $marker)) {
            New-Item -ItemType File -Path $marker | Out-Null
        }
    }
} else {
    $candidates = @(
        (Join-Path $root 'target\x86_64-pc-windows-gnu\debug\protogrid-new.exe'),
        (Join-Path $root 'target\x86_64-pc-windows-gnu\debug\protogrid.exe'),
        (Join-Path $root 'target\x86_64-pc-windows-gnu\release\protogrid.exe'),
        (Join-Path $root 'target\x86_64-pc-windows-gnu\release\protogrid-new.exe'),
        (Join-Path $root 'target\release\protogrid.exe'),
        (Join-Path $root 'dist\ProtoGrid-windows-x64\protogrid.exe')
    )
    $exe = $candidates |
        Where-Object { Test-Path -LiteralPath $_ } |
        Sort-Object { (Get-Item -LiteralPath $_).LastWriteTime } -Descending |
        Select-Object -First 1
}

if (-not $exe) {
    if ($Portable) {
        Write-Host 'No portable build found in dist\ProtoGrid-*-portable.' -ForegroundColor Red
        Write-Host 'Run scripts/package-windows.sh once, or copy the portable zip contents here.'
    } else {
        Write-Host 'No built protogrid.exe found.' -ForegroundColor Red
        Write-Host 'Build once first, then this launcher will reuse the binary:' -ForegroundColor Yellow
        Write-Host '  cargo build -p protogrid-desktop --target x86_64-pc-windows-gnu'
    }
    if (-not $List) { Read-Host 'Press Enter to close' }
    exit 1
}

if ($List) {
    Write-Output $exe
    exit 0
}

$running = Get-Process -Name 'protogrid' -ErrorAction SilentlyContinue
if ($running) {
    $pids = ($running | ForEach-Object { $_.Id }) -join ', '
    Write-Host "ProtoGrid is already running (PID: $pids)." -ForegroundColor Yellow
    Write-Host 'Close the running instance first, then start this launcher again.'
    Read-Host 'Press Enter to close'
    exit 0
}

Write-Host "Starting: $exe" -ForegroundColor Cyan
if ($Admin) {
    Start-Process -FilePath $exe -Verb RunAs
} else {
    Start-Process -FilePath $exe
}
