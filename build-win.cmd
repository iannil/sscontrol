@echo off
setlocal enabledelayedexpansion

set FFMPEG_DIR=E:\Code\vcpkg\installed\x64-windows
set FFMPEG_LIB_DIR=%FFMPEG_DIR%\lib
set FFMPEG_INCLUDE_DIR=%FFMPEG_DIR%\include
set LIBCLANG_PATH=C:\Users\huhet\scoop\apps\llvm\current\bin
set PATH=%FFMPEG_DIR%\bin;%LIBCLANG_PATH%;%PATH%

echo FFMPEG_DIR=%FFMPEG_DIR%
echo LIBCLANG_PATH=%LIBCLANG_PATH%
echo.

cargo build --release --features "h264,webrtc,security,service,tunnel"

if %errorlevel% equ 0 (
    echo.
    echo Build successful!
    set DIST_DIR=dist\windows-x86_64
    if not exist "!DIST_DIR!" mkdir "!DIST_DIR!"
    copy /y "target\release\sscontrol.exe" "!DIST_DIR!\sscontrol.exe"
    echo Output: !DIST_DIR!\sscontrol.exe

    rem Copy FFmpeg DLLs
    copy /y "%FFMPEG_DIR%\bin\*.dll" "!DIST_DIR!\" >nul 2>&1
    echo FFmpeg DLLs copied to !DIST_DIR!
) else (
    echo Build failed with error level %errorlevel%
)

endlocal
