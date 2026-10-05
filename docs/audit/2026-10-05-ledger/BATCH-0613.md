# BATCH-0613 · `ignite.sh` 结项：尾部**四个入口**（`/app/` · `/render` · `/api/v1/mods` · 自测清单）—— 而**只有自测清单带路径**

Phase 1 · 域覆盖 · 脚本（`scripts/ignite.sh` 211 行 · **本批结项**）

## 跑的命令（全部只读）
```
sed -n '110,126p' scripts/ignite.sh
sed -n '196,211p' scripts/ignite.sh
```
未跑脚本（**运行脚本不在允许清单内**）。

## 两段逐行
```
# ---- 1) 读配置里的 LLM/TTS 端点 -------------------------------------------
:113 if ! command -v python3 >/dev/null 2>&1; then
:114   echo "[error] 需要 python3 读取 live2d-ai.toml"; exit 1
:116 read -r LLM_BASE TTS_BASE <<EOF
:117 $(python3 - <<PY
:118 import pathlib, tomllib
:119 llm = "http://127.0.0.1:11434/v1"        # ⭐ 缺省值在**脚本里**
:120 tts = ""
:122 if p.is_file():
:123   d = tomllib.loads(p.read_text(encoding="utf-8"))
:124   llm = d.get("llm", {}).get("base_url", llm)
:125   tts = d.get("tts", {}).get("base_url", "")

# 尾部
:199 pkill -f "live2d-ai-desktop --web" 2>/dev/null || true
:200 sleep 0.5
:203 echo "  点火入口： http://127.0.0.1:${PORT}/app/"
:204 echo "  渲染面  ： http://127.0.0.1:${PORT}/render"
:205 echo "  Mod 状态： http://127.0.0.1:${PORT}/api/v1/mods"
:206 echo "  自测清单： docs/verification/ignition-core-loop.md"     # ⭐ 唯一带路径的一条
:209 echo "==> 启动 web 服务（**音频单源=browser**：由浏览器出声并驱动口型）"
:210 exec ./target/debug/live2d-ai-desktop --web --http-port "${PORT}"
```

## 六个可核点
1. ⭐⭐⭐⭐⭐⭐⭐ **⇒ 尾部给的是**四个入口**、三个是 URL、一个是**仓内文档路径**（`docs/verification/ignition-core-loop.md`）
   ⇒⇒ **⇒ 与 AGENTS 的「默认文档入口只推销 `--web` / `scripts/ignite.sh`」**同一族**：**从脚本里指路** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐⭐⭐ **⇒ 而 `:119` 的**缺省 LLM 端点 `http://127.0.0.1:11434/v1` 写在脚本里**、而 `:120` 的 TTS 缺省是**空串**
   ⇒⇒ **⇒ 与 B0453 核的「回落值不能与真实值同义」**是同一判据的**脚本侧**：**「没配 TTS」不能假装成「配了某个 TTS」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
4. ⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `:209` 把**架构裁决**写进启动回显**：「**音频单源=browser：由浏览器出声并驱动口型**」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒ 与 AGENTS 那条「**音频播放走媒体元素，不走 Web Audio**（用户裁决）**」**是同一条的两次出现**（一次在 AGENTS、一次在脚本回显**）** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

## `ignite.sh` 结项（本批共 6 处可核）
`:47/51` 开关 · `:54` 用法自读 · `:61-108` 体检（**两条件断言 + 实测值 + `exit "$fail"`**）· `:113-125` toml 预检
（**python3 缺失即退** + **LLM 有缺省 / TTS 无缺省**）· `:164-168` 缺失才 `trunk build`（**并注明 trunk 需联网**）·
`:196-211` **四入口回显**（**三个 URL + 一条仓内清单**）+ **架构裁决写进回显** + `exec` 启动。

## 记为观察（一条）
`:206` 的 `docs/verification/ignition-core-loop.md` ⇒⇒ **本仓有**三个形近目录**（`verification/` · `verify/`（不存在）· `docs/verification/`）** ⇒⇒
按 B0440 纪律**先定位、不猜**；而**这一条**「脚本指的那条路径在不在」**未核**（下一批第一件事）。

## 未核
`docs/verification/ignition-core-loop.md` 是否存在（**下一批第一件事**）· `ignition-precheck.sh` 本体（402 行只读 10 行）·
`verify_edge_alpha.py` 本体（177 行只读 14 行）· `diag-stale.sh` 后续 32 行 ·
`adb_ui_tap.py` / `deploy_android.sh` / `setup_linux.sh`。
