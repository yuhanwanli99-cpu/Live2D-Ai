#!/bin/bash
# Live2Dai — Desktop smoke test
set -e

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CD="$PROJECT_DIR/Live2Dai-pc/open-llm-vtuber"

echo "==================================="
echo "  Live2Dai Desktop Smoke Test"
echo "==================================="
echo ""

echo "[1/5] Python version: $(python3 --version 2>/dev/null || python --version 2>/dev/null)"

echo "[2/5] Checking conf.yaml ..."
if [ -f "$CD/conf.yaml" ]; then
  grep -q "deepseek-v4-flash" "$CD/conf.yaml" && echo "  ✓ Model: deepseek-v4-flash"
  grep -q "edge_tts" "$CD/conf.yaml" && echo "  ✓ TTS: edge_tts"
  grep -q "live2d_fps: 120" "$CD/conf.yaml" && echo "  ✓ FPS: 120"
fi

echo "[3/5] Checking shared persona ..."
python3 -c "
import yaml, sys
with open('$PROJECT_DIR/shared/persona.yaml') as f:
    d = yaml.safe_load(f)
print(f'  ✓ {d[\"name\"]} | model={d[\"model\"]} | prompt_len={len(d[\"system_prompt\"])}')
" 2>/dev/null

echo "[4/5] Testing DeepSeek API (requires DEEPSEEK_API_KEY env var) ..."
HTTP_CODE=$(curl -s -o /dev/null -w '%{http_code}' \
  https://api.deepseek.com/v1/models \
  -H "Authorization: Bearer ${DEEPSEEK_API_KEY:?DEEPSEEK_API_KEY not set}" 2>/dev/null)
echo "  ✓ HTTP $HTTP_CODE"

echo "[5/5] Live2D models ..."
ls "$CD/live2d-models/"*/runtime/*.model3.json 2>/dev/null && echo "  ✓ models found" || echo "  ⚠ no model3.json"

echo ""
echo "To start: cd $CD && uv run python run_server.py"
echo "Then open http://localhost:12393"
