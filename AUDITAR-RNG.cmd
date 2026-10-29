@echo off
setlocal EnableExtensions EnableDelayedExpansion
title RNG 6x6 - Auditoria de seguridad
cd /d "%~dp0"
echo ----------------------------------------------
echo   LABORATORIO RNG: GENERADOR vs ATACANTE
echo ----------------------------------------------
echo.
if not exist "%~dp0RNG-6x6-AUDITOR.exe" (
  echo ERROR: No esta RNG-6x6-AUDITOR.exe. Descomprime el ZIP completo.
  pause
  exit /b 1
)
set /p PAQUETES="Cantidad de paquetes por modelo [10000]: "
if "%PAQUETES%"=="" set PAQUETES=10000
set "MODE=all"
echo.
echo Modos: 1=Todos, 2=Historico sin reposicion, 3=Actual independiente
set /p MODE_CHOICE="Elegir [1]: "
if "%MODE_CHOICE%"=="2" set "MODE=without"
if "%MODE_CHOICE%"=="3" set "MODE=independent"
set "GPUFLAG="
if not "%MODE%"=="independent" (
  set /p GPU_CHOICE="Utilizar GPU AMD/NVIDIA si esta disponible? (S/N) [N]: "
  if /I "!GPU_CHOICE!"=="S" set "GPUFLAG=--gpu"
)
echo.
echo Ejecutando auditoria...
if exist "%~dp0salida_rng\catalogo_8192.json" (
  echo Usando el catalogo ACTUAL del generador de paquetes.
  "%~dp0RNG-6x6-AUDITOR.exe" --packages %PAQUETES% --mode %MODE% --catalog "%~dp0salida_rng\catalogo_8192.json" --out "%~dp0auditoria_rng" %GPUFLAG%
) else (
  echo No hay catalogo previo; se creara un catalogo de prueba.
  "%~dp0RNG-6x6-AUDITOR.exe" --packages %PAQUETES% --mode %MODE% --out "%~dp0auditoria_rng" %GPUFLAG%
)
if errorlevel 1 (
  echo.
  echo La auditoria fallo. Si fue la GPU, repite el test en modo CPU.
) else (
  echo.
  echo Informe generado en: %~dp0auditoria_rng
)
echo.
pause
