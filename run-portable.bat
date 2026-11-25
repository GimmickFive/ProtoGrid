@echo off
rem Launches the newest portable build from dist\ProtoGrid-*-portable (no build).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\run-protogrid.ps1" -Portable %*
