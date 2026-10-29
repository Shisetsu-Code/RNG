@echo off
setlocal EnableExtensions
title Diagnostico de RNG 6x6
chcp 65001 >nul
cd /d "%~dp0"
set "LOG=%TEMP%\RNG-6x6-diagnostico.txt"
echo Creando diagnostico en: %LOG%
echo.
(
  echo ===== Diagnostico RNG 6x6 Windows =====
  echo Carpeta: %CD%
  echo.
  echo --- Archivos ---
  dir /b "%~dp0RNG-6x6*" 2^>nul
  echo.
  echo --- PowerShell ---
  where powershell.exe
  echo.
  echo --- Verificar GUI ---
  if exist "%~dp0RNG-6x6-GUI.ps1" (
    powershell.exe -NoProfile -ExecutionPolicy Bypass -STA -File "%~dp0RNG-6x6-GUI.ps1" -SelfTest
    powershell.exe -NoProfile -ExecutionPolicy Bypass -STA -File "%~dp0RNG-6x6-GUI.ps1" -TestForm
  ) else (
    echo ERROR: No esta RNG-6x6-GUI.ps1
  )
  echo.
  echo --- Prueba RNG con 2 paquetes de 100 ---
  if exist "%~dp0RNG-6x6-CLI.exe" (
    "%~dp0RNG-6x6-CLI.exe" --packages 2 --out "%TEMP%\RNG-6x6-smoke" --compact
  ) else (
    echo ERROR: No esta RNG-6x6-CLI.exe
  )
  echo.
  echo --- Fin del diagnostico ---
) > "%LOG%" 2>&1
type "%LOG%"
echo.
echo El resultado se guardo en:
echo %LOG%
echo.
pause
