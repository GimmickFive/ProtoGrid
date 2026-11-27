@echo off
rem Removes stale Wintun tunnel adapters left by ProtoGrid/sing-box after a
rem crash or a too-fast reconnect. Fixes the TUN start error:
rem   configure tun interface: (create adapter: Cannot create a file when that
rem   file already exists. | open existing adapter: Element not found.)
rem Only Wintun devices are touched; TAP/OpenVPN/WireGuard are left alone.
rem Double-click to run - administrator rights are requested automatically.
rem Options: -List (show only), -Force (also attached adapters).
setlocal
set "SCRIPT=%~dp0scripts\reset-tun.ps1"
if not exist "%SCRIPT%" set "SCRIPT=%~dp0reset-tun.ps1"
if not exist "%SCRIPT%" (
    echo reset-tun.ps1 not found next to this file.
    pause
    exit /b 1
)
set "WAIT="
if "%~1"=="" set "WAIT=-Pause"
powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%" %WAIT% %*
exit /b %ERRORLEVEL%
