@echo off
chcp 65001 >nul
cd /d "%~dp0"
title Generátor švédských křížovek (cwgen)

echo =======================================================
echo          GENERÁTOR ŠVÉDSKÝCH KŘÍŽOVEK (cwgen)
echo =======================================================
echo.

set /p ROZMER="Zadejte rozměr křížovky (např. 10x10, 15x15, 20x20, max 30x30) [výchozí: 12x12]: "
if "%ROZMER%"=="" set ROZMER=12x12

echo.
echo Vyberte preferovanou obtížnost definic:
echo   [1] Snadná (Easy)
echo   [2] Střední (Medium) [výchozí]
echo   [3] Těžká (Hard)
echo   [4] Libovolná / Mix
set /p OBT="Zvolte obtížnost [1-4, výchozí: 2]: "
if "%OBT%"=="" set OBT=2

echo.
set /p TAJENKA="Zadejte tajenku (volitelné, Enter pro bez tajenky): "

set CMD_ARGS=--size %ROZMER% -D %OBT%
if not "%TAJENKA%"=="" set CMD_ARGS=%CMD_ARGS% -s "%TAJENKA%"

echo.
echo Generuji křížovku... Čekejte prosím...
echo.

if exist "target\release\cwgen.exe" (
    "target\release\cwgen.exe" %CMD_ARGS%
) else if exist "cwgen.exe" (
    "cwgen.exe" %CMD_ARGS%
) else (
    cargo run --release --bin cwgen -- %CMD_ARGS%
)

if %ERRORLEVEL% equ 0 (
    echo.
    echo =======================================================
    echo Křížovka byla úspěšně vygenerována do složky "output\"!
    echo Otevírám vygenerovanou HTML křížovku v prohlížeči...
    echo =======================================================
    for /f "delims=" %%F in ('dir /b /o-d "%~dp0output\*.html" 2^>nul') do (
        start "" "%~dp0output\%%F"
        goto :done
    )
) else (
    echo.
    echo Generování skončilo s chybou.
)

:done
echo.
pause
