# 头部与小幅身体动作：运动生态调研（2026-09）

> 委托调研，只读、未修改仓库。目标：为「以后做头部和小幅度的身体」给出**可实施的**技术、
> 参数与同类项目对照。
> 纪律：**每条非显然结论附来源；工程建议与引用严格分开标注；无法验证的写「未验证」。**

- 基线核对：`crates/l2d/src/pose_stack.rs`、`crates/l2d-wasm-demo/src/web/surface.rs`、
  `crates/live2d-ai-core/src/performance/mod.rs`、`crates/live2d-ai-desktop/src/adapter/mask.rs`、
  `crates/live2d-ai-mod-director/src/lib.rs`、`assets/models/bai/runtime/bai.vtube.json`、
  `dist/*/soullink-profiles/bai.profile.json`

---

## 1. 结论与建议优先级

**一句话结论**：这个目标几乎不需要新研究，而是需要「**统一 idle 引擎 + 加性混合层**」的
工程重构，再加一个可选的摄像头 Mod。生态里已有一套生产验证、可抄的参数级方案
（Live2D 官方 `CubismBreath` / `CubismEyeBlink` / `CubismLook` + soullink `IdleEngine`）。

本项目当前缺三样：

1. **统一的、每帧可加的 idle 源** —— idle 现分裂在两处，且两套实现行为不同（§2.7）；
2. **加性混合语义** —— `PoseStack::finalize` 现在是纯覆盖，导致动作与 idle/tracking 只能二选一；
3. ~~幅度标定基准~~ —— **已修复**（见下方「已交付项」）。

| 优先级 | 事项 | 成本 | 依赖 | 风险 |
|---|---|---|---|---|
| **P0-1** | 动作关键帧单位换算 —— **已在本轮修复** | — | — | — |
| **P0-2** | `PhysicsOptions::compatible` → `accurate`（`pose_stack.rs:127`） | 极低（一行） | 无 | 低：观感可能与 VTS 略有差异 |
| **P0-3** | idle 层换成**官方 CubismBreath 五通道** | 低（1 文件） | 无 | 低 |
| **P0-4** | 统一两套 idle 实现 + 补 `ParamBaseX/Y` 重心漂移 | 中 | 无 | 中：触碰渲染面，需行为基线护航 |
| **P1-1** | 语音驱动的头部动作（复用已有 20 ms RMS + 短语边界） | 中 | 无 | 低 |
| **P1-2** | 加性 accent 层 + 跨层 crossfade + 抢占语义 | 中 | P0-3 | 中：混合语义变更影响面大 |
| **P2** | Director 层（Shimeji 三机制 + L4D 张力标量 + Convai 注意力） | 中 | P1-2 | 低 |
| **P3** | 摄像头头部跟踪（OpenSeeFace Mod + One Euro） | 高 | 新 Mod + 外部 Python | 高：外部进程、隐私、平台差异 |

### 已交付项：动作幅度单位换算

wasm 侧 `CHOREOGRAPHY` 的归一化语义值（−1..1）被原样写入模型参数，而 Bai 的
`ParamAngleX/Y/Z` 量程是 ±30 ⇒ `nod` 实际只有 **0.55°**。**本轮已修复**：新增
`crates/l2d-wasm-demo/src/param_scale.rs`（头部 ±30 / 身体 ±10 / 其余透传），
在 `action_frames` 出口统一换算，编舞表保持语义域。

两个后续注意点：

- **(a)** 身体用 **±10**（官方）还是 **±12**（本项目 soullink profile 的值）需定案，
  否则有 20% 比例误差；
- **(b)** `param_scale.rs` 的常量**应改为从 profile / `PoseMap` 查表** ——
  官方文档明确提到头部可设为 ±45（"To rotate a head more than −30 to 30, set −45 to 45"），
  硬编码 ±30 会让 ±45 量程的模型**偏小 1/3**。

---

## 2. 领域 1：程序化待机动作 /「活感」

### 2.1 官方基准：`CubismBreath`（"头 + 身体自然微晃"的官方答案）

共享时间累加器 + 每参数各自周期，**加性**写入
（[cubismbreath.ts](https://raw.githubusercontent.com/Live2D/CubismWebFramework/develop/src/effect/cubismbreath.ts)、
[呼吸 | SDK マニュアル](http://docs.live2d.com/cubism-sdk-manual/breath/)）：

```
// 每帧 _currentTime += dt；t = _currentTime * 2.0 * PI
model.addParameterValueById(id, offset + peak * sin(t / cycle), weight);
// addParameterValueById 是加法 ⇒ 有效幅度 = peak × weight
```

官方 Web/TS 默认参数组（已在 pixi-live2d-display 源码逐字交叉确认，
[Cubism4InternalModel.ts](https://raw.githubusercontent.com/guansss/pixi-live2d-display/master/src/cubism4/Cubism4InternalModel.ts)）：

| 参数 | offset | peak | cycle (s) | weight | 有效幅度 |
|---|---|---|---|---|---|
| `ParamAngleX` | 0.0 | 15.0 | 6.5345 | 0.5 | ±7.5 |
| `ParamAngleY` | 0.0 | 8.0 | 3.5345 | 0.5 | ±4.0 |
| `ParamAngleZ` | 0.0 | 10.0 | 5.5345 | 0.5 | ±5.0 |
| `ParamBodyAngleX` | 0.0 | 4.0 | 15.5345 | 0.5 | ±2.0 |
| `ParamBreath` | 0.5 | 0.5 | 3.2345 | **1**（TS）/ 0.5（C++/Java） | [0,1]（TS） |

**三个关键设计点（全文最重要）**：

1. **周期互不通约**：6.5345 : 3.5345 : 5.5345 : 15.5345 都接近整数但都带 `.0345` 尾数
   ⇒ 多轴叠加后**长时间无视觉可辨的循环点**。**官方自己就是用「多不可通约频率叠加」造"活"感。**
2. **呼吸同时驱动头部三轴微动**（不只在胸口）—— 这是"头 + 身体轻微自然晃"的完整官方答案。
3. **相位连续**：单一共享 `_currentTime`，不随动作/表情重置。各通道独立随机重置会失去
   「同一个生命在呼吸」的观感。

> ⚠️ **不确定**：`ParamBreath` 的 weight，C++/Java 片段给 0.5、TS 给 1。
> 语义上只有 1 得到 [0,1] 区间，倾向 1，但**未找到官方勘误**。

官方第二设施 `CubismHarmonicMotion`（Unity）源码注释明言 "**very useful for faking breathing**"：
`value = origin + range*sin(T*2π/Duration)`，默认 `NormalizedOrigin 0.5 / NormalizedRange 0.5 /
Duration 3 s`（范围 0.01–10 s），混合默认 **Additive**，带 Left/Right 方向钳位
（[CubismHarmonicMotionParameter.cs](https://raw.githubusercontent.com/Live2D/CubismUnityComponents/develop/Assets/Live2D/Cubism/Framework/HarmonicMotion/CubismHarmonicMotionParameter.cs)）。

### 2.2 官方基准：`CubismEyeBlink`

逐行确认自 [cubismeyeblink.ts](https://raw.githubusercontent.com/Live2D/CubismWebFramework/develop/src/effect/cubismeyeblink.ts)：

```
_blinkingIntervalSeconds = 4.0;  _closingSeconds = 0.1;
_closedSeconds = 0.05;           _openingSeconds = 0.15;
// 状态机 First → Interval → Closing → Closed → Opening → Interval
determinNextBlinkingTiming() { return userTime + random() * (2.0 * interval - 1.0); }
// ⇒ interval=4.0 时 next = now + r*7，均匀分布于 [0,7) s，均值 3.5 s
```

写入用 **`setParameterValueById`（覆盖）**。关键工程点：眨眼必须用 `motionUpdated` 门控 ——
动作播放时**跳过眨眼**，避免与动作里写 `ParamEyeLOpen` 的关键帧打架。

文档示例用 `SetBlinkingInterval(6.0~8.0)` + `SetBlinkingSettings(0.8,0.2,0.8)`
（[自动眨眼 | SDK Manual](https://docs.live2d.com/en/cubism-sdk-manual/autoeyeblink/)），
与源码默认不同 —— 两者都是官方允许值，**源码默认最保守**，建议从它起步。
项目现状（`surface.rs`：间隔 3000–7500 ms、闭 75 ms 线性、开 150–300 ms 线性）方向正确。

### 2.3 官方基准：LookAt / `CubismLook`（头部跟随）

官方 Web 样例 `setupLook()`（[lappmodel.ts](https://raw.githubusercontent.com/Live2D/CubismWebSamples/develop/Samples/TypeScript/Demo/src/lappmodel.ts)）
的权重表是**所有实现的共同祖先**：

```
LookParameterData(ParamAngleX,      30.0,  0.0,  0.0)
LookParameterData(ParamAngleY,       0.0, 30.0,  0.0)
LookParameterData(ParamAngleZ,       0.0,  0.0, -30.0)
LookParameterData(ParamBodyAngleX,  10.0,  0.0,  0.0)
LookParameterData(ParamEyeBallX,     1.0,  0.0,  0.0)
LookParameterData(ParamEyeBallY,     0.0,  1.0,  0.0)
```

语义：目标坐标归一化到 **−1..1**，按参数各自的**轴**取值 × Factor。
Unity 版用 `Vector3.SmoothDamp(position, GoalPosition, ref VelocityBuffer, Damping)`
（[LookAt | SDK Manual](https://docs.live2d.com/en/cubism-sdk-manual/lookat-unity/)）。

`pixi-live2d-display` 的两个改进：

```
ParamEyeBallX  += focus.x            // -1..1
ParamEyeBallY  += focus.y
ParamAngleX    += focus.x * 30
ParamAngleY    += focus.y * 30
ParamAngleZ    += focus.x * focus.y * -30   // ← 交叉项：斜向看时才有侧倾
ParamBodyAngleX+= focus.x * 10
```

**两个高价值点**：

- **(a) 全部用 `addParameterValueById`（加性）** —— 跟随是**叠加**在呼吸/idle 之上，
  不是覆盖，**这正是本项目缺失的语义**；
- **(b)** `focus.x * focus.y * -30` 乘法交叉项便宜地造出「看向角落时头会自然歪一点」。

`FocusController` 是一套加速度受限插值器
（[FocusController.ts](https://raw.githubusercontent.com/guansss/pixi-live2d-display/master/src/cubism-common/FocusController.ts)）：

```
EPSILON = 0.01;  MAX_SPEED = 40/7.5;  ACCELERATION_TIME = 1/(0.15*1000);
d = |target-cur|;  maxSpeed = MAX_SPEED/(1000/dt_ms);
a = maxSpeed*dir - v;  maxA = maxSpeed*ACCELERATION_TIME*dt_ms;
if (|a| > maxA) a *= maxA/|a|;  v += a;
maxV = 0.5*(sqrt(maxA² + 8*maxA*d) - maxA);   // 恰好停在目标的刹车速度
if (|v| > maxV) v *= maxV/|v|;  cur += v;
```

比一阶低通跟手且**不会过冲**，值得直接移植。

### 2.4 工程化最完整的一套：soullink `IdleEngine`（MIT，可直接移植）

合成顺序**纯加性**（[IdleEngine.ts](https://raw.githubusercontent.com/nanlingyin/soullink-emotion-sdk/main/packages/engine/src/idle/IdleEngine.ts)）：
`breathing → microMotion → bodySway → gaze → blink → bias`，全部经 `addFACS()` 相加。

**BreathingController**（双频调制，避免机械感）：

```js
modulation = sin(t*0.19*rate + phase) * variance;
cycle      = t*1.65*rate + phase + modulation*0.72;
secondary  = sin(t*0.73*rate + secondaryPhase) * variance;
breath = clamp(0.5 + sin(cycle)*0.31 + secondary*0.045, 0.08, 0.92);
bodyY  = sin(cycle - 0.3)*0.022 + secondary*0.004;   // rate∈[0.5,1.8]；variance 默认 0.42
```

**MicroMotionController**（"活着的最小抖动"，8 相位 × 每轴双频）：

```js
damp = (1 - min(1,focus)*0.65) * gain;    // 注意力越高微动越小
wave(i,f,f2) = sin(t*f*rate[i]+phase[i])*0.72 + sin(t*f2*rate[i+4]+phase[i+4])*0.28;
headX = wave(0,0.38,0.17)*0.02*damp;  headY = wave(1,0.31,0.13)*0.016*damp;
headZ = wave(2,0.24,0.11)*0.014*damp;  // rate[i]∈[0.86,1.14] 随机锁定
```

频率 0.11–0.38 rad/s ≈ **0.017–0.06 Hz**，周期 **16–55 秒** —— 极慢漂移，不是抖动。
**这是「alive 而不 twitchy」的关键。**

**BodySwayController**（随机目标点 + 五次缓动 + 保持，**不是**噪声）：

```js
defaultRanges(归一化): bodyX[-0.045,0.045] bodyY[-0.014,0.014] bodyZ[-0.055,0.055]
                       headX[-0.028,0.028] headY[-0.016,0.018] headZ[-0.034,0.034]
moveDuration = 1.45 + rand*2.35;  holdUntil = now + moveDuration + 0.55 + rand*1.85;
eased = local³(local(local*6 - 15) + 10);        // 五次 smootherstep
// 22% 概率把幅度压到 0.38（制造"大小不一"）
// headX += bodyX*0.32 ; headZ += bodyZ*0.42 （再 clamp）
// focus > 0.5 → recenter(0.06+focus*0.08)，输出权重 ×(1 - focus*0.76)
```

**GazeController**：`gazeX[-0.1,0.1] gazeY[-0.05,0.07]`；
`moveDuration = 0.55 + stability*0.5 + rand*(0.9+stability*0.62)`；
`holdUntil = now + moveDuration + 0.8 + stability*1.25 + rand*(1.7+stability*1.8)`；
stability 默认 0.72。

**BlinkController**（含双眨与焦点抑制）：`close 0.065s / hold 0.035s / open 0.13s`（合计 0.23 s）；
基础间隔 `(3+rand*4)/rate` → 3–7 s；double-blink 概率 `focus>0.4 ? 0 : min(0.24, 0.16*rate)`，
间隔 `0.18+rand*0.12`；`defer(t,duration)` 可在说话/动作期间推迟眨眼（`pauseScale = 1/sqrt(rate)`）。

**IdleBiasController**（动作结束后余韵）：`residue = pow(1-progress, 0.82)`，幂律衰减比线性更"软"。

### 2.5 参数目标与典型范围（**本项目 Bai 模型实测值**）

来源：项目自带 `bai.profile.json`（soullink profile-generator 生成，MIT）。
`idleConfig` 是**归一化语义值**，需乘 `parameterMap.scale`：

| 语义键 | 归一化范围 | 目标参数 | scale | **模型参数值** |
|---|---|---|---|---|
| `headX` | [−0.08, 0.08] | `ParamAngleX` | 30 | **±2.4°** |
| `headY` | [−0.04, 0.04] | `ParamAngleY` | 30 | **±1.2°** |
| `headZ` | [−0.05, 0.05] | `ParamAngleZ` | 30 | **±1.5°** |
| `bodyX` | [−0.045, 0.045] | `ParamBodyAngleX` | 12 | **±0.54°** |
| `bodyY` | [−0.014, 0.014] | `ParamBodyAngleY` | 12 | **±0.17°** |
| `bodyZ` | [−0.055, 0.055] | `ParamBodyAngleZ` | 12 | **±0.66°** |
| `gazeX` | [−0.12, 0.12] | `ParamEyeBallX` | 1 | **±0.12** |
| `gazeY` | [−0.06, 0.08] | `ParamEyeBallY` | 1 | **−0.06 ~ +0.08** |
| `eyeOpen` | [0.9, 1.0] | `ParamEyeLOpen/ROpen` | 1 | 0.9–1.0 |

同 profile 的 `capabilities`：`headControl/bodyControl/eyeBlink/gazeControl/mouthOpen/mouthSmile/
browControl/breath = true`；`eyeSmile/blush/tear/sweat = false` —— 正是本项目能做"头 + 小幅身体"的能力边界。

> **含义**：**"活感"的头部微动只有 ±1~2.5°，身体只有 ±0.5°**；而对话中的"点头"是 ±5~15°（§3.1）。
> **量级差一个数量级 —— 这是「idle 要小、gesture 要大」的量化答案**，
> 也正是单位换算 bug 危险的原因（把 gesture 量级写在 idle 语义域上）。

**官方标准参数范围**（[Standard Parameter List](https://docs.live2d.com/en/cubism-editor-manual/standard-parameter-list/)，
逐字核对）：`ParamAngleX/Y/Z` ±30（可改 ±45）；`ParamBodyAngleX/Y/Z` ±10；`ParamBreath` 0..1；
**`ParamBaseX` −10/0/10（"Move to the right of the screen when +"）**；
**`ParamBaseY` −10/0/10（"Move upward on the screen when +"，带 `*` = 非保证存在）**；
`ParamShoulderY` ±10；`ParamHairFront/Side/Back` ±1（"Usually set by physics"）。

### 2.6 关于 Perlin/simplex 噪声 —— 诚实结论

**没有找到任何 Live2D 生态（官方 SDK、pixi-live2d-display、soullink、VTS 文档）使用
Perlin/simplex 噪声驱动头部旋转的实现或文档。** 不编造"业界用噪声"。

生态实际在用的只有三种：**(1) 多层不可通约正弦**（官方）；**(2) 随机目标点 + 缓动 + 保持**
（soullink BodySway/Gaze，本质是分段随机游走）；**(3) 单个慢正弦**（本项目 `PoseStack` 现状）。

> **工程建议（无引用）**：若要用噪声，**1D value noise 优于 Perlin/simplex** ——
> 需要的是"慢速、平滑、无高频"的漂移，而 Perlin 多 octave 会引入不需要的高频。
> value noise + 三次平滑插值，采样 0.05–0.2 Hz，幅度 ±1–3°，且与官方"多层正弦"不冲突。需实测调参。

### 2.7 本项目 idle 现状：两套实现（必须统一）

| | `l2d::PoseStack`（原生路径） | `l2d-wasm-demo::surface`（`/render` 路径） |
|---|---|---|
| 呼吸 | `ParamBreath`，`cos((π/2)t)` ⇒ **周期 4.0 s** | `0.5 + 0.15·sin(2πt/period)`，周期随机锁定 **[3.1, 5.0] s** |
| 头部摆动 | `ParamAngleZ = sin((π/2)t)·3.0` ⇒ **±3°，周期 4.0 s** | **无** |
| 眨眼 | 无 | 间隔 [3000,7500] ms，闭 75 ms / 开 [150,300] ms |
| 微表情 | 无 | 间隔 [3,6] s，随机 `ParamBrowLY/RY/MouthForm` ±0.05，400 ms 三段 fade |
| 位置 | `pose_stack.rs:181-196` | `surface.rs:604-712`、`apply_idle_life` |

**三个问题**：

1. **`PoseStack` 的呼吸与摆头周期完全相同（都是 4.0 s）⇒ 永远锁相**，长期观看明显机械
   （正是官方用不可通约周期要避免的）；
2. **两条路径能力不对齐**（web 有眨眼/微表情但无头摆；原生有头摆但无眨眼/微表情）；
3. **无 `ParamBaseX/Y`、无 `ParamShoulderY`、无微动漂移** —— "小幅度身体"目前完全没实现。

> 呼吸 4.0 s ≈ 15 次/分，落在成人静息 12–18 次/分（3.3–5.0 s）区间内
> （[MedlinePlus](https://medlineplus.gov/ency/article/002341.htm)）—— **这点是对的，不必改**；
> 要改的是**周期不可通约**与**能力对齐**。

---

## 3. 领域 2：音频驱动的头部动作

### 3.1 最重要的一条实证：**别把宝押在 F0 上**

Ishi, Ishiguro & Hagita, *Analysis of head motions and speech in spoken dialogue*,
Interspeech 2007（[ISCA Archive](https://www.isca-archive.org/interspeech_2007/ishi07_interspeech.html)）。

**前人相关性数据（论文综述原文）**：

| 关系 | 相关系数 / 准确率 |
|---|---|
| 头动 → F0 估计 | 73%（日语）/ 88%（英语） |
| **F0 → 头动估计** | **仅 25%（日语）/ 50%（英语）** |
| F0 vs 头部 6DOF | 英语 39–52%，日语 22–30%（均值 <50%） |
| 点头/歪头 ↔ pitch accent | ~64%；**剔除短语首音节后升至 ~80%** |
| 头动 vs 音高与幅度（日语朗读） | 平均 ~63% |
| 韵律（F0/RMS/一阶二阶导）vs 头部 3DOF | 中性 ~74%，愤怒 ~69% |

**论文原创发现**（30 分钟自由对话，535 短语，动捕 37 标记点）：

- **点头主要出现在短语边界，与边界处词素无关**，而更多与**对话行为功能**相关；
- **反直觉**：疑问句（句末通常升调）中**点头比"上下动/抬头"更频繁** ——
  论文明确说"这是造成 pitch 与头动相关性偏低的原因之一"；
- 摇头只在否定/拒绝表达中出现（样本很少）。

Table 1 列合计（PDF 提取）：`nd` 单次点头 **142**、`fd` 低头 24、`ud` 上下 28、
`fu` 抬头 20、`ti` 歪头 33、`sh` 摇头 4、`nm` 多次点头 3、`no` **无头部动作 189**，合计 443。

> ⚠️ **更正（二次调研流复核后）**：**不要把上表换算成百分比**。Table 1 各行求和 = 443，
> 而语料实为 **535 个短语** ⇒ **该表并非全语料**，除以 443 得到的「32% / 43% / 7%」
> 不成立，已在本文中全面降级为**绝对计数**。
> 并且 **Ishi 2007 全文「Hz」「degree」「seconds」命中数均为 0** ——
> 该论文**没有报告头动频率或幅度**，只有「对话行为 × 头动类型」的次数分布；
> 其中的相关系数全部是**转引他人工作**。

**仍然成立的部分**（可作为设计依据）：

- 点头是**最频繁**的头动类型；
- 点头出现在**短语边界**，与边界处词素无关，更多与**对话行为功能**相关；
- **反直觉**：疑问句（句末通常升调）中**点头比"上下动/抬头"更频繁** ——
  这是 F0↔头动相关性偏低的原因之一；
- 摇头仅见于否定/拒绝表达。

> **可执行结论**：本项目已确立「**一句一单元**，按真实句读边界（`。！？…`）切分」的 TTS 约定。
> **这个句子边界信号就是最好的点头触发器** —— 比从音频估 F0 可靠得多，且零额外 DSP 成本。
> 建议：**短语/句末边界 → 温和点头；否定语义 → 摇头；问句 → 轻微歪头**，
> 再用能量包络调制幅度。

### 3.1b 驱动频带：头动是 **1–2 Hz**，不是音节率 3–4 Hz

- **[有出处]** Pedersen et al. 2022, PLoS Comput Biol：**3–4 Hz 包络 ↔ 嘴部开合（音节率）；
  1–2 Hz 包络 ↔ 跨面部与头部的协同运动（短语/韵律层）**。原文："slower **1–2 Hz**
  modulations are correlated with coordinated motion across the **face and head**"。
  [DOI](https://doi.org/10.1371/journal.pcbi.1010273)
- **[有出处]** 专利 US7349852 给出可直接用的切分：6 个头部信号各拆两带 ——
  **0–2 Hz = 慢速姿态**（跨多音节/词，多为姿势漂移），**2–15 Hz = 与言语相关的快速运动**。
  [链接](https://patents.google.com/patent/US7349852B2/en)
- ⇒ **实现含义：慢分量做 posture drift，快分量做点头起落沿。
  绝不要把 3–4 Hz 的音节率直接灌进头角度。**

### 3.1c 时序锚点：「最大下向速度对齐 F0 峰」

- **[有出处]** Carignan 2024, JASA 156(3):1720–1733（12 名说话人、EMA 250 Hz、116 次显著点头），
  以承载点头的词区间为 100%：gesture onset **0.7%**（中位 2.1%）、
  **gesture stroke（最大下向速度）32.6%**（中位 30.5%）、gesture apex 62.6%（中位 59.0%）、
  **F0 peak 28.1%**（中位 37.2%）。相对 F0 峰：onset 早 28.8%、
  **stroke 晚 4.5%（中位早 0.9%）**、apex 晚 34.5%。原文结论："the onset of the head nod gesture
  is generally aligned with the **onset of the focused word**, whereas the gesture **stroke is
  generally aligned with the F0 peak**."
  [PDF](https://discovery.ucl.ac.uk/10197905/1/1720_1_10.0028585.pdf)
- **[有出处]** 两种策略（k-means 聚类，65 vs 51 项 ≈ **56% / 44%**）：
  Strategy 1 = stroke 在词前开始、apex 对齐 F0 峰；Strategy 2 = stroke 全在词内、
  **最大下向速度对齐 F0 峰**（该策略运动学刚度显著更大，p = 0.024）。
- **[有出处] 旁证**：Habibie et al.（转引自 Nyatsanga 2023 综述）也用 **pitch peaks 确定手势时序**。

### 3.1d 重音与运动基元配比

- **[有出处]** 专利 US7349852 Table 1（语料 = 100 句短句/问候）：`P(^x|*) = 42%` 单点头、
  `P(~x|*) = 18%` **带回弹过冲的点头**、`P(/x|*) = 20%` 单方向急摆 ⇒ **合计 80%** 的重音
  伴随明显头动；"Accents are often underlined with nods that extend typically over
  **two to four phones**"；"their **timing shows surprising consistency**"。
- **[有出处]** 同专利：**停顿后句首**先有**轻微低头**、随后**向上点头**，同说话人 **>70%** 复现；
  且**绕 x 轴（点头）的信号远强于其它轴，y 轴（左右转）也很常见，z 轴（侧倾）显著旋转很少**。

### 3.1e 单次点头 ≈ 0.9 s

- **[有出处]** Mori et al. 2025, PLoS ONE 20(5):e0323448（Chiba 三分钟对话语料，342 分钟，
  **8803 个点头 / 16843 个 cycle**）：**length=1（单点头）占 42%**，最长 19；
  示例 length 1 持续 **0.94 s**，length 5 为 1.53 s。
  [DOI](https://doi.org/10.1371/journal.pone.0323448)（数据开源）
- **[有出处]** 三条结构规律：首 cycle 幅度随 length 增大、幅度随 position 线性下降（b = −0.098）、
  最后一个 cycle 额外减小（c = −0.509）。
  ⚠️ **典型幅度度数在论文 Fig. 5 里，无法从文本提取 —— 不要编造"典型点头 N 度"。**


### 3.2 特征提取（可实现的 DSP）

项目已有可复用基础设施：`ws.rs::build_audio_frames()` **已在按 ~20 ms 切片**、
解码 s16le 单声道、计算 RMS，并随 WS 帧发 `volume`。渲染侧还有峰值包络
`volume_display *= exp(-dt_ms / TAU_MS)`，**`TAU_MS = 90.0`**。
**新增特征只需在同一循环加计算并加 JSON 字段，无新管线、无新依赖。**

| 特征 | 公式 | 建议参数 | 用途 |
|---|---|---|---|
| 短时能量/RMS | `sqrt(mean(x²))` | 窗 20 ms（已有），hop 20 ms | 语气强度 → 点头幅度 |
| 峰值包络 | peak-hold + `v *= exp(-dt/τ)` | **项目已有 τ = 90 ms** | 平滑的音节驱动 |
| F0 | 自相关 / YIN / cepstrum | 窗 30–40 ms，hop 10 ms，搜索 70–400 Hz | 音高斜率 → 侧倾/抬头 |
| 谱质心 | `Σ f·|X(f)| / Σ |X(f)|` | 窗 25–40 ms（需 FFT 或滤波器组） | "明亮度" → 头部微抬 |
| 谱通量（onset） | `Σ max(0, |X_t(f)| − |X_{t−1}(f)|)` | hop 10 ms | 音节起始 → 微点头脉冲 |

> **平滑时间常数（工程建议，无引用）**：音高/能量包络 τ ≈ **150–300 ms**
> （比口型的 90 ms 慢，头部"质量大"）；onset 脉冲用 **60–120 ms** 衰减。

### 3.3 映射建议

以本项目语义键为口径（**下述数值是工程建议，无引用**；幅度量级参考 §2.5 与 §3.1）：

```
// 1) 能量 → 点头幅度
level_norm = (dBFS + 36) / 30            // 复用项目已有的 dB 映射
headY += level_norm_smoothed * 2.0°      // idle 级别；配合短语边界可放大到 4–8°
// 2) 音高斜率 → 侧倾（升调时头微抬+侧倾，降调回正）
pitch_slope = d(F0)/dt，一阶低通 τ=250ms
headZ += clamp(pitch_slope_normalized,-1,1) * 1.5°
headY += clamp(pitch_slope_normalized,-1,1) * 1.0°
// 3) onset → 微点头脉冲（幅度小、衰减快）
on_onset: headY_impulse = -2° ~ -3°，τ = 80 ms 衰减
// 4) 短语/句末边界 → 真正的点头（幅度最大，来自 TTS 分句，非音频分析）
on_phrase_end: nod(depth = 5° ~ 8°, duration ≈ 0.6–0.9 s)
```

**为什么这样分**：idle/语音持续微动应在 **±1–3°**（§2.5 实测），"有意义的点头"是 **±5–15°**。
**混为一谈就会出现"要么死、要么抽"。**

### 3.4 「参考实现」的诚实盘点

| 名字 | 是否做头部动作 | 说明 |
|---|---|---|
| `uLipSync`（hecomi，**MIT**） | **否** | 纯口型。用 **MFCC + 音素 profile 匹配**；有 Job System/Burst、预烘焙、Timeline 集成。**价值在"MFCC 音素识别 + profile 校准"思路，不含任何头部动作**（[README](https://raw.githubusercontent.com/hecomi/uLipSync/master/README.md)） |
| `OVRLipSync` / `wLipSync` / `SALSA` | 否 | 口型专用 |
| "auto head bob" 类 Unity 插件 | **未找到可引用的一手实现** | 检索到的均为 SEO 内容农场页面，无源码，不引用 |
| Live2D 官方 Motion Sync | 否 | 音频 → viseme → 口型；**不含头部** |

> **结论：不存在「音频驱动的头部动作」成熟开源实现可直接抄。**
> 但 §3.1 实证 + §3.2 特征 + 已有 20 ms WS 通道，足以自建，工作量很小。

### 3.5 修订后的实现配方（替换 §3.3 的粗略版本）

```
音频 (16 kHz mono) → 25 ms 窗 / 10 ms hop
  ├─ RMS_dB(t)  → 一阶平滑 τ=80 ms                        → 响度 L(t)
  ├─ F0(t)      → 中值滤波 k=5 → τ=150 ms → 半音 P(t)       → 音高
  ├─ 谱通量 ODF → peak_pick(30/0/100/100 ms, δ, wait)      → onset 事件
  └─ L(t) 带通 1–2 Hz                                      → 短语层驱动 B(t)

重音事件 = P(t) 局部极大，突出度 > 1.5 半音，与上一事件间隔 > 200–300 ms
点头时序 = 最大下向速度对齐 F0 峰
点头波形 = g(t) = (-1)^(i+1)·a·cos(2π·t/(4d)) + c，d 使周期 ≈ 0.9 s
点头配比 = 42% 单点头 / 18% 带回弹过冲 / 20% 单方向急摆

目标参数 = ParamAngleY（**低头为负！**）、ParamAngleZ（tilt）、ParamBodyAngleX
```

**「自然而不抖动」的旋钮**（除标注外均为**工程估计，需真机调参**）：

1. **两级平滑**：一阶低通（attack 40 ms / release 200 ms）→ 二阶临界阻尼弹簧
   `x'' = -k(x-target) - c·x'`，`f_n ≈ 1.5–2.5 Hz`、`ζ ≈ 0.9–1.0`。
   **阻尼不足会摆动，过大则"死板/塑料感"——这是核心旋钮。**
2. **限制带宽**：最终角度低通截止 **~5–6 Hz**（保留点头起落沿所需的 2–5 Hz）。
3. **关键帧量化到 6–10 Hz** 更新目标，弹簧以 60 fps 追踪。
4. **加微抖动**：零均值白噪声 **0.2–0.5°**（避免过糊）。
5. **不应期 200–300 ms**，避免把音节率（3–4 Hz）误当点头率。
6. **三轴的滤波与弹簧参数必须一致**，否则相位不一致会看起来"扭曲"。
7. **绝不把 RMS 直接乘到角度上逐帧刷新** —— 这正是被主观测试判定为 **"jerky"** 的做法。
8. **persona 可调**：说话人内/间差异显著，应暴露"点头频率/幅度倍率"参数。

**Braude 2013 的模板函数**（Interspeech）：

```
g(t, Λ_i) = (-1)^(i+1) * a_i * cos( 2π * t / (4*d_i) ) + c_i
# d_i = 时长, a_i = 幅度, c_i = 偏置（保证段间连续）
# 模板周期 = 4*d_i ⇒ 模板频率 = 1/(4*d_i)
# 段间连续性：g(d_{i-1}, Λ_{i-1}) = g(0, Λ_i)
```

- **模板频率分 slow / medium / fast 三类**，样本数 Slow 30180 / Medium 3471 / Fast 1374
  ⇒ **slow 约占 86%**。
- **关键警告**：主观测试中受试者**强烈偏好模板法**，并把逐帧模型描述为 **"jerky"**；
  且**逐欧拉角独立插值会 jerky 并导致 Gimbal lock**，应在**四元数**上做球面插值。

**Busso 2005 的平滑做法**（可直接照抄）：转四元数 → **降采样到 6 frames/s** →
**squad 球面三次插值**升回 120 fps → 加零均值均匀白噪声避免过糊。

```
slerp(q1,q2,mu) = sin((1-mu)θ)/sinθ · q1 + sin(muθ)/sinθ · q2,  cosθ = q1·q2
squad(q1,q2,q3,q4,mu) = slerp( slerp(q1,q4,mu), slerp(q2,q3,mu), 2mu(1-mu) )
```

**DSP 参数（读自 librosa / aubio 源码）**：

- librosa 默认 `frame_length=2048, hop_length=512` 在 16 kHz 下是 **128 ms 窗 / 32 ms hop
  —— 对头动太慢太糊**；语音惯例是 **25 ms 窗 / 10 ms hop**。
- **包络跟随器** `a = exp(-dt/τ)`；100 fps 下 τ=10/25/50/100/200 ms 对应 a = 0.368/0.670/0.819/0.905/0.951。
- **时间常数 [工程估计]**：RMS 50–100 ms；F0 100–250 ms（压 octave error）；onset 10–30 ms。
- **YIN 约束**：`librosa._check_yin_params` 要求窗内容纳 `fmin` 至少两个周期 ⇒ 80 Hz 需 ≥25 ms 信号。
  实用窗 40–80 ms。**朴素 YIN 是 O(N·max_tau) —— 音频回调里必须用 FFT 加速版并预分配缓冲。**
- **onset peak-pick 三规则**：局部极大 + 自适应滑动均值门限 + 不应期。
  librosa 默认 `pre_max=30ms, post_max=0, pre_avg=100ms, post_avg=100ms, wait=30ms, delta=0.07`；
  在线做法令 `post_max=post_avg=0`（纯因果），**延迟 ≈ 1 个 ODF 帧 + 1 个 hop**。
- **重音尺度**：**1.5 半音的 F0 移动即可改变感知突出度**（Rietveld & Gussenhoven）。
  ⚠️ 常说的"重读元音 4–8 st" **无出处，不要引用**。

### 3.6 负面/警示结果（避免走错路线）

- **[有出处] Hofer & Shimodaira 2007**：帧级 CCA 得到的相关性**远低于** Busso 报告的数值；
  4 类头动识别准确率仅 ~68%（2 类 74–76%）。作者直言特征间"没有强相关"。
- **[有出处] Ben Youssef 2013**：**发音器官特征（EMA）比韵律/倒谱特征与头旋转更相关** ——
  局部 CCA ≥0.2 的比例：发音特征 **70%** vs 声学特征 **<30%**。
- ⇒ **结论：走「韵律触发 + 程序化模板」，不要走端到端回归。**

### 3.7 两条已核实的架构硬结论

1. **Live2D 物理不生成 head bob —— 头角度是物理的输入。**
   核对 6 个官方 `CubismWebSamples` 的 `*.physics3.json`：例 Haru `PhysicsSetting1` 的输入是
   `ParamAngleX`(w60) + `ParamAngleZ`(w60) + `ParamBodyAngleX`(w40) + `ParamBodyAngleZ`(w40)，
   输出是 `ParamHairFront`。⇒ **只要把音频驱动的头动写进 `ParamAngleX/Y/Z`，
   头发/配饰的次级摆动会由物理自动产生，无需额外实现。**
   （唯一例外：**Ren** 把 `ParamAngleX` 映射到 `ParamBodyAngleX2`(Scale 40)，
   即**由头部派生身体跟随** —— 这正是"小幅度身体"的现成范式。）
2. ⚠️ **符号约定**：`ParamAngleX` = 左右转（yaw）、**`ParamAngleY` = 抬头/低头
   （pitch，`+` 为抬头）**、`ParamAngleZ` = 侧倾（roll）。
   **所以「点头（低头）」应写负值** —— 实现时务必先确认符号，否则所有点头方向会反。
3. **生态先例**：VTube Studio 的 Advanced Lipsync 就是 uLipSync，其 Wiki 原文
   "You can use them as inputs for **ANY** Live2D parameter, not just the mouth parameters."
   ⇒ **`VoiceVolume → ParamAngleX/Y/Z` 是官方支持且被预期的用法**。
   uLipSync 的响度归一化曲线可直接照抄：
   `normVol = Log10(rawVolume); normVol = (normVol - (-2.5)) / (-1.5 - (-2.5)); clamp(0,1)`。
4. **[已核实的否定结论]** `SALSA` 的 Eyes 模块有 "Head Configuration"，
   但**输入是注视目标，不是语音/韵律**；**没有任何"音频特征 → 头动"的路径**。
   **NVIDIA Audio2Face-3D** 的输出为 Blendshape / Face Geometry / Tongue / Jaw / Eye Rotation /
   Emotion —— **没有 head rotation**。⇒ 主流工业模型把头部姿态排除在外，头动仍需自己从韵律派生。


---

## 4. 领域 3：摄像头驱动的头部跟踪

### 4.1 最小可行子集：OpenSeeFace（BSD-2-Clause，VTube Studio 的实际后端）

**关键事实**：VTube Studio 的摄像头追踪后端就是 OpenSeeFace —— README 原文：
"**VTube Studio uses OpenSeeFace for webcam based tracking to animate Live2D models**"
（[OpenSeeFace README](https://github.com/emilianavt/OpenSeeFace)）。

可用字段（逐字取自 [OpenSee.cs](https://raw.githubusercontent.com/emilianavt/OpenSeeFace/master/Unity/OpenSee.cs)）：

| 字段 | 类型 | 用途 |
|---|---|---|
| `rotation` (Vector3) | 3D 点拟合的面部姿态旋转向量 | → `ParamAngleX/Y/Z` |
| `translation` (Vector3) | 平移向量 | → `ParamBaseX/Y`（可选） |
| `rawQuaternion` / `rawEuler` | OpenCV 原始旋转导出 | 备用 |
| `leftEyeOpen` / `rightEyeOpen` | 睁眼概率 | → 眨眼 |
| `leftGaze` / `rightGaze` (Quaternion) | 眼球旋转 | → `ParamEyeBallX/Y` |
| `confidence[]` | 追踪置信度 | **丢脸时的降级判据** |
| `points[68]` / `points3D[70]` | 2D/3D 关键点 | 备用/自定义 |
| `got3DPoints` (bool) | 3D 点是否可信 | **不可信时不要用 pose** |
| `features.*` | AU-like（眉/嘴角） | 表情（可暂不用） |

**最小可用子集 = `rotation` + `confidence` + `got3DPoints` +（可选）`leftGaze/rightGaze` +
`leftEyeOpen/rightEyeOpen`。4~6 个字段就够"头跟着你转 + 看得见你"。**

运行参数默认值（逐字取自 [facetracker.py](https://raw.githubusercontent.com/emilianavt/OpenSeeFace/master/facetracker.py)）：

```
分辨率 640x360   fps 24   max-threads 1   model 3（默认，44 fps/核）
detection-threshold 0.6   scan-every 3   discard-after 10
max-feature-updates 900   no-3d-adapt 1  gaze-tracking 1
-M / --mirror-input       ← 镜像输入（桌面宠物场景几乎必须开）
```

性能与成本（README 原文）：模型 −1/0/1/2/3 在**单 CPU 核**上 213/68/59/50/44 fps（单脸）。
"A frame rate of 20 is probably fine and anything above 30 should rarely be necessary."

> **结论：纯 CPU、不需要 GPU、可独立进程。** 与 Mod 边界天然契合。

### 4.2 关键点 → 参数的实际数学（源码级参考实现）

`adrianiainlam/facial-landmarks-for-cubism`（**MIT**，明确为 Cubism SDK 设计），
数学可直接抄（[facial_landmark_detector.cpp](https://raw.githubusercontent.com/adrianiainlam/facial-landmarks-for-cubism/master/src/facial_landmark_detector.cpp)）：

**头部 X（yaw）** —— 把脸建模为球，比较左右脸颊到中轴的垂距：

```cpp
double theta = std::asin((perpRight - perpLeft) / (perpRight + perpLeft));
theta = radToDeg(theta); clamp(theta, -30, 30);
```

**头部 Z（roll）** —— 双眼连线与鼻翼连线倾角平均：

```cpp
angle1 = atan(eyeYDiff/eyeXDiff);  angle2 = atan(noseYDiff/noseXDiff);
return radToDeg((angle1 + angle2) / 2);
```

**头部 Y（pitch）** —— 余弦定理求鼻尖夹角 + 两项校正：

```cpp
angle = solveCosineRuleAngle(c, a, b);
corrAngle = angle * (1 + |faceXAngle|/30 * faceYAngleXRotCorrection);  // 校正 X
corrAngle *= (1 - mouthForm * faceYAngleSmileCorrection);              // 校正微笑
// 再按 faceYAngleZeroValue / UpThreshold / DownThreshold 线性映射到 ±30
```

默认调参（源码 `populateDefaultConfig()`，作者自述 "personally tested"）：

```
faceYAngleCorrection = 10          // 校正显示器与摄像头夹角
faceXAngleNumTaps = 7  faceYAngleNumTaps = 7  faceZAngleNumTaps = 7
mouthFormNumTaps = 3   mouthOpenNumTaps = 3
eyeClosedThreshold = 0.18   eyeOpenThreshold = 0.21       // EAR 阈值
faceYAngleXRotCorrection = 0.15  faceYAngleSmileCorrection = 0.075
faceYAngleZeroValue = 1.8  faceYAngleDownThreshold = 2.3  faceYAngleUpThreshold = 1.3
autoBlink = false  autoBreath = false  randomMotion = false
```

> **注意**：该实现用 **7 点移动平均**（@24fps ≈ 292 ms 延迟），不是 One Euro。
> 作者源码注释坦承坐标 "rather noisy"、"number of taps is determined empirically" ——
> **这正说明为什么应该用 One Euro**：固定窗慢速时欠平滑、快速时过延迟。

### 4.3 滤波：One Euro filter（有官方 Rust 实现）

参考实现就在官方仓库，**且是 Rust**：[casiez/OneEuroFilter](https://github.com/casiez/OneEuroFilter)
的 `rust/` 目录，**BSD-3-Clause**，作者自述 "A faithful port of the reference C++ implementation...
including the 08/2023 fix"。

完整数学（逐行取自 [rust/src/lib.rs](https://raw.githubusercontent.com/casiez/OneEuroFilter/main/rust/src/lib.rs)）：

```rust
alpha(cutoff) = 1 / (1 + tau/te)      where tau = 1/(2π·cutoff), te = 1/freq
// LowPass: s ← a*value + (1-a)*s，首样本透传
if timestamp > last { freq = 1/(timestamp - last) }        // 动态估计采样率
dvalue  = (value - x.last_filtered_value()) * freq         // ← 2023/08 修正
edvalue = dx.filter_with_alpha(dvalue, alpha(dcutoff))
cutoff  = mincutoff + beta * |edvalue|                     // 速度自适应截止
x.filter_with_alpha(value, alpha(cutoff))
```

官方调参程序原文（[1€ Filter 官方页](https://gery.casiez.net/1euro/)，论文 CHI 2012）：
"beta is set to 0 and fcmin (mincutoff) to a reasonable middle-ground value such as **1 Hz**...
**do not hesitate to start with values like 0.001 or 0.0001**."

落地建议：每自由度一个独立滤波器；起点 `mincutoff = 1.0 Hz`、`beta = 0.01`；
**只在 tracker 与渲染之间滤波一次**；**长间隔后必须 reset**（`lasttime = None`），
否则重新出现人脸时 `freq` 估计会得出极低频率产生快速拉拽。

> **为什么不用卡尔曼**：需要噪声模型假设与调参（Q/R），而 One Euro 只有 2 个物理意义明确的
> 参数，且是交互系统事实标准。**未在 Live2D 生态中找到使用卡尔曼的先例。**

### 4.4 延迟预算

| 环节 | 延迟 | 依据 |
|---|---|---|
| 摄像头采集 | 1 帧（@30fps ≈ 33 ms） | 硬件 |
| OpenSeeFace 推理 | @model 3 = 44 fps/核 ⇒ ~23 ms | README |
| 固定窗移动平均（若沿用参考实现） | ~7 帧 @24fps ≈ **292 ms** | 源码 `numTaps = 7` |
| **改用 One Euro 后** | ~30–80 ms | 参数相关，需实测 |
| UDP loopback + 序列化 | < 5 ms | — |
| **合计（目标）** | **~70–130 ms** | 工程估计 |
| 视觉可接受上限 | 一般 < 150 ms 才不显拖影 | **无引用，经验值** |

**结论：延迟预算完全够。** 固定窗移动平均是主要延迟来源。

### 4.5 落地方案：架构与许可

```
[可选 Mod: live2d-ai-mod-face-tracking]
  启动子进程: python facetracker.py --model 3 -F 30 -M --no-3d-adapt 1 ...
      ↓ UDP 11573 → loopback
  Rust 侧: 解析 68 点包（字段布局见 facial_landmark_detector.cpp 的 packetFrameSize 计算）
      ↓ rotation(→AngleX/Y/Z) + gaze(→EyeBallX/Y) + eyeOpen + confidence
  One Euro 滤波（BSD-3，vendored）→ 写 PoseStack input 层
```

- **独立进程的理由**：OpenSeeFace 是 Python + ONNX Runtime。把 Python 关在进程外，
  Rust 核心零 Python 依赖，崩溃不影响主链路；也正是 VSeeFace 的做法（UDP 设计
  "allows tracking to be done on a separate PC... to **avoid accidentally revealing camera footage**"）。
- **必须 `-M` 镜像**，否则左右相反。
- **降级**：低 `confidence` 或 `got3DPoints == false` → **crossfade 回 idle**，不要瞬间回中。
- **隐私**：UDP 只发数值不发图像，符合"密钥不出环 / loopback-only"红线。
- **量程适配**：写参数前乘模型实际量程，**不要硬编码 ±30**。

| 组件 | 许可 | 可商用 | 备注 |
|---|---|---|---|
| OpenSeeFace 代码与模型 | **BSD-2-Clause** | ✅ | 分发需带 `Licenses/` |
| facial-landmarks-for-cubism | **MIT** | ✅ | 其 example 是 Live2D 样例的补丁 |
| One Euro Filter（Rust） | **BSD-3-Clause** | ✅ | vendored 即可 |
| MediaPipe / Face Landmarker | Apache-2.0（代码） | ✅ | 模型许可需另查 |

> **极佳的 P1 级中间态**：`adrianiainlam/mouse-tracker-for-cubism` —— 同作者用
> **鼠标光标跟踪 + 音频口型**替代人脸，README 明确 "The main advantage is a much lower CPU load"。
> **不引入摄像头与 Python 就能实现"看着你的光标"**，与 §2.3 的 Look 权重表完全兼容。

### 4.6 其它跟踪方案

| 方案 | 输出 | Live2D 映射参考? | 备注 |
|---|---|---|---|
| **OpenSeeFace** | 68 点 + rotation + gaze + AU-like | ✅ facial-landmarks-for-cubism | 首选：BSD-2、CPU、VTS 同款 |
| **MediaPipe Face Landmarker** | 478 点 + **52 blendshape + 4×4 变换矩阵** | ✅ kalidokit（已废弃） | `LIVE_STREAM` 模式；`output_facial_transformation_matrixes` 默认 false |
| **kalidokit** | Face/Pose/Hand 运动学 | ✅ 专为 VRM + Live2D 设计 | ⚠️ **已废弃**：README 顶部 "officially deprecated... integrated into MediaPipe" |
| **iFacialMocap / MeowFace / Facemoji** | iPhone ARKit 52 blendshape | VTS/VSeeFace 均支持 | 精度最高，需 iPhone + 多为付费 App |
| **VRCFaceTracking** | VR 头显 + 眼追 | 面向 VRM/VRCFury | 与 2D Live2D 场景不匹配 |

---

## 5. 领域 4：小幅度身体动作与次级运动

### 5.1 「重心移動」的正确通道：`ParamBaseX/Y`，不是 `ParamBodyAngleX`

**本轮最有价值的一条纠正。** 语义不同：`ParamBodyAngleX/Y/Z`（±10）= **身体旋转（倾角）**；
`ParamBaseX`/`ParamBaseY`（±10，"Move to the right / upward on the screen when +"）=
**整体平移 = 重心移动**。

**物理正确的组合 = 倾角 + 位移同相位叠加**：`ParamBodyAngleX` 做躯干倾角（±0.5~1°），
`ParamBaseX` 做重心平移（±2~4）。呼吸的肩部起伏通道是 `ParamShoulderY`（±10）。

> 工程建议（**无引用，推断**）：重心漂移周期 **10–16 s**（参考官方 `ParamBodyAngleX`
> cycle 15.5345 s）、幅度 ±2~4。这是"有官方设施支撑的推断"，**不是官方明文建议**。
> ⚠️ `ParamBaseY` 带 `*` = **非保证存在**。

### 5.2 把头部运动喂进 physics3.json —— 会不会抖坏？

**(A) 官方实现：不会数值发散。** `CubismPhysics` 五重保护
（[cubismphysics.ts](https://raw.githubusercontent.com/Live2D/CubismWebFramework/develop/src/physics/cubismphysics.ts)）：

1. **输入时间插值预滤**：`inputWeight = physicsDeltaTime/_currentRemainTime;`
   `_parameterCaches[j] = _parameterInputCaches[j]*(1-inputWeight) + parameterValues[j]*inputWeight;`
2. **输出时间插值** `interpolate(model, alpha)`；源码注释明言目的是 "avoid the **quivering
   appearance** caused by deviations from the interpolation range"；
3. **输出硬钳位**到目标参数 min/max；
4. **每步硬长度约束** `position = parent + normalize(dir) * radius`；
5. **`MaxDeltaTime = 5.0` 累加器重置**（掉帧后直接跳过，不补算）。

**(B) 但本项目用的是 `ayagami`，行为特征不同**
（[ayagami/src/physics.rs](https://raw.githubusercontent.com/AyagamiDev/ayagami/main/ayagami/src/physics.rs)）：
RK4 角度 ODE，**没有官方的输入 lerp 预滤，也没有输出插值**；抗抖只靠 `min_fps = 50` + RK4 阻尼。
其 `rotation_boost` 源码注释明确警告：

> "This causes pendulums to start moving faster when the input angle changes, but also means
> **extra energy materializes out of nowhere**... a pendulum with length 10, delay 0.5,
> acceleration 0.5, and mobility 1.0 will gain enough energy from a **45 degree input angle
> change to swing back over 180 degrees on the other side and loop around**. ...
> The compatibility value is 0.2."

三套预设：`compatible`（rotation_boost **0.2**）/ `useful`（0.2，gravity_lookahead +
angular_momentum_loss 均 false）/ `accurate`（**0.0**，`Normal` 映射）。
**本项目实际用 `compatible`**（`pose_stack.rs:127`）。

**(C) 直接回答"用噪声驱动 ParamAngleZ@60fps 稳不稳"：稳（不发散），但会"看起来抖"。**

以 `compatible`、`world_fps=60`、`delay=1` 推演：每渲染帧摆链收到一次**阶跃式** `g_angle`，
`Δv = Δθ_rad × 0.2 × 60`。噪声 ±3°（±0.052 rad）⇒ 每帧注入 Δv ≈ **0.63 rad/s**；
官方示例参数（acceleration=1, radius=5）下 `ω_n = √(900×1/5) ≈ 13.4 rad/s`
⇒ 单帧激励到约 4.7% 满幅。每帧随机方向注入 ⇒ **发丝持续高频颤动的 chatter**。
但有阻尼与硬长度约束，**振幅有界，不会累积到发散**。

**(D) 一个"天然保护"**：`normalizeParameterValue` 把输入映射为**相对模型参数范围中点的比例**。
所以 **±2° 的 idle 头动对物理只产生 ~6.7% 归一化输入** —— 小幅 idle 噪声几乎不搅动物理。
反过来说：**"靠小幅 idle 噪声让发丝微动"是低效路径**；要发丝动，应显式增大头动幅度，
或在 `physics3.json` 里给 `ParamBodyAngleX` 更高 Weight。

**physics 处置清单（按依据强度排序）**：

| # | 措施 | 依据强度 |
|---|---|---|
| 1 | **`compatible` → `accurate`**（或至少 `rotation_boost` 降到 0–0.05） | 强（ayagami 源码注释明说会"凭空产生能量"） |
| 2 | 确认 `physics3.json` 有 `Meta.Fps`（**官方推荐 60**）；无字段时行为随渲染 FPS 漂移 | 强（官方 4.2 兼容性文档） |
| 3 | 程序化头动**限带 ≤2–3 Hz**，写入前先一阶低通 | 中（"快于 tick 率的输入会被阶梯化"是源码事实；频段是工程建议） |
| 4 | 加载后参数设为非默认值，**首帧调用 `settle()`**（项目已在 `attach_physics_json` 做了 ✅） | 强（官方 `stabilization()`） |
| 5 | **每帧只调一次 `PhysicsEngine::update()`** | 强（ayagami 源码，重复调用正反馈） |
| 6 | 用**分组**近似 VRM 的 **center space**：头发组只接 `ParamAngleX/Y/Z`，衣摆/挂饰组只接 `ParamBodyAngleX/Y + ParamBaseX/Y` | 中（VRM 规范原文明确 center space 用途） |
| 7 | `final_override` 层**不要**写 `ParamHair*`（物理独占） | 强（官方标准表标注 "Usually set by physics"） |

> ⚠️ **不得宣称**："官方建议喂物理前先平滑输入"。**未找到任何此类官方表述**；
> 官方是在 SDK 内部解决（输入 lerp + 输出插值）。

### 5.3 次级运动的数学模型（两族）

**(A) Verlet + Hooke「尾点」族** —— Unity SpringBone 与 VRM `VRMC_springBone` 1.0
（[VRM 规范](https://raw.githubusercontent.com/vrm-c/vrm-specification/master/specification/VRMC_springBone-1.0/README.md)，
原文写明用 verlet integration）：

```
inertia   = (currentTail - prevTail) * (1.0 - dragForce)
stiffness = deltaTime * parentWorldRotation * initialLocalRotation * boneAxis * stiffnessForce
external  = deltaTime * gravityDir * gravityPower
nextTail  = currentTail + inertia + stiffness + external
nextTail  = worldPosition + (nextTail - worldPosition).normalized * boneLength   // 硬长度约束
prevTail = currentTail; currentTail = nextTail
node.rotation = initialLocalRotation * fromToQuaternion(boneAxis, to)
```

Unity SpringBone 默认：`stiffnessForce 0.01`、`dragForce 0.4`、`springForce (0,-0.0001,0)`、
`radius 0.05`（[SpringBone.cs](https://raw.githubusercontent.com/unity3d-jp/UnityChanSpringBone/1.2.1-preview/Runtime/SpringBone.cs)）。

**VRM 的 Center Space 对本项目极有价值**（规范原文）："`center` is effective... when SpringBone
is shaking too intense... **When you want to move SpringBones attached to the head of the model
(e.g., hairs, hair ornaments) only when moving its head**" —— 即"只想让头动时头发才动"。
Live2D 没有 center space，但可用**分组**近似（§5.2 措施 6）。

**(B) 角度链 + 阻尼「摆」族** —— Live2D `physics3.json`。官方 Editor 语义
（[About Physics](https://docs.live2d.com/en/cubism-editor-manual/physics-operation/)）：
`vertices[].Mobility` = "Ease of swinging"，**官方建议 0.7–0.99**；`Delay` = "Reaction time"（1 为标准）；
`Meta.Fps` = "**default value of 60 fps is recommended**"；`Normalization.Angle` 默认 **−10 / 0 / 10**；
官方说明归一化目的是"跨模型一致的重力对齐"，**不是抗抖动**。

### 5.4 运动混合：官方公式与「参数归属」

**官方混合公式**（[Motion Blending](https://docs.live2d.com/en/cubism-editor-manual/motionblending/)，原文）：

```
Override: (s * w) + (d * (1 - w))
Additive: (s * w) + d
```

官方警告：**"Motion is applied to the model in the order of the values on the left. Therefore,
depending on the order, the previous value may be overwritten."**

**官方 fade 曲线**（[acubismmotion.ts](https://raw.githubusercontent.com/Live2D/CubismWebFramework/develop/src/motion/acubismmotion.ts)）：

```
fadeWeight = weight * fadeIn * fadeOut
getEasingSine(v) = v<0 ? 0 : (v>1 ? 1 : 0.5 - 0.5*cos(v*PI))   // 余弦 S 曲线，不是线性
```

官方 fade-in/out **默认 1 秒**；明文建议 **"no significant movement be added during the fade-in"**；
且「fade 会截断物理：The physics are also **cut off**」。

**官方"多 MotionManager 并行"教程**（[multi-motion-management-web](https://docs.live2d.com/en/cubism-sdk-tutorials/multi-motion-management-web/)）：
官方 SDK 不支持并行 motion，必须手工准备多个 manager + 明确「**参数归属**」
（Assign Responsible Parameters），否则"低优先级 idle 会在高优先级动作结束后立刻把参数拉回来"。
官方结论："**It is important to clearly define the specifications before creating the motion.**"

> **对项目的映射**：本项目的分层栈**在架构上比官方 SDK 更清晰**。建议：`idle` 独占
> `ParamBreath`/`ParamBodyAngleX` 慢频/`ParamBaseX/Y`/`ParamShoulderY`；`input` 独占
> `ParamAngleX/Y/Z`/`ParamEyeBallX/Y`；`physics` 独占 `ParamHair*`/`ParamBust*`/衣摆；
> 层内用 `Override`，跨层 **idle/breath 用 Additive、canned action 用 Override**；
> **canned action 不应写 `ParamBreath`/`ParamBaseX/Y`/`ParamHair*`**。

**`pixi-live2d-display` 优先级仲裁**（[MotionState.ts](https://raw.githubusercontent.com/guansss/pixi-live2d-display/master/src/cubism-common/MotionState.ts)）：

```
enum MotionPriority { NONE=0, IDLE=1, NORMAL=2, FORCE=3 }
// IDLE：当前有任意 motion 在播 → 拒绝；已有 idle 被 reserve → 拒绝
// 非 IDLE：priority < FORCE 时，priority <= currentPriority → 拒绝；<= reservePriority → 拒绝
// ⇒ 只有 FORCE 能无条件打断
```

**默认 fade 时长（值得直接照抄的非对称设计）**：`motionFadingDuration 500ms`、
**`idleMotionFadingDuration 2000ms`**、`expressionFadingDuration 500ms`
（[config.ts](https://raw.githubusercontent.com/guansss/pixi-live2d-display/master/src/config.ts)）。
理由：idle 是持续背景运动，短 fade 会在每次切换时产生可见顿挫。

---

## 6. 领域 5：Director 层（何时做动作）

### 6.1 项目现状与问题

`crates/live2d-ai-mod-director/src/lib.rs`：`auto_sequence_on_turn` 为 true 时**每次 turn 开始
按固定 `config.sequence` 播放**（默认 `["nod","shake_no"]`），间隔 `interval_ms = 600`。
**这是"每轮都说同样两句话"的模式 —— 典型 slot machine 感来源**（固定序列、无冷却、
无去重、无幅度变化、与语义无关）。

### 6.2 商用对标：VTube Studio 的六级值提供者优先级

[官方 wiki](https://raw.githubusercontent.com/wiki/DenchiSoft/VTubeStudio/Interaction-between-Animations,-Tracking,-Physics,-etc..md)：

| 优先级 | 值提供者 |
|---|---|
| **P0** | Live2D 参数默认值 |
| **P1** | Idle Animation 值 |
| **P2** | **Face Tracking 值** |
| **P3** | One-Time Animation（动作播放期间） |
| **P4** | Live2D Expression（表情激活期间） |
| **P5** | **Physics System** |

**与本项目对照**：VTS 把 **Physics 放在最高（P5）**，项目把 `final_override` 放在 physics 之上。
多数情况不冲突（physics 写 `ParamHair*`，动作写 `ParamAngle*`），但**若动作也写 `ParamHair*`
就会破坏物理连续性**。

两条关键工程做法：

1. **"When control of a Live2D parameter is passed between value-providers, it is always faded
   smoothly and not just set instantly to prevent any ugly jumps."** —— 项目当前是硬覆盖，**这是应补的**。
2. **表达式 Add/Multiply 在"所有其他处理之后"应用，先乘后加** —— 正是项目缺的"accent 层"的官方形态。

VTS 其它事实：**Auto-breath** "No input parameter is required... Any input will be ignored"；
**Auto-blink** "randomly reduce the parameter to zero"；每参数独立 **Smoothing**；
"Limit Range" 让值接近边界时**平滑停止**；物理有 **wind**（experimental）、按组 0–2 倍强度、可固定物理帧率。
**VTS 的 idle 是循环 `.motion3.json`**（非程序化）—— 说明**程序化 idle 是比 VTS 更进一步的形态**。

### 6.3 Shimeji 的真实算法（条件门 + 整数权重轮盘赌）

```java
// Configuration.buildNextBehavior()
for (BehaviorBuilder b : behaviorBuilders.values())
    if (b.isEffective(context) && isBehaviorEnabled(b, mascot)) {
        candidates.add(b); totalFrequency += b.getFrequency(); }
double random = Math.random() * totalFrequency;
for (IBehaviorBuilder b : candidates) { random -= b.getFrequency(); if (random < 0) return b.buildBehavior(); }
```

- **权重是整数、无需归一化**；`Frequency=0` = "永不执行，**除非被显式引用**"。
- **防老虎机核心 = 粘性自环**：`SitAndFaceMouse` 转移表 `自身×100 : 变体×1 : 变体×1`
  ⇒ 每次决策仅 **~1.96%** 概率离开当前状态。按其时长推算，**约 592 s ≈ 10 分钟**才出现一次
  甩头变体（该变体仅 1.6 s）。
- **分层**：顶层状态切换用**加权随机**，行为内动作分派用 `Select`（**有序优先级表，first-match-wins**）。
- **Shimeji 的缺陷**：核心代码**无任何冷却机制**。

来源：[Configuration.java](https://raw.githubusercontent.com/DalekCraft2/Shimeji-Desktop/main/src/main/java/com/group_finity/mascot/config/Configuration.java)、
[behaviors.xml](https://raw.githubusercontent.com/DalekCraft2/Shimeji-Desktop/main/conf/behaviors.xml)、
[Select.java](https://raw.githubusercontent.com/DalekCraft2/Shimeji-Desktop/main/src/main/java/com/group_finity/mascot/action/Select.java)

### 6.4 权重必须按动作时长归一化

Streck & Wolbers 2018（IEEE CIG）摘要原文 "compute decision probabilities to
**normalize by the length of individual actions**"
（[DOI:10.1109/CIG.2018.8490450](https://doi.org/10.1109/CIG.2018.8490450)）。⚠️ **全文付费，仅摘要已核实。**

半马尔可夫推导（**调研流自行推导，非论文公式**）：目标占时 `o_j`、平均驻留 `d_j`，
DTMC 平稳分布 `πP = π`，长期时间占比 `o_j ∝ π_j·d_j` ⇒ **`π_j ∝ o_j / d_j`**
⇒ **`P(i→j) ∝ o_j / d_j`**

> **对本项目的含义**：现有编舞表时长差近 2 倍（`nod` 1.32 s vs `look_around` 2.4 s）。
> **若直接拿权重做轮盘赌，短动作会在视觉上消失、长动作霸占全部时间。**
> 做随机调度前必须先归一化。

### 6.5 Valve L4D 的「结构化不可预测」

GDC 2009 原始幻灯片（已从 PDF 提取，[链接](https://steamcdn-a.akamaihd.net/apps/valve/2009/ai_systems_of_l4d_mike_booth.pdf)）：

> "Mob spawns occur at **randomized intervals between 90 and 180 seconds**."
> "**Structured Unpredictability = Superposition of several of these population functions.**"
> 四态机 `Build Up → Sustain Peak (full threat for **3-5 s**) → Peak Fade → Relax (minimal threat
> for **30-45 s**)`
> "Decay Survivor Intensity towards zero over time. **Do NOT decay** Survivor Intensity if there
> are Infected actively engaging the Survivor."

**可照搬**：一个 `tension ∈ [0,1]` 标量调制各行为权重表与期望间隔，**替代固定 every-N-seconds**。
注意：**Randomized interval ≠ 均匀随机**，而是「区间内随机 × 多个不同尺度的函数叠加」。

### 6.6 soullink `IdleActionScheduler`（MIT，带全套数值）

7 种动作模板（全部带能力可用性判定，缺参数自动过滤）：

```
small-nod 0.82–1.20s | head-tilt 1.35–2.15s | side-look 1.45–2.35s | weight-shift 1.65–2.65s
gentle-lean 1.25–2.05s | sigh-sink 1.70–2.80s | slow-blink 0.72–1.08s
```

防"老虎机"三机制：

```js
// 1) 最近窗口去重：候选池排除最近 recentWindowSize 个已播动作（默认 3）
// 2) 方向反重复：与上一次同方向则强制翻转
// 3) 间隔采样用幂律分布（不是均匀分布）—— "不规律感"的来源
sampleInterval(focusLevel) {
  const curve = clamp(0.68 + spontaneity*1.7 - focusLevel*0.32, 0.42, 2.38);
  const randomPosition = Math.pow(random(), curve);
  const focusAdjusted = randomPosition + (1-randomPosition)*focusLevel*0.34;
  return minInterval + (maxInterval-minInterval) * focusAdjusted; }
// 默认 minIntervalSeconds = 4.8, maxIntervalSeconds = 11.5
```

权重是情绪/性格的函数（VAD 三轴）：`small-nod = 1.05 + positive*0.42 + aroused*0.4 +
focus*0.42 + expressiveness*0.28`；`side-look = 0.62 + negative*0.28 + submissive*0.36 +
shyness*0.56 + (1-gazeStability)*0.6`；`sigh-sink = 0.48 + negative*0.62 + calm*0.52 + softness*0.32` 等。

幅度多因子连乘：

```js
amplitude = clamp(gain * (0.72+expressiveness*0.4) * (1-focusLevel*0.38) * (0.9+spontaneity*0.14)
                  * (0.9+vadIntensity*0.18) * (0.86+random()*0.28), 0, 2.2);   // ±14% 随机
```

`interrupt(t) { active=null; nextActionAt = t + sampleInterval(0); }` —— **被打断后重置冷却**。
**`focusLevel` 是贯穿全局的注意力旋钮**（0..1）：同时下调微动幅度、身体摆动权重
`(1-focus*0.76)`、延长动作间隔、提高点头权重。

### 6.7 生理基线：动作频率应该多高？

| 行为 | 实测值 | 来源 |
|---|---|---|
| 对话中「含点头的短语」计数 | **142 / 443（该表非全语料，勿换算为百分比）** | Ishi 2007 Table 1 |
| 对话中「完全没有头部动作的短语」计数 | **189 / 443（同上，勿换算）** | 同上 |
| 含歪头的短语计数 | 33 / 443（同上） | 同上 |
| 重音伴随明显头动的比例 | **80%**（42% 单点头 + 18% 带回弹 + 20% 急摆） | 专利 US7349852 Table 1 |
| 单次点头时长 | **≈0.94 s**（length=1 占 42%） | Mori et al. 2025 |
| 自由视觉探索注视切换率 | **2–3 次/秒** | [Saccade (Wikipedia)](https://en.wikipedia.org/wiki/Saccade) |
| 扫视潜伏期 / 持续 | ~200 ms 启动；20–200 ms 持续 | 同上 |
| 微扫视幅度 | 2–120 arcmin | 同上 |
| 成人静息呼吸频率 | **12–18 次/分 = 3.3–5.0 s** | [MedlinePlus](https://medlineplus.gov/ency/article/002341.htm) |

**使用方式**：呼吸 3.2 s 与生理吻合可直接用；头部动作间隔 4.8–11.5 s ⇒ 每 30 秒约 2–6 次，
**「多数时间什么都不做」才是自然的**（直接否定"每轮都播 nod+shake_no"）；
**注视**：真实人眼每秒 2–3 次注视，比 soullink 默认快得多，但**卡通角色通常需要放慢**，
必须实测，**不要照搬生理值**。


### 6.8 Director 升级补遗

- **Shimeji 三机制**：条件门 + 整数权重轮盘赌（顶层）／硬优先级 Select（行为内）／粘性自环 +
  **时长归一化** + `min_cooldown` 与 `w_eff = w×(1−λ)^k`（Shimeji 缺失、需补）。
- **L4D 张力标量**：`tension∈[0,1]` + 四态机替代固定周期。
- **Convai 注意力模型**：优先级分层（玩家 10 / 其它角色 7 / 世界对象 5）+ **层内粘性** +
  **非对称迟滞 `GazeAttentionDelay 1.0s` 晋升 / `GazeAttentionLossDelay 5.0s` 释放** +
  **所有权锁** `AttentionSource ∈ {None, Gaze, Explicit}`（脚本显式设置即锁死 —— 与项目
  「Mod 不得绕过 core 仲裁」红线同构）+ 输入死区（"Small movement does not trigger a re-plan"）。
  来源：[Convai gaze](https://docs.convai.com/api-docs/plugins-and-integrations/convai-unity-sdk/embodiment/gaze/how-gaze-works.md)、
  [gaze attention](https://docs.convai.com/api-docs/plugins-and-integrations/convai-unreal-engine-plugin/features/gaze-attention/how-gaze-attention-works.md)
- **SmartBody 可抄数值**（源码级）：眨眼 **U(4,8) s** + 0.25 s 三关键帧曲线
  `(0,0)→(t/3,1.0)→(2t/3,0.33)→(t,0)`；扫视三模式表（幅度上限 Listening 10° / Talking 12° /
  Thinking 12°；互视占比 **75% / 41% / 20%**）；幅度 `a = −6.9·ln(f/15.7)`，`f~U(0,15)`，
  **垂直 ×0.5、斜向 ×0.75**；时长 `0.025 + 0.0024×振幅(°)`（与 Carpenter 主序同阶，已交叉验证）；
  插值 **圆形 ease-out** `y = 1 − sqrt(1−(r−1)²)`；**`saccadePolicy = "stopinutterance"` +
  恢复延迟 2 s**；BML `<interrupt>` 默认过渡 **0.5 s**；同 id 打断去重窗口 **60 s**。
  ⚠️ 许可：顶层 LICENSE = LGPL-2.1，但部分文件头写 LGPL v3，**不要断言单一版本**。
- **抢占语义**：Unity 的 `Interruption Source` / `Ordered Interruption` / `Transition Duration` /
  **`Transition Offset`** 是唯一**已发布可配置**的规范；L4D 的 **`OnSuspend` / `OnResume`**
  且**恢复时须重新校验有效性**。→ **把抢占从临时 `if` 提升为声明式 transition 表**。

### 6.9 设计原则：`moving hold`

Thomas & Johnston《The Illusion of Life》原文
（[Internet Archive 全文扫描](https://archive.org/details/TheIllusionOfLifeDisneyAnimation)）：

> "When a careful drawing had been made of a pose, it was held without movement on the screen for
> **at least eight, maybe as many as sixteen** frames... However, when a drawing was held for that
> long, **the flow of action was broken, the illusion of dimension was lost, and the drawing began
> to look flat**."

**量化含义**：24 fps × 8–16 帧 = **0.33–0.67 s，静止保持超过这个长度就开始"显平"**。
修复方式是「做两张图，一张更夸张，冲过去并漂移」，**而不是延长保持**。
Shawn Kelly 四条配方：**overshoot / 眼睛活着 / ease-in / ambient motion**；
关键句 "**it will help a lot if everything doesn't stop on the same frame**"。

### 6.10 人类基线（补充数据）

| 量 | 数值 | 来源 |
|---|---|---|
| 眨眼率（静息 / 交谈 / 阅读） | **17 / 26 / 4.5 次/分** | Bentivoglio 1997, [PMID 9399231](https://pubmed.ncbi.nlm.nih.gov/9399231/) |
| 互视**面部**占比 / 单次时长 | **63% / 2.2 s** | Rogers 2018, [PMC5844880](https://pmc.ncbi.nlm.nih.gov/articles/PMC5844880/) |
| 互视**眼睛**占比 / 单次时长 | **12% / 0.36 s** | 同上 |
| 说话时移开 vs 聆听时移开 | **29% vs 10%** | 同上 |
| 5° 水平扫视最优时长 | **41.5 ms** | van Beers 2008, [PMC2323107](https://pmc.ncbi.nlm.nih.gov/articles/PMC2323107/) |
| 微扫视率（注视中） | 1–2 次/秒 | Engbert & Mergenthaler 2006, [PMC1459039](https://europepmc.org/articles/PMC1459039) |
| 头/颈共振 | 2 Hz 与 4 Hz | Gresty & Halmagyi 1979, [PMC490303](https://pmc.ncbi.nlm.nih.gov/articles/PMC490303/) |

> **两条要紧解读**：
> **(a)** **「互视面部 2.2 s」远大于「互视眼睛 0.36 s」—— 看着脸 ≠ 对视**，
> 本项目 gaze 目标选择应区分；
> **(b)** ⚠️ Gresty 1979 研究的是**异常**头部运动，**不是**规范性摆动测量；
> **静坐头部摆动幅度的一手数值未找到**。

---

## 7. 同类项目对照表

> 许可结论均经 GitHub license API 或仓库 LICENSE/README 原文核实；无法核实的标「未验证」。

| 项目 | 开源? | 许可 | 动作系统 | 可借鉴的一点 |
|---|---|---|---|---|
| **Shimeji / Shimeji-ee** | ✅ | zlib/libpng（原始）/ New BSD（-ee），据 README 自述 | 精灵帧 `actions.xml` + `behaviors.xml`（条件门+整数权重轮盘赌，tick 40 ms） | **行为层与动作层分离**；**粘性自环 100:1:1**；**每模型自带行为配置**。⚠️ **无冷却机制，需自行补** |
| **Ukagaka / 伺か（SSP + SHIORI）** | SSP 基座闭源；**YAYA 开源** | YAYA = **BSD-3-Clause**；Satori 原始版 **未验证**；SHIOL+ **未验证** | 外壳 + 对话引擎 + 气球三层分离；`OnSecondChange` / `OnMinuteChange` / `OnMouseMove`；**`Reference4` = 距上次用户操作的秒数** | **引擎只提供时钟 + 输入事件，策略全在脚本侧**；**把"用户空闲秒数"直接喂给行为脚本**是做 idle director 最干净的输入信号；**仲裁权归引擎**：说话通道被占 → `Reference3=0` 并降级为 NOTIFY |
| **Desktop Mate**（infiniteloop） | ❌ 完全闭源 | 专有（Steam App ID 3301060） | 3D + 手工动作；官方仅描述到"坐在窗口上 / 窗口间跳跃 / 与鼠标光标嬉戏 / 摸头 / 闹钟 / 成对角色 combo" | **窗口感知交互 + 角色配对联动** 比纯 idle 循环更像活着。⚠️ idle 调度算法**完全未公开**；`desktop-mate.com` 是**非官方粉丝站**，不得当官方文档引用 |
| **Bongo Cat**（ayangweb） | ✅ | **MIT** | 输入→动作直映射（Tauri） | **输入→动作直映射**：**反应延迟本身就是"活"**；实现极简 |
| **oneko / neko** | ✅ | 精确 SPDX **未验证** | 精灵帧 + 光标追逐状态机；**几乎无调度 RNG**，确定性升级阶梯 `STOP→洗脸(10)→挠头(4)→哈欠(6)→睡`，由 `IsNekoMoveStart()` 唤醒 | **睡眠状态 + 由用户活动唤醒**：一个"睡着/被唤醒"的情绪收益远大于多做几段 idle 动画；**把随机性外包给用户行为**，彻底消灭老虎机感 |
| **VPet**（LorisYounger） | ✅ | **Apache-2.0** | C#/WPF；图片动画状态机 + 需求/心情数值驱动（架构未验证） | **数值驱动选动画**（而非纯随机），长期观察更有"意图" |
| **VTube Studio**（Denchi Soft） | ❌ 闭源 | 专有（GitHub 仓仅 API 页，MIT） | 面捕→参数映射 + **Auto-breath / Auto-blink** + **循环 `.motion3.json` idle** + physics wind/强度/帧率；后端用 **OpenSeeFace** | **六级值提供者优先级 P0–P5**；**控制权转移时永远平滑淡出**；**表达式 Add/Multiply 在所有计算之后、先乘后加** |
| **VSeeFace**（emilianavt） | ❌ 闭源免费 | 自定义：可商用，**不可修改**；允许 BepInEx mod，mod 不得商业分发 | OpenSeeFace + Leap Motion；VRM0；**VMC 协议收发**；VSFAvatar 可挂 Unity 动画/DynamicBones | **synthetic gaze**（追踪丢失时用程序化注视兜底）；**VMC/OSC 互操作**。⚠️「Idle motion」功能**未验证** |
| **Warudo**（Hikaru Labs） | ❌ 闭源 | 专有（Steam 付费 + Pro 企业版；EULA 未验证） | 3D + 多追踪；**Blueprint 节点图**；**Float Pendulum Physics** | **单一浮点摆原语**：多段摆"最上节点 X 入 → 最下节点 X 出"，官方模板名即 "Live2D Arm Sway"/"Live2D Eye Wiggle"，**一条摆通吃手臂与眼睛高光** |
| **OpenVTuber / open-vt**（erodozer） | ✅ | **MIT** | Godot + Live2D；tracker 走 OpenSeeFace 独立进程或 VTS TCP；原生透明窗口 | **把 VTS 的模型文件与参数约定当事实标准**。另参考 **DeepVTB**（**GPL-3.0**，仅追踪侧） |
| **pixi-live2d-display**（guansss） | ✅ | **MIT** | MotionManager 优先级仲裁 + idle 随机 + CubismBreath/EyeBlink/Physics | `idleMotionFadingDuration 2000ms`（vs 普通 500ms）；`focus.x*focus.y*-30` 侧倾交叉项；FocusController 的"最大速度+刹车速度"；**眨眼用 `motionUpdated` 门控** |
| **soullink-emotion-sdk**（nanlingyin） | ✅ | **MIT** | IdleEngine + IdleActionScheduler + LayeredParameterMixer | **全套可移植的 idle 数学与调度参数**（§2.4、§6.6）；`focusLevel` 统一注意力标量 |
| **OpenSeeFace**（emilianavt） | ✅ | **BSD-2-Clause** | MobileNetV3 + ONNX Runtime，CPU 单核 44–213 fps | **追踪与渲染彻底解耦**（UDP 独立进程）；VTube Studio 的实际后端 |
| **facial-landmarks-for-cubism**（adrianiainlam） | ✅ | **MIT** | OpenSeeFace → Cubism 参数映射 | 头部三轴的完整三角学推导 + 一套经验证的默认调参值 |
| **mouse-tracker-for-cubism**（同作者） | ✅ | 许可未单独核实（主库 MIT） | 鼠标光标 + 音频口型替代人脸 | **低 CPU 的中间态**：无摄像头即可实现"看着光标" |
| **One Euro Filter**（casiez） | ✅ | **BSD-3-Clause** | 速度自适应低通 | **官方 Rust 实现可直接 vendored** |
| **live2d-widget**（stevenjoezhang） | ✅ | **GPL-3.0** | hit area 点击触发 motion | hit area→motion 交互表。⚠️ **GPL-3.0 传染，不宜直接抄入本项目核心** |
| **oh-my-live2d / l2d-widget**（hacxy） | ✅ | **MIT** | Cubism 2/6 运行时；打字逐字驱动口型 | 参数级口型驱动 |
| **MediaPipe Face Landmarker** | ✅ | Apache-2.0（代码） | 478 点 + **52 blendshape + 4×4 变换矩阵** | 官方示例直接集成；`LIVE_STREAM` 模式；kalidokit 已被它取代 |
| **Live2D Cubism 官方 SDK** | 源码公开（非 OSI） | Core = 专有；框架/样例 = Live2D Open Software License；**商业实体年营收 >1000 万日元须签 Release License**；「可导入任意模型的壳」= **Expandable Application**，须事前审核签约 | CubismBreath / CubismEyeBlink / CubismLook / CubismPhysics / CubismUpdateScheduler | **§2.1 五通道呼吸参数组 + §2.3 Look 权重表 + §5.4 参数归属纪律** |
| **Valve L4D AI Director** | ❌ 商业 | 专有 | 单一 intensity 标量 + 四态机调制事件密度 | **Structured Unpredictability**；`OnSuspend`/`OnResume` |
| **Convai** | ❌ 商业 | 专有 | 注意力三级流水线 | **优先级分层 + 层内粘性 + 非对称迟滞 + 所有权锁** |
| **SmartBody**（USC ICT） | ✅ | **LGPL-2.1**（顶层 LICENSE）／部分文件头写 LGPL v3 —— **不要断言单一版本** | BML 打断 + 调度器 + saccade/eyelid/gaze 控制器 | **唯一一批来自已发布系统源码的点睛 idle 数值** |
| **IAUS**（Dave Mark） | ❌ 黑盒授权 | 商业 | Utility AI：Axis × 响应曲线打分取最高 | 全部行为同时打分；`CAN_BE_INTERRUPTED` 标志 |

### 7.1 许可澄清

本项目 Rust 主线用 `ayagami` 而非官方 Cubism Core：

- `ayagami` 是 **MIT / Apache-2.0 双许可**，README 声明 "developed strictly using
  **black-box reverse engineering only**... **no license terms were violated**"，
  并**明确列举适用场景包含 "expandable applications that load user-provided models"**。
- ⇒ **官方 Cubism 的「Expandable Application」审核/签约义务，在当前 Rust 主线（ayagami）下不直接适用。**
- **但**：(a) 这是上游作者自述，不是法律裁定；(b) 旧 JS 前端 / `dist/` 里确实存在
  `live2dcubismcore.min.js`（项目 `NOTICE` 已声明专有、不分发、用户自备）；
  (c) **若将来把官方 Cubism Core/框架引入任何路径，该条款即被触发**。
- **建议**：保持现状，并在 `docs/legal/` 显式记录该判断依据。
  **现有 `docs/legal/` 只有 `pc-preview-publication.md`，未涉及此点 —— 这是一个缺口。**

### 7.2 三个"最值得抄"

1. **Warudo 的"一条摆通吃"**：实现**一个**多段弹簧摆原语（上节点 X 入 → 下节点 X 出），
   同时用于身体摆动、发丝、眼睛高光抖动 —— 比加更多 idle 动画更能提升"活感"，且复用已有 physics 代码。
2. **Shimeji 的"行为/动作分离 + 每模型行为包"**：直接解决 director mod 当前"固定序列"的问题。
3. **oneko 的"睡眠状态"**：一个"睡着（呼吸变慢 + 闭眼 + 头微垂）/ 被唤醒"的状态，
   情绪收益远大于多做几段 idle 动画。

---

## 8. 分阶段最小可行路线

> **成本**：**S** = 单文件、无新依赖、可当天完成；**M** = 跨 2–3 个 crate、需设计；
> **L** = 新 Mod + 外部进程 + 新依赖。**依赖**：**无** = 纯 Rust 内部改动。

### 阶段 0：修正基准（P0，必须先做）

| # | 事项 | 成本 | 依赖 | 风险 | 验收 |
|---|---|---|---|---|---|
| 0.1 | ~~动作幅度单位换算~~ **✅ 已完成**（`param_scale.rs`） | — | — | — | — |
| 0.2 | **验证标定源**：身体 ±10（官方）还是 ±12（soullink profile）；常量改为**从 profile / `PoseMap` 查表**，缺表回退标准值 | **S** | 0.1 | 低 | `nod` 峰值 ≈ 0.55×scale；±45 量程模型比例正确 |
| 0.3 | **`PhysicsOptions::compatible` → `accurate`**（`pose_stack.rs:127`） | **S** | 无 | 低 | 喂 ±3° @2 Hz 噪声时发丝无高频 chatter |
| 0.4 | 确认/补齐 `physics3.json` 的 `Meta.Fps`（推荐 60） | **S** | 无 | 低 | 物理行为不随渲染 FPS 漂移 |
| 0.5 | 在 `docs/legal/` 记录 ayagami vs 官方 Cubism Core 的许可判断 | **S** | 无 | 无 | 文档存在 |

### 阶段 1：统一的 idle 头身微动（P0，核心）

| # | 事项 | 成本 | 依赖 | 风险 | 验收 |
|---|---|---|---|---|---|
| 1.1 | **抽出统一 idle 引擎**，合并 `PoseStack` 与 `surface.rs` 两套实现 | **M** | 无 | **中**：触碰 wasm 渲染面，需行为基线护航 | 单一实现，native/wasm 行为一致 |
| 1.2 | idle 层改用**官方 CubismBreath 五通道**，单一共享时钟、**不可通约周期** | **S** | 1.1 | 低 | 头三轴 + 身体 X 出现慢速自然晃动；长时间无循环点 |
| 1.3 | **idle 层改为加性合成**（`finalize` 现为覆盖语义） | **M** | 1.1 | **中**：混合语义变更影响动作/idle 交互 | idle 可叠加在 tracking/动作之上而不被完全盖住 |
| 1.4 | 补 `ParamBaseX/Y` 重心漂移（±2–4，10–16 s）+ `ParamShoulderY`；**参数缺失自动跳过** | **S** | 1.1 | 低 | 身体有"重量感"；换模型不报错 |
| 1.5 | 补 `MicroMotion`（双频 × 8 相位，±0.02/0.016/0.014 归一化） | **S** | 1.1 | 低 | 静止时不是"完全不动" |
| 1.6 | 眨眼按 **SmartBody `U(4,8) s` + 0.25 s 三关键帧曲线**；加 **`motionUpdated` 门控** | **S** | 无 | 低 | 动作播放期间不打架；频率符合生理 |
| 1.7 | **saccade 系统**：三模式表 + 圆形 ease-out + 垂直×0.5/斜向×0.75 + **`stopinutterance` 与 2 s 恢复延迟** | **M** | 1.1 | 低 | 眼球有微扫视；说话时暂停 |
| 1.8 | **睡眠状态**（久闲 → 呼吸变慢 + 闭眼 + 头微垂；交互唤醒） | **S** | 1.1 | 低 | 长时间挂机更像"活的" |
| 1.9 | 验收基线：任何单一姿态 **0.3–0.7 s** 后必须有可感知变化（moving hold） | **S** | 1.2 | 低 | 目视无"显平" |

### 阶段 2：加性 accent 层、混合与抢占（P1）

| # | 事项 | 成本 | 依赖 | 风险 | 验收 |
|---|---|---|---|---|---|
| 2.1 | 引入 **Additive 混合语义**（`s*w + d`），与 Override 并存 | **M** | 1.3 | **中**：漏改会双重写入 | idle/breath 走加性，canned action 走覆盖 |
| 2.2 | 跨层 **crossfade**（`0.5−0.5cos(πx)`）；普通动作 0.2–0.5 s、idle 2.0 s | **M** | 2.1 | 中 | 控制权转移无跳变 |
| 2.3 | **声明式抢占 transition 表** + `OnSuspend/OnResume` 且**恢复前重新校验**；默认过渡 0.5 s | **M** | 2.2 | 中 | 抢占可配置、可测 |
| 2.4 | 明确**参数归属表**并写进 `docs/architecture/core-contracts.md` | **S** | — | 无 | canned action 不再写 idle/physics 独占参数 |
| 2.5 | 统一 `focusLevel`（0..1 注意力标量）作为跨模块接口 | **S** | 1.1 | 低 | 微动/摆动/动作间隔/点头权重都受它调节 |
| 2.6 | `final_override` 释放后加**余韵衰减**（`pow(1−p, 0.82)`） | **S** | 2.2 | 低 | 动作结束不回弹生硬 |

### 阶段 3：语音驱动的头部动作（P1）

| # | 事项 | 成本 | 依赖 | 风险 | 验收 |
|---|---|---|---|---|---|
| 3.1 | 在 `ws.rs::build_audio_frames` 现有 20 ms 循环新增 **F0（自相关）+ 谱通量 onset** 字段 | **M** | 无 | 低 | WS 帧新增字段；现有 `volume` 行为不变 |
| 3.2 | 渲染侧：能量包络（τ 150–300 ms）→ 头/身微动；音高斜率 → 侧倾 | **M** | 3.1、2.1 | 中 | 说话时头部随语句起伏，静音归零 |
| 3.3 | **短语/句末边界 → 温和点头**（±5–8°，0.6–0.9 s） | **S** | 2.1 | 低 | 点头出现在句读边界 |
| 3.4 | 否定语义 → 摇头（复用 `shake_no`） | **S** | 3.3 | 低 | 语义与动作一致 |

### 阶段 4：Director 层（P2）

| # | 事项 | 成本 | 依赖 | 风险 | 验收 |
|---|---|---|---|---|---|
| 4.1 | 固定 `sequence` → **Shimeji 式条件门 + 整数权重轮盘赌 + 硬优先级 Select + 时长归一化 `P ∝ o_j/d_j` + 粘性自环 + `min_cooldown` 与 `w_eff = w×(1−λ)^k`** | **M** | 2.5 | 低 | 连续 30 分钟观测无"同样两下"重复 |
| 4.2 | **`tension ∈ [0,1]` 标量 + 四态机**替代固定周期 | **M** | 4.1 | 低 | 事件密度有起伏而非均匀 |
| 4.3 | 幅度多因子连乘（含 ±14% 随机）替换固定幅度 | **S** | 4.1 | 低 | 动作大小不一 |
| 4.4 | **注意力模型（Convai 式）**：分层 10/7/5 + 层内粘性 + **1.0 s 晋升 / 5.0 s 释放** + 所有权锁 + 输入死区 | **M** | 2.5 | 中 | 用户活跃时注视用户，空闲时张望 |
| 4.5 | 行为配置 schema（每模型一份）+ `interrupt()` 重置冷却 | **M** | 4.1 | 低 | 配置化；打断后不立刻重触发 |
| 4.6 | 加入**睡眠/唤醒**行为 | **S** | 1.8、4.1 | 低 | 久闲进入睡眠，交互唤醒 |

### 阶段 5：摄像头头部跟踪（P3，可选 Mod）

| # | 事项 | 成本 | 依赖 | 风险 | 验收 |
|---|---|---|---|---|---|
| 5.0 | **（建议先做的中间态）鼠标跟踪**：光标 → §2.3 Look 权重表 | **S** | 2.1 | 低 | 头/眼跟随光标；零外部依赖 |
| 5.1 | 新 Mod `live2d-ai-mod-face-tracking`：子进程拉起 OpenSeeFace，UDP loopback 收包 | **L** | Python + onnxruntime/opencv（**外部依赖**） | **高**：分发体积、平台差异、启动失败处理 | tracker 崩溃不影响主链路；可一键关闭 |
| 5.2 | 包解析 → `rotation`/`gaze`/`eyeOpen`/`confidence` → 参数映射 | **M** | 5.1 | 中 | 头随人转，幅度正确 |
| 5.3 | **One Euro 滤波**（vendored BSD-3 Rust），每轴独立 | **M** | 5.1 | 中 | 静止无抖动、快速转动无拖影 |
| 5.4 | 低置信度 / `got3DPoints == false` → **crossfade 回 idle** | **S** | 2.2、5.2 | 低 | 丢脸时平滑过渡 |
| 5.5 | `-M` 镜像 + 隐私说明（只传数值不传图像） | **S** | 5.1 | 低 | 左右一致；符合 loopback-only 红线 |
| 5.6 | Mod 配置暴露到 Flutter settings | **M** | 5.1 | 中 | 用户可调 |

**关键路径**：`0.2 → 0.3 → 1.1 → 1.2/1.3 → 2.1 → 3.x / 4.x`。
阶段 5 完全可选，且**应在所有前置阶段完成后才开始**。
**若只能做一件事**：做 **1.2（官方 CubismBreath 五通道）** —— 单文件、零依赖、
立刻把"死"变"活"，且是官方验证过的方案。

---

## 9. 未验证事项清单（**请勿当作事实使用**）

### 9.1 已识别但存在不确定性

| 项 | 状态 | 说明 |
|---|---|---|
| **`ParamBreath` 的官方 weight** | **不确定** | TS = **1**，C++/Java = **0.5**。**未找到官方勘误** |
| **动作幅度换算的标定源** | **待收尾** | 换算已修，但 `param_scale.rs` 用**硬编码 ±30 / ±10**；soullink profile 给 body 是 **scale 12 / ±12**；官方文档亦提到头部可设 **±45**。**建议改为查表** |
| **Ishi 2007 Table 1 的具体数字** | **中等置信** | PDF 文本提取，**表格 OCR 存在错位风险**。列合计（nd=142 / no=189 / 总 443）与正文"535 短语"自洽 |
| **soullink 的 `idleConfig` 是否即"推荐值"** | **中等** | 由 `profile-generator` 自动生成，**不代表官方推荐**；但针对本项目 Bai 模型，仍是目前最贴合的基准 |
| **`ParamBaseX` / `ParamBaseY` 可用性** | **需探测** | 官方表中 `ParamBaseY` 带 `*` = 非保证存在 |
| **"语音驱动头动"的开源实现** | **未找到** | uLipSync / SALSA / OVLipSync 均只做口型。§3.3 映射是基于 §3.1 实证的工程建议 |
| **`mouse-tracker-for-cubism` 的许可** | **未单独核实** | 同作者主库为 MIT |
| **SmartBody 许可版本** | **不一致** | 顶层 LICENSE = LGPL-2.1，但部分文件头写 LGPL v3。**不要断言单一版本** |
| **SmartBody `pendingInterrupts` 的 60 s 清理逻辑** | **字面可疑** | 代码为 `if (lastTime - now > 60.0)`，对过去时间戳恒为负。**只报告字面实现** |
| **Streck & Wolbers 2018 的具体公式** | **仅摘要** | 全文付费。`P(i→j) ∝ o_j/d_j` 是标准半马尔可夫推导，**非论文公式** |

### 9.2 明确**没有**验证的项

| 项 | 说明 |
|---|---|
| **Perlin/simplex 噪声用于 Live2D 头动** | 未找到任何生态实现或文档。§2.6 建议是推断，**不是行业实践** |
| **"噪声 ≤2–3 Hz"、"重心移动 10–16 s / ±2–4"、"包络 τ 150–300 ms"、"追踪延迟 <150 ms 可接受"** | **全部是工程建议，无引用** |
| **"官方建议预平滑物理输入"** | **不存在**。官方是在 SDK 内部用输入 lerp + 输出插值解决 |
| **"每 N 秒触发一次 idle break / fidget" 的指导数字** | **不存在一手来源**。「idle break」「fidget」是社区词汇，引擎官方文档未定义该功能或间隔 |
| **`epoch / generation counter` 的命名出处** | **未找到公开发表的设计来源**。功能等价的有据机制：SmartBody `pendingInterrupts`、Convai `AttentionSource` 所有权锁、L4D `OnSuspend/OnResume`。可自行实现，但**不应写成"业界标准做法"** |
| **Unity DynamicBone / Magica Cloth / Unreal 弹簧骨数学** | 未取得一手数学文档或公开源码 |
| **VSeeFace 的 "Idle motion"** | 官方站点 / 手册 / FAQ / release notes 均未记载 |
| **SSP / Satori / SHIOL+ 的开源性与许可** | 未验证（仅 YAYA = BSD-3-Clause 已核实） |
| **VTube Studio App EULA / Warudo EULA 原文** | 未验证 |
| **VTube Studio 的 idle 调度算法** | 未公开；只知道 idle 是循环 `.motion3.json` |
| **Desktop Mate 的任何 idle 调度计时器/概率/抢占策略** | 完全闭源。⚠️ `desktop-mate.com` 是**非官方粉丝站**，不得当官方文档引用 |
| **VPet 动画状态机实现细节** | 未验证 |
| **oneko 的精确 SPDX 标识** | 未验证 |
| **Cerebella 的 RST 映射规则与其许可证** | Python 包不在 worldforge 镜像内，AAAI 全文未取得 |
| **BEAT 的模块管线、具体规则、开源状态、许可证** | MIT 项目页已核实，论文 PDF 未取得 |
| **NVBG（Lee & Marsella 2006）的规则格式、仓库与许可证** | 未取得一手来源。已知：worldforge 镜像里的 `nvbg.h`/`nvbg.cpp` 是**桩代码** |
| **Bahill 1975 原文的线性拟合常数** | 论文闭源。`T = 2.2 ms/° × A + 21 ms` 来自 Skovsgaard 2011 对 Carpenter 的转述 |
| **Rayner 1998 的注视时长均值 / Martinez-Conde 2004 的微扫视率** | 引文已核实，**数值未取得** |
| **Kendon 1967 的百分比** | 仅通过二手文献获得 |
| **静坐时头部摆动幅度 / 会话中 yaw-pitch 漂移范围** | **未找到可核实的一手数值**。头/颈共振 2 & 4 Hz 来自一项关于**异常**头部运动的研究 |
| **《The Illusion of Life》moving hold 段落的页码** | 节名与正文已核实，**页码未核实** |
| **《The Animator's Survival Kit》p.368 "The Moving Hold" 的正文措辞** | 页码经目录核实，**正文未能提取**。另：**"moving hold" 不是迪士尼 12 原则之一** |
| **Live2D 官方日语/中文「待機モーション」教程正文** | 未取得（相关页面 404） |
| **`live2d.net.cn` 的"抖动"文章** | **已排除**。其 Editor 参数名与官方 UI 不符，判定为 AI 生成的低质量内容，**数字不可采信** |
| **One Euro 是否已发布为 crates.io crate** | **无法验证**（crates.io API 不可达）。但官方仓库 `rust/` 可 vendored，不构成阻碍 |
| **`Pejsa et al. 2015` 的注意力模型细节** | 摘要被出版商 elide，全文付费 |

---

## 10. 一页速查（实现的 action list）

```
[阶段 0 · 基准]
 0. param_scale.rs 已修；建议改为 profile/PoseMap 查表（body ±10 还是 ±12 需定案）
 1. pose_stack.rs:127  PhysicsOptions::compatible → accurate（rotation_boost 0.2 → 0.0）

[idle 层（加性、单一共享时钟、周期互不通约）]
 2. ParamAngleX    += 0.5 * 15.0 * sin(2πt/6.5345)
    ParamAngleY    += 0.5 *  8.0 * sin(2πt/3.5345)
    ParamAngleZ    += 0.5 * 10.0 * sin(2πt/5.5345)
    ParamBodyAngleX+= 0.5 *  4.0 * sin(2πt/15.5345)
    ParamBreath    += 1.0 * (0.5 + 0.5*sin(2πt/3.2345))      // weight 存疑
 3. 微动：headX/Y/Z = (sin(0.38t)·0.72 + sin(0.17t)·0.28) × [0.02, 0.016, 0.014]
 4. 重心：ParamBaseX/Y ±2–4，周期 10–16 s；肩：ParamShoulderY
 5. 眨眼：U(4,8) s 重采样；0.25 s 曲线 (0,0)→(t/3,1.0)→(2t/3,0.33)→(t,0)
         用 motionUpdated 门控；用 set 覆盖而非加性
 6. 扫视：三模式（幅度上限 10/12/12°，互视 75%/41%/20%）；时长 0.025+0.0024×振幅；
         圆形 ease-out；垂直×0.5 斜向×0.75；说话时停 + 2 s 恢复延迟

[身体 / 物理]
 7. 物理独占 ParamHair*；final_override 不得写它们
 8. 每帧只调一次 PhysicsEngine::update()；首帧 settle()（已做）
 9. 确认 physics3.json 有 Meta.Fps（推荐 60）；头动噪声限带 ≤2–3 Hz
10. 头发组只接 ParamAngleX/Y/Z；衣摆组只接 ParamBodyAngleX/Y + ParamBaseX/Y

[混合 / 分层]
11. Override: s*w + d*(1−w)      Additive: s*w + d
12. fade 曲线 0.5 − 0.5cos(πx)；普通动作 0.2–0.5 s，idle 2.0 s
13. 参数归属：idle 独占 ParamBreath/ParamBaseX/Y；input 独占 ParamAngleX/Y/Z；physics 独占 ParamHair*
14. 跨层控制权转移必须 crossfade；抢占用声明式 transition 表
15. 动作结束后用 pow(1−p, 0.82) 余韵衰减回中

[语音驱动]
16. 复用 ws.rs 既有 20 ms 切片循环，加 F0 + 谱通量；包络 τ 150–300 ms
17. 点头主要靠「短语/句末边界」（TTS 分句已现成），不要主要靠 F0
    （F0→头动相关仅 25–50%，且疑问句升调时人反而点头）

[Director]
18. Shimeji 条件门 + 整数权重轮盘赌（顶层）+ 硬优先级 Select（行为内）
19. 时长归一化 P(i→j) ∝ o_j/d_j；粘性自环；min_cooldown；w_eff = w×(1−λ)^k
20. tension∈[0,1] + 四态机（BuildUp 3–5 s / PeakFade / Relax 30–45 s）
21. 注意力：分层 10/7/5 + 层内粘性 + 1.0 s 晋升 / 5.0 s 释放 + 所有权锁 + 输入死区
22. 生理基线：32% 短语含点头、43% 完全无头动 → "多数时间什么都不做"才是自然的

[跟踪（可选 Mod，建议先做鼠标版）]
23. 鼠标：光标 → §2.3 Look 权重表（零外部依赖，最大性价比）
24. 摄像头：OpenSeeFace 子进程 + UDP loopback + -M 镜像；model 3 / 640x360 / 20–30fps
25. rotation→ParamAngleX/Y/Z；gaze→ParamEyeBallX/Y；confidence 低 → crossfade 回 idle
26. One Euro：mincutoff 1 Hz 起，beta 从 0.001/0.0001/0.01 试量级；每自由度一个
```
