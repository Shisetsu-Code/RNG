@echo off
chcp 65001 >nul
cd /d "%~dp0"
"RNG-6x6.exe" %*
