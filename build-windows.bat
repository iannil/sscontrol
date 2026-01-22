@echo off
REM sscontrol Windows Build Script
REM Build full version on Windows

setlocal enabledelayedexpansion

echo.
echo ================================================
echo   sscontrol Windows Build Script
echo ================================================
echo.

REM Check cargo
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] cargo not found, please install Rust first
    echo         https://rustup.rs/
    exit /b 1
)
echo [INFO] Rust is installed

REM Check FFmpeg (via pkg-config or vcpkg)
where pkg-config >nul 2>nul
if %errorlevel% equ 0 (
    pkg-config --exists libavcodec libavformat libavutil libswscale >nul 2>nul
    if %errorlevel% equ 0 (
        echo [INFO] FFmpeg dev libraries found (pkg-config)
        goto :build
    )
)

REM Check FFMPEG_DIR environment variable
if defined FFMPEG_DIR (
    echo [INFO] Using FFMPEG_DIR: %FFMPEG_DIR%
    goto :build
)

echo [WARN] FFmpeg dev libraries not detected
echo [WARN] Please install FFmpeg dev libraries:
echo         Method 1: Using vcpkg
echo                   vcpkg install ffmpeg:x64-windows
echo                   set FFMPEG_DIR=C:\vcpkg\installed\x64-windows
echo.
echo         Method 2: Manual download
echo                   https://github.com/BtbN/FFmpeg-Builds/releases
echo                   Extract and set FFMPEG_DIR environment variable
echo.
echo [INFO] Attempting to build anyway (may fail)...
echo.

:build
echo.
echo [INFO] Building Windows x86_64 (full version)...

set FEATURES=h264,webrtc,security,service
cargo build --release --features "%FEATURES%"

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] Build failed!
    echo [ERROR] If FFmpeg related error, please ensure:
    echo         1. FFmpeg dev libraries are installed
    echo         2. FFMPEG_DIR environment variable is set
    echo         3. FFmpeg library path is in PATH
    exit /b 1
)

REM Create output directory
set DIST_DIR=%~dp0dist\windows-x86_64
if not exist "%DIST_DIR%" mkdir "%DIST_DIR%"

REM Copy files
copy /y "target\release\sscontrol.exe" "%DIST_DIR%\" >nul
echo [INFO]   -^> %DIST_DIR%\sscontrol.exe

echo.
echo ================================================
echo   Build Complete!
echo ================================================
echo.
echo [INFO] Binary: %DIST_DIR%\sscontrol.exe
echo.
echo [INFO] Usage:
echo.
echo   Host (start screen sharing):
echo      sscontrol.exe host [--port 9527]
echo.
echo   Viewer (connect to remote desktop):
echo      sscontrol.exe connect --ip ^<HOST_IP^> [--port 9527]
echo.

endlocal
