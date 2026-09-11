#!/bin/bash
# Live2D-Ai — Android deploy helper
#
# 环境:
#   手机: 192.168.0.108:5555 (无线调试)
#   锁屏密码: 69470
#   uiauto控制器: F:\uiauto\uiautodev-desktop.exe
#
# 首次连接步骤:
#   1. 手机打开: 设置 → 开发者选项 → 无线调试
#   2. 点击"无线调试"进入详情，记录显示的 端口号 和 配对码
#   3. adb pair 192.168.0.108:<配对端口>  # 输入手机上显示的6位配对码
#   4. adb connect 192.168.0.108:5555

set -e

PHONE="${PHONE:-192.168.0.108:5555}"
PHONE_PASSWORD="69470"
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
APK_PATH="$PROJECT_DIR/Live2D-Ai-Android/app/build/outputs/apk/debug/app-debug.apk"

# 优先使用 uiauto 自带的 adb（Windows ADB）。
# 注意: `/mnt/f` 是旧机器(WSL 迁移前)的盘符挂载；新机器只有 C:/D:。
# 检测到旧盘不存在时直接回退系统 adb，不再依赖 wine 探测。
UIAUTO_ADB="/mnt/f/uiauto/tools/adb.exe"
SYSTEM_ADB="$(which adb 2>/dev/null || echo "")"
ADB=""

if [ -f "$UIAUTO_ADB" ]; then
    # UIAuto ADB 是 Windows exe，在 WSL 中用 wine 或直接调
    if command -v wine &>/dev/null; then
        ADB="wine $UIAUTO_ADB"
    else
        ADB="$SYSTEM_ADB"
    fi
else
    # 旧盘不可达（新机器）或 uiauto 未安装：直接用系统 adb
    if [ -z "$SYSTEM_ADB" ]; then
        # 尝试 Windows 侧的 adb.exe（新机器 Windows 路径）
        WINDOWS_ADB="$(which adb.exe 2>/dev/null || echo "")"
        if [ -n "$WINDOWS_ADB" ]; then
            ADB="$WINDOWS_ADB"
        fi
    else
        ADB="$SYSTEM_ADB"
    fi
fi

if [ -z "$ADB" ] || [ "$ADB" = "" ]; then
    echo "[✗] ADB not found. Install Android SDK platform-tools."
    exit 1
fi

connect_phone() {
    echo "[*] Connecting to $PHONE ..."
    $ADB connect "$PHONE" 2>&1
    sleep 2
    if $ADB devices | grep -q "$PHONE"; then
        echo "[✓] Connected to $PHONE"
        return 0
    else
        echo "[✗] Failed to connect."
        echo ""
        echo "首次连接步骤:"
        echo "  1. 手机打开: 设置 → 开发者选项 → 无线调试"
        echo "  2. 记录显示的配对端口和6位配对码"
        echo "  3. adb pair 192.168.0.108:<配对端口>"
        echo "     (输入手机上显示的6位码)"
        echo "  4. 重新运行本脚本"
        echo ""
        echo "锁屏密码: $PHONE_PASSWORD (如果中途锁屏)"
        return 1
    fi
}

case "${1:-install}" in
    install)
        echo "[*] Building APK ..."
        cd "$PROJECT_DIR/Live2D-Ai-Android"
        ./gradlew assembleDebug
        echo "[✓] APK built: $APK_PATH"

        connect_phone || exit 1

        # vivo FuntouchOS 不允许 `adb install -r` 覆盖安装，必须先卸载。
        echo "[*] Uninstalling previous version (vivo 要求完整卸载重装) ..."
        $ADB -s "$PHONE" uninstall com.live2d.ai.android 2>&1 || true

        # `adb install` 在 vivo 上会弹出"未知来源/继续安装"安全弹窗，且 adb 命令本身
        # 会挂起等待用户在弹窗上确认（不会自己超时报错）。用后台方式发起安装，
        # 然后用 adb_ui_tap.py 主动 dump UI 树找真实按钮坐标点击，而不是盲点固定坐标
        # （盲点坐标在不同分辨率/弹窗位置下不可靠——见 .pi/spoq-state.json 里记录的教训）。
        echo "[*] Installing APK (async, UI树自动过安全弹窗) ..."
        $ADB -s "$PHONE" install "$APK_PATH" &
        INSTALL_PID=$!

        python3 "$PROJECT_DIR/scripts/adb_ui_tap.py" wait-and-tap "$PHONE" com.live2d.ai.android --timeout 90
        TAP_STATUS=$?

        wait "$INSTALL_PID" 2>/dev/null || true

        if [ $TAP_STATUS -ne 0 ]; then
            echo "[✗] 安装未在预期时间内确认完成，请检查手机屏幕上是否有未识别的新弹窗文案"
            echo "    （脚本已打印最后一次 UI 树里出现的文案，可以加进 scripts/adb_ui_tap.py 的 BUTTON_PATTERNS）"
            exit 1
        fi
        echo "[✓] Installed!"

        echo "[*] Launching app ..."
        $ADB -s "$PHONE" shell am start -n com.live2d.ai.android/.MainActivity
        echo "[✓] App launched!"
        ;;
    reboot)
        $ADB kill-server 2>/dev/null || true
        sleep 1
        $ADB start-server 2>/dev/null || true
        sleep 1
        connect_phone
        ;;
    logcat)
        connect_phone || exit 1
        echo "[*] Streaming logcat (filter: Live2D-Ai) ..."
        $ADB -s "$PHONE" logcat -v time | grep -iE "(live2d|DeepSeek|EdgeTts|GLM)"
        ;;
    unlock)
        # 用锁屏密码解锁手机（屏幕可能熄灭）
        connect_phone || exit 1
        echo "[*] Unlocking phone ..."
        $ADB -s "$PHONE" shell input keyevent 26  # 点亮屏幕
        sleep 1
        $ADB -s "$PHONE" shell input touchscreen swipe 300 1000 300 300  # 上滑
        sleep 1
        $ADB -s "$PHONE" shell input text "$PHONE_PASSWORD"  # 输入密码
        sleep 0.5
        $ADB -s "$PHONE" shell input keyevent 66  # 回车确认
        echo "[✓] Phone unlocked (if screen was on and locked)"
        ;;
    *)
        echo "Usage: $0 [install|reboot|logcat|unlock]"
        echo ""
        echo "Commands:"
        echo "  install   构建APK → 安装 → 启动 (默认)"
        echo "  reboot    重启 ADB 服务器"
        echo "  logcat    查看手机日志 (过滤 Live2D-Ai)"
        echo "  unlock    解锁手机屏幕 (密码: $PHONE_PASSWORD)"
        ;;
esac
