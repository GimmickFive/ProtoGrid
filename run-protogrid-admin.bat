@echo off
rem Same as run-protogrid.bat, but elevated (required for TUN mode).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\run-protogrid.ps1" -Admin
