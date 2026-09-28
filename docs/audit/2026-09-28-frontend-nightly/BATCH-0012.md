# BATCH-0012 — Phase 1 收口 · test/ 覆盖矩阵 + 假绿灯全量扫描

> 账本：`AUDIT-B/`。方法不是通读 93 个测试文件，而是**三步盘覆盖**（§6-J 的正题）。

## 一、符号级覆盖矩阵（`lib/**` 的类名 → `test/**` 的命中）

- lib 113 个 .dart / test 93 个 .dart。
- **类名在测试里零命中**的 lib 文件只有 6 个（其余 107 个至少有一个类被测到）：

| lib 文件 | 零命中类 | 判定 |
|---|---|---|
| lib/api/ws_client.dart | `WsClient` | **合理**：依赖 `package:web`，VM 加载不了；它的判据已被抽出（`ws_liveness.dart`）并有测试 ✓ |
| lib/live2d/live2d_host_web.dart | `WebIframeTransport` | **合理**：同上（平台视图 + DOM），且收发校验是 8 行可直读代码 |
| lib/main.dart | `Live2DShellApp` / `ShellRoot` | **合理（但有代价）**：组合根测不了，F-0007-1 那种接缝缺陷因此无网 |
| **lib/settings/sections/llm_section.dart** | `LlmSection` | **缺口（F-0012-2）**：tts_section 有 5 条 asset_guard，llm_section 一条没有 |
| **lib/ui/stage_corner_controls.dart** | `StageCornerControls` | 缺口（P3）：缩放三键 + 「有 ack 才显示百分比」这条判据无测试（历史上「缩放读数永远是 —」正是真机才发现的） |
| **lib/ui/streaming_indicator.dart** | `StreamingIndicator` | 缺口（P3，与 F-0005-7 同域）：三点呼吸 + reduced-motion 分支零覆盖 |

## 二、假绿灯模式全量扫描（93 个文件）

| 模式 | 命中 | 处置 |
|---|---|---|
| `readAsStringSync().contains(…)` 源码扫描 | **28** 个文件 | 其中 **12** 个先剥注释（判别力好：`asset_guard_director_cue` / `asset_guard_preset_dispatch` / `asset_guard_stage_attach_resend` / `chat_bubble_labels` / `design_tokens_lint` / `design_tokens` / `director_observer` / `glass_rim` / `motion_wiring` / `no_backdrop_filter` / `no_client_side_preset_ttl` / `transient_results`）；**16** 个不剥（含 `wiring_test` / `semantics_test` / `stage_clock_test` 等，本轮逐个看过，每个断言字符串在对应文件里只出现一次，判别力可接受） |
| 恒真/弱断言（`greaterThan(0)` / `isNotEmpty` / `isNotNull` / `isTrue`） | 205 处 | 绝大多数有语义（如「必须真的扫到文件」）；**确认为假绿灯的只有 1 处**：`chat_bubble_labels_test.dart:117`（F-0005-3） |
| `expect(常量, 常量)` 恒真式 | 0 处（除 0005 已登记的 `expect(kBubbleReadingWidth, 480.0)`，那是**钉常量**、有判别力） | — |
| 空泵（树里没有被测组件） | **1 处**：`semantics_test.dart:292-308`（F-0005-7） | 已登记 |
| `expect(常量, 常量)` 之外的「删掉实现仍绿」 | **2 处**：F-0005-1（扫错文件）、F-0005-6（同词另一处） | 已登记 |
| 断言总数 / 带 `reason:` 的 | 3143 / 684（22%） | 注释密度与断言密度不匹配是可维护性隐患，不是缺陷 |

## 发现
- **F-0012-1（P1）** 连通性自检的**成功/失败判定靠显示文案**（`contains('ok')/('ms')/('毫秒')`）⇒ 上游错误响应里带 "ms" 时，失败被当成成功，而 `FieldActionRow` 的成功结果**根本不渲染** ⇒ **点了「测试连接」，界面毫无反应**（LLM 与 TTS 两处同款）。
- **F-0012-2（P2）** `LlmSection`（189 行，含密钥写入、自检、思考开关、token 上限）**整块零测试覆盖**——与 `tts_section` 的 5 条 asset_guard 形成不对称。

## 本批核对过、不成发现的（正面记录）
- **107/113 的 lib 文件至少有一个类被测试命中**——覆盖率不是这个项目的病；病在**接缝**（CONSOLIDATION 组 C）与**断言对象**（组 B）。
- `ws_client` / `live2d_host_web` 的零覆盖是**架构决定的**（package:web），且关键判据都被抽成了纯函数并有回归——这正是 §10.2「能被单测的东西不要放在 web 依赖后面」这条纪律的执行结果。
- `test/wiring_test.dart` 这类「逐个点名 + 写清没接上会怎样」的接线守卫，是本仓对「代码写好了但没人用」最有效的手段（它抓到过 StageHost/shortcutHelp/PersonaSection.onImport 三处真实静默失效）。

## 本批未核实
- `StageCornerControls` 与 `StreamingIndicator` 两处缺口我只做了「是否有类名命中」这一层判定，没读实现细节找具体缺陷（留 Phase 2 的 D/J 横扫）。
- 93 个测试文件中我**全读**的有 12 个（见 INDEX），其余是模式扫描 + 抽查；「205 处弱断言里没有第二处假绿灯」这一结论建立在模式扫描 + 抽样上，不是逐条人工判读（标 未核实/中）。
