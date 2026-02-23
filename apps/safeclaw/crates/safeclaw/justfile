# SafeClaw development commands

# Default: show available commands
default:
    @just --list

# Start SafeClaw with local config
run:
    cargo run -- -c safeclaw.local.hcl gateway

# Start SafeClaw + ngrok tunnel, print Feishu callback URL + ngrok logs
dev:
    #!/usr/bin/env bash
    set -euo pipefail

    if ! command -v ngrok &>/dev/null; then
        echo "❌ ngrok not found. Install: brew install ngrok"
        exit 1
    fi
    if ! ngrok config check &>/dev/null; then
        echo "❌ ngrok not configured. Run:"
        echo "   ngrok config add-authtoken <YOUR_TOKEN>"
        echo "   Get token at: https://dashboard.ngrok.com/get-started/your-authtoken"
        exit 1
    fi

    # Start ngrok in background, tee logs to file + stderr
    ngrok http 18790 --log=stdout --log-format=json 2>&1 | tee /tmp/ngrok.log | \
        grep --line-buffered -o '"msg":"[^"]*"\|"url":"[^"]*"\|"lvl":"[^"]*"' | \
        sed 's/"msg":"//;s/"url":"//;s/"lvl":"//;s/"//g' | \
        while IFS= read -r line; do
            echo "[ngrok] $line"
        done &
    NGROK_PID=$!
    trap "kill $NGROK_PID 2>/dev/null; pkill -f 'ngrok http' 2>/dev/null || true" EXIT

    # Wait for ngrok tunnel
    echo "⏳ Waiting for ngrok tunnel..."
    for i in $(seq 1 30); do
        NGROK_URL=$(curl -s http://127.0.0.1:4040/api/tunnels 2>/dev/null \
            | grep -o '"public_url":"https://[^"]*' | head -1 | cut -d'"' -f4) || true
        if [ -n "${NGROK_URL:-}" ]; then break; fi
        sleep 1
    done

    if [ -z "${NGROK_URL:-}" ]; then
        echo "❌ Failed to get ngrok URL. Check /tmp/ngrok.log"
        exit 1
    fi

    CALLBACK="${NGROK_URL}/api/v1/gateway/webhook/feishu"

    cargo run -- -c safeclaw.local.hcl gateway 2>&1 | while IFS= read -r line; do
        echo "$line"
        if [[ "$line" == *"runtime listening"* ]]; then
            echo ""
            echo "🛡️  SafeClaw + Feishu Dev Mode"
            echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            echo "📡 ngrok URL:      ${NGROK_URL}"
            echo "🔗 Feishu回调地址: ${CALLBACK}"
            echo "📋 ngrok 日志:     tail -f /tmp/ngrok.log"
            echo "🌐 ngrok 控制台:   http://127.0.0.1:4040"
            echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            echo ""
            echo "👉 复制回调地址到飞书开放平台:"
            echo "   https://open.feishu.cn/app/cli_a91c7696d3f89cc0"
            echo "   → 事件与回调 → 事件配置 → 请求地址"
            echo ""
            echo -n "${CALLBACK}" | pbcopy 2>/dev/null && echo "📋 已复制到剪贴板!" || true
            echo ""
        fi
    done

# Start ngrok only (if SafeClaw is already running)
tunnel:
    #!/usr/bin/env bash
    set -euo pipefail

    if ! command -v ngrok &>/dev/null; then
        echo "❌ ngrok not found. Install: brew install ngrok"
        exit 1
    fi
    if ! ngrok config check &>/dev/null; then
        echo "❌ ngrok not configured. Run:"
        echo "   ngrok config add-authtoken <YOUR_TOKEN>"
        echo "   Get token at: https://dashboard.ngrok.com/get-started/your-authtoken"
        exit 1
    fi

    ngrok http 18790 --log=stdout --log-format=json 2>&1 | tee /tmp/ngrok.log | \
        grep --line-buffered -o '"msg":"[^"]*"\|"url":"[^"]*"' | \
        sed 's/"msg":"//;s/"url":"//;s/"//g' | \
        while IFS= read -r line; do echo "[ngrok] $line"; done &
    NGROK_PID=$!
    trap "kill $NGROK_PID 2>/dev/null; pkill -f 'ngrok http' 2>/dev/null || true" EXIT

    echo "⏳ Waiting for ngrok tunnel..."
    for i in $(seq 1 30); do
        NGROK_URL=$(curl -s http://127.0.0.1:4040/api/tunnels 2>/dev/null \
            | grep -o '"public_url":"https://[^"]*' | head -1 | cut -d'"' -f4) || true
        if [ -n "${NGROK_URL:-}" ]; then break; fi
        sleep 1
    done

    if [ -z "${NGROK_URL:-}" ]; then
        echo "❌ Failed to get ngrok URL. Check /tmp/ngrok.log"
        exit 1
    fi

    CALLBACK="${NGROK_URL}/api/v1/gateway/webhook/feishu"
    echo ""
    echo "📡 ngrok URL:      ${NGROK_URL}"
    echo "🔗 Feishu回调地址: ${CALLBACK}"
    echo "🌐 ngrok 控制台:   http://127.0.0.1:4040"
    echo "📋 ngrok 日志:     tail -f /tmp/ngrok.log"
    echo ""
    echo -n "${CALLBACK}" | pbcopy 2>/dev/null && echo "📋 已复制到剪贴板!" || true
    echo ""
    echo "Press Ctrl+C to stop tunnel"
    wait $NGROK_PID

# Build
build:
    cargo build

# Check
check:
    cargo check

# Test
test:
    cargo test

# Format
fmt:
    cargo fmt

# Lint
lint:
    cargo clippy -- -D warnings

# Clean build artifacts
clean:
    cargo clean
