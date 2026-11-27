@echo off
setlocal
chcp 65001 >nul
title ProtoGrid phone log
if "%~1"=="" (
  echo Usage: run-phone-log.bat [IP:PORT] [lite^|standard]
  echo Example: run-phone-log.bat 192.168.1.42:41437
  echo Without arguments the first device found by adb is used.
  echo.
)
powershell -NoProfile -NoLogo -ExecutionPolicy Bypass -File "%~dp0scripts\phone-log.ps1" %*
set "RC=%ERRORLEVEL%"
echo.
pause
exit /b %RC%
