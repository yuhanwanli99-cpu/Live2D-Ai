# 阶段5 范围与裁决（导演可观测 A–D + 单模型动作强度 E）（2026-09-26）

> 落盘人：顶层管理与代码审查。工作树 = `/home/skystar/Live2D-Ai-l1`（分支 `mod/l1-product`）。
> 本文件 = **阶段5 冻结范围 + 裁决**；编排者写 `STAGE5-plan-*` 时必须引用本文件、不得改其裁决。
> 前置：阶段4 已收口（Gate 4/4f，HEAD `279d6081`）。**不 bump / 不 push / 不打 tag**（由维护者在 Gate 5 执行）。

## 0. 用户裁决（2026-09-26）

| # | 裁决 |
| --- | --- |
| U1 | **TTS 净化「不加复杂度」**：不新增 WS 帧、不动 `supervisor.rs`。D 栏降级为零后端（见 D41）。 |
| U2 | **E 轨粒度 = 每模型 × 三倍率**（`head` / `body` / `expression`）。每参数粒度（head X/Y/Z 分别调）为 P2，不做。 |
| U3 | 阶段5 跑完后：维护者做一次**代码审查 + 提交（Gate 5）**，并**并入 `main`、发布 0.2.0 最后一个版本**。 |

## 1. 裁决（D40–D43）

| # | 裁决 |
| --- | --- |
| **D40** | 每皮套动作幅度 = `live2d-ai.toml` 的 `[action]`（全局默认）+ 可选 `[action.models.<model_id>]`；**三键各自可选、各自回落全局**；优先级「本模型覆盖 > 全局」。**修订 O10** 的「按皮套那份住映射表」：**用户旋钮住 toml**（热重载 + PATCH + 持久化），`assets/actions/field_map.json` 只管**每通道峰值/方向**。 |
| **D41** | **撤销 S6**：**不新增** `dev_tts_clean` 帧，**不碰** `supervisor.rs` / `supervisor/handlers.rs` / `conversation/**` / `ws/events.rs`。D 栏零后端。 |
| **D42** | D 栏 = dev 工具：左侧 = 前端已收 `text_delta`（真正送 TTS 的净化文本）；右侧 = `/api/v1/logs` 最近的 `sentence_ready` 行（原始段，dev_mode 才有）。**按出现顺序并排 + 字符数**，标注「来源=日志端点，不承诺逐句精确匹配」。**正文仍不进日志、前端不落盘**。 |
| **D43** | 阶段5 收口后由维护者：代码审查 → **Gate 5 = 并 `main` + 发布**（版本口径见 §4，待用户最终确认）。worker 与编排者全程**禁 git 写操作**。 |

## 2. 冻结范围

**A–D（可观测，设置 → 开发工具 →「导演可观测」，dev_mode 可见）**

| 栏 | 内容 | 来源 | 后端 |
| --- | --- | --- | --- |
| A 决策参数 | `emotion`/`intent`/`suggested_tts{speed,pitch}`/`preset_id`/`seq`/`turn` + 计数 | `GET /api/v1/mods/director/state` | 无 |
| B 事件流 | `action_cue`、渲染面 ack 四条 + `segment-ended`、`stage-clock`、`turn_state`、`error`；时间戳 + 类型过滤 + 上限 200 | 前端已收 WS 帧 + 渲染面 ack（复用 `render_events.dart`） | 无 |
| C 传参对照 | 左「请求」= 前端 `preset` 帧；右「生效」= 渲染面 ack 最终值（`clamped`/`degraded`/`reason`） | 前端 `live2d_bridge` + ack | 无 |
| D 送 TTS 文本 | 净化后文本列表 + 字符数 / 原始段列表（来源标注） | `text_delta` + `/api/v1/logs` | 无 |

**E（单模型动作强度，外观与互动 →「动作幅度」）**

| 项 | 冻结 |
| --- | --- |
| 配置 | `[action]` 全局 + 可选 `[action.models.<id>]`（`head_scale`/`body_scale`/`expression_scale`，各键可选） |
| 生效 | 有效值 = 覆盖 or 全局（逐键）；前端按 `active_model_id` 计算 → 既有 `ActionScalesSyncer` 下发；**渲染面零改动** |
| UI | 「本模型覆盖」开关 + 当前模型名 + 三条滑条 + 「恢复跟随全局」 |
| 量程 | 沿用 `[0.2, 2.2]`；越界钳位 |
| 持久化 | 写 `live2d-ai.toml`（就地改值、保留注释、热重载生效） |

## 3. 波次与文件归属（零重叠）

| 波次 | Worker | 独占文件 | 并行 |
| --- | --- | --- | --- |
| 5.1 | **W5a**（Flutter A–D） | `settings/sections/dev_tools_section.dart`、`settings/sections/director_observer_section.dart`(新)、`live2d/render_events.dart`、`main.dart`、`test/director_observer_test.dart`(新)、`test/render_events_test.dart` | 与 W5r 并行 |
| 5.1 | **W5r**（Rust E 配置面） | `runtime/src/settings.rs`、`runtime/src/settings/{view,patch,patch_tests}.rs`、`desktop/src/web_api/settings_routes/**`、`live2d-ai.toml.example` | 与 W5a 并行 |
| 5.2 | **W5e**（Flutter E UI） | `settings/sections/appearance_section.dart`、`api/settings_models.dart`、`app/shell_settings.dart`、`live2d/action_scales_sync.dart`、`main.dart`（等 W5a settle）、`test/appearance_section_test.dart`、`test/action_scales_wiring_test.dart` | 串行 |
| 5.3 | **W5c**（文档） | `architecture/performance-protocol-v1.md`（§12.3 + D40 修订 O10）、`architecture/action-packs-v0.md`（§11 落点）、`AGENTS.md`、`docs/plans/STAGE5-CLOSEOUT-2026-*.md` | 串行 |

**禁区（本阶段无一例外）**：`topics.rs` / `mod_registry.rs` / `supervisor.rs` / `desktop/src/main.rs` / `conversation/**` / `web_api/ws/events.rs`。

## 4. 待用户最终确认（仅一条）

**发布版本号**：`0.2.0-rc.4`（0.2.0 线最后一个 RC）还是 **`0.2.0`**（GA）？维护者建议 `0.2.0-rc.4`——本轮仍是 RC 质量口径（真表演层端点 D19 未取证）。

## 5. Gate 5 记录与维护者裁决（D44–D48，2026-09-26）

| # | 事项 | 裁决 |
| --- | --- | --- |
| **D44** | 复核发现两处缺陷，维护者**直接补修**（改动随阶段5 Flutter 提交） | ① **通道 B 判据被常量化规避**：`action_cue_test` 原按字面 `state('director')` 匹配，观测面用常量 `kDirectorObserverModId` 读同一只读端点即可绕过。已改为**语义断言**（常量化读点也计入「读 director 状态面」+ 只读面不得出现 `applyPreset`）。② **B 栏被 30ms `stage-clock` 灌满**：`ObserverBuffer` 按 (type, seq) **折叠**该连续信号，200 条容量只留每段最新一条。 |
| **D45** | `web_api/dispatch.rs` 两处一行透传（`active_model_id`）不在 W5r 字面授权表 | **追认**（D32/D37 同类：计划表不完整，非越权）；`active_model_id` 只能由持有 registry 的 dispatch 给出。 |
| **D46** | `patch_tests.rs` **992 行** > AGENTS 测试文件 800 行上限 | **本版接受**（已加头注理由、无自动门禁、test-only、无功能风险），**记 P1 债**并写入发布说明已知问题；下轮拆分。 |
| **D47** | B 栏事件环被时钟挤占 | **已由 D44② 修复**，并在 `flutter test` 1091 全绿下复验。 |
| **D48** | 调试面板 `source=debug` 的 preset 请求未进 C 栏（调用点在 `shell_settings.dart`，W5a 无授权） | **接受为 backlog**（小改，需单独授权该文件）。 |

**Gate 5（维护者执行）**：阶段5 三条提交（`a30c0268` / `6abddbaa` / `f95449e2`）→ 复核全绿 → 发布提交 + **并 `main`（fast-forward，不 push）+ tag `v0.2.0-rc.4`**。
版本口径 = **`0.2.0-rc.4`**（用户 2026-09-26 定）。

