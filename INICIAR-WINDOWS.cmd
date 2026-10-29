@echo off
setlocal EnableExtensions
title RNG 6x6 - Inicio Windows
chcp 65001 >nul
cd /d "%~dp0"

if not exist "%~dp0RNG-6x6-GUI.ps1" (
  echo ERROR: No esta RNG-6x6-GUI.ps1.
  echo Extrae TODOS los archivos del ZIP en una misma carpeta.
  goto error
)
if not exist "%~dp0RNG-6x6-CLI.exe" (
  echo ERROR: No esta RNG-6x6-CLI.exe.
  echo Extrae TODOS los archivos del ZIP en una misma carpeta.
  goto error
)

if "%~1"=="" (
  echo Abriendo la interfaz grafica del RNG...
  powershell.exe -NoProfile -ExecutionPolicy Bypass -STA -File "%~dp0RNG-6x6-GUI.ps1"
  if errorlevel 1 goto error
  exit /b 0
)

echo Ejecutando RNG con parametros: %*
"%~dp0RNG-6x6-CLI.exe" %*
if errorlevel 1 goto error
echo.
echo Ejecucion completada.
pause
exit /b 0

:error
echo.
echo El RNG no pudo iniciarse correctamente.
echo Ejecuta DIAGNOSTICO-WINDOWS.cmd para identificar el problema.
pause
exit /b 1
