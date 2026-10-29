@echo off
setlocal EnableExtensions EnableDelayedExpansion
title Diagnostico RNG 6x6 (Rust nativo)
cd /d "%~dp0"
set "LOG=%TEMP%\RNG-6x6-diagnostico.txt"
echo Ejecutando diagnostico del motor y la interfaz...
echo.
(
 echo ===== Diagnostico RNG 6x6 =====
 echo Carpeta: %CD%
 echo.
 echo --- Archivos del paquete ---
 dir /b "%~dp0RNG-6x6*.exe" 2^>nul
 echo.
 echo --- Prueba interfaz Rust nativa y 3 paquetes ---
 if exist "%~dp0RNG-6x6.exe" (
   "%~dp0RNG-6x6.exe" --self-test
   echo Resultado de prueba interfaz: !errorlevel!
 ) else (
   echo ERROR: Falta RNG-6x6.exe
 )
 if exist "%TEMP%\rng-gui-self-test-error.txt" (
   type "%TEMP%\rng-gui-self-test-error.txt"
 )
 echo.
 echo --- Prueba directa motor Rust: 2 paquetes ---
 if exist "%~dp0RNG-6x6-CLI.exe" (
   "%~dp0RNG-6x6-CLI.exe" --packages 2 --out "%TEMP%\RNG-6x6-smoke" --compact
   echo Resultado del motor: !errorlevel!
 ) else (
   echo ERROR: Falta RNG-6x6-CLI.exe
 )
 echo.
 echo --- Fin del diagnostico ---
) > "%LOG%" 2>&1
type "%LOG%"
echo.
echo Informe: %LOG%
pause
