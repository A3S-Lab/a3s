#!/bin/bash
# SafeClaw 安装脚本
# 自动将应用复制到 Applications 并移除隔离属性

set -e

echo "🦅 SafeClaw 安装程序"
echo ""

# 检查是否在 DMG 中运行
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_PATH="$SCRIPT_DIR/SafeClaw.app"

if [ ! -d "$APP_PATH" ]; then
    echo "❌ 错误：找不到 SafeClaw.app"
    echo "请确保此脚本在 DMG 中与 SafeClaw.app 同级目录"
    exit 1
fi

echo "📦 正在复制 SafeClaw.app 到 /Applications..."
cp -R "$APP_PATH" /Applications/

echo "🔓 正在移除隔离属性..."
xattr -cr /Applications/SafeClaw.app

echo ""
echo "✅ 安装完成！"
echo ""
echo "现在可以从 Launchpad 或 Applications 文件夹启动 SafeClaw 了。"
echo ""

# 询问是否立即打开
read -p "是否立即打开 SafeClaw？(y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    open /Applications/SafeClaw.app
fi
