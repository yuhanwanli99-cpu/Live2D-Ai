# BATCH-0609 · 两脚本**不是重复**：`ignite.sh` **只构建 Flutter Web**，`run-web.sh` **只构建 Rust + WASM**

Phase 1 · 域覆盖 · 脚本（`scripts/ignite.sh` 211 行**头注** vs `scripts/run-web.sh` 30 行，**首次对照**）

## 跑的命令（全部只读）
```
wc -l scripts/ignite.sh
sed -n '1,12p' scripts/ignite.sh
cat -n scripts/run-web.sh          # 上一批已全文读
```
未跑脚本（**运行脚本不在允许清单内**）。

## 两者的职责（B0608 那条观察要按此**修正**）

| | `ignite.sh`（211 行） | `run-web.sh`（30 行） |
|---|---|---|
| 头注自述 | 「**它做四件事**：① **预检**（读 toml、探测 LLM/TTS 端点）② **确保渲染面产物**（`l2d-wasm-demo/dist`）③ **确保前端产物**（`shell/flutter/build/web`，可选 `--build` 自动构建）④ 启动并打印点火 URL」 | 「**一键构建+启动**」；`--no-build` 之外：`trunk build`（WASM）+ `cargo build -p live2d-ai-desktop` |
| **构建什么** | ⭐ **Rust（`--build`）+ Flutter Web（`--build`）** | ⭐ **WASM + Rust desktop** |
| 端点预检 | ✅ ① | ❌ |
| 打印点火 URL | ✅ ④ | 只 `echo http://127.0.0.1:${PORT}` |
| 环境变量 | — | `LIVE2D_AI_WS_AUDIO=1` |

## 五个可核点
1. ⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 二者**不是重复**：`ignite.sh` 负责**前端产物 + 端点预检**，`run-web.sh` 负责**WASM + Rust desktop**
2. ⭐⭐⭐⭐⭐⭐ **⇒ 而 B0608 那条「两个一键入口」要**收窄**：**两者覆盖的是不同环节**（前端 vs 后端产物），
   而 **`ignite.sh` 的 ② 只「确保」WASM dist、不说它自己构建** ⇒⇒ **⇒ 那个「确保」是否含 `trunk build` 未核** ⇒⇒ **按 B0440 记为待核**。
3. ⭐⭐⭐⭐⭐ **⇒ 而 `run-web.sh` 独有 `LIVE2D_AI_WS_AUDIO=1`** ⇒⇒ **⇒ 这一行是「WS 音频」开关，AGENTS 未提** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
4. ⭐⭐⭐⭐ **⇒ 而 `ignite.sh` 的用法只有两种**（默认「**预检 + 启动（不重新构建）**」· `--build`）⇒⇒ **⇒ 而 `run-web.sh` 默认就构建** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
5. ⭐⭐⭐⭐ **⇒ 而 AGENTS 那句「`ignite.sh` 会把 `LIVE2D_AI_FLUTTER_WEB_DIR` 锚定成仓库内绝对路径，**所以 `cwd` 不对 → `/app/` 503** 不该再出现**」
   写的正是**两个变量名 + 一个症状** ⇒⇒ **⇒ 与 B0593 核的「A 段给了什么、给了哪几方」**同形**（**引用方 + 症状**，不是「为什么会这样」）⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

## 结论
- **B0608 观察①（两个一键入口）**不成立为「重复」** ⇒ **撤回该表述**；准确的说法是：
  **两个脚本覆盖不同环节，而 `ignite.sh` 是 AGENTS 承认的那一个** ⇒⇒ 剩下的问题只有一个：
  **`run-web.sh` 不构建 Flutter Web ⇒ 用它启动会拿到**旧的 `/app/` 产物**，而它**没有提示这一点** ⇒⇒ **记为 P3 观察**
  （修法一行：头注加「**本脚本不构建 Flutter Web；前端改动请用 `ignite.sh --build`**」）。
- **本仓已有「指路 + 症状」这条写法的先例**（AGENTS 的 `cwd` → 503）⇒ ⇒ **记为 P3 观察而非发现**。

## 未核
`ignite.sh` 的「确保 WASM dist」**是否含 `trunk build`**（第 2 点 · 下一批第一件事）·
`ignition-precheck.sh` 本体（402 行只读 10 行）· `verify_edge_alpha.py` 本体（177 行只读 14 行）·
`diag-stale.sh` 后续 32 行 · `adb_ui_tap.py` / `deploy_android.sh` / `setup_linux.sh`。
