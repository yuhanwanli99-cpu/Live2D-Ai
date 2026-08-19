# Live2D-Ai 核心融合引擎调研报告 —— AI ⟷ Live2D 三层融合深度对比

> 调研员：researcher agent（只读）｜日期：2026-08-05｜原则：只打磨核心融合引擎，不调研外围功能；每条结论附来源；抓取失败标「未找到公开信息」；不编造。
> 复用：`docs/research/NEURO_LIVE2D_RESEARCH.md`、`docs/research/benchmark-neko-vs-neurosama.md`、`docs/research/tts-research-report.md`。`文件:行号`= `G:/git/Live2D-Ai` main 分支实测。

---

## 0. 总览（TL;DR）

- **三层融合，Live2D-Ai 当前全在「级1（最浅）」**：级1 表情关键词→硬切、级1 嘴型正弦波、级1 Idle 单循环不跟内容。
- **社区最深可证实基准**：级2/3 表情=`entropy622/LLM_Live2D`（LLM 结构化 `expressionMix`+`parameterOverrides`）；级3/4 口型=`organics2016/pymouth`（DTW 元音置信度，移动端 CPU 可跑）与 Live2D 官方 Motion-Sync（A/I/U/E/O 视素）；级3 动作联动=`my-neuro` 的 `emotion-motion-mapper`（情绪标签带**文本位置**，TTS 播到该字符触发动作）。
- **Neuro-sama 表情实现闭源** → 未见多维情感向量证据，社区共识上限约级2-3：`docs/research/NEURO_LIVE2D_RESEARCH.md:76-96`。
- **关键发现：Live2D-Ai 基础设施并不缺**——Android 已有 `EyeBlinkManager`、`BreathManager`、0.5s fade 的 `ExpressionManager`、Idle Motion（§4）。**缺的是「把 LLM 内容/语音接到这些基础设施、且不打断」的接线逻辑**，多为接线而非造轮子。
- **优先级**：P0＝级1.5 表情平滑化（不硬切、句子间隙切换、修 fallback 硬切）＋级2 口型（音量 RMS 替正弦）；P1＝级3 表情（强度/混合）＋级3 口型（元音，PC 用 pymouth）；P2＝级4 口型＋级3 动作文本位置联动＋Idle 多样化。

---

## 1. 融合链路对比

```
[当前 Live2D-Ai]（Android 与 PC 同模式）
 文本/语音→LLM→[emotion]标签→EmotionController 关键词→index(硬切,fallback无fade)→setExpression
 LLM回复→TTS→isSpeaking开关→LipSyncMath.mouthValue=0.65+0.25*sin(frame)(正弦,不随内容)→setLipSyncParam
 Idle 单循环(startIdleMotion) 不随对话/情绪

[业界最深可证实]（my-neuro / LLM_Live2D / pymouth / Live2D Motion-Sync）
 文本/语音→LLM→结构化情感(强度/混合/参数覆盖)→fade平滑过渡(句子间隙切)
 LLM回复→TTS音频→音量RMS/频段/元音置信度→ParamMouth* 随实际口语开合(分a/i/u/e/o)
 情绪标签带[文本位置]→TTS播放到该字符→触发对应.motion3→动作与内容精确联动
```

---

## 2. 层1：LLM→Live2D 表情（情感映射深度）

### 2.1 分级
| 级 | 描述 | 代表 |
|---|---|---|
| 1 | 关键词→表情索引，硬切换 | **Live2D-Ai 现状**；Open-LLM-VTuber；Neuro 复刻 |
| 2 | fade 平滑过渡，句子间隙触发 | my-neuro `emotion-motion-mapper`；Live2D Expression Motion |
| 3 | 情感强度/混合（expressionMix）+参数覆盖 | `entropy622/LLM_Live2D`；学术 PAD/EVOKE |

### 2.2 业界真实深度
- **主流开源 Vtuber 均「关键词→表达式索引」（级1/2），无多维情感向量落地**。Open-LLM-VTuber `extract_emotion` 纯标签查找：按 `emo_map` key 匹配 `[]` 后 `expression_list.append(self.emo_map[key])`——https://github.com/Open-LLM-VTuber/Open-LLM-VTuber/blob/19b58b1f/src/open_llm_vtuber/live2d_model.py （本文已抓取，:188-214）。DeepWiki 印证「keyword-matching approach based on model_dict.json」：https://deepwiki.com/Open-LLM-VTuber/Open-LLM-VTuber/10.2-emotion-mapping-and-expression-extraction
- **确有「情感强度/混合」开源实现——`entropy622/LLM_Live2D`（实验性前端）**：让 LLM 返回结构化 JSON（回复文本＋`expressionMix`＋可选 `parameterOverrides`）叠加到表情/参数。原句「LLM 返回结构化 JSON，包含回复文本、expressionMix 和可选的 parameterOverrides」「支持表情混合控制」「受 Neuro Sama 启发」——https://github.com/entropy622/LLM_Live2D 。**这是普通用户能体会「同样是 joy、强度不同」的唯一开源产品路径**。注：SRSS 未能二次抓仓库全文，以此 README 为准。
- **多维情感向量（valence/arousal/dominance,PAD）在动作方向有学术/原型，未接到 Live2D 产品链路**：PAD（Pleasure/Arousal/Dominance）https://en.wikipedia.org/wiki/PAD_emotional_state_model ；PAD 驱动动作 https://github.com/HerouFenix/Emotionally-Expressive-Motion-Controller-for-Virtual-Characters ；EVOKE→3D avatar https://arxiv.org/html/2401.06957 。**均未落地主流 Live2D Vtuber → 层级未到产品级，Live2D-Ai 不必直接用**。
- **Neuro-sama 是否用多维向量/强度 → 未找到公开信息（渲染闭源）**；社区共识上限约级2：`docs/research/NEURO_LIVE2D_RESEARCH.md:76-96` 与 `docs/research/benchmark-neko-vs-neurosama.md`。

### 2.3 隐性参数（眨眼/呼吸/动作速度）
- 这些是「自然感」标配基础，**Live2D-Ai 已具备绝大部分**——见 §4。社区参考 pixi-live2d-display `live2dAutoBlinkEnabled`/`live2dIdleAnimationEnabled`：`docs/research/NEURO_LIVE2D_RESEARCH.md:60-72`。
- **差距：这些管理器存在但未被 LLM 内容驱动**（固定节奏），也无「情绪→呼吸/眨眼频率」正反馈。

### 2.4 产品差异
- 普通用户最痛的不是多维向量，而是**切换是否平滑/是否打断说话/频率是否自然**。级1 硬切（尤其 fallback 硬写嘴参）→「表情闪烁跳变」；级2/3 fade+句子间隙切→「情绪过渡自然」。`docs/research/benchmark-neko-vs-neurosama.md` §3.4；`docs/research/NEURO_LIVE2D_RESEARCH.md:172`（闪烁已知痛点）。

---

## 3. 层2：TTS→口型（LipSync 分级）

### 3.1 分级
| 级 | 描述 | 实现 |
|---|---|---|
| 1 | 仅播放状态开关，固定波形 | **Live2D-Ai 现状**（正弦 `LipSyncMath`） |
| 2 | 音量 RMS/分贝→嘴开合 | Live2D 官方 `lipsync-from-wav`；pymouth `DBAnalyser`；live2d-py |
| 3 | 频段/元音→分元音口型（viseme） | Live2D Motion-Sync（A·I·U·E·O+.motionsync3.json）；Live2DFrequencyLipSync |
| 4 | 实时音素/时序精确匹配 | pymouth（DTW 元音+softmax）；OpenFaceFX |

### 3.2 各项目级别
- **Open-LLM-VTuber**：前端 `pixi-live2d-display-lipsyncpatch`（Cubism 3-5），口型能力=pixi 自带（音量级）→ **~级2**：https://open-llm-vtuber.github.io/en/docs/user-guide/live2d/ ；DeepWiki「Audio Payload & Lip-Sync」https://deepwiki.com/Open-LLM-VTuber/Open-LLM-VTuber/5.3-output-transformers-and-audio-payload
- **my-neuro**：`AnalyserNode` 音量/频域→`ParamMouthOpenY` → **~级2**：`docs/research/NEURO_LIVE2D_RESEARCH.md:57-60`。
- **Live2D-Ai**：`VoiceIoController.startLipSync`→`LipSyncMath.mouthValue=0.65+0.25*sin(frame*0.5)`（纯正弦，每100ms）→ **级1**。见 §4。
- **级3/级4 可落地**：
  - **pymouth**（Python，PC 直接可用）：DTW 匹配元音＋softmax 置信度，明确「移动端 CPU 也绰绰有余」「不是AI模型」「VTubeStudio 只是可选 Adapter，可用 Low Level API」。→ **PC 升级3/4 最省力**。https://github.com/organics2016/pymouth 与 https://github.com/organics2016/pymouth/blob/master/README-en.md （本文已抓 README）。
  - **Live2D 官方 Motion-Sync**（级3，Editor/SDK 内置）：语音转 viseme 时序，按 A/I/U/E/O 五元音视素混合生成口型，输出 `.motionsync3.json`；测试模型 Kay 自带。原句「converted into a time-series of visemes and mouth motions... generated by blending the corresponding shapes」「Viseme: Silence, A, I, U, E, O」。→ **模型层是否支持的问题**。https://docs.live2d.com/en/cubism-editor-manual/motion-sync/ 、https://docs.live2d.com/en/cubism-sdk-manual/motion-sync-setting-unity/ 、https://docs.live2d.com/en/cubism-editor-manual/motion-sync-bake/
  - **VTube Studio Advanced Lipsync**（级3）：https://github.com/DenchiSoft/VTubeStudio/wiki/Lipsync
- **Neuro-sama 本体口型实现 → 未找到公开信息（闭源）**；社区复刻约级2。

### 3.3 产品差异
- 级1 正弦=「嘴巴在动」（节奏与人声无关，假）；级2 音量=「嘴巴跟着响动开合」（最低可用自然度）；级3 元音=「嘴巴真的说文里的音」。Live2D 官方承认原始 Cubism lip-sync「只用声音幅度决定嘴开多大」（`docs/research/NEURO_LIVE2D_RESEARCH.md:263-264`），频段/元音更自然：https://github.com/DenchiSoft/Live2DFrequencyLipSync
- **判断**：「级1→级2」观感提升最大且成本最低（正弦换真 RMS）；「级2→级3/4」更精确但边际收益递减、成本上升。

---

## 4. Live2D-Ai 现状印证（一次抓取，成本估算依据）

### Android
| 项目 | 文件:行 | 现状 |
|---|---|---|
| 表情映射 | `EmotionController.kt:24-30` | 8 关键词→索引（neutral/fear/sadness/anger/disgust/joy/smirk/surprise） |
| 触发时机 | `EmotionController.kt:118-166` `parseAndApply`；`:171-183` `setEmotion` | 收到整条 LLM 消息即取最后有效标签**立即硬切**（不等说话间隙）；index 不变才跳过（同类不加强度） |
| 表情主路径（有fade） | `Live2DRenderer.kt:311-346`→`AnimationSystem.kt:964-1060 ExpressionManager` | 预加载走 0.5s fade+weight 混合（`expressionFadeTime=0.5f`，:973） |
| **表情兜底（无fade）** | `Live2DRenderer.kt:1118-1134 setExpressionFallback` | 表达式未预加载时硬写嘴参、无 fade 无叠加 → **闪烁直接来源之一** |
| 眨眼 | `AnimationSystem.kt:371,929,1111-1120` | 已有 EyeBlinkManager+autoBlinkEnabled=true |
| 呼吸 | `AnimationSystem.kt:526,971,1121` | 已有 BreathManager（ParamAngleX）+autoBreathEnabled |
| Idle | `Live2DRenderer.kt:1010-1013,234-236` | startIdleMotion 每模型一次、单循环，不随对话/情绪 |
| 口型 | `LipSyncMath.kt:30-38 mouthValue=0.65+0.25*sin` | **级1 正弦** |
| 口型驱动 | `VoiceIoController.kt:245-296` | isSpeaking→启/停正弦协程，100ms 写 setLipSyncParam（ParamA+ParamMouthOpenY，`AnimationSystem.kt:1120`） |
| 参数冲突 | fallback 写 mouth & LipSync 也写 mouth | 二路径可同时覆盖同一嘴参 → 抖动加剧（需核时序） |
| 触发链路 | `MainActivity.kt:547-578` | setEmotion→speak，先切表情再播语音，说话前硬跳 |

### PC（open-llm-vtuber）
| 项目 | 文件 | 现状 |
|---|---|---|
| 表情驱动 | `src/open_llm_vtuber/live2d_model.py:188-214 extract_emotion` | 关键词→索引（级1） |
| 表情提示 | `prompts/utils/live2d_expression_prompt.txt` | 让 LLM 嵌 `[keyword]` 标签 |
| 提取接线 | `agent/transformers.py:81-88` | 句子→actions.expressions |
| 动作/口型 | 前端 bundle `startRandomMotion`(6)、`setExpression`(10) | 有随机/Idle，未按内容触发；口型约级2（pixi） |

---

## 5. 升级成本估算（难度 S<半天 / M1-3天 / L3-5天；行数=单端净增/改）

### 层1：LLM→表情
| 升级 | 改动点 | 行数 | 难度 | 备注 |
|---|---|---|---|---|
| **1.5 表情平滑化(P0,两端)** | ①修 `setExpressionFallback:1118` 走 fade 或确保全预加载；②「收整条消息立即切」改「说话间隙/下句开头切」+缓存待切表情（`EmotionController:118`/`VoiceIoController`）；③复用 ExpressionManager fade | Android ~150-250；PC ~100-150（前端压缩 JS，改 src 重build） | M | 不造轮子，走通 fade+改句子间隙；**直接命中闪烁**；两端都要改 |
| **3 强度/混合(P1,两端)** | LLM 输出升级为结构化（类别+强度+多表情混合+参数覆盖，参考 LLM_Live2D）；Android `EmotionController` 加 expressionMix（复用 `ExpressionManager.apply`）；PC `live2d_model.py`/`transformers.py` 解析 JSON | Android ~300-500；PC ~250-400 | M-L | 需改两端提示词/输出格式，属核心链路 |
| **3+ 隐性参数驱动(P1,Android)** | 复用 BreathManager/EyeBlinkManager 幅度/频率随情感强度/唤醒度缓变 | Android ~150-300 | M | 复用既存，加「输入→增益」接线 |

### 层2：口型
| 升级 | 改动点 | 行数 | 难度 | 备注 |
|---|---|---|---|---|
| **2 音量RMS(P0,Android)** | TTS 播放器采 PCM RMS 替正弦（`LipSyncMath`/`VoiceIoController:245`）；EdgeTTS 流式边播边采 | Android ~120-200+音频源调研 | M | 成本最低、观感提升最大；需确认各 TTS Provider 暴露 PCM 回调 |
| **2 音量(P1,PC)** | 前端切段交给 Python 侧 RMS/DB（参考 pymouth DBAnalyser），或沿用 pixi | PC ~100-200 | M | PC 前端已约级2 |
| **3 元音viseme(P1,PC)** | `pip install pymouth`（DTW 元音置信度）替水位 RMS | PC ~100-250 集成 | M | **pymouth 是 PC 升级3/4 最省力路径**；Android 无 motion-sync 可先不升 |
| **3 Editor motion-sync(可选)** | 若模型 Editor 导出 `.motionsync3.json`，SDK 自动 viseme 混合 | 0 代码（模型导出）+核 MaoPro/Shizuku 是否含元音参数 | S | 依赖模型支持 |
| **4 实时音素(P2,两端)** | 逐音素匹配（DTW/forced-alignment 时间戳） | 两端各 ~300-600 | L | 收益递减，级3 落地后再说 |

### 层3：动作-内容联动
| 升级 | 改动点 | 行数 | 难度 | 备注 |
|---|---|---|---|---|
| **1.5 Idle 多样化(P1,两端)** | Idle 随机子动作+频率抑制不重复（PC 已 startRandomMotion，移到 Android `startIdleMotion:1013`） | Android ~80-150 | S-M | 消动作重复观感 |
| **3 文本位置触发(P2,PC优先)** | 复刻 my-neuro：情绪标签带字符位置，TTS 进度到该字符触发 .motion3/表情（`parseEmotionTagsWithPosition`+`triggerEmotionByTextPosition`，`docs/research/NEURO_LIVE2D_RESEARCH.md:20-54`）；需字级时间戳 | PC ~300-450 | L | 与层1.5 句子间隙切重叠；EdgeTTS 是否给字级时间戳需核实 |
| **3 动作-语义绑定(P2,两端)** | 动作随情绪/语气绑定，而非 idle/talk 循环 | 两端各 ~200-400 | L | 依赖层1 结构化情感 |

---

## 6. 优先级建议

### P0 —— 必做（成本低，直击「表情闪烁 / 嘴型僵硬」）
1. **层1.5 表情平滑化**：走通现有 fade + 句子间隙切换 + 修 fallback 硬切。M，Android ~150-250 行（两端）。
2. **层2 口型→级2 音量 RMS**：正弦换真 RMS，LipSync 与表情 fallback 嘴参互斥。M，Android ~120-200 行。
   - 理由：均由「已存在基础设施+不改链路」支撑，最小改动换最大自然感；`LipSyncMath`/`EmotionController` 已纯函数/可 mock，易测。

### P1 —— 显著提升（成本适中，命中「整体自然 / 声音有感情」）
3. **层1 级3 情感结构化+强度/混合**：两端提示词 + EmotionController/transformers 解析。M-L。
4. **层2 级3 元音 viseme（PC 用 pymouth）**：Android 先核模型支持，能则升。
   - 理由：`pymouth` 是「能分元音却移动端 CPU 可跑、无 AI 依赖」的罕见低成本方案；结构化情感是「人物不面瘫」灵魂。

### P2 —— 锦上添花（长期，成本高，收益边际递减）
5. **Idle 多样化+随机抑制**。S，Android ~80-150 行。
6. **层2 级4 实时音素**。L，~600-1200 行。
7. **层3 动作-文本位置联动**（复刻 my-neuro emotion-motion-mapper）。L。
   - 理由：上限追求，依赖 P0/P1 铺垫，收益不如前二者明显。

---

## 7. 完整性声明 / 未找到项
- ✅ 分级、各级代表、Live2D-Ai 现状逐行，基于可核证来源（本文抓取 pymouth README、Open-LLM-VTuber live2d_model.py、my-neuro emotion-motion-mapper.js、Live2D 官方 motion-sync 文档；`文件:行号` 为 main 分支实测）。
- ⚠️ **Neuro-sama 是否用多维情感向量/强度/隐性参数** → 渲染闭源，未找到公开信息，按社区共识推断上限级2-3，不猜测。
- ⚠️ **`entropy622/LLM_Live2D` 的 expressionMix 细节**经搜索聚合源交叉一致，但 SSRF 未能二次抓仓库全文，以其 README 为准 → 做层1级3 前建议人工复核。
- ⚠️ 价格/star 为调研时点，可能变动；未涉及外围功能（记忆/视觉/主动陪伴/多形态/TTS 换声）——按任务书要求跳过。
