# sscontrol Windows 安装脚本
#
# 用法:
#   .\scripts\install_windows.ps1          # 安装服务
#   .\scripts\install_windows.ps1 remove   # 卸载服务

param(
    [Parameter(Position=0)]
    [ValidateSet("install", "uninstall", "remove")]
    [string]$Action = "install"
)

$ErrorActionPreference = "Stop"

# 获取脚本目录
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir
$BinaryPath = Join-Path $ProjectDir "target\release\sscontrol.exe"

# 颜色输出函数
function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Green
}

function Write-Warn {
    param([string]$Message)
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

# 检查是否为管理员
function Test-Administrator {
    $currentUser = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($currentUser)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# 检查二进制文件是否存在
function Test-Binary {
    if (-not (Test-Path $BinaryPath)) {
        Write-Error "二进制文件不存在: $BinaryPath"
        Write-Info "请先运行: cargo build --release"
        exit 1
    }
}

# 安装服务
function Install-Service {
    Write-Info "安装 sscontrol 服务..."

    if (-not (Test-Administrator)) {
        Write-Error "此脚本需要管理员权限"
        Write-Info "请以管理员身份运行 PowerShell 或使用: Start-Process powershell -Verb runAs -File `"$PSCommandPath`""
        exit 1
    }

    Test-Binary

    # 运行安装命令
    & $BinaryPath service install

    Write-Info "服务已安装，使用以下命令管理:"
    Write-Host "  启动:  $BinaryPath service start" -ForegroundColor Cyan
    Write-Host "  停止:  $BinaryPath service stop" -ForegroundColor Cyan
    Write-Host "  状态:  $BinaryPath service status" -ForegroundColor Cyan
    Write-Host "  卸载:  .\scripts\install_windows.ps1 remove" -ForegroundColor Cyan
}

# 卸载服务
function Uninstall-Service {
    Write-Info "卸载 sscontrol 服务..."

    if (-not (Test-Administrator)) {
        Write-Error "此脚本需要管理员权限"
        exit 1
    }

    if (-not (Test-Path $BinaryPath)) {
        Write-Error "二进制文件不存在: $BinaryPath"
        exit 1
    }

    # 运行卸载命令
    & $BinaryPath service uninstall

    Write-Info "服务已卸载"
}

# 主函数
switch ($Action) {
    "install" {
        Install-Service
    }
    "uninstall" {
        Uninstall-Service
    }
    "remove" {
        Uninstall-Service
    }
}
