#!/bin/bash
# sscontrol macOS 编译脚本
# 在 macOS 上编译本机架构的完整版本

set -e

# 颜色
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# 创建输出目录
DIST_DIR="$SCRIPT_DIR/dist"
mkdir -p "$DIST_DIR"

# Features (完整版本，包含视频编码和公网隧道)
FEATURES="h264,webrtc,security,service,tunnel"
RELEASE_FLAG="--release"

# 检测本机架构
NATIVE_ARCH=$(uname -m)
if [ "$NATIVE_ARCH" = "arm64" ]; then
    NATIVE_TARGET="aarch64-apple-darwin"
    ARCH_NAME="macos-aarch64"
else
    NATIVE_TARGET="x86_64-apple-darwin"
    ARCH_NAME="macos-x86_64"
fi

echo ""
info "================================================"
info "  sscontrol macOS 编译脚本"
info "================================================"
info "  架构: $NATIVE_ARCH ($NATIVE_TARGET)"
echo ""

# 检查工具
check_tools() {
    info "检查编译工具..."

    if ! command -v cargo &> /dev/null; then
        error "未找到 cargo，请先安装 Rust"
        exit 1
    fi

    # 检查 FFmpeg
    if pkg-config --exists libavcodec libavformat libavutil libswscale 2>/dev/null; then
        info "已安装 FFmpeg 开发库"
    else
        error "未检测到 FFmpeg 开发库，视频编码需要 FFmpeg"
        error "请先安装: brew install ffmpeg"
        exit 1
    fi

    echo ""
}

# 编译
build() {
    info "编译 macOS $NATIVE_ARCH (完整版本)..."
    cargo build $RELEASE_FLAG --target $NATIVE_TARGET --features "$FEATURES"

    local out_dir="$DIST_DIR/$ARCH_NAME"
    mkdir -p "$out_dir"
    cp "target/$NATIVE_TARGET/release/sscontrol" "$out_dir/"
    info "  -> $out_dir/sscontrol"
}

# 打包
package() {
    echo ""
    info "打包发布文件..."

    cd "$DIST_DIR"

    if [ -d "$ARCH_NAME" ]; then
        tar czf "sscontrol-$ARCH_NAME.tar.gz" -C "$ARCH_NAME" .
        info "  -> sscontrol-$ARCH_NAME.tar.gz"
    fi

    cd "$SCRIPT_DIR"
}

# 显示摘要
summary() {
    echo ""
    info "================================================"
    info "  编译完成！"
    info "================================================"
    echo ""

    # 显示文件大小
    local bin="$DIST_DIR/$ARCH_NAME/sscontrol"
    if [ -f "$bin" ]; then
        local size=$(ls -lh "$bin" | awk '{print $5}')
        info "二进制文件: $bin ($size)"
    fi
    echo ""

    # 使用说明
    info "使用说明:"
    info ""
    info "  被控端 (启动屏幕共享服务):"
    info "     ./sscontrol host [--port 9527]"
    info ""
    info "  被控端 (启用公网隧道):"
    info "     ./sscontrol host --tunnel"
    info ""
    info "  控制端 - 局域网连接:"
    info "     ./sscontrol connect --ip <被控端IP> [--port 9527]"
    info ""
    info "  控制端 - 公网连接:"
    info "     ./sscontrol connect --url wss://xxx.trycloudflare.com"
    echo ""
}

# 主流程
check_tools
build
package
summary
