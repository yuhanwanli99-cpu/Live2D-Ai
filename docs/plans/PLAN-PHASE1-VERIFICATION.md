# PLAN-PHASE1 技术可行性验证报告

> 验证时间: 2026-08-08 | 验证代理: technical-verifier
> 验证对象: docs/plans/PLAN-PHASE1.md（架构师 Phase 1 规划，2026-08-08 产出）
> 结论概览: 每次验证均基于代码实读（双端仓库）+ 官方文档/权威源网络检索双重证据。
> 总体判断: 6 个假设中 4 个通过、2 个部分通过（含风险），无完全"不通过需替代方案"项。
> 但有一个影响全局的重大偏差需架构师知悉（见 §7）。

---

## 假设 1: Android FFT 元音分类可行性 - ✅ 验证通过（但算法需修正为 MFCC+DTW）

### 结论
**通过**。Android 端做元音 viseme 分类**可行**，但架构师描述的"纯 Kotlin FFT 频谱分析 → 元音分类"**不等于** PC 端已验证的 pymouth 算法。真正要复刻的是 **MFCC 特征 + DTW 模板匹配**，单纯 FFT 频谱能量分类精度会显著打折。路线可行，但具体算法假设需修正。

### 证据

**① PC 端 pymouth_viseme.py 算法实读（决定 Kotlin 移植难度）**
读取 pymouth_viseme.py：
- 用 pymouth.VowelAnalyser（纯 Python，Apache-2.0），非自研 FFT，帧长默认 150ms
- VOWEL_MAP 定义 A/I/U/E/O/SIL → (mouth_open, mouth_form)，映射到 ParamMouthOpenY / ParamMouthForm
- 依赖 soundfile + pymouth + numpy，缺失时静默降级返回 []

**② pymouth 核心算法实读（决定移植难度）**
读取 pymouth/analyser.py 的 VowelAnalyser._audio2vowel()，算法链：
1. channel_conversion（立体声取单声道）
2. MFCC 提取：librosa.feature.mfcc(y, sr, n_fft, dct_type=1, n_mfcc=3)[1:].T → 每帧取 2 个 MFCC 系数
3. DTW 模板匹配：与 6 个硬编码模板 V_A/V_I/V_U/V_E/V_O/V_Silence（各 9x2 数值矩阵）算 DTW 距离
4. 带温度 softmax：si_distance → -dist → softmax(temperature=10.0) 得 6 类概率

**关键结论（对 Kotlin 移植）：**
- 官方 README：pymouth 用 DTW 动态时间规划匹配元音 + softmax，不依赖 AI 模型，移动端 CPU 绰绰有余。
- 算法结构简单、可移植：MFCC 的 FFT 可用现成库，DTW 是几十行经典 DP，6 个模板是现成矩阵（已完整取得）。
- 但架构师"纯 FFT → 频谱 → 分类"缺 MFCC + DTW 环节，需在 VisemeAnalyzer 计划补上。

**③ Android 端现状实读**
- Android app/src/ grep viseme|VOWEL|pymouth 零匹配 → 完全没有元音 viseme 逻辑。
- AnimationSystem.kt:1188 setLipSyncValue() + LipSyncMath.kt + Live2DRenderer.kt:357 已建立口型开合度链路（ParamA/ParamMouthOpenY），VisemeAnalyzer 只需产出 viseme 帧 → 口型参数接入。

**④ 现成 FFT 库（网络验证）**
- JTransforms（纯 Java，多线程 FFT，MIT）：轻量、Android 可用。
- TarsosDSP（纯 Java 音频处理）：自带 FFT + 实时分析。

**⑤ Android 拿 PCM 的 API 路径**
- AudioRecord read(byte[]/short[]/ByteBuffer) 拉取 PCM，支持 PCM_16BIT/FLOAT。
- 项目 SherpaOnnxAsrProvider.kt 已现成使用 AudioRecord + recordPcm() 做 ASR。

### 风险
- FFT 频谱分类 ≠ MFCC+DTW 精度，若不修正算法，口型可能退化。
- MFCC 需至少 5 帧（~160ms@16k）才有识别意义，需确认实时性预算。

---

## 假设 2: PC 前端能否修改 - ✅ 验证通过（路径更优：fork 上游 Web 仓库重构建）

### 结论
**通过**。前端确为纯预编译打包产物、无源码。找到了更优路径——上游前端源码在独立仓库 Open-LLM-VTuber/Open-LLM-VTuber-Web，可用 npm run build 自行重构建后替换。

### 证据
**① 本地 frontend/ 形态实读（确认无源码）**
frontend/ 目录：
- assets/main-nu7uwxNJ.js + main-QEkl09-0.css（带 content-hash 的 Vite 打包产物）
- libs/ 第三方库
- index.html 只引用打包 js
- 无 package.json、无 src/、无 vite.config.ts — 全是构建产物。

**② 上游源码仓库（网络验证）**
- Open-LLM-VTuber/Open-LLM-VTuber-Web（122 stars，TypeScript）独立前端仓库，Electron-Vite + React + SWC。
- 支持 npm install → npm run dev / build:win / build:mac / build:linux。

**③ 消费 visemes 的可行路径（实读协议）**
- stream_audio.py prepare_audio_payload 已把 visemes: [{timestamp_ms, vowel_label, mouth_open, mouth_form}] 放进 WS payload。
- 可行路径：fork Open-LLM-VTuber-Web → 在 React 渲染器消费 visemes → 映射口型参数 → 重新 build → 替换 frontend/ 产物。

### 风险
- PC 端 visemes 是整句预计算（timestamp_ms 驱动），前端消费需匹配协议。

---

## 假设 3: Live2D Cubism 是否支持 expressionMix（多表情混合） - ✅ 验证通过（自研层已具备基础）

### 结论
**通过**（基于本项目自研渲染层）。官方 Cubism SDK 原生不支持同时激活多个表情，但 Android 端用的是自研 ExpressionManager（已支持交叉淡变 + Add/Multiply/Overwrite 混合），天然可扩展为多表情权重混合。

### 证据
**① Android 端 ExpressionManager 实读**
- setExpression(expression, intensity)：单表情 + 强度
- 交叉淡变：previousExpression + currentExpression 双权重
- applyExpression(exp, weight) 按 Add/Multiply/Overwrite 三模式叠加到参数
- 核心判断：现有 applyExpression 已是"带权重+blend叠加"通用函数。实现 expressionMix 只需把单表达式泛化为 List<(ExpressionData, weight)> 循环叠加。当前代码已具备 80% 基础设施。

**② Android 端渲染层实读（PurismModel.kt）**
- PurismModel 是自研 JNI 封装（Live2DNative），编译了官方 liblive2dcubismcore.so。
- 提供 setParameterValue 直接控制任意参数，parameterOverrides 可直接用 setParameterValue 落地。

**③ 官方 Cubism 能力（网络验证）**
- 官方文档：Expression Motion blend 有 Add/Multiply，但由 CubismMotionManager 管理、一次播一个。
- 官方明确：要同时播多个 motion，需增加多个 CubismMotionManager 实例。

**④ 降级方案评估**
- 快速切换多表情模拟混合：不推荐，会跳变。
- parameterOverrides：本项目 setParameterValue 可直接用，无需降级，直接在自研 ExpressionManager 扩展。

### 风险
- 多表情叠加可能参数竞争，需定义叠加顺序（Add 先、Multiply 后、Overwrite 最后）。

---

## 假设 4: TTS Provider 音频格式兼容性 - ⚠️ 部分通过（PCM 流未暴露，需改造接口）

### 结论
**部分通过（有风险）**。sherpa-onnx 确实现成返回 PCM（float samples），但 Edge/CosyVoice 返回 MP3、系统 TTS 完全无 PCM；更重要的是所有 Provider 都走"合成 → 临时文件 → MediaPlayer 播放"黑盒，没把 PCM 流暴露给外部做逐帧 FFT。"可以从 TtsProvider 获取 PCM"假设在现契约下不成立，需改造接口或播放链路。

### 证据（4 个 Provider 全部实读）
**① TtsProvider.kt 接口（决定性）**
- speak(text) 只进不出，合成结果内部消化，接口无输出 PCM 通道。

**② EdgeTtsProvider.kt（MP3）**
- WebSocket 收 MP3 → ByteArray → tts_temp.mp3 → MediaPlayer。

**③ CosyVoiceTtsProvider.kt（MP3）**
- DashScope WebSocket 收 MP3 帧 → ByteArray → cosyvoice_tts_temp.mp3 → MediaPlayer。

**④ SherpaOnnxTtsProvider.kt（PCM，唯一例外）**
- SherpaOnnxEngine.synthesize() 返回 SherpaTtsAudio(samples: FloatArray, sampleRate) — 现成的 PCM float。
- 但立刻 writeWav() 转 WAV → MediaPlayer。最易接入 FFT（写 WAV 前截取即可）。

**⑤ SystemTtsProvider.kt（完全拿不到 PCM）**
- TextToSpeech.speak() 直出扬声器，只有 onStart/onDone 回调，只能 AudioRecord 回采。

**⑥ MP3 解码方案（网络验证）**
- MediaCodec（createDecoderByType("audio/mpeg")）官方支持 MP3→PCM。
- MPG123-Android（readFrame() 读 short[] PCM）。

### 可行性
| Provider | 当前音频 | 拿 PCM 难度 | 方案 |
| sherpa-onnx | PCM float | 低 | 写 WAV 前截取 samples |
| CosyVoice | MP3 | 中 | MediaCodec/MPG123 解码，或 AudioTrack 旁路 |
| Edge TTS | MP3 | 中 | 同上 |
| 系统 TTS | 无 | 高 | AudioRecord 回采（不推荐） |

建议：优先 sherpa（离线、PCM现成），MP3 用 MediaCodec；实时逐帧需改造 TtsProvider 增加 PCM 流回调。

---

## 假设 5: LLM 结构化 JSON emotion 输出可靠性 - ⚠️ 部分通过（重大偏差：现有机制是标签式非 JSON）

### 结论
**部分通过（有风险）**。存在与现有生产链路的方向性偏差：
1. Open-LLM-VTuber 现有 emotion 机制是关键词标签（[joy]），不是 JSON 块。
2. DeepSeek json_object 只保证合法 JSON，不保证 schema。
3. 多数 LLM 的 JSON 可靠性靠 JSON Mode + schema 校验 + 重试三层，纯 prompt 引导最不可靠。

### 证据
**① 现有机制实读**
- live2d_expression_prompt.txt：LLM 嵌 [<insert_emomap_keys>] 标签进自然语言。
- live2d_model.py extract_emotion()：逐字符扫 [ 匹配 emu_map 返回表情索引。
- agent/transformers.py actions_extractor：从 SentenceWithTags 提取。

**② emotionMap 已验证（model_dict.json）**
- { neutral:0, anger:2, disgust:2, fear:1, joy:3, smirk:3, sadness:1, surprise:3 }

**③ DeepSeek JSON 能力（网络验证）**
- 支持 response_format {"type":"json_object"} 保证合法 JSON。
- 坑：使用 JSON Output 时必须也在 prompt 指示，否则可能刷空白。
- 不保证 schema，要强 schema 需 function calling/tool 模式。

**④ 通用可靠性结论**
- 三种可靠方案：provider 原生 structured output、解析后 schema 校验、失败重试。纯 prompt 在末尾。

### 建议
在现有标签链路稳定基础上叠加 JSON 解析尝试（失败回退标签），而非 JSON 取代后失败才降级。

---

## 假设 6: 开源项目已验证参数 - ✅ 验证通过

### 结论
**通过**。emotionMap 范例、temperature 经验值均取到手。

### 证据
**① emotionMap 配置范例（实读 model_dict.json）**
- niziiro_mao 完整 emotionMap（8 情感 → 4 表情索引，现名 Niziiro Mao）。

**② temperature 配置（实读 conf.ZH.default.yaml）**
- 各 LLM 默认 temperature 1.0；唯二例外：deepseek_llm 0.7。
- 无专门表情温度，统一用主对话温度。

**③ pymouth 温度验证（实读 analyser.py）**
- VowelAnalyser(temperature=10.0) 默认 10.0，softmax(x/temperature) 高温度使概率更平滑、嘴型更真实。

---

## §7 全局重大发现（超假设范围）

1. PC 端 visemes 是整句预计算非实时逐帧（stream_audio.py 先 TTS 生成→导出 wav→pymouth 全句切帧）。Android 若与 PC 协议一致可同样整句处理；若要实时逐帧 FFT 必须走假设4 的接口改造。两种实现量差异巨大，需架构师定标。
2. Android 端级2-B 随机波动已是现状，VisemeAnalyzer 落地点清晰。
3. 不动区（ChatService/EmotionController/VoiceIoController/Live2DRenderer/PluginManager）不包含 AnimationSystem/TtsProvider，表情混合与 PCM 改造位均允许改动，无契约冲突。
4. sherpa-onnx 是最优 viseme 音频源（PCM 现成、无第三方解码依赖），建议优先对接。

---

## 附：验证命令与证据清单

| 假设 | 证据来源 | 验证方式 |
| 1 | pymouth_viseme.py、pymouth/analyser.py、LipSyncMath.kt、SherpaOnnxAsrProvider.kt、JTransforms/TarsosDSP/AudioRecord 文档 | 实读+网络 |
| 2 | frontend/ 目录、index.html、stream_audio.py、Open-LLM-VTuber-Web 仓库 | 实读+网络 |
| 3 | AnimationSystem.kt、PurismModel.kt、Cubism 官方文档 | 实读+网络 |
| 4 | TtsProvider.kt、Edge/CosyVoice/Sherpa/SystemTtsProvider.kt、MediaCodec/MPG123 文档 | 实读+网络 |
| 5 | live2d_expression_prompt.txt、live2d_model.py、transformers.py、model_dict.json、DeepSeek 文档 | 实读+网络 |
| 6 | model_dict.json、conf.ZH.default.yaml、analyser.py | 实读+网络 |

**总体建议**: PLAN-PHASE1 六个假设方向性可行，但含两项实现期调整——《A》VisemeAnalyzer 应实现 MFCC+DTW 而非纯 FFT、需明确整句预计算 vs 实时逐帧；《B》LLM 表情建议以现有标签链路为基、JSON 作为叠加增强并回退，而非完全替换。PCM 接入需改造 TtsProvider 接口（sherpa 为低成本切入点）。
