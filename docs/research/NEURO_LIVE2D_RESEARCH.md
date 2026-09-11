# Neuro‑sama Live2D 技术调研报告

> 调研员：researcher agent（只读）
> 任务编号：Q1‑Neuro‑sama Live2D 做法 + Q5 社区「自然」实践 + Q6 CosyVoice 个性化
> 原则：每条结论附 URL/行号；抓取失败标注「未找到，待人工补充」；不编造数据。
> 目标读者：Live2D-Ai（Android Kotlin + PC Python，Purism Core + CubismJavaFramework + CosyVoice 专属 key）。

---

## 目录
1. [Neuro‑sama 的 Live2D 实现方式](#1-neurosama-的-live2d-实现方式)
2. [动作 / 表情 / 口型 / 眨眼「自然」的技术方案](#2-动作--表情--口型--眨眼自然的技术方案)
3. [渲染优化细节](#3-渲染优化细节)
4. [其他社区「自然化」实践工具清单](#4-其他社区自然化实践工具清单)
5. [CosyVoice 个性化官方参数与示例](#5-cosyvoice-个性化官方参数与示例)
6. [对 Live2D-Ai 落地建议](#6-对-live2d-ai-落地建议)
7. [未找到 / 需人工补充项](#7-未找到--需人工补充项)

---

## 1. Neuro‑sama 的 Live2D 实现方式

### 1.1 模型：确认为官方免费示例模型 Hiyori Momose（桃瀬ひより）
- **结论**：Neuro‑sama 的 Live2D 形象基于 Live2D 官方免费示例模型 **Hiyori Momose**，社区 LoRA 描述明确写明 "whose live2d model is identically Momose Hiyori"。
- **来源**：SeaArt https://www.seaart.ai/models/detail/3de0a0c57380fe6db6d2bf6fb956f724 ；AIEasyPic https://aieasypic.com/inspire/models/detail/neuro-sama-momose-hiyori-lora-v10-14795 ；Live2D 官方免费示例 https://www.live2d.com/en/learn/sample/ 与 https://www.live2d.com/en/learn/sample/momose-hiyori-video/
- **证据**（SeaArt/AIEasyPic 原文）："Neuro-sama, whose live2d model is identically Momose Hiyori... (e.g. blue eyes rather than black eyes, neck bow rather than neck ribbon)"。
- **意义**：可直接下载官方 free 模型做原型，无需自建。

### 1.2 引擎 / SDK 版本：渲染侧未开源，**无法核实 Cubism 具体版本**
- **结论**：Vedal987/Neuro、VedalAI 公开仓库 `neuro-sdk`/`neuro-game-sdk` **只开源了「让 Neuro 玩游戏」的 WebSocket API SDK（Unity/Godot）**，未开源 VTuber 形象渲染。故「Neuro‑sama 用 Cubism 哪个版本」**无公开一手来源，标记「未找到，待人工补充」**。
- **来源（反向证据）**：https://github.com/VedalAI/neuro-sdk （SDK 让 Neuro 玩游戏）；Unity README https://raw.githubusercontent.com/VedalAI/neuro-game-sdk/main/Unity/README.md （Build for Unity 2022.3，游戏 SDK，无 Live2D 渲染、无 Cubism 版本字样）；https://en.wikipedia.org/wiki/Neuro-sama ；https://en.neurosama.info/wiki/Neuro-sama
- **给项目的含义**：与其逆向闭源渲染，不如直接采用下方**已验证的开源复刻技术栈**（pixi‑live2d‑display / pymouth / Open-LLM-VTuber 等），它们是小成本实现「自然感」的成熟来源。

---

## 2. 动作 / 表情 / 口型 / 眨眼「自然」的技术方案

> Neuro‑sama 本体渲染未开源，本节聚焦**验证过的开源复刻与官方 SDK 机制**。

### 2.1 复刻参照 A：morettt/my‑neuro（Python + Electron，「受 neuro‑sama 启发」）
- **来源**：仓库 https://github.com/morettt/my-neuro ；README https://github.com/morettt/my-neuro/blob/main/README_English.md ；已抓取源码（raw.githubusercontent.com/morettt/my-neuro/main/live-2d/js/model/model-setup.js 与 .../ui/emotion-motion-mapper.js）。
- **渲染加载**（已抓取 model-setup.js）：PIXI + `pixi-live2d-display` 加载 `.model3.json`：
  ```js
  const app = new PIXI.Application({ view:document.getElementById("canvas"), transparent:true, width:actualWidth*2, height:actualHeight*2 });
  const model = await PIXI.live2d.Live2DModel.from("2D/肥牛/feiniu.model3.json");
  ```
- **动作/表情机制**（已抓取 emotion-motion-mapper.js）：
  - 动作是**预定义 .motion3.json + motion group**，非实时程序化生成：`this.currentMotionGroup="TapBody";`；`playConfiguredEmotion()` → `motionFiles[Math.floor(Math.random()*motionFiles.length)]` 随机选动作文件 → `findMotionIndexByFileName()`→`playMotion()`。
  - 情绪用 `#[emotion]#` 标签嵌入文本（`parseEmotionTagsWithPosition`→`triggerEmotionByTextPosition`），**随 TTS 播放到指定文本位置触发对应动作/表情**。
- **结论**：「自然感」= 预定义 Idle/情绪 motion 组 + 按文本位置触发 + 随机选动作避免重复。

### 2.2 复刻参照 B：moeru‑ai/airi（Vue + pixi-live2d-display）
- **来源**：`packages/stage-ui-live2d/src/composables/live2d/motion-manager.ts` https://github.com/moeru-ai/airi/blob/911572fe/packages/stage-ui-live2d/src/composables/live2d/motion-manager.ts
- **证据（已抓取）**：接口含 `live2dAutoBlinkEnabled`、`live2dForceAutoBlinkEnabled`、`live2dIdleAnimationEnabled`、`live2dEyeFocusSourceActive`（眼神注视跟随）、`isIdleMotion`、`startRandomMotion`。
- **结论**：社区公认「自然」= **自动眨眼 + 注视跟随 + 空闲(idle)动作循环** 三者组合，均可经 `pixi-live2d-display` 的 `internalModel.*` 控制。

### 2.3 pixi‑live2d‑display 默认眨眼过渡参数（可复用数值）
- **来源**：`src/cubism2/Live2DEyeBlink.ts` https://github.com/guansss/pixi-live2d-display/blob/1214833/src/cubism2/Live2DEyeBlink.ts
- **证据（已抓取）**：`blinkInterval:4000ms; closingDuration:100ms; closedDuration:50ms; openingDuration:150ms; eyeState=EyeState.Idle`。
- **结论**：眨眼 = 时序状态机（Idle→Closing→Closed→Opening）+ 可调 interval/时长，即「自然眨眼」标准做法。
- 附：Cubism 4 自动眨眼/口型经 `.model3.json` 的 `"Groups"`（`Target:"Parameter"`, `Name:"LipSync"/"EyeBlink"`）定义参数列表。来源：https://docs.live2d.com/en/cubism-sdk-manual/lipsync/

### 2.4 口型同步（LipSync）两种主流做法
- **方案 A（默认/最简）：音频响度 RMS/音量 → ParamMouthOpenY（简单开合）**
  - 官方 Native：https://docs.live2d.com/en/cubism-sdk-tutorials/native-lipsync-from-wav-native/ ；Web：https://docs.live2d.com/en/cubism-sdk-tutorials/native-lipsync-from-wav-web/ （"based on the volume of the audio data in the wav file"）。
  - my‑neuro：`AnalyserNode` 采样音频音量/频域映射到 `ParamMouthOpenY`。来源：https://deepwiki.com/morettt/my-neuro/5.5-real-time-interaction-pipeline （指 `live-2d/js/voice/tts-playback-engine.js#L51-L58`）。
  - EasyLive2D/live2d-py（Python）：算 wav 每帧 RMS → `model.SetParameterValue("ParamMouthOpenY",0,rms)`（`WavHandler` + `lipSyncN`）。来源：https://github.com/EasyLive2D/live2d-py/wiki/口型同步
- **方案 B（高级，能区分元音）**：
  - DenchiSoft/Live2DFrequencyLipSync：**频段驱动**口型，比只看音量自然（可区分元音）；官方 SDK 自带只用量。来源：https://github.com/DenchiSoft/Live2DFrequencyLipSync
  - organics2016/pymouth（Python，适合 PC 侧）：**DTW 匹配音频元音 + softmax 元音置信度**，非 AI 模型，移动端 CPU 可跑；VTubeStudio 仅可选 Adapter。来源：https://github.com/organics2016/pymouth
  - liyao1520/live2d-motionSync（TS）：`live2d-motionsync` 声音驱动实时口型。来源：https://github.com/liyao1520/live2d-motionSync
- **结论**：最省事的自然口型 = RMS→ParamMouthOpenY（Purism Core / CubismJavaFramework 均支持）；想更「像人」可在 PC Python 侧引入 pymouth(DTW 元音) 或 Live2DFrequencyLipSync(频段)。

### 2.5 表情过渡（expression transition）官方机制
- **结论**：Live2D 官方 SDK 原生支持表情过渡：(1) 表情为 `.exp3.json`，继承 `ACubismMotion`，由 MotionManager 管理；(2) 用 **fade（淡变）** 让当前表情随时间过渡到新表情；(3) 可指定计算方法（加算/正片叠底/覆盖）。
- **来源**：
  - Expression Motion：https://docs.live2d.com/en/cubism-sdk-manual/expression/ ；zh：https://docs.live2d.com/zh-CHS/cubism-sdk-manual/expression/
  - Expression Transition（Cubism 4.2→5 行为差异、默认方法）：https://docs.live2d.com/en/cubism-sdk-manual/blending-expression/
  - 多 motion 同时播放（多轨）：增加 `CubismMotionManager` 数量，可分开控不同部位；**避免同参冲突**。来源：https://docs.live2d.com/en/cubism-sdk-manual/motion/
- **结论两点**：
  1. **表情过渡 = motion/expression 的 fade + 可选加算/乘/覆盖**，SDK 默认自带。
  2. **多轨混合 = 官方支持（多个 CubismMotionManager）**，但**同一参数只保留最后更新的轨，否则 fade 不干净**。解法：让各轨尽量作用于不同参数（呼吸→ParamBreath、物理→Physics 参数、说话→ParamMouth*、表情→各部表情参数）。

### 2.6 肢体物理 / 呼吸
- **结论**：Live2D 的身体动感主要来自 **Cubism Physics（`.physics3.json`）**，由 Editor 导出、SDK 每帧自动积分驱动，与程序代码无关。呼吸/眼神这类「程序叠加运动」放独立 motion 轨或参数脚本。
- **来源**：Cubism 官方 Native SDK 参考（Physics 属 SDK 内置能力）：https://cubism.live2d.com/sdk-doc/reference/

---

## 3. 渲染优化细节

- **2x 超采样/HiDPI（已抓取）**：my‑neuro 将 Canvas 内部设为窗口 2 倍（`width:actualWidth*2`）+ `canvasScaleFactor=2`，CSS 尺寸回实际 px → 高分辨率抗锯齿。来源：`live-2d/js/model/model-setup.js`（https://raw.githubusercontent.com/morettt/my-neuro/main/live-2d/js/model/model-setup.js ）。
- **HiDPI 属宿主责任**：官方 `compatibility-with-cubism-5` 页写明 HiDPI "No SDK support, as this is Cubism Editor functionality"。即分辨率/抗锯齿需宿主渲染器自行处理（本项目 Android=OpenGL ES、PC=自渲染，需自行 MSAA/超采样）。来源：https://docs.live2d.com/en/cubism-sdk-manual/compatibility-with-cubism-5/
- **着色器**：Cubism 着色器含 Normal/Add/Multiply × Mask/Inverted × PremultipliedAlpha，及 ColorBlend/AlphaBlend 扩展；瞳孔/高光/腮红依赖模型纹理与 Add/Screen 混合层着色器（需 Cubism blending 而非普通 alpha）。本项目 `CubismShaderAndroid` 已实现（见项目内 `docs/architecture/renderer-support.md`，非本次网络调研）。
- **未抓取项**：Neuro‑sama 的帧率 / MSAA 档位 / 眼高光具体着色器 → **未找到公开配置，待人工补充**。

---

## 4. 其他社区「自然化」实践工具清单

| 手段 | 项目 / 链接 | 一句话做法 | 来源 |
|---|---|---|---|
| 预设映射 + 自动呼吸 | **VTube Studio** 模型设置 | 任一输入参数(面部/鼠标)映射到任一 Live2D 输出参数；无输入时开「Auto‑breath」让输出参数据呼吸节奏渐上渐下 | https://github.com/DenchiSoft/VTubeStudio/wiki/VTS-Model-Settings |
| 自动眨眼/表情/注视统一封装 | **moeru‑ai/airi** | `live2dAutoBlinkEnabled`/`live2dIdleAnimationEnabled`/`live2dEyeFocusSourceActive` 等可开关组合 | https://github.com/moeru-ai/airi/blob/911572fe/packages/stage-ui-live2d/src/composables/live2d/motion-manager.ts |
| PixiJS 端 Live2D 框架 | **guansss/pixi-live2d-display** | 内置 eyeBlink 状态机、lipSync、motion 保留、注视/点选 | https://github.com/guansss/pixi-live2d-display |
| 频段驱动口型(能分元音) | **DenchiSoft/Live2DFrequencyLipSync**(Unity) | 频带分割驱动口型，优于仅音量 | https://github.com/DenchiSoft/Live2DFrequencyLipSync |
| 元音置信度口型(Python/DTW) | **organics2016/pymouth** | DTW 匹配元音 + softmax 输出，移动端可跑 | https://github.com/organics2016/pymouth |
| 声音驱动 MotionSync 口型(TS) | **liyao1520/live2d-motionSync** | `live2d-motionsync` 实时口型 | https://github.com/liyao1520/live2d-motionSync |
| 音频驱动口型(Web) | **neka-nat/kugutu** `packages/runtime-web/src/lipsync.ts` | attachAudioLipSync(实时) + mouthCurveFromAudioBuffer(离线 RMS 包络) | https://github.com/neka-nat/kugutu/blob/main/packages/runtime-web/src/lipsync.ts |
| LLM 直接控制 Live2D | **akukanara/live2d-mcp** | MCP 工具 `set_expression`/`play_motion`/`get_model_info` 让 LLM 驱动 | https://github.com/akukanara/live2d-mcp |
| LLM 结构化输出表情混合 | **entropy622/LLM_Live2D** | LLM 返回 `expressionMix`+`parameterOverrides`，自动发现 `.exp3.json` | https://github.com/entropy622/LLM_Live2D |
| LLM 表情/口型/平滑过渡 | **nanlingyin/SoulLink_Live2D** | 按帧生成表情/口型 + easeInOutCubic 平滑 + 环境光照 | https://github.com/nanlingyin/SoulLink_Live2D |
| 开源 LLM VTuber 端到端 | **Open-LLM-VTuber** | LLM→情感空间→表情/动作索引驱动；口型同步 | https://github.com/Open-LLM-VTuber/Open-LLM-VTuber |
| 角色/用户级参数预设 | my‑neuro `emotion_actions.json`、SoulLink model_dict/conf、llmvtuber 文档 | 每角色独占表情/动作组配置 | https://github.com/morettt/my-neuro ；https://docs.llmvtuber.com/en/docs/user-guide/live2d/ |

- **关于「lip-sync-for-live2d」**：独立仓库名「未找到」；但同类口型项目已列出（Live2DFrequencyLipSync / pymouth / live2d-motionSync / kugutu / pixi-live2d-display PR#122“Live2D with Lipsync” https://github.com/guansss/pixi-live2d-display/pull/122）；EasyLive2D/live2d-py 自带 `live2d.utils.lipsync.WavHandler`（Python 侧，适合本 PC）。来源：https://github.com/EasyLive2D/live2d-py/wiki/口型同步

---

## 5. CosyVoice 个性化官方参数与示例

> 背景：项目用阿里云百炼 DashScope 的 **CosyVoice 专属 key（仅限语音）**。以下为官方文档可核对项。

### 5.1 一句话结论
- **支持个性化**。CosyVoice（v2/v3/v3.5 系列）提供：系统音色选择、**声音复刻（克隆任意人声）**、**声音设计（文本描述造音色）**、以及 **`instruction` 指令控制**（方言/情绪/语速/音量/风格）。
- **关键限制（务必注意）**：
  1. **Instruction 非所有音色支持**——仅音色列表标注「支持 Instruct」的系统音色与 v3/v3.5 克隆/设计音色可传 `instruction`；格式不符会报错。
  2. **`cosyvoice-v3.5-plus/-flash` 仅北京地域，且只支持声音复刻+声音设计（无系统音色）**。
  3. **`instruction` 是请求参数能力（非计费项）**，但该专属套餐是否开通克隆/指令权限，文档未见按套餐区分表 → 建议实测一次（§5.5、§7）。

### 5.2 可调参数清单（官方核实）
| 参数 | 说明 | 来源 |
|---|---|---|
| `voice` | 音色：系统音色名 / 复刻音色 ID / 设计音色 ID | https://help.aliyun.com/zh/model-studio/cosyvoice-voice-list ；https://help.aliyun.com/en/model-studio/cosyvoice-tts-python-sdk |
| `rate` | 语速（1=原速，<1 慢 >1 快） | https://docs.qwencloud.com/api-reference/speech-synthesis/cosyvoice/websocket-api ；https://help.aliyun.com/en/model-studio/cosyvoice-ios-sdk |
| `pitch` | 音调（1=原调） | 同上 |
| `volume` | 音量（示例 50） | 同上 |
| `sample_rate` | 采样率（16000/22050/24000…） | 同上 |
| `format` | wav/mp3/pcm 等 | 同上 |
| `text_type` | PlainText 等 | QwenCloud WebSocket API（同上） |
| `enable_ssml` | 是否启用 SSML | https://help.aliyun.com/en/model-studio/cosyvoice-ios-sdk |
| `language_hints` | 目标语言提示（zh/en/fr/de/ja…），**当前只处理第一个元素，建议只传一个**；用于修正数字/符号/小语种朗读 | https://help.aliyun.com/en/model-studio/cosyvoice-tts-python-sdk ；zh：https://help.aliyun.com/zh/model-studio/cosyvoice-tts-python-sdk |
| `instruction` | **个性化核心**：指令控制方言/情绪/语气/语速/音量/角色；**仅支持 Instruct 的音色可用**；≤100 字符 | https://help.aliyun.com/zh/model-studio/cosyvoice-voice-list ；指令示例 https://platform.qianwenai.com/docs/api-reference/speech-synthesis/cosyvoice/java-sdk ；能力清单 https://www.alibabacloud.com/help/en/model-studio/realtime-tts-user-guide |
| `hot_fix` | 文本热修复：指定词自定读音/替换；**不支持 qwen-audio 与 cosyvoice-v2** | https://help.aliyun.com/en/model-studio/cosyvoice-tts-python-sdk |
| `enable_word_timestamp`/`word_timestamp_enabled` | 逐字时间戳（流式），部分音色 | CosyVoice 音色列表 + QwenCloud 文档 |
| `enable_markdown_filter` | 合成前移除 Markdown 符号（仅 cosyvoice-v3-flash） | https://platform.qianwenai.com/docs/api-reference/speech-synthesis/cosyvoice/java-sdk |

### 5.3 `instruction` 官方示例（Java SDK，已抓取）
```
请用粤语说话。（支持的方言：粤语、东北话、甘肃话、贵州话、河南话、湖北话、江西话、
闽南语、宁夏话、山西话、陕西话、山东话、上海话、四川话、天津话、云南话。）
请尽可能大声地说一句话。/ 请尽可能慢地说一句话。/ 请尽可能快地说一句话。/ 请用很轻的声音说一句话。
你能说慢一点吗？ / 你能说得非常快吗？ / 你能说得非常慢吗？
```
- **来源**：https://platform.qianwenai.com/docs/api-reference/speech-synthesis/cosyvoice/java-sdk （Instruction 示例段）。另 CosyVoice2 论文说明 instruction 覆盖情绪/口音/角色风格/细粒度控制：https://funaudiollm.github.io/pdf/CosyVoice_2.pdf (2.6 Instructed Generation)；FunAudioLLM/CosyVoice 支持多语言/方言/情绪/语速/音量指令：https://github.com/FunAudioLLM/CosyVoice

### 5.4 声音复刻 / 声音设计（更强的「个性化」）
- **声音复刻（clone）**：上传音频 URL + `prefix` 前缀 → 建音色 → 合成时 `voice=音色ID`。支持华北2(北京) v3.5/v3/v2/v1、新加坡 cosyvoice-v3-plus；新加坡 `cosyvoice-v3-flash` 暂不支持复刻音色合成。API：`POST .../audio/tts/customization`。
- **声音设计（text-to-voice）**：`voice_prompt`（自然语言描述声音）+ `preview_text`（预览文本）造音色；仅北京 v3.5/v3 系列。
- **来源**：https://help.aliyun.com/zh/model-studio/cosyvoice-clone-design-api ；https://help.aliyun.com/zh/model-studio/voice-design-user-guide ；https://www.alibabacloud.com/help/zh/model-studio/realtime-tts-user-guide
- **结论**：若痛点在「音色不个性化/不标准」，正确路径不是调系统音色，而是**声音复刻（自己的采样）** 或 **声音设计（文本造音色）**，再配 `instruction` 控制语气风格。

### 5.5 对「专属 key（仅语音）」的核验结论
- **结论**：文档层面 `instruction`/`voice`/`rate`/`pitch`/`volume` 均为 CosyVoice 普通请求参数，未标注需额外签证包/额外费用；声音复刻/设计需调 `customization` 接口。**但未找到「专属 key 与通用 key 在指令/克隆上的权限差异表」** →「专属 key 是否已开通指令/克隆」标记为【需账号侧实测/向阿里云确认，见 §7】。
- **可读到的最近说明（地域差异）**：北京 vs 新加坡是否存在复刻音色能力差异、v3.5 仅北京等，来自 https://www.alibabacloud.com/help/en/model-studio/realtime-tts-user-guide 与 https://help.aliyun.com/zh/model-studio/cosyvoice-clone-design-api

---

## 6. 对 Live2D-Ai 落地建议
1. **原型模型直接用官方免费 Hiyori Momose**（与 Neuro‑sama 同款），省自建模。来源 §1.1。
2. **「自然感」= 自动眨眼 + 口型 + 表情过渡 + 多轨分离，别只盯动作文件**：
   - 眨眼状态机（interval ~4s、close/closed/open=100/50/150ms，参考 pixi 默认）。来源 §2.3。
   - 口型：Audio→`ParamMouthOpenY`（RMS）；进阶用频段/元音（pymouth / Live2DFrequencyLipSync）。来源 §2.4。
   - 表情过渡：motion/expression fade + 加算/乘/覆盖。来源 §2.5。
   - 多轨：官方支持多 MotionManager，但**同参只留最后更新的轨**；让 呼吸(ParamBreath)/物理(Physics)/说话(ParamMouth)/表情 尽量分离参数。来源 §2.5。
3. **抗锯齿**：渲染目标 2x 超采样 + 宿主 MSAA；HiDPI 属宿主责任（官方无 SDK 自动）。来源 §3。
4. **PC Python 侧**：优先 `pymouth`(DTW 元音口型) 与 Open-LLM-VTuber 的「LLM 情感→表情/动作」；Android 侧保持 Purism Core + volume→ParamMouthOpenY + CubismJavaFramework 表情 fade。来源 §2.4/§4。
5. **CosyVoice 个性化三选一**：(a) 换「支持 Instruct」音色并传 `instruction`（方言/情绪/语速/音量，≤100 字符）；(b) 声音复刻专属音色（上传采样）；(c) 声音设计（`voice_prompt` 文本）。三者先验证专属 key 权限。来源 §5。
6. **低延迟/多模型**：参考 my‑neuro「1 秒响应、多模型 + 每模型独占 motion/expression 配置」。来源 §1.2/§2、https://github.com/morettt/my-neuro

---

## 7. 未找到 / 需人工补充项
- **Neuro‑sama 实际渲染的 Cubism SDK 精确版本 / Unity rig 细节**：官方只开源「玩游戏 API」，渲染未开源 → **未找到，需向 Vedal/社区确认**。来源 §1.2。
- **Neuro‑sama 帧率 / MSAA / 眼高光着色器档位**：未公开 → 未找到，待人工补充。来源 §3。
- **DashScope「专属 key 仅限语音」是否已开通 Instruct/声音复刻/声音设计权限**：文档无套餐差异表 → **需账号侧实测或向阿里云工单确认**。来源 §5.5。
- **`lip-sync-for-live2d` 独立仓库**：未找到同名独立仓库；同类项目见 §4 清单 → 待人工确认是否为所指项目。
