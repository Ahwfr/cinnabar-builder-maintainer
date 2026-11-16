@echo off
powershell.exe -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0Start-Cinnamatica.ps1" -ImportOnly
pause
