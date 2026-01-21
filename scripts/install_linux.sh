#!/bin/bash
# sscontrol Linux 安装脚本
#
# 用法:
#   sudo ./scripts/install_linux.sh          # 安装服务
#   sudo ./scripts/install_linux.sh remove   # 卸载服务

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY_PATH="$PROJECT_DIR/target/release/sscontrol"
INSTALL_PATH="/usr/local/bin/sscontrol"
CONFIG_PATH="/etc/sscontrol/config.toml"

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

# 检查是否为 root 用户
check_root() {
    if [ "$EUID" -ne 0 ]; then
        error "此脚本需要 root 权限，请使用 sudo 运行"
        exit 1
    fi
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

    check_root
    check_binary

    # 复制二进制文件到系统目录
    info "安装二进制文件到 $INSTALL_PATH"
    cp "$BINARY_PATH" "$INSTALL_PATH"
    chmod +x "$INSTALL_PATH"

    # 创建配置目录
    mkdir -p /etc/sscontrol

    # 复制配置文件示例（如果不存在）
    if [ ! -f "$CONFIG_PATH" ]; then
        if [ -f "$PROJECT_DIR/config.toml.example" ]; then
            cp "$PROJECT_DIR/config.toml.example" "$CONFIG_PATH"
            info "已创建配置文件: $CONFIG_PATH"
            warn "请编辑配置文件以设置服务器地址和其他参数"
        fi
    fi

    # 运行安装命令
    "$INSTALL_PATH" service install

    info "服务已安装，使用以下命令管理:"
    echo "  启动:  sudo systemctl start sscontrol"
    echo "  停止:  sudo systemctl stop sscontrol"
    echo "  状态:  sudo systemctl status sscontrol"
    echo "  日志:  sudo journalctl -u sscontrol -f"
    echo "  卸载:  sudo $0 remove"
}

# 卸载服务
uninstall_service() {
    info "卸载 sscontrol 服务..."

    check_root

    if [ ! -f "$INSTALL_PATH" ]; then
        error "二进制文件不存在: $INSTALL_PATH"
        exit 1
    fi

    # 运行卸载命令
    "$INSTALL_PATH" service uninstall

    # 删除二进制文件
    rm -f "$INSTALL_PATH"

    info "服务已卸载"
    warn "配置文件保留在 /etc/sscontrol"
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
            echo "用法: sudo $0 [install|uninstall]"
            exit 1
            ;;
    esac
}

main "$@"
