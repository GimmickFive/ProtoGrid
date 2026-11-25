@echo off
rem Portable build, elevated (needed for TUN mode).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\run-protogrid.ps1" -Portable -Admin
