# CONSOLIDATION-01 — 前 7 批对账（账本 `AUDIT-B/`）

> 基线 HEAD `5ef879f4` · 2026-09-28 · 覆盖 BATCH-0001…0007（其中 0001–0004 为快照继承）
> 纪律：逐条**回到源码重读**再判定，笔记会腐烂、代码在变；不成立就改写、不辩护。

## 1. 未关闭 P0/P1 源码复核（逐条重读）

| ID | 复核动作 | 结论 |
|---|---|---|
| **F-0001-1** P1 | 重读 `main.dart:1004-1014 _gotoSection` 全函数 + `grep -n "_ui\.\|openSettings" lib/main.dart lib/app/*.dart` | **仍成立**。`_gotoSection` 只做 `flush + setState(_section)+_clearTransientResults + _ensureSettingsLoaded`，**没有** `openSettings()`；全仓 `openSettings` 只有一个调用点（`AppShell` 的公开方法，被 main.dart:1230 快捷键调用）。另**新增一条正面事实**：`_gotoSection` 首行是 `if (next == _section) return;` ⇒ 重复点同一分区 chip 不会清结果（B5 阶段的 P3 猜想被证伪，不记） |
| **F-0002-1** P1 | 重读 `main.dart:1134-1139` → `_updatePrefs` → `_update` | **仍成立**。`Slider.onChanged` 直连 `_updatePrefs(copyWith(volume: clampVolume))` → `_update` 无条件 `saveDisplayPrefs(next)` ⇒ 拖动中**每个像素**一次 localStorage 写。与项目自己在 `chat_controller.dart:497` 写下的「逐条写会把主线程拖垮」是同一条纪律 |
| **F-0003-1** P1 | 已被 **F-0006-1 升级并取代**（触发频率从「每次宿主重建」修正为「每个 text_delta」） | **升级后仍成立**，且严重度上调。旧记法低估了频率 |
| **F-0003-2** P1 | 重读 `field_row.dart:588-631 FieldActionRow` + `llm_section.dart:171-179` 调用点 | **仍成立**。`result` 槽唯一去处是 `error: resultIsError ? result : null` ⇒ 成功结果**无处可显**；而调用点 description 逐字写「结果内联显示」 |
| **F-0005-1** P1 | 重读 `test/no_backdrop_filter_test.dart:246-252` + `test/semantics_test.dart:310-313` + `grep -rn RepaintBoundary lib/` | **仍成立**。`lib/app/app_shell.dart` 内该词**仅出现在 :592 注释**（内容是「不要这么做」），真保护在 `lib/ui/stage_host.dart:68`。删掉它两条测试仍全绿 |
| **F-0005-2** P1 | 重读 SDK `transitions.dart:111-137`（`AnimatedWidget`/`_AnimatedState`）+ 接线三层 | **仍成立**，机制闭环。特别记录一条**新增**的证伪尝试：曾怀疑内层 `ListenableBuilder` 的 `didUpdateWidget` 会跳过重建——读过源码后确认**不会**（它只转移监听，框架随后仍 `updateChild` 触发 build） |
| **F-0005-3** P1 | 重读 `chat_bubble_labels_test.dart:102-118` + `message_bubble.dart:147-167/:190-196` + `tokens.dart:750` | **仍成立**，且比原记录更硬：补算出 `Space.s2 = 8` ⇒ 空泡高度恒为 16 px；表达式 `message.text.isNotEmpty \|\| !onlyReasoning` 在空文本时仍为 true ⇒ **空泡确实被画出来** |
| **F-0006-1** P1 | 重读 `app_shell.dart:765` + `shell_backdrop.dart:181-193/:47-59` + SDK `image_provider.dart:1726-1734` | **仍成立**（本批新写，已双证据闭环） |
| **F-0007-1** P1 | 重读 `shell_chat.dart:38-46` + `chat_controller.dart:130-164` + `grep "_ui\."` + **后端** `chat_routes.rs:154/:487-497` | **仍成立**。后端侧已确认 503/429 两条失败路径**不广播任何 WS 帧** ⇒ 不是「可能自愈」，是「一定不自愈」 |

**P0 计数：0（7 批 58 个源文件 + 34 个测试文件，未见数据丢失/权限绕过/离线红线被打破/注入面）。**

## 2. 假绿灯对账（§6-J 专项，四条逐一构造反例）

| ID | 反例（删掉实现，测试仍绿？） | 结论 |
|---|---|---|
| F-0005-1 | 删 `stage_host.dart:68` 的 `RepaintBoundary(` → 两条测试仍绿 | 成立 |
| F-0005-3 | 空泡改成正常渲染文字 → `expect(size.height, greaterThan(0))` 仍真 | 成立（断言恒真） |
| F-0005-6 | 删 `message_bubble.dart:64-66` 的系统行分流 → 断言仍真（`:403` 还有同词） | 成立（同组另三条**有效**，已区分） |
| F-0005-7 | 删 `state_pill.dart:175` 的 `!reduced` → 本用例仍绿（树里根本没有 StatePill） | 成立 |

**共同根因（三条归一）**：**「测试名/断言理由」与「断言实际检查的对象」不是同一件**。三种变体：
1. 扫错文件（扫的是注释所在的文件，真实现身处别处）；
2. 断言不触达被测行为（测高度恒真 / 树里没有被测组件）；
3. 标识符扫描命中了「同词的另一处用法」，失效模式与断言理由无关。

**可复用的修法**（本仓已有现成件）：`test/chat_bubble_labels_test.dart:129-173` 的 `_stripComments` + `test/font_subset_test.dart:115-181` 的 `stringLiterals` 词法器。**判别力自证**应当成为所有源码扫描式守卫的固定配件：断言「若把被测符号改名/删掉，本测试必红」。

## 3. 同根因归并（主发现 / 派生）

### 组 A · **重建放大链**（本轮最有价值的单点）
- **主发现：F-0005-2**（每个 `text_delta` 重建整棵 AppShell 子树，含常驻树内的设置分区）
- 派生/被放大：F-0006-1（背景整图重解码）、F-0003-1（同一现象的旧记法）、F-0006-4（指针 setState）
- **修 A 的主发现一条，可同时降三处的成本。** 改法建议写进某个 backlog：外壳不要整体订阅 `_chat`；聊天列表、设置分区各自订阅。

### 组 B · **假绿灯三兄弟** — 见 §2。

### 组 C · **接缝无覆盖**（新增，本轮第二次遇到）
- F-0007-1（`UiStateTracker` ⇄ `ChatController`）、F-0005-4（`MessageBubble` 语义子树）、F-0003-6（`_ModConfigTile` 的 `identical` 判据）
- 形态：两侧组件各自都有真断言，**接缝本身**没有任何测试能覆盖（`ChatController` 依赖 `package:web`，VM 加载不了；`MessageBubble` 的语义被 `excludeSemantics` 折叠后，widget 树查找器看不见）。
- 建议：凡是「A 的输出喂给 B 的判据」这种接缝，把判据抽成纯函数（项目已有成熟范式：`turn_liveness.dart` 8 个判据 / 30 条真断言；`epoch_gate.dart` / `audio_gate.dart` 同款）。**这是 Phase 2 之后值得单独立项的工作项。**

### 组 D · **令牌/门禁写下了规则但覆盖不到自己** — F-0006-2（contentFaint 自注「仅装饰」却被当文字色 + 对比度门禁缺该组合）、F-0006-3（断点规则只匹配 `maxWidth` 字面量）、F-0005-5（字体门禁只覆盖源码字面量，不覆盖运行时文本）。三者同形：**门禁的覆盖面 < 规则的适用范围**。

## 4. 覆盖率（INDEX 为准）

- **lib：58 / 113 全读（51.3%）+ 30 个局部读**（`lib/main.dart`、`lib/app/shell_settings.dart`、`lib/settings/sections/dev_tools_section.dart`、`lib/live2d/live2d_stage.dart` 等只读了关键段）。
- **test：34 个文件**（含 6 份门禁型测试的**全读**）。
- **完全未审的目录**：`lib/api/`（11）、`lib/audio/`（6）、`lib/live2d/`（13）、`lib/voice/`（4）、`web/`、`tool/`、`pubspec.yaml`。
  → Phase 1 队列里 8/9/10/11 正好对应前四个，`web/+pubspec` 是 11。
- 已审域内**零发现**的部分：design/（5 文件）、state/（3）、chat/（5）、data/（5，B2）——**设计层质量明显高于接线层**。

## 5. 跨批模式提炼（7 批的真正产出）

1. **注释是这个仓库最强的资产，也是最强的伪装。** 每一处实现几乎都带一段「为什么这样写」的论证，正因如此，**注释与实现不一致的地方格外危险**（F-0004-3 的 sed 残留、F-0003-4 的星号、F-0006-3 的规则字面量都是「注释/理由说一套、代码做另一套」）。审计时凡读到长注释，必须反查它描述的代码是否还在。
2. **「派生值」是本项目最常见的 bug 温床**：派生值不随源刷新（F-0004-1）、派生值不驱动源（F-0001-2）、派生值与源不同步（F-0005-2/F-0006-1）。凡是「界面里有一份从服务端/存储取来的快照」，就要问「谁在它过期时刷新它」。
3. **高频信号进 Widget 树**这条纪律只被守住了「口型 30 Hz」（`stage_host` 的 RepaintBoundary + `no_backdrop_filter`），**没有被推广到「文本 delta（毫秒级）」与「指针移动」**——F-0005-2/F-0006-4 是同一条纪律的另外两个漏口。**纪律的推广方向应该是「凡是外部信号驱动的重建，都问一次：频率是多少、子树多大」。**
4. **接缝比组件更容易坏**（组 C）。组件级测试做得越好，接缝的盲区越显得安全。
5. **可测性被当作设计约束执行得很好**（`turn_liveness` / `epoch_gate` / `breakpoints` / `panelAlphaFor` / `emphasisSpans` 全是零依赖纯函数），但**这条纪律没有被强制**：凡是 `import 'package:web'` 的文件（`chat_controller`、`main.dart` 及 4 个 part）里的逻辑，默认进不了回归网——**F-0007-1 就住在那里**。

## 6. 质量自评（诚实版）

- 本 7 批共 24 条发现：P0 **0** / P1 **8** / P2 **11** / P3 **5**（含快照继承的 0001–0004 共 11 条）。
- **硬发现**（有原文摘录 + 跨文件调用链 + 可执行的反例/验证）：21 条。
- **推断类**（标了 未核实/中）：3 条 —— F-0005-2 的**幅度**、F-0006-1 的**耗时**、F-0005-5 的**触发条件**。机制层面三条都已闭环，只有「有多严重」需要真机。
- **本轮（0005–0007）新增 13 条里，4 条是可复现的假绿灯/守卫失效**（占 31%）。这是与前四批最大的差别：前四批找的是「缺陷」，后三批开始系统性地攻击**测试本身**——下一步（Phase 4 对抗日）应当继续这条线，因为本项目历史上 12 个浏览器 bug 全部逃过全绿测试。
- **自评里最该记的教训**：F-0005-2 我最初差点判成「内层 ListenableBuilder 会跳过重建 ⇒ 不是问题」，是读了 SDK 源码才确认成立；F-0006-4 我最初差点判成「鼠标移动会重建整个设置面板」，也是读 `framework.dart:4027` 的短路才排除。**两次都是「差一点报错」**——在长跑里，这比漏报更危险。
