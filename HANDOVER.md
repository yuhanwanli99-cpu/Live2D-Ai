# ⚠️ 本文档已作废（2026-08-21 的 v0.1.0 归档）

> **不要再按本文档理解项目。** 它描述的是「Android 主端 + PC 端 Open-LLM-VTuber +
> Melo TTS」那一代（v0.1.0），双端都已归档：Python 端在 tag `py-legacy`，
> Android 端在远端 `android-archive` 分支。
>
> **当前项目的正确入口**：
> - `AGENTS.md`（分层、门禁、语音契约）
> - `docs/plans/HANDOFF-2026-09-11.md`（**当前交接**：v0.4.13 → v0.5.1 前端重做 +
>   真机验收 + 错误可观测性；§12 是未提交改动的清单与提交切分）
> - `docs/design/web-ui-spec-v3.md`（前端设计规格，实现者照此写代码）
> - `CHANGELOG.md`（v0.4.0 起的逐版变更）

---

# Live2D-Ai — 项目交接文档（v0.1.0 定稿）

> 更新日期: 2026-08-21（PC 接手 + 新计划落盘）  
> 项目定位: Live2D + AI + TTS 最小验证项目 — 验证「文本/语音 → LLM → 表情/口型/动作」闭环。双端：Android 主端 + PC（Open-LLM-VTuber）。  
> 版本: **v0.1.0**（Android 冻结；PC 按新计划推进）

## 〇、当前执行计划（2026-08-21 接手，PC 端）

- **当前唯一执行口径**：`docs/plans/PLAN-V2-PC-LOCAL-TTS.md`（仅 PC，附带本地 Melo TTS 需求；取代 PLAN-V1）。
- **目标**：把 Melo TTS 接成 PC 设置中心「本地/免费」分组的一等公民 Provider，并新增稳定本地 TTS HTTP 接口（`POST /api/tts/local/synthesize`），全程无需云端 TTS Key。
- **范围边界**：不碰 Android；Melo 定位为本地多语种音色合成，参考音频克隆仍走 `voice_library.py` + CosyVoice/GPT-SoVITS。
- **执行状态（2026-08-21）**：阶段 1–5 的代码、设置 UI、REST 接口、测试与绿门已完成；**外部 Melo 环境已对接**（`melo_worker.py` + `melo_external.py` 常驻子进程，`MELO_PYTHON` 可覆盖解释器路径），默认 `device=cpu`，并限制单次 `MELO_MAX_TEXT_LEN=200` 字 / `MELO_MAX_SENTENCES=4` 句。已用真实 ZH 模型跑通 worker 与 `MeloTTSEngine.generate_audio` 端到端合成。绿门：`verify_all.py --pc` 5/5、PC pytest 325 passed、前端 vitest 375 passed、tsc+build 通过。**剩余**：EN/ES/FR/JP/KR 首次下载合成与 cuda 并发冒烟。

## 〇、v0.1.0 最终状态（2026-08-15 大重构收尾）

### Android（主端，全部真机验证）
| 模块 | 状态 |
| --- | --- |
| 核心环路 | ✅ 输入→DeepSeek→CosyVoice 云 TTS（minimp3 解码）→嘴型 RMS 同步→Live2D 渲染；sherpa-onnx 离线兜底 |
| 情绪联动 | ✅ N.E.K.O. 架构：独立 LLM 分类器→单一标签→参数帧表→表情→5s 收敛（冻结免疫） |
| 渲染 | ✅ 官方 CubismNativeFramework vendor + PurismCore（MIT）；渲染质量三档设置 |
| 结构免疫 | ✅ 双播报（LoopStateMachine 单消费链）/ 冻结（表情收敛）/ 嘴型无帧（PCM直出+minimp3）三 bug 根除 |
| 观感 | ✅ 待机间歇轻晃、眨眼正常、FPS 角标、气泡无标签残留 |
| 测试 | ✅ JVM 663/663 绿（78 类） |

### PC（Open-LLM-VTuber）
- ✅ CosyVoice 云（新 Key+专属 host 已同步 conf.yaml）、人设 shared/persona.yaml 联动
- ✅ PC-L1 打断崩溃修复、PC-L4 mood_tier 五档接通；pytest 150/150

### 已知限制（v0.2 计划）
1. 实际呈现 ~50fps（渲染循环 120）——官方 C++ 渲染器模型加载接线（最后一公里）
2. sherpa 音色选择器（100 中文音色）、声音克隆（CosyVoice 已具备条件）
3. vivo 屏幕刷新率需手动设"高"

## 一、大重构脉络（2026-08-14 → 08-15）

用户方针：自研代码段换开源项目代码段，大重构非打补丁；帧率 90-120fps、清晰度 ≥1080p；只做核心环路。

- Wave 0-1：基线固化 + 官方 CubismNativeFramework vendor（300 文件，45de2122）+ engine 骨架 + ABI 混编实证（-include stddef/stdint）
- Wave 2：三片替换（渲染 seam render/supersample/双轨 LEGACY 默认 + sherpa 单引擎 + core 状态机/壳）——JVM 819/819
- Wave 3：动画 seam（updater 注册矩阵/合成 motion 生成器/待机桥/情绪标签表）——866/866
- Wave 4：三片汇合（LoopWire/SpeechSinkAdapter/MainActivity 注册链）+ Compose 可观察化 + VisualExpressionSink——867/867
- Wave 5：非核心全删（插件 5 模块/ASR 链/旧 TTS/遥测/确认门/悬浮窗/旧 Chat 链）——645/645
- Wave 6：big-bang 真机验收（9/10 PASS；帧率项=设备刷新策略）+ 用户反馈批次（N.E.K.O. 情绪分类器/FPS 角标/间歇轻晃/三档画质/CosyVoice 重加/minimp3/眨眼修复）——663/663

## 二、关键架构事实（v0.1）

### Android 核心环路
```
MainScreen 输入 → LoopCoordinator(IDLE→SENDING) → HttpLlmLink(SSE 冷流单消费者)
  → LlmCompleted(cleanText, emotion) → SpeechSpeak + ExprApply(VisualExpressionSink→setParametricExpression 眼键过滤)
  → SpeechSinkAdapter→VoiceIoController(TtsProviderRegistry: cosyvoice_ws 主用 → sherpa 兜底)
  → 播放 → SpeakFinished(下降沿) → MouthZero + ExprRelease(5s 收敛) → IDLE
```

### TTS 双引擎
- cosyvoice_ws：DashScope WS（专属 host），二进制 MP3 块 → minimp3（CC0 vendored cpp/minimp3）→ PCM 帧流（150ms/首帧0/非递减）
- sherpa_onnx_kokoro：离线 Kokoro-82M-v1.1-zh（PCM 直出），模型目录 externalFilesDir/models/kokoro_tts/

### 渲染
- 双轨：BackendSelector（默认 LEGACY）；官方引擎已编译未接模型加载（v0.2）
- 三档画质：SMOOTH(1080p)/BALANCED(MSAA4x，重启生效)/HD(1440×3200 超采样)，即时切换

### 关键文件
| 文件 | 职责 |
| --- | --- |
| core/LoopStateMachine.kt / LoopCoordinator.kt | 三态状态机 + I1-I7 不变式（双播报/冻结免疫核心） |
| core/HttpLlmLink.kt / EmotionClassifier.kt | LLM SSE + N.E.K.O. 情绪分类器（3s 可取消超时） |
| core/VisualExpressionSink.kt / data/EmotionTagTable.kt | 表情视觉落点（眼键剔除） |
| data/IdleMotionController.kt | 间歇轻晃（动 5s 停 10s 首静 3s） |
| CosyVoiceTtsProvider.kt + cpp/minimp3/ | 云 TTS + 解码 |
| render/ + supersample/ + cpp/framework/ + cpp/engine/ | 官方渲染 seam + 引擎 |

## 三、装机/真机 SOP（沿用，必读）

- vivo 卸载重装：adb uninstall → pm install -t /data/local/tmp/x.apk（后台 &）→ 弹窗 tap **(329,2079) 勾选 → (540,2227) 确认**（2026-08-15 视觉实测定点，旧坐标 540,2082/540,2228 已失效）
- 验证：firstInstallTime==lastUpdateTime + 代码特征日志
- Git Bash：export MSYS_NO_PATHCONV=1
- 模型：卸载会清 externalFilesDir → 重装后重推 kokoro 模型（PC 端 .pi/spoq/models/kokoro_extract/）
- 帧率：手机屏幕刷新率需固定"高"（vivo 智能切换会降 60Hz）

## 四、v0.1 遗留与 v0.2 候选

| # | 项 | 优先级 |
| --- | --- | --- |
| 1 | 官方渲染器模型加载接线（ENGINE 路径）→ 呈现 90-120fps | 高 |
| 2 | sherpa 音色选择器（100 中文音色）+ 默认 zf_001 验证 | 中 |
| 3 | CosyVoice 声音克隆（5 秒录音定制音色） | 中 |
| 4 | FPS 角标显示呈现帧率（当前显示渲染循环 120，实际呈现 ~50——考虑双数显示或标注） | 低 |
| 5 | PC 端 interrupt 路径补端到端 pytest | 低 |

*详细交接见 .pi/spoq/handover-2026-08-14.md 及各波 GREEN/FIX 报告。*
