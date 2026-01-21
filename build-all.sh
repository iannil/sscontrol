#!/bin/bash
# sscontrol 多平台编译脚本
# 在 macOS 上编译 Mac、Windows、Linux 三个平台的二进制文件

set -e

# 颜色
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
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

# Features (h264 需要 FFmpeg，交叉编译复杂，暂时跳过)
FEATURES="webrtc,security,service"
RELEASE_FLAG="--release"

echo ""
info "================================================"
info "  sscontrol 多平台编译脚本"
info "================================================"
echo ""

# 检查工具
check_tools() {
    info "检查编译工具..."

    if ! command -v cargo &> /dev/null; then
        error "未找到 cargo，请先安装 Rust"
        exit 1
    fi

    # 检查是否安装了 cross（用于 Linux 交叉编译）
    if command -v cross &> /dev/null; then
        HAS_CROSS=true
        info "已安装 cross 工具"
    else
        HAS_CROSS=false
        warn "未安装 cross 工具，Linux 平台将使用 cargo（可能需要额外配置 linker）"
        warn "安装 cross: cargo install cross --git https://github.com/cross-rs/cross"
    fi

    # 检查 Docker（cross 需要）
    if ! "$HAS_CROSS" || ! docker info &> /dev/null; then
        if ! "$HAS_CROSS"; then
            warn "未安装 Docker 或 Docker 未运行"
        else
            warn "Docker 未运行，Linux 编译可能失败"
        fi
    fi

    echo ""
}

# 添加编译 target
add_targets() {
    info "添加交叉编译 targets..."

    rustup target add x86_64-apple-darwin 2>/dev/null || true
    rustup target add aarch64-apple-darwin 2>/dev/null || true
    rustup target add x86_64-pc-windows-gnu 2>/dev/null || true
    rustup target add x86_64-unknown-linux-gnu 2>/dev/null || true

    info "Targets 已添加"
    echo ""
}

# 编译 macOS (Intel)
build_macos_x64() {
    info "编译 macOS x86_64..."
    cargo build $RELEASE_FLAG --target x86_64-apple-darwin --features "$FEATURES"

    local out_dir="$DIST_DIR/macos-x86_64"
    mkdir -p "$out_dir"
    cp "target/x86_64-apple-darwin/release/sscontrol" "$out_dir/"
    info "  -> $out_dir/sscontrol"
}

# 编译 macOS (Apple Silicon)
build_macos_arm64() {
    info "编译 macOS aarch64 (Apple Silicon)..."
    cargo build $RELEASE_FLAG --target aarch64-apple-darwin --features "$FEATURES"

    local out_dir="$DIST_DIR/macos-aarch64"
    mkdir -p "$out_dir"
    cp "target/aarch64-apple-darwin/release/sscontrol" "$out_dir/"
    info "  -> $out_dir/sscontrol"
}

# 编译 macOS Universal Binary
build_macos_universal() {
    info "创建 macOS Universal Binary..."
    local x64_bin="target/x86_64-apple-darwin/release/sscontrol"
    local arm_bin="target/aarch64-apple-darwin/release/sscontrol"
    local universal_dir="$DIST_DIR/macos-universal"

    mkdir -p "$universal_dir"

    if [ -f "$x64_bin" ] && [ -f "$arm_bin" ]; then
        lipo -create -output "$universal_dir/sscontrol" "$x64_bin" "$arm_bin"
        info "  -> $universal_dir/sscontrol"
    else
        warn "无法创建 Universal Binary（缺少架构文件）"
    fi
}

# 编译 Windows
build_windows() {
    info "编译 Windows x86_64..."
    cargo build $RELEASE_FLAG --target x86_64-pc-windows-gnu --features "$FEATURES"

    local out_dir="$DIST_DIR/windows-x86_64"
    mkdir -p "$out_dir"
    cp "target/x86_64-pc-windows-gnu/release/sscontrol.exe" "$out_dir/" 2>/dev/null || \
    cp "target/x86_64-pc-windows-gnu/release/sscontrol" "$out_dir/sscontrol.exe"
    info "  -> $out_dir/sscontrol.exe"
}

# 编译 Linux
build_linux() {
    info "编译 Linux x86_64..."

    if [ "$HAS_CROSS" = true ] && docker info &> /dev/null; then
        # 使用 cross 工具编译
        cross build $RELEASE_FLAG --target x86_64-unknown-linux-gnu --features "$FEATURES"
    else
        # 直接使用 cargo（可能需要配置 linker）
        warn "使用 cargo 直接编译 Linux，如果失败请安装 cross 工具"
        cargo build $RELEASE_FLAG --target x86_64-unknown-linux-gnu --features "$FEATURES"
    fi

    local out_dir="$DIST_DIR/linux-x86_64"
    mkdir -p "$out_dir"
    cp "target/x86_64-unknown-linux-gnu/release/sscontrol" "$out_dir/" 2>/dev/null || true
    info "  -> $out_dir/sscontrol"
}

# 打包
package() {
    echo ""
    info "打包发布文件..."

    cd "$DIST_DIR"

    # macOS x64
    if [ -f "macos-x86_64/sscontrol" ]; then
        tar czf "sscontrol-macos-x86_64.tar.gz" -C macos-x86_64 sscontrol
        info "  -> sscontrol-macos-x86_64.tar.gz"
    fi

    # macOS ARM64
    if [ -f "macos-aarch64/sscontrol" ]; then
        tar czf "sscontrol-macos-aarch64.tar.gz" -C macos-aarch64 sscontrol
        info "  -> sscontrol-macos-aarch64.tar.gz"
    fi

    # macOS Universal
    if [ -f "macos-universal/sscontrol" ]; then
        tar czf "sscontrol-macos-universal.tar.gz" -C macos-universal sscontrol
        info "  -> sscontrol-macos-universal.tar.gz"
    fi

    # Windows
    if [ -f "windows-x86_64/sscontrol.exe" ]; then
        zip -q "sscontrol-windows-x86_64.zip" windows-x86_64/sscontrol.exe
        info "  -> sscontrol-windows-x86_64.zip"
    fi

    # Linux
    if [ -f "linux-x86_64/sscontrol" ]; then
        tar czf "sscontrol-linux-x86_64.tar.gz" -C linux-x86_64 sscontrol
        info "  -> sscontrol-linux-x86_64.tar.gz"
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
    info "输出目录: $DIST_DIR"
    echo ""
    info "编译产物:"
    ls -la "$DIST_DIR"/*.tar.gz "$DIST_DIR"/*.zip 2>/dev/null || echo "  (无打包文件)"
    echo ""

    # 显示文件大小
    info "二进制文件大小:"
    for dir in "$DIST_DIR"/*/; do
        if [ -d "$dir" ]; then
            for bin in "$dir"sscontrol "$dir"sscontrol.exe; do
                if [ -f "$bin" ]; then
                    local size=$(ls -lh "$bin" | awk '{print $5}')
                    local name=$(basename "$(dirname "$bin")")
                    info "  $name: $size"
                fi
            done
        fi
    done
    echo ""
}

# 主流程
check_tools
add_targets

build_macos_x64
build_macos_arm64
build_macos_universal
build_windows
build_linux

package
summary
