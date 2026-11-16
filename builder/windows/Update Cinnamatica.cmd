@echo off
powershell.exe -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0Update-Cinnamatica.ps1" %*
pause
