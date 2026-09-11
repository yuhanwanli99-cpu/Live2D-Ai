# Phase 0 最终侦察 — 开源参数与架构参考

> 2026-08-07 | 侦察员: architect

---

## 1. 可直接复用的社区验证参数

### 眨眼参数对比

| 参数 | 我们当前 | pixi-live2d-display (社区验证) | 建议采用 |
|---|---|---|---|
| 闭合耗时 | 60ms | **100ms** | 100ms |
| 闭合保持 | 50ms | **50ms** | 50ms |
| 睁开耗时 | 80ms | **150ms** | 150ms |
| 总周期 | **190ms (太快)** | **300ms** | 300ms |
| 间隔 | 1.5-6.0s 随机 | **4000ms** 固定 | 3-5s 随机 (折中) |

> **来源**: pixi-live2d-display `Live2DEyeBlink.ts` — 社区数千项目验证的自然眨眼参数

### 表情过渡参数

| 参数 | 我们当前 | 社区标准 | 来源 |
|---|---|---|---|
| fade 时长 | 0.5s (ExpressionManager) | 0.3-0.5s | pixi/SoulLink/LLM_Live2D |
| 缓动函数 | 线性 | **easeInOutCubic** | SoulLink_Live2D |
| 切换时机 | 立即 | **句子间隙** | my-neuro |

### 口型参数

| 参数 | 社区标准 | 来源 |
|---|---|---|
| RMS→mouthOpen | volume→ParamMouthOpenY，attack 20ms / release 80ms | soullink-emotion-sdk |
| 元音映射 | A:(0.8, 0.2), I:(0.3, 0.8), U:(0.2, 0.0), E:(0.5, 0.5), O:(0.6, 0.7) | pymouth (我们PC端已用) |
| 帧长 | 25ms@16kHz (400 samples) | pymouth/pixi |

### Idle 动作参数

| 参数 | 社区值 | 来源 |
|---|---|---|
| 呼吸 | 非周期性 (非正弦) | soullink-emotion-sdk |
| 微动幅度 | ~0.02-0.05 参数单位 | soullink |
| Idle 增益 | 1.05 | soullink |
| 动作重复抑制 | avoidWindow ~5-10s | soullink |
| Idle 动作频率 | 每 5-15s 随机 | my-neuro |

---

## 2. 架构模式参考

### soullink-emotion-sdk 分层模式 (最值得参考)

```
@soullink-emotion/
├── engine          ← 核心引擎：呼吸/眨眼/微动/Idle/口型 (框架无关)
├── runtime-core    ← 无头运行时：Session/TTS/Audio/Planner
├── planner-openai  ← LLM 规划器：反应生成/说话动作
└── sdk             ← meta package
```

> **启示**: 把"渲染层参数动画"抽成纯引擎（无Android/PC依赖），两边共用逻辑

### my-neuro 的情绪-动作映射模式

```
LLM文本 "[开心] 主人今天真好" 
  → parseEmotionTagsWithPosition → {emotion: "开心", charIndex: 0}
  → TTS播放到 charIndex=0 → triggerEmotionByTextPosition
  → 随机选 motionFiles[emotion] → playMotion()
```

### LLM_Live2D 的结构化情感输出

```json
{
  "reply_text": "主人好呀喵~",
  "expressionMix": [
    {"name": "joy", "weight": 0.8},
    {"name": "surprise", "weight": 0.2}
  ],
  "parameterOverrides": [
    {"id": "ParamBreath", "value": 0.7}
  ]
}
```

---

## 3. 可直接借用的开源轮子

| 轮子 | 许可 | 用途 | 集成难度 |
|---|---|---|---|
| pymouth (organics2016) | Apache 2.0 | PC元音viseme | ✅ 已集成 |
| pixi-live2d-display 参数 | MIT | 眨眼/口型/呼吸默认值 | ✅ 直接引用值 |
| soullink-emotion-sdk 设计 | MIT? | 架构分层参考 | 📐 参考模式 |
| LLM_Live2D prompt格式 | - | 结构化情感JSON | 📐 参考格式 |
| my-neuro emotion-motion-mapper | Apache 2.0 | 文本位置触发动作 | 📐 参考逻辑 |

---

## 4. 关键发现修正

1. **我们眨眼太快 (190ms vs 社区300ms)** → 直接替换为 pixi 默认值
2. **我们呼吸是正弦波** → soullink 的非周期性呼吸更自然
3. **我们 Idle 是单循环** → my-neuro/soullink 都是随机多样化
4. **PC 端 pymouth 已集成但前端不消费** → 需要改前端或简化方案
5. **LLM 表情输出可统一为结构化JSON** → 两端共享同一 schema
