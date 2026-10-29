@echo off
setlocal EnableExtensions
title RNG 6x6 - Generador de paquetes
cd /d "%~dp0"

if not exist "%~dp0RNG-6x6.exe" (
  echo ERROR: Falta RNG-6x6.exe.
  echo Descomprime TODOS los archivos del ZIP en una misma carpeta.
  goto error
)
if not exist "%~dp0RNG-6x6-CLI.exe" (
  echo ERROR: Falta RNG-6x6-CLI.exe.
  echo Descomprime TODOS los archivos del ZIP en una misma carpeta.
  goto error
)

if "%~1"=="" (
  echo Abriendo ventana nativa de RNG 6x6...
  "%~dp0RNG-6x6.exe"
  if errorlevel 1 goto error
  exit /b 0
)

echo Ejecutando motor Rust con parametros: %*
"%~dp0RNG-6x6-CLI.exe" %*
if errorlevel 1 goto error
echo Ejecucion correcta.
pause
exit /b 0

:error
echo.
echo No se pudo iniciar o ejecutar el programa.
echo Ejecuta DIAGNOSTICO-WINDOWS.cmd y comparte el resultado.
pause
exit /b 1
