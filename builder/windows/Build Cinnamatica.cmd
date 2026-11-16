@echo off
powershell.exe -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0Build-Cinnamatica.ps1"
pause
