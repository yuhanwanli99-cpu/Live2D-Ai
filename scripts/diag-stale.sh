#!/usr/bin/env bash
# 诊断：为什么你看到的是"老产物"
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

echo "===== 1. 当前在跑的 live2d-ai-desktop 进程 ====="
ps aux | grep -E "live2d-ai-desktop" | grep -v grep || echo "（无进程）"

echo
echo "===== 2. 本地二进制构建时间 ====="
ls -la target/debug/live2d-ai-desktop 2>/dev/null | awk '{print $6,$7,$8,$NF}'

echo
echo "===== 3. WASM dist 构建时间与 hash ====="
ls -la crates/l2d-wasm-demo/dist/*_bg.wasm | awk '{print $6,$7,$8,$NF}'
W=$(ls crates/l2d-wasm-demo/dist/ | grep '_bg.wasm$' | sed 's/.*-\([a-f0-9]*\)_bg.wasm/\1/')
echo "hash: $W"

echo
echo "===== 4. 服务端实际返回（启动 3 秒探针） ====="
pkill -f 'live2d-ai-desktop --web' 2>/dev/null; sleep 0.5
LIVE2D_AI_WS_AUDIO=1 ./target/debug/live2d-ai-desktop --web --http-port 18999 >/tmp/diag.log 2>&1 &
sleep 3
echo "-- /render 引用的 wasm:"
curl -s http://127.0.0.1:18999/render | grep -oE 'l2d-wasm-demo-[a-f0-9]+_bg\.wasm'
echo "-- /render/<wasm> 大小:"
curl -s "http://127.0.0.1:18999/render/l2d-wasm-demo-${W}_bg.wasm" | wc -c
echo "-- 主页 index 里的 stage-toolbar（新 UI 特征）:"
curl -s http://127.0.0.1:18999/ | grep -c "stage-toolbar"
echo "-- index 里的 settings-modal（新设置特征）:"
curl -s http://127.0.0.1:18999/ | grep -c "settings-modal"
pkill -f 'live2d-ai-desktop --web' 2>/dev/null

echo
echo "===== 5. 诊断结论 ====="
echo "若第 4 步显示的 hash == 第 3 步 hash 且 stage-toolbar/settings-modal 均 >0："
echo "  → 服务端是新的。你看的是浏览器缓存/旧标签页：关闭所有该站点标签页，"
echo "    重开浏览器无痕窗口访问 http://127.0.0.1:18080"
echo "若 hash 不一致或特征计数为 0："
echo "  → 服务器进程是旧的：用 scripts/run-web.sh 重启（它会杀旧进程）。"
