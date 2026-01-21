#!/bin/bash
# sscontrol 一键启动脚本
# 用法: ./start.sh [模式]
#
# 模式:
#   local   - 单机模式（直接查看本地屏幕，无需服务器）
#   all     - 完整模式（启动信令服务器 + 被控端）
#   server  - 仅启动信令服务器
#   host    - 仅启动被控端（需要先启动服务器）
#   client  - 启动控制端

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

# 获取模式
MODE="${1:-local}"

# 1. 检查构建
build_if_needed() {
    if [ ! -f "target/release/sscontrol" ]; then
        info "首次运行，正在构建..."
        cargo build --release --features "h264,webrtc,security,service"
    fi
}

# 2. 单机模式：直接显示本地屏幕
mode_local() {
    info "=== 单机模式 ==="
    info "直接查看本地屏幕（无需服务器）"

    # 检查 ffplay
    if ! command -v ffplay &> /dev/null; then
        error "需要安装 ffmpeg（包含 ffplay）"
        info "macOS: brew install ffmpeg"
        info "Linux: sudo apt install ffmpeg"
        exit 1
    fi

    build_if_needed

    info "按 Ctrl+C 退出"
    info "使用 ffplay 查看屏幕..."

    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS - 设备3是屏幕捕获（不是摄像头）
        ffplay -f avfoundation -framerate 30 -pixel_format uyvy422 -i "3:0" \
            -vf "scale=1280:-1" -fflags nobuffer -flags low_delay \
            -window_title "SSControl Local" -x 1280 -y 800
    else
        # Linux
        ffplay -f x11grab -framerate 30 -i :0.0 \
            -vf "scale=1280:-1" -fflags nobuffer -flags low_delay \
            -window_title "SSControl Local" -x 1280 -y 800 \
            -v quiet 2>/dev/null
    fi
}

# 3. 完整模式：启动服务器 + 被控端
mode_all() {
    info "=== 完整模式 ==="
    info "同时启动信令服务器和被控端"

    build_if_needed
    mkdir -p ~/.config/sscontrol

    # 生成临时 API key
    API_KEY=$(openssl rand -hex 16 2>/dev/null || echo "test-key-12345")
    export SSCONTROL_API_KEY="$API_KEY"

    info "API Key: $API_KEY"

    # 启动信令服务器（后台）
    info "启动信令服务器..."
    cargo run --example signaling_server --features security &
    SERVER_PID=$!
    sleep 2

    # 启动被控端
    info "启动被控端..."
    ./target/release/sscontrol \
        --server "ws://localhost:8080" \
        --fps 30 \
        -v

    # 清理
    kill $SERVER_PID 2>/dev/null || true
}

# 4. 仅服务器
mode_server() {
    info "=== 信令服务器模式 ==="
    info "仅启动信令服务器"

    export RUST_LOG=info
    cargo run --example signaling_server --features security
}

# 5. 仅被控端
mode_host() {
    info "=== 被控端模式 ==="
    info "连接到 ws://localhost:8080"

    build_if_needed
    mkdir -p ~/.config/sscontrol

    ./target/release/sscontrol \
        --server "ws://localhost:8080" \
        --fps 30 \
        -v
}

# 6. 控制端
mode_client() {
    info "=== 控制端模式 ==="
    info "连接到 ws://localhost:8080"

    build_if_needed

    cargo run --example webrtc_client --features webrtc \
        -- --server "ws://localhost:8080"
}

# 帮助
show_help() {
    cat << EOF
用法: ./start.sh [模式]

模式:
  local   - 单机模式（直接查看本地屏幕，推荐）
  all     - 完整模式（服务器 + 被控端）
  server  - 仅信令服务器
  host    - 仅被控端
  client  - 控制端

示例:
  ./start.sh local     # 单机查看屏幕
  ./start.sh all       # 启动完整系统
  ./start.sh           # 默认：local 模式
EOF
}

# 主逻辑
case "$MODE" in
    local)
        mode_local
        ;;
    all)
        mode_all
        ;;
    server)
        mode_server
        ;;
    host)
        mode_host
        ;;
    client)
        mode_client
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        error "未知模式: $MODE"
        show_help
        exit 1
        ;;
esac
