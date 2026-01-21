#!/bin/bash
# sscontrol macOS 安装脚本
#
# 用法:
#   ./scripts/install_macos.sh          # 安装服务
#   ./scripts/install_macos.sh remove   # 卸载服务

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY_PATH="$PROJECT_DIR/target/release/sscontrol"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 打印信息
info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查二进制文件是否存在
check_binary() {
    if [ ! -f "$BINARY_PATH" ]; then
        error "二进制文件不存在: $BINARY_PATH"
        info "请先运行: cargo build --release"
        exit 1
    fi
}

# 安装服务
install_service() {
    info "安装 sscontrol 服务..."

    check_binary

    # 确保二进制文件可执行
    chmod +x "$BINARY_PATH"

    # 运行安装命令
    "$BINARY_PATH" service install

    info "服务已安装，使用以下命令管理:"
    echo "  启动:  $BINARY_PATH service start"
    echo "  停止:  $BINARY_PATH service stop"
    echo "  状态:  $BINARY_PATH service status"
    echo "  卸载:  $0 remove"
}

# 卸载服务
uninstall_service() {
    info "卸载 sscontrol 服务..."

    if [ ! -f "$BINARY_PATH" ]; then
        error "二进制文件不存在: $BINARY_PATH"
        exit 1
    fi

    # 运行卸载命令
    "$BINARY_PATH" service uninstall

    info "服务已卸载"
}

# 主函数
main() {
    case "${1:-install}" in
        install)
            install_service
            ;;
        uninstall|remove)
            uninstall_service
            ;;
        *)
            echo "用法: $0 [install|uninstall]"
            exit 1
            ;;
    esac
}

main "$@"
