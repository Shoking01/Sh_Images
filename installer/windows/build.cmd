@echo off
REM ===========================================================================
REM  build.cmd — compila el MSI de Sh_Images con WiX 3.14
REM
REM  Requisitos:
REM    - WiX 3.14 en PATH (choco install wix --version=3.14.0)
REM    - cargo build --release previamente ejecutado
REM    - icon.ico en este directorio (generado por build.rs o copiado por CI)
REM
REM  Variables de entorno (opcionales):
REM    VERSION       — número de versión del MSI (default: 0.1.0)
REM    RELEASE_DIR   — ruta a los binarios release (default: ..\..\target\release\)
REM ===========================================================================
setlocal EnableDelayedExpansion

REM --- Locate WiX ---
set "WIXDIR=%WIX%"
if "%WIXDIR%"=="" set "WIXDIR=C:\Program Files (x86)\WiX Toolset v3.14\bin"
if not exist "%WIXDIR%\candle.exe" (
    set "WIXDIR=C:\Program Files\WiX Toolset v3.14\bin"
)
if not exist "%WIXDIR%\candle.exe" (
    echo ERROR: WiX no encontrado. Instala WiX 3.14 ^(choco install wix --version=3.14.0^)
    exit /b 1
)

REM --- Version ---
set "VERSION=%VERSION%"
if "%VERSION%"=="" set "VERSION=0.1.0"

REM --- Release dir ---
set "RELEASE_DIR=%RELEASE_DIR%"
if "%RELEASE_DIR%"=="" set "RELEASE_DIR=..\..\target\release"

REM --- Validate inputs ---
if not exist "%RELEASE_DIR%\sh_images.exe" (
    echo ERROR: sh_images.exe no encontrado en %RELEASE_DIR%
    echo Ejecuta 'cargo build --release' primero.
    exit /b 1
)
if not exist "icon.ico" (
    echo ERROR: icon.ico no encontrado en installer\windows\
    exit /b 1
)

echo Building Sh_Images MSI v%VERSION%
echo   Release dir: %RELEASE_DIR%
echo   WiX dir:     %WIXDIR%

REM --- Compile .wxs to .wixobj ---
"%WIXDIR%\candle.exe" ^
    -arch x64 ^
    -dVersion=%VERSION% ^
    -dReleaseDir="%RELEASE_DIR%\" ^
    -ext WixUtilExtension ^
    -out Product.wixobj Product.wxs

if %ERRORLEVEL% NEQ 0 (
    echo ERROR: candle fallo en Product.wxs
    exit /b %ERRORLEVEL%
)

"%WIXDIR%\candle.exe" ^
    -arch x64 ^
    -dVersion=%VERSION% ^
    -dReleaseDir="%RELEASE_DIR%\" ^
    -out Files.wixobj Files.wxs

if %ERRORLEVEL% NEQ 0 (
    echo ERROR: candle fallo en Files.wxs
    exit /b %ERRORLEVEL%
)

"%WIXDIR%\candle.exe" ^
    -arch x64 ^
    -out Associations.wixobj Associations.wxs

if %ERRORLEVEL% NEQ 0 (
    echo ERROR: candle fallo en Associations.wxs
    exit /b %ERRORLEVEL%
)

"%WIXDIR%\candle.exe" ^
    -arch x64 ^
    -out Shortcuts.wixobj Shortcuts.wxs

if %ERRORLEVEL% NEQ 0 (
    echo ERROR: candle fallo en Shortcuts.wxs
    exit /b %ERRORLEVEL%
)

REM --- Link to MSI ---
set "MSI_NAME=sh_images-%VERSION%-x64.msi"
"%WIXDIR%\light.exe" ^
    -out %MSI_NAME% ^
    Product.wixobj Files.wixobj Associations.wixobj Shortcuts.wixobj ^
    -ext WixUIExtension ^
    -b . ^
    -O1

if %ERRORLEVEL% NEQ 0 (
    echo ERROR: light fallo al generar %MSI_NAME%
    exit /b %ERRORLEVEL%
)

echo.
echo SUCCESS: %MSI_NAME% generado en %CD%
endlocal
