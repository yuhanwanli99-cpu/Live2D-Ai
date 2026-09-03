# N.E.K.O（猫娘计划）同类项目调研报告（2026-09-03）

> 调研 Agent 产出，主 Agent 归档。来源：官方 docs（README/architecture/api/design/frontend/guide/plugins）+ 同类项目对比。纯 web 调研未 clone。
> 结论：3 条可行借鉴 + 1 条保持。详细版见本报告（含来源索引）。

## 对比摘要

| 项目 | 栈 | 桌宠窗口 | Live2D | 本地推理 | 点击穿透 |
|---|---|---|---|---|---|
| N.E.K.O | Python/FastAPI + Electron(闭源壳) | Electron transparent | live2d.min.js (WebGL) | Ollama/GPT-SoVITS | X11 ✅ / Wayland 部分 |
| my-neuro | Python + Electron | Electron transparent | pixi-live2d-display | GPT-SoVITS | Windows ✅ |
| NekoAI | Tauri + Rust + React | Tauri | sprite（非 Live2D） | Ollama | ✅ |
| Live2D-Ai | **Rust + Web UI** | winit transparent | **wgpu 原生** | Mod spawn | Linux X11 ✅ / Wayland 检测 |

## 三条可行借鉴

### 1. 口型音频协议 + interruption 防护（N.E.K.O tts-pipeline）
- N.E.K.O：48kHz mono s16le PCM → WS `audio_chunk` JSON header（**speech_id**）+ **二进制 PCM frame**（非 base64）；`__interrupt__`/done-marker 防迟到音频；upload_model 时**清除 motion 文件里的口型曲线**防止运行时口型冲突。
- 我们：F6 已是 20ms base64 帧 + epoch 闸门（同构）；但可借鉴：
  - [ ] `l2d` motion loader 增加清除 motion3 口型曲线（防动作播放与口型打架）
  - [ ] WS 音频帧加 speech_id 级别版本闸（现用 epoch，等价已达成——确认即可）

### 2. 动作编排配置 schema（emotion_mapping）
- N.E.K.O：LLM 文本标签 → emotion_mapping JSON（`{"motions":{"happy":[motion3...]}}`）→ motion3/exp3 播放；`常驻` group = expression-only 不可抢占。
- 我们：6 语义动作 + 显式抢占（更强仲裁，动画离散受限）。
- 借鉴：[ ] `mod-director` 引入 emotion→动作序列 JSON 配置 schema + `persistent` flag（对应"常驻"）。

### 3. 桌面内容感知框架（proactive vision）
- N.E.K.O：`proactiveVisionEnabled`（user-owned 永不自动开）+ interval → `POST /api/capture/screenshot`（loopback-only, renderer 截屏）→ vision LLM → 主动对话（probability+cooldown 决策）。
- 我们：external-input 被动输入；Web UI 浏览器无法截屏主机。
- 借鉴（需 native host）：[ ] `pet-desktop` Mod 加 proactive_vision user-owned settings；winit host 截屏 → JPEG → vision LLM；沿用 loopback 约定。

## 保持项
- WASM 交互（pointer drag + wheel scale + ptr capture + dblclick reset）：与 N.E.K.O 同构，F-a 已落地，无需改。
- 不引入 Electron/Python/ZMQ——与我们 95% Rust 单二进制目标冲突（N.E.K.O.-PC 壳闭源不可参考）。

## 与当前 P3.1 的关系
- F-a（拖动/滚轮）方案与 N.E.K.O 同构 → 交互路线确认正确。
- F-c（动作参数层）→ 若需更丰富动画，长期走借鉴 2 的 motion3 路线；v1 参数等效动画保留。
- 口型（用户实测第 3 条）→ 走 F-d 指引装 kokoroi-rs 后实测；借鉴 1 的 motion 曲线清洗列入后续。

## 来源
官方仓库文档 14+ URL 与 Electron PR/同类项目，完整索引见调研原文（本报告为主 Agent 摘要归档）。
