# Live2D-Ai V2 自然度修复计划

> 调研人：Orchestrator（直接读源码 + 查 SDK）
> 日期：2026-08-06 | 目标：修表情闪烁 / LipSync 僵硬 / TTS 无个性
> 来源：AnimationSystem.kt / Live2DRenderer.kt / CosyVoiceTtsProvider.kt / dashscope SDK speech_synthesizer.py / morettt/my-neuro / pixi-live2d-display 源码

---

## 1. 三个确认的根因

### 根因 1：表情切换无交叉淡变 → 闪烁

**代码证据**（已核实）：
`AnimationSystem.kt:980-989` — `setExpression(expression)` 直接把 `currentExpression = expression; expressionWeight = 0f`。旧表情立即消失（对应参数跳回默认值），新表情从 0 淡入。

**效果**：切换瞬间人物表情"弹回 neutral"→ 再"淡入新表情"——每切一次闪一次。

**业界做法**（pixi-live2d-display / Cubism 官方）：
- 表情切换 = **交叉淡变（crossfade）**：旧表情 keep 一段时间（weight 1→0）+ 新表情 weight 0→1 叠加
- 或至少保持当前表达式在切换时不跳回默认值

**修复量**：~30 行（在 ExpressionManager 里加 `previousExpression` + 交叉淡变逻辑）

### 根因 2：LipSync 是正弦信号，不是音频驱动 → 僵硬

**代码证据**（已核实）：
`VoiceIoController.kt` 的 `startLipSync` — 播放时 `mouth = 0.65+0.25*sin(frame*0.5)`，不管说什么都一样。

**业界做法**（pymouth / Live2DFrequencyLipSync / 官方 Native LipSync 教程）：
- 最简：音频 RMS/响度 → `ParamMouthOpenY`（音量大口型大，音量小口型小）
- 进阶：频段分析 → 区分元音（不同元音口型不同）
- Android 端可行方案：TTS 播放时用 `MediaPlayer` 的 `getCurrentPosition/getRMS` 或 `AudioTrack` 读取音量帧 → 映射到 mouth open

**修复量**：~60 行（改 `startLipSync` 从正弦 → RMS 驱动，或即时态优化为随机波动 + 停顿模拟）

### 根因 3：CosyVoice 未加 instruction → 标准播音腔

**代码证据**（已核实）：
`CosyVoiceTtsProvider.kt` 的 `sendRunTask` — parameters 里**没有 `instruction` 字段**。但官方 SDK `speech_synthesizer.py:219-221` 明确支持 `instruction` 参数（≤100 字符，指定方言/情绪/语速）。

**验证方法**（本机 SDK 已确认）：
```python
SpeechSynthesizer(
    instruction="用可爱的猫娘萝莉语气说话，声音甜美带点傲娇和撒娇",
    rate=1.2,     # 语速
    pitch=1.1,    # 音调升高
)
```

**修复量**：~15 行（加 instruction 参数 + 从 SettingsRepository 读取）

---

## 2. 三个修复模块（实施顺序）

### 模块 A：TTS 个性化（P0，最快见效）— ~15 行
- `CosyVoiceTtsProvider.kt` 的 `sendRunTask` 加 `"instruction": instruction` 参数
- `SettingsRepository` 加 `cosyvoiceInstruction`（默认 "用可爱的猫娘萝莉语气说话，声音甜美带点傲娇和撒娇"）
- 设置页可选：TTS 指令输入框（调试用）

### 模块 B：表情交叉淡变（P0）— ~30 行
- `AnimationSystem.ExpressionManager` 加 `previousExpression` + 淡出逻辑
- 切换表情时：新表情 `weight = timer/fadeTime`，旧表情 `weight = 1 - (timer/fadeTime)`
- 保留 `expressionFadeTime = 0.5f`（0.5s 交叉过渡）

### 模块 C：LipSync 音频驱动（P1）— ~60 行
- 方案：TTS 播放期间用"随机化波动"替代正弦（`0.4 + random*0.4`），加入停顿间隙（`0.0`）模拟说话节奏→ 比正弦更自然
- 或真 RMS 方案（需要 AudioTrack 交互，稍复杂）留 V2.1

### 模块 D（选）：CosyVoice voice 切换 — ~10 行
- 当前音色 `longxiaochun`（中性女声，偏成熟）→ 替换为更萌的音色（需确认 CosyVoice 音色列表里有哪些可选）
- 可通过 DashScope Models API 获取音色列表

---

## 3. 验收标准（按量化指标）

| 修复 | 指标 | 标准 |
|---|---|---|
| A. TTS 个性化 | 用户主观评分 | MOS ≥3（从 1 分提升到"可接受"） |
| B. 表情交叉淡变 | 视觉检查 | 切换表情不跳回 neutral（录屏对比） |
| C. LipSync | 嘴型自然度 | 不再出现"均匀正弦波"的机器感 |

## 4. 实施 Wave

```
Wave 0: A(TTS个性化) + B(表情交叉淡变) — 并行，无冲突
Wave 1: C(LipSync) — 依赖 A 的播放链路
```

总计 ~105 行新代码。确认后按 SPOQ 走 Developer→Tester 流水线。
