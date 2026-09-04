# Live2D-Ai 架构设计哲学对照报告（2026-09 综合）

> 主 Agent 综合已有调研（docs/research/*）+ 自身判断。不派子代理。
> 目的：为核心链路 / 表现层 / Mod 分层 + UI 重做 + 渲染技术选型提供**设计哲学**依据（不是功能清单）。

## 0. 三条总纲（从同类项目提炼的架构哲学）

1. **渲染器只执行，编排归上层**：Live2D 渲染器（wasm wgpu）只做"把 ParameterFrame 字段映射到参数 ID + 执行"，不该决定"何时空闲/播什么待机/怎么编排"。这对应你定的"待机/动作=表演层，非渲染层"。
2. **无预设跨皮套是硬约束**：不依赖手作 motion3/exp3/逐模型模板。用 `L1 官方标准参数 ID(L2 FACS语义映射)L3 运行时枚举+能力检测降级` 三层契约——这是 soullink(MIT) 验证过的、可抄的。
3. **适配无 motion 的任意人形皮套，而不是让皮套适配我们**：这是产品核心（你明确"皮套作者适配不考虑，我们通用"）。

## 1. 模型渲染技术（r/c/java/py）架构哲学

| 技术 | 代表 | 架构哲学 | 适合度 |
|---|---|---|---|
| **Rust (wgpu/ayagami)** | 我们 l2d | 类型安全前端渲染，GPU 抽象(wgpu 跨平台)，确定性可测，零 run-time 依赖 | ✅ 主选——95% Rust 目标，P0 渲染性能 |
| C/C++ (Cubism SDK 官方) | Live2D Cubism Core | 性能最优，**但专有许可**（Redistributable 须官方授权） | ⚠️ 不能抄/不能分发——避开 |
| Java (CubismJavaFramework) | Android 端 | 绑定官方 SDK，Android 生态 | 仅 Android 壳，非主线 |
| **py (open-llm-vtuber)** | 我们旧 py | 开发快，但**渲染粗糙**（Cubism JS runtime + PixiJS） | ⚠️ 仅参照架构/UI，不作渲染主线 |

**哲学结论**：渲染核心= **Rust wgpu**（性能+确定性+P0 目标），建模层用**标准参数 ID 抽象**（ParameterFrame 已是），渲染适配层做"字段→参数 ID 映射"。**学习 Cubism 官方的 L1 标准 ID 契约 + soullink 的 FACS 映射，但不引 Cubism 专有 SDK**。

## 2. 前端 UI 架构设计哲学（NEKO 对标 + py 骨架）

NEKO UI 对标（已有调研 `neko-ui-alignment-and-gaps.md`）5 大差距给我的教训：
1. **无桌面壳** → 透明/穿透/托盘是"是否是桌宠"的身份缺口（→ F4 Tauri 壳）
2. **两套割裂视觉系统** → 必须**统一 design token**（--accent/--radius/字号阶梯/字体栈/阴影），不能各写各的
3. **几乎无动效过渡** → 面板开关/消息上屏/tab 切换需过渡体系（不能 display:none 硬切）
4. **聊天不持久** → 历史 API（向后端补）
5. **VAD 打断无反馈** → 语音可见反馈

**设置 UI（用户指出的"平铺太差"）正确形态** = py 骨架验证过的：
- 模态覆盖层 + **左侧导航 rail**（tab 列表）+ 右侧**一次一个 section** + 底部条
- **不是** 8 分组平铺。tab：LLM/TTS/形象背景/外部接入/密钥/人设提示词/系统高级/记忆/插件
- 这是"抄 py 骨架"（MIT 合规），不自己重写 UI 表现。

**UI 哲学**：
- **统一 design token**（主题色/圆角/间距/字阶/阴影单一来源）
- **左导航 rail + 一次一 section**（信息架构清晰，非平铺）
- **动效/过渡体系 + 空状态/加载态/错误反馈**（"看着像样"的核心）
- **capability 驱动**（后端能力决定显隐，不渲染不支持的字段）

## 3. 分层归属（你定的正确边界）

| 层 | 内容 | 归属 |
|---|---|---|
| **核心链路** | 消息链路(文本→LLM→TTS→口形)、Web UI 主体、仅模型缩放、背景导入 | main/fcore-p0 |
| **表现层** | 待机 idle(blink/breath/sway)、6动作、眼嘴组合、皮套表演样本 | feature/perf-layer，**独立** |
| **Mod** | 5 Mod + 配置管道 | feature/mod |

**关键**：表现层（待机/动作）用的是**标准参数抽象**(ParameterFrame) + 渲染器只执行——与渲染层解耦，可独立演进。

## 4. NEKO"走复杂了"的教训（别重蹈）

NEKO 用 Python/FastAPI + Electron + ZMQ 插件沙箱（复杂）。我们的哲学：
- **单一 Rust 二进制**（95% Rust 目标），不引 Electron/Python/ZMQ
- **Mod 静态编译 trait 注册**（轻量互操作），非进程沙箱
- **Web UI 为主入口**（浏览器即客户端，天然跨平台）
- 复杂度只加在**必要处**（性能/表现），不堆砌架构层次

## 5. 决策依据汇总（供你拍板）

| 待决 | 依据 | 建议 |
|---|---|---|
| 渲染技术 | Rust wgpu(性能/确定性)+标准参数ID抽象 | 维持 wgpu，不引专有 Cubism SDK |
| 设置 UI 重做 | py 骨架(左导航 rail+一次一 section) MIT 可抄 | 按 py 骨架重做，统一 design token |
| 待机归属 | 独立表现层，渲染只执行 | feature/perf-layer 独立做 |
| 无预设皮套 | L1/L2/L3 三层契约(soullink MIT 可抄) | 参数动画派系，任何皮套通用 |
| Web UI 主体 | NEKO 5 差距 + py 骨架 | 统一 token + rail + 动效体系 + capability 驱动 |
