# Live2D AI 猫娘伴侣 —— 量化验收标准调研报告

> 调研日期: 2026-08-06
> 调研员: researcher agent（只读 + 联网抓取）
> 背景: 项目（Android+PC 双端：TTS 语音合成 + Live2D 表情/口型 + LLM 对话）此前只有自动化（单测+链路）与人工感官验收，缺一套量化评分体系。用户要求"少一套量化的标准，去网上搜相关的验收标准"。
> 目标: 覆盖 **TTS / 数字人视觉 / AI 对话 / 端到端体验** 四领域，每个指标给出：指标名、量纲/评分范围、权威来源、最小可执行方案。
> 说明: 报告中每条结论均标注来源（URL 或本地文件:行号）。未在来源确认的具体数值，标注"待测/常识区间"，绝不编造。

---

## 0. 总览：四领域量化指标总表

| 领域 | 指标 | 量纲/评分范围 | 权威来源 | 我们怎么测（最小方案） | 成本/优先级 |
|---|---|---|---|---|---|
| **1. TTS** | 主观 MOS（自然度） | 1–5，P.800 | ITU-T P.800 | 听测 5 句×N 人（§1.1） | 中 / P0 |
| **1. TTS** | 客观 MOS（DNSMOS P.808） | 1–5 近似 MOS | Microsoft P.808/DNSMOS | 脚本批量跑音频 | 低 / P0 |
| **1. TTS** | PESQ/POLQA（MOS-LQO） | 0–5 有参考 | ITU-T P.862/863 | 需参考音频，TTS 不适用→跳过 | 高 / 跳过 |
| **1. TTS** | 可懂度 WER/CER/PER | %越低越好 | arXiv:2006.01463; CosyVoice2 | ASR 识别合成音频对比文本 | 低 / P0 |
| **1. TTS** | 说话人相似度（克隆） | 0–1 cosine | CosyVoice2 论文 | 语音 embedding cosine | 中 / P1 |
| **1. TTS** | 情感/表现力 | 1–5 多维护声; 情感准确率% | P.800; SSW/Interspeech | 表现力专项听测表 | 中 / P1 |
| **1. TTS** | 中文发音/韵律 | 清晰度、正确率 | 《汉语TTS评价方法》; MOE 2006 | 中文专项评测集 | 低 / P1 |
| **2. 数字人** | LipSync 口型延迟 | ms；±45 前导/+125 滞后 | ITU-R BT.1359-1 | 录屏逐帧对齐 onset | 低 / P0 |
| **2. 数字人** | 视听一致感知 | 1–5 | ECCV'24 PEAVS | 打分 + PEAVS 自动 | 低 / P1 |
| **2. 数字人** | 帧率 FPS | ≥30fps 目标；SDK 15/30/60/120 | Live2D 官方 | perf 统计 | 低 / P0 |
| **2. 数字人** | 眨眼频率自然度 | ~12–20次/分(常识) | Live2D SDK; Disney | 60s 录像统计 | 中 / P2 |
| **2. 数字人** | 表情切换平滑 | ms / 1–5 | Live2D 官方 motion 提示 | 过渡时长统计 | 中 / P2 |
| **3. 对话** | 人工对话质量（多维 1–5） | 1–5/维度 | G-Eval; MT-Bench-101 | 20 轮评测表多人打分 | 中 / P0 |
| **3. 对话** | LLM-as-Judge / G-Eval | 1–5 | G-Eval (EMNLP'23) | 脚本调 LLM 打分 | 低 / P0 |
| **3. 对话** | BERTScore | 语义相似度 | arXiv:1904.09675 | 有参考时计算 | 低 / P1 |
| **3. 对话** | 人设一致性 OOC | OOC命中率% | ACL'25 OOC; PersonaArena | 规则+LLM+人工 OOC 检测 | 中 / P1 |
| **3. 对话** | 多轮上下文保持 | 命中率% / 1–5 | MT-Bench-101; MT-Eval | 事实回检法 | 中 / P1 |
| **4. 端到端** | TTFT（首 token） | ms；<1s 目标 | AWS Agentic AI Lens; NVIDIA | 埋点统计分布 | 低 / P0 |
| **4. 端到端** | TTS 首帧 TTFA | ms；<1s，理想<300ms | NVIDIA; Twilio | 埋点 | 低 / P0 |
| **4. 端到端** | 端到端口到耳 | ms；预算≈1.1s | Twilio; softcery | 全链路埋点 | 中 / P1 |
| **4. 端到端** | SUS 可用性 | 0–100（68平均/80良） | measuringU | 10 题问卷 | 低 / P0 |
| **4. 端到端** | CSAT 满意度 | %（75–85%良好） | HubSpot/Zendesk | 交互后 1 题 | 低 / P1 |

> 表列"优先级"：P0=个人开发者成本低、见效快、立即上; P1=中成本、有明确价值; P2=锦上添花。（详见 §6）

---

## 1. TTS 语音合成量化评估

### 1.1 主观 MOS（Mean Opinion Score）—— ITU-T P.800 听测法

**指标定义与量纲**
- **MOS**: Mean Opinion Score，ITU-T P.800 定义的主观话音质量分级量表，**1–5 分**：5=优秀(Excellent)、4=良好(Good)、3=一般(Fair)、2=较差(Poor)、1=很差(Bad)。
- P.800 是"主观传输质量评定方法"权威标准，含听测组织、评测量表、统计报告要求。附 CCR(Comparison Category Rating) 比较评分程序。
- 来源: https://www.itu.int/rec/T-REC-P.800-199608-I（Approved 1996-08-30, In force）
- 证据: "This Recommendation describes methods and procedures for conducting subjective evaluations of transmission quality"；"the addition of an annex describing the Comparison Category Rating (CCR) procedure"。
- **MOS 是 Likert 式问卷**。来源: https://stefan.winklerbros.net/Publications/mmsj2016.pdf（"MOS is a Likert-style questionnaire"）。

**听测组织（样本数/人数/统计显著性）—— 关键实证**
- **>30 名听测者**：测自然度 MOS 需**超过 30 名听测者才达到稳定统计显著性**（基于 Blizzard 2013 数据分析）。
- **<20 人陷阱**：Interspeech 2014 合成语音听测论文中 **>60% 结论基于 <20 听测者**，统计不可靠——论文直接批判的现象。
- 听测报告须**记录量表标签、听测人数、样本数**（Interspeech/SSW 2021-22 多数论文未报告量表标签，是方法论缺陷）。
- 来源: https://www.isca-archive.org/interspeech_2015/wester15c_interspeech.pdf
- 证据: "for a MOS test measuring naturalness a stable level of significance is only reached when more than 30 listeners are used"；"in more than 60% of papers conclusions are based on listening tests with less than 20 listeners"。

**我们怎么测（最小可执行——个人开发者 30 人难凑，给分级）**
- **正式发布级（严谨）**：≥30 听测者；每组 5 句（自然度），混入**人类真实语音黄金基准**（"undistorted human speech is the participants' internal gold standard"—— edlund24）。这是判断 TTS 是否"拟人"的关键参照。
- **个人快速级（最低可接受）**：≥5 听测者（3 天内同组），**同一语音重复测 2 次验证稳定性**，报告置信区间。
- 注意：MOS 只给整体自然度，**不定位错误** → 需配客观指标（1.2/1.3）。

### 1.2 客观指标 —— PESQ / POLQA / DNSMOS / WER

| 指标 | 标准 | 量纲 | 说明 | 我们用否 |
|---|---|---|---|---|
| **PESQ** | ITU-T **P.862** | raw −0.5~4.5；映射 **MOS-LQO 0–5**(P.862.1) | 有参考、偏电信编解码质量，不适合 TTS | ❌ 跳过 |
| **POLQA** | ITU-T **P.863** | ~1–5 | P.862 后继(超宽带)，2024 起替代 P.862；仍有参考 | ❌ 跳过 |
| **DNSMOS** | Microsoft arXiv:2010.15258 / github microsoft/DNS-Challenge | **≈1–5 MOS**（P.808 SIG/BAK/OVRL） | **无参考**非侵入式自动 MOS | ✅ 推荐批量跑 |
| **WER/CER/PER** | arXiv:2006.01463; CosyVoice2 | %（越低越好） | ASR 识别与 GT 算字/词错率，测可懂度/content consistency | ✅ 推荐 |

- 来源:
  - P.862: https://www.itu.int/rec/T-REC-P.862（"P.862... out of date and were deleted on 5 January 2024. Please refer to P.863"）
  - P.863: https://www.itu.int/rec/T-REC-P.863-201803-I
  - P.862.1 映射: https://www.itu.int/ITU-T/recommendations/rec.aspx?rec=7044（"raw P.862 scores -0.5 to 4.5"→MOS-LQO/P.800.1）
  - DNSMOS: https://ar5iv.labs.arxiv.org/html/2010.15258
  - 有/无参考综述（P.861/P.862/P.863/STOI/BSSEval）: https://docs.feishu.cn/v/wiki/Ojc9wtsPhi9LF5kGLEqc7vo8nKe/a6
- CosyVoice2 评测同时用 content consistency(WER)、speech quality(NMOS/DNSMOS)、speaker similarity(SS)。来源: https://arxiv.org/html/2412.10117

**我们怎么测（最小方案）**
1. **DNSMOS 批量打分**：全部候选 TTS 产出的音频批量过 DNSMOS(P.808 无参考)，得 1–5 自动 MOS，作主观 MOS 的廉价代理与回归门禁。阈值需与人工 MOS 标定（提示 ≥3.5 可接受/≥4.0 良好为建议起点，须标定，不视为编造硬值）。
2. **WER/CER 可懂度门禁**：用 ASR（sherpa-onnx / Whisper 中文）识别合成音频，对比输入文本算 WER/CER。取 **≥50 句**多样本（口语/疑问/多音字/网络语各≥10）统计 CER。达标线以我们的 ASR+语料标定。

### 1.3 情感 / 表现力语音（expressive TTS）评估

**问题**：标准 MOS 只评整体自然度，**无法评"是否传达要求的情感/表现力"**。
- 来源: https://www.isca-archive.org/ssw_2021/gutierrez21_ssw.pdf
- 证据: "MOS tests... only offer a general measure of overall quality—i.e., the naturalness of an utterance—and so cannot tell us where exactly synthesis errors occur. This can make evaluation of the appropriateness of prosodic variation within utterances inconclusive."
- 上下文影响：听测对合成 TTS 的主观评价显著受 **contextual framing（上下文框架）** 影响 → 设计听测须固定上下文。来源: https://www.isca-archive.org/interspeech_2024/edlund24_interspeech.pdf

**量化维度（每项 1–5，或用情感分类准确率）**
1. 自然度 Naturalness(MOS)
2. **情感传达准确率 Emotion accuracy**：给定情感标签(高兴/伤心/惊讶/撒娇等)，生成后用人耳或情感识别模型判断命中率 %
3. 表现力/韵律适配 Expressiveness/Prosodic fit：情感表达是否贴切
4. 可懂度（情感语音常牺牲可懂度，需并行复用 WER）

**最小可执行方案**
- "表现力专项听测表"：N 句×M 种情感（建议 3 情感×5 句=15 条），每条听测者给「自然度 1–5」「情感传达准确 1–5」「情感强弱恰当 1–5」。
- 自动替代（辅助，不代表人工）：用中文情感识别模型预测合成语音标签，算 Top-1 命中率。
- 中文人为感基准参考：Audio Turing Test 中文 TTS——https://arxiv.org/html/2505.11200v1

### 1.4 中文 TTS 特有：发音准确 / 儿化 / 轻声 / 韵律

- **中文评测权威**：1994 年起全国评测用**语言清晰度测试**；标准含「语音清晰度(articulation)+语音自然度(naturality)」；语言学模块测切词、多音字、数字串、符号、单位等文本处理能力。
- 典型设计：16 名大学生(男8女8)听写、多点 MOS 测自然度、辅音知觉混淆矩阵诊断。
- 来源:
  - 《汉语语音合成系统评价方法》: https://www.jac.ac.cn/article/doi/10.15949/j.cnki.0371-0025.1998.01.003
  - 教育部/国家语委 2006 文语转换系统评测标准 PDF: https://yywz.sues.edu.cn/_upload/article/files/d4/17/116e9f864a35b845bc5a98c1183b/cb807dfb-57dc-4b83-9bab-491c077fb31e.pdf
  - 证据(后者): "语音清晰度是指输出语音是否容易听清楚，语音自然度是指输出语音听起来是否自然"；"语言学模块评测内容包括切词、多音字、数字串、符号和单位等的文本处理能力的测试"。

**我们怎么测（最小方案）—— 中文专项评测集**
- 构造 ~50 句中文字元测试集：多音字(重/行/乐/数)、轻声(事情/告诉/石头)、儿化(哪儿/一会儿/歌儿)、数字串(电话/日期)、符号(百分号/括号)、歧义断句、网络热词。
- 指标：**清晰度**(听写正确/ASR WER/CER)、**发音正确率 %**、自然度 MOS。
- 客观辅助：50 句跑 ASR 算 CER，检出系统性发音错误（如某 TTS 老读错某多音字）。

---

## 2. 数字人 / Live2D / 虚拟主播视觉验收

### 2.1 LipSync 口型同步（音画同步）量化

**权威阈值（音画同步误差可感知边界）**
- **ITU-R BT.1359-1**：声音相对画面，检测误差边界为 **音频超前视频 +45ms、滞后 +125ms**（感知不对称）。
- 更严工业实践 **ATSC**：音频**绝不超前 >15ms**、绝不超过滞后 **+45ms**。
- 另一参考（DiVAS CVPR'24）：**45ms 音视频差已足以让观众察觉**需人工检查（影视/直播画质缺陷）。
- 来源:
  - ITU-R BT.1359-1: https://www.itu.int/rec/R-REC-BT.1359-1-199811-I/en
  - 汇总转发: https://www.tvtechnology.com/news/managing-lip-sync 证据: "ITU found that errors could be detected at +45ms and -125ms for timing of sound relative to vision... asymmetric... a property of human perception"；"the ATSC recommends that the sound program should never lead the video program by more than 15ms and should never lag... by more than..."
  - https://openaccess.thecvf.com/content/CVPR2024/html/Fernandez-Labrador_DiVAS_Video_and_Audio_Synchronization_with_Dynamic_Frame_Rates_CVPR_2024_paper.html 证据: "Even a discrepancy as short as 45 millisecond can degrade the viewer's experience enough to warrant manual quality checks over entire movies."

**我们怎么测（最小方案）—— LipSync 延迟测量**
1. **自动逐帧对齐**：合成含清晰元音起始的音频；录屏(60/120fps)；对音轨 onset 与画面口型张合 onset 的时间差逐段取偏移。
2. 计 **平均对齐偏移(ms)** 与 **P95 最大偏移**。
3. **验收门禁**：目标平均偏移 **≤45ms**（感知阈值下界）；**任何单句偏移 >125ms 标为缺陷**（BT.1359 滞后上限）。
4. 人耳 20 条"音画是否同步"打分作补充。

### 2.2 视听一致感知评分

- **PEAVS（Perceptual Evaluation of Audio-Visual Synchrony）**：ECCV 2024 提出 **5 点制自动化视听同步质量评分**，基于 100+ 小时人工标注同步错误数据训练。
- 来源: https://www.ecva.net/papers/eccv_2024/papers_ECCV/papers/10198.pdf（"PEAVS... a novel automatic metric with a 5-point scale"）
- **最小方案**：人工对 N 条短视频打 1–5 同步分；≥4 视为好。作 LipSync 量化偏移的感知层校验。

### 2.3 渲染帧率（FPS）标准

- **Live2D Cubism 官方**：物理运算(**physics)计算 FPS 可选 15/30/60/120**，建议与场景 FPS 匹配。
- 来源: https://docs.live2d.com/en/cubism-editor-manual/physics-operation/ 证据: "FPS can be selected from the following four types... 15/30/60/120""matching the FPS of the scene being used"。
- **我们怎么测**：Android 用 Choreographer/FramePacing 统计丢帧率，PC 用 profiler。验收：**目标 ≥30fps 稳定**，60fps 更佳；**<25fps 判定不达标**（个人音画互动基准，标注为本项目目标值，非权威硬标准）。

### 2.4 眨眼频率与表情自然度

- **眨眼自动化**：Live2D 用 `CubismAutoEyeBlinkInput` 周期性驱动眨眼参数。来源: https://docs.live2d.com/en/cubism-sdk-tutorials/eyeblink/
- **眨眼生理参考**：真实人眨眼眼睑运动有速度曲线，需 ease-in-out；（Disney 论文用~300fps 采集人眼）。来源: https://la.disneyresearch.com/wp-content/uploads/Modeling-and-Animating-Eye-Blinks-Paper.pdf —— 论文未在此给平均眨眼次数；**人类静息眨眼频率 ~12–20 次/分钟为通识区间（非本页来源，标注为常识参考）**。
- **表情切换平滑**：Live2D 官方提示"真人不是瞬间停住的，动作需缓入缓出/错帧"，表情若与身体动作完全同步会显机械。来源: https://docs.live2d.com/en/cubism-editor-tutorials/motion-hint/ 证据: "If the face and body move at exactly the same time, it still looks robotic""Humans are not able to stop moving instantly"。
- **最小方案**：录一道 60s 剧情对话，统计：眨眼是否周期性且在静息区间、表情过渡时长是否合理（如 100~300ms，经验区间）、有无跳变；人工 1–5「自然度」复核；P95 表情跳变次数作缺陷计数。

---

## 3. AI 对话 / LLM 交互验收

### 3.1 人工对话质量评测（1–5 分维度）

**权威维度框架（G-Eval + MT-Bench 系）**
- **G-Eval**（EMNLP 2023）：用 GPT-4 做 reference-free 评估，**每维度按 1–5 定制标准**（如 coherence 1–5），用 **chain-of-thought 逐步推理最后给分**。天然适配"相关性/连贯性/人设一致"等人设维度。
  - 来源: https://aclanthology.org/2023.emnlp-main.153.pdf
  - 证据: "Evaluation Criteria: Coherence (1-5) - the collective quality of all sentences... We align this dimension with the DUC quality question"。
- **MT-Bench-101**（ACL 2024）：多轮对话**细粒度**评测基准，覆盖多轮纠错/记忆归因等能力。
  - 来源: https://aclanthology.org/2024.acl-long.401/
  - 证据: "MT-Bench-101, specifically designed to evaluate the fine-grained abilities... overlooked the complexity... of multi-turn dialogues"。

**我们怎么测（人工评测表——20 轮方案）**
- 构造 **5 个固定人设场景 × 4 轮追问 = 20 轮**（猫娘陪伴：打招呼/日常聊天/伤心安慰/俏皮调侃/主动推荐）。
- 每轮由 ≥2~3 名评审按 **5 维度各打 1–5**：相关性、连贯性、人设一致性、有用性、无害性/安全。
- 加 2 道「上下文保持」交叉项：第 4 轮回检第 1 轮事实（见 3.4）。
- 统计各维度均值+单轮标准差；设回归门禁（阈值经多次基线标定，不编造硬值）。

### 3.2 自动化替代：LLM-as-Judge / G-Eval / BERTScore

| 方法 | 来源 | 量纲 | 说明 |
|---|---|---|---|
| **LLM-as-Judge（G-Eval）** | G-Eval EMNLP'23 | 1–5/维度 | 强 LLM 打分，CoT 提一致；适合人设维度 |
| **BERTScore** | arXiv:1904.09675 | 语义相似度 | reference-based，需参考；优于 BLEU/ROUGE 相关 |
| **MT-Bench-101/MT-Eval** | ACL'24/EMNLP'24 | 维度分 | 多轮能力基准 |

- BERTScore 来源: https://arxiv.org/abs/1904.09675v3 证据: "similarity for each token... contextual embeddings... correlates better with human judgments"。
- **可靠性警示**：LLM-as-Judge 有 position/verbosity/self-enhancement 偏差，与人类一致率非 100%；**人设类评分甚至更不可靠** → 个人开发者应**先清洗 bias，用 LLM-as-Judge 做回归冒烟，发版前人工复核**。
  - https://arxiv.org/html/2306.05685v4（"position, verbosity, and self-enhancement biases"）
  - https://aclanthology.org/2024.findings-emnlp.592/（"LLM-as-a-Personalized-Judge is less reliable..."）

### 3.3 人设一致性量化（角色崩坏检测 / OOC）

- **OOC(Out-of-Character) 检测**：逐原子级评估生成内容是否偏离给定人设。来源: https://aclanthology.org/2025.findings-acl.1349/ 证据: "LLMs often exhibit Out-of-Character (OOC) behavior... single scores... struggle to capture subtle persona misalignment"。
- **PersonaArena**（ACL Findings 2026）：动态模拟评测人设级角色扮演，强调在**开放式多轮**测人设而非自报问卷。来源: https://aclanthology.org/2026.findings-acl.471.pdf
- **LLM 裁判判人设并不可靠**：PersonaEval 显示即使 top LLM 在角色判断(role identification)也会失败 → **人设不能只靠 AI 裁判**。
  - 来源: https://openreview.net/forum?id=drdrFhKYjP（"Even top models fail basic role identification, highlighting a need for more human-like reasoning in LLM evaluators"）

**我们怎么测**
- **规则+模型双通道**：a) 规则清单（人设关键词/禁止口吻，如"猫娘不该说'作为AI助手'"）；b) LLM 裁判给 1–5「人设贴合度」。
- 量化为 **OOC 命中占比 %**（20 轮中崩人设回复数÷总回复数）。
- 验收加**人工复核**（AI 裁判不可靠），≥2 人确认 OOC。

### 3.4 多轮对话上下文保持

- 多轮能力须专门评，单轮指标无法覆盖记忆/纠错/指代。来源: https://aclanthology.org/2024.emnlp-main.1124.pdf（MT-Eval 分类多轮交互模式：recall/纠错等）；https://aclanthology.org/2024.acl-long.401/（MT-Bench-101）
- **最小方案（事实回检法）**：4 轮剧本，第 1 轮给关键事实(姓名/喜好/颜色/时间)，第 4 轮问指向该事实的问题。测 **事实回检命中率 %**。每场景 5 组→统计保持率；加 LLM 裁判「多轮连贯性 1–5」。

---

## 4. 端到端体验

### 4.1 延迟指标：TTFT / TTS 首帧 / 端到端

| 指标 | 权威目标 | 权威来源 |
|---|---|---|
| **TTFT(首 token)** | 首响应**数百 ms 内即"fast"** | AWS Agentic AI Lens: https://docs.aws.amazon.com/wellarchitected/latest/agentic-ai-lens/agentperf02-bp04.html（"A response that begins within a few hundred milliseconds typically feels fast"）；AssemblyAI: https://www.assemblyai.com/blog/time-to-first-token-voice-agents |
| **TTFA(TTS 首帧/首音频)** | **<1s**；300–500ms 即时感，500–1000ms 可接受，**>1s 打断感** | NVIDIA: https://perspectives.nvidia.com/nemotron-speech/task/faq/what-end-to-end-latency-should-i-target-for-a-conversational-voice-agent-to-feel/（"Target under 1 second... below roughly 300 to 500 ms feel immediate, 500-1000ms acceptable for a thinking agent, anything above 1 second interrupts"）；smallest.ai |
| **端到端口到耳** | 分项预算 STT≈350+LLM≈375+TTS≈100+网络/缓冲≈**~1.1s**；人类答话间隙~200ms；**>1s 偏慢，1.4s≈7倍偏慢** | Twilio: https://www.twilio.com/en-us/blog/developers/best-practices/guide-core-latency-ai-voice-agents（"STT(350ms), LLM(375ms), TTS(100ms)... ≈1.1s total mouth-to-ear"）；softcery: https://softcery.com/lab/voice-agent-latency-budget-microphone-to-speaker（"Humans hold that gap near 200 ms... a voice agent that takes 1.4s to start replying is roughly seven times slower"）|

**重点（针对本项目）**：本项目是「文字输入 + LLM + TTS 播放」，**无 STT 阶段** → 核心延迟 = LLM TTFT + TTS 首帧。
- 建议验收目标（基于来源区间推导，标注为项目目标非标准）：
  - **TTFT ≤ 800ms(P50) / ≤ 2s(P95)**
  - **TTFA(首个音频字节) ≤ 1s**（直接引 NVIDIA 硬阈值）
  - **端到端(文字提交→首个音频) ≤ 2s(P50)**

### 4.2 可用性 / 满意度：SUS 与 CSAT

**SUS（System Usability Scale）**
- 10 题量表，**0–100**。**68=平均线，80 以上良好(约 B+/A−)，50 以下差**。
- 评分：奇数项−1，偶数项 5−响应，求和×2.5→0–100。
- 来源:
  - https://measuringu.com/sus/（"For odd items: subtract one... For even-numbered items: subtract the user responses from 5... multiply that total by 2.5... 0 to 100"）
  - https://measuringu.com/sample-sizes-for-sus-benchmark-tests/（"below 50 is pretty horrible, 68 is about average, and above 80 is pretty darn good (just above the boundary between B+ and A-)"）
  - 开源工具: https://sus.tools/
- **最小方案**：≥5~8 名用户完成 10 题 SUS，用 sus.tools 算分；验收 **SUS≥68**，发布目标≥72~80。

**CSAT（Customer Satisfaction Score）**
- 1–5 单题满意度转百分比=满意占比。
- **良好基准 75–85%**，>80% 强，<70% 需排查。
- 来源: https://blog.hubspot.com/service/customer-satisfaction-score（"a good score typically falls between 75% and 85%"）；https://www.zendesk.com/blog/customer-experience/loyalty/customer-loyalty/customer-satisfaction-score/；https://getperspective.ai/blog/customer-satisfaction-score-csat-formula-benchmarks-and-limits（"75-85% healthy range, above 80% strong, sustained below 70% a signal to investigate"）
- **最小方案**：每次交互弹 1 题「这次对话满意吗？」1–5；统计满意占比；验收 **CSAT≥75%**（个人产品参考）。--- 评分卡与优先级见续篇 docs/verification/acceptance-metrics-report-part2.md ---
