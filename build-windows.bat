@echo off
REM sscontrol Windows 编译脚本
REM 在 Windows 上编译完整版本

setlocal enabledelayedexpansion

echo.
echo ================================================
echo   sscontrol Windows 编译脚本
echo ================================================
echo.

REM 检查 cargo
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] 未找到 cargo，请先安装 Rust
    echo         https://rustup.rs/
    exit /b 1
)
echo [INFO] Rust 已安装

REM 检查 FFmpeg (通过 pkg-config 或 vcpkg)
where pkg-config >nul 2>nul
if %errorlevel% equ 0 (
    pkg-config --exists libavcodec libavformat libavutil libswscale >nul 2>nul
    if %errorlevel% equ 0 (
        echo [INFO] 已安装 FFmpeg 开发库 (pkg-config)
        goto :build
    )
)

REM 检查 FFMPEG_DIR 环境变量
if defined FFMPEG_DIR (
    echo [INFO] 使用 FFMPEG_DIR: %FFMPEG_DIR%
    goto :build
)

echo [WARN] 未检测到 FFmpeg 开发库
echo [WARN] 请安装 FFmpeg 开发库:
echo         方法 1: 使用 vcpkg
echo                 vcpkg install ffmpeg:x64-windows
echo                 set FFMPEG_DIR=C:\vcpkg\installed\x64-windows
echo.
echo         方法 2: 手动下载 FFmpeg 开发包
echo                 https://github.com/BtbN/FFmpeg-Builds/releases
echo                 解压后设置 FFMPEG_DIR 环境变量
echo.
echo [INFO] 尝试继续编译 (可能会失败)...
echo.

:build
echo.
echo [INFO] 编译 Windows x86_64 (完整版本)...

set FEATURES=h264,webrtc,security,service
cargo build --release --features "%FEATURES%"

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] 编译失败！
    echo [ERROR] 如果是 FFmpeg 相关错误，请确保:
    echo         1. 已安装 FFmpeg 开发库
    echo         2. 设置了 FFMPEG_DIR 环境变量
    echo         3. FFmpeg 库路径在 PATH 中
    exit /b 1
)

REM 创建输出目录
set DIST_DIR=%~dp0dist\windows-x86_64
if not exist "%DIST_DIR%" mkdir "%DIST_DIR%"

REM 复制文件
copy /y "target\release\sscontrol.exe" "%DIST_DIR%\" >nul
echo [INFO]   -^> %DIST_DIR%\sscontrol.exe

echo.
echo ================================================
echo   编译完成！
echo ================================================
echo.
echo [INFO] 二进制文件: %DIST_DIR%\sscontrol.exe
echo.
echo [INFO] 使用说明:
echo.
echo   被控端 (启动屏幕共享服务):
echo      sscontrol.exe host [--port 9527]
echo.
echo   控制端 (连接并查看远程桌面):
echo      sscontrol.exe connect --ip ^<被控端IP^> [--port 9527]
echo.

endlocal
