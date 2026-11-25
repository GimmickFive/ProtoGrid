@echo off
rem Starts the newest built ProtoGrid without compiling anything.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\run-protogrid.ps1" %*
