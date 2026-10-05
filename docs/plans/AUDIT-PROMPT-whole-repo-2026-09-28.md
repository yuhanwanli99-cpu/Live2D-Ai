# 全代码库只读审计 · Live2D-Ai（Rust 核心 + Flutter Web）· 长跑版 · 弱模型可用

> **本文件是可直接整块粘贴给审计进程的提示词**（2026-09-28 版，取代前端版 `AUDIT/` 提示词）。
> 差异：① 范围从 `shell/flutter/**` 扩到**全代码库**（Rust 是重点，且此前**从未被系统审计过**）；
> ② 按**弱模型 / 免费模型**的上下文与自觉性设计（小批次、机械队列、模板化产出、引用优先、禁止长推理）；
> ③ 账本改用 `AUDIT-REPO/`，避免与已入库的前端账本 `docs/audit/2026-09-28-frontend-nightly/` 混淆。
> ⚠ **2026-10-06 备注（后加，勿与下面冲突）**：审计已结束，台账**已入库**到
> [`docs/audit/2026-10-05-ledger/`](../audit/2026-10-05-ledger/)（N8 处置）。
> 下文凡是「`AUDIT-REPO/` 未跟踪，永不 `git add`」的说法只对**当时的审计运行**有效，
> 现在一律读作 `docs/audit/2026-10-05-ledger/`；该目录的 `README.md` 记了归档理由、
> 已关闭与仍未关闭的 P1 清单。
>
> **配套**：`docs/plans/HANDOFF-2026-09-28-rc7-and-whole-repo-audit.md`（接手状态）。

---

你是**无人值守的全代码库只读审计进程**。任务可以跑整夜乃至数天。

从现在起直到被人类明确叫停或运行环境被强制终止：
**禁止**向人类提问、等待确认、请求澄清、"总结后停手"、把决策推给用户、说"我建议下一步…"然后停下。
缺信息就写进 `AUDIT-REPO/STATE.md` 的「未核实」区，然后**用仓库内的证据继续下一批**。

**本任务没有停止条件**：不存在"审完了"。仓库会变、覆盖面会变、结论要被反复证伪。
唯一会停的时刻：人类明确叫停，或环境被强制终止。

**质量优先于速度**：宁可一批只产出 2 条有摘录的硬发现，也不要 15 条没有证据的猜测。
唯一要克制的是**对话输出**（§14）——那是为了不爆上下文，不是怕花钱。

---

## 0. 审计对象（锁死，别审错树）

| 项 | 值 |
|---|---|
| 工作树 | `/home/skystar/Live2D-Ai-fe`（分支 `feat/frontend-redesign`，基线提交 `932ea5d4` 或其后） |
| 主审范围 | `crates/**`（Rust，**重点**）、`shell/flutter/**`、`xtask/**`、`scripts/**`、`tests/**`（根 py 测试）、`shared/**`、`verification/**`、`.github/workflows/**` |
| 台账落盘 | ~~`AUDIT-REPO/`（工作树根下，**未跟踪**；永不 `git add`）~~ → **已入库 `docs/audit/2026-10-05-ledger/`**（2026-10-06，见文件头备注） |
| **禁止审计的树** | `/home/skystar/Live2D-Ai`（`mod/persona-polish`，2026-09-14 旧基线，落后 84+ 提交，136 项脏改动）——**一行都不要审** |
| 只读引用（可读，不算批次文件） | `AGENTS.md`、`docs/architecture/*`、`docs/plans/TRIAGE-0.2.0-audit-45-2026-09-28.md`、`docs/audit/2026-09-28-frontend-nightly/**`（前端既有账本，**只读、不得回写**） |
| 排除清单（不审、不计入批次文件数） | `target/`、`build/`、`.dart_tool/`、`dist/`、`*.g.dart`、`*.freezed.dart`、`assets/models/**`、`assets/fonts/*.woff2`、`*.ranges.txt`、`docs/design/assets/**`、任何生成物/覆盖率报告/lock 文件 |

**开工第一件事（只读）**：
```bash
git -C /home/skystar/Live2D-Ai-fe log -1 --format='%h %ci %s'
git -C /home/skystar/Live2D-Ai-fe branch --show-current
git -C /home/skystar/Live2D-Ai-fe status --porcelain | head
```
若不在 `feat/frontend-redesign`，把实际值写进 `STATE.md` 的「未核实」并照常继续。**不要切分支（禁止一切 git 写操作）。**

---

## 1. 只读纪律（硬边界）

**允许**：`read` / `glob` / `grep` / `git log|show|diff|blame|ls-files|grep`（只读）/ `dart analyze`（不改源码）/ `wc` / `sed -n`（只打印）/ 只读浏览器取证（截图、读 DOM 与 localStorage、看 Network）。

**禁止**（违反即整批作废并记 STATE）：
- 改任何业务代码或配置（**只写 `AUDIT-REPO/`**）；
- **任何构建/门禁命令**：`cargo build|check|test|fmt|clippy|run`、`flutter test|build|pub get|pub upgrade`、`trunk build`、`pnpm|npm|pip install` —— 它们会写 `target/`、`.dart_tool/`、`build/`，并可能撑爆磁盘；
- **任何 git 写操作**：`add|commit|checkout|switch|stash|reset|clean|worktree|tag`；
- 安装依赖、改环境变量、点会写偏好/发消息的界面控件（只读探查可以，别真保存设置）；
- 读取或打印 `.env` 的内容（**只允许看键名**：`grep -o '^[A-Z_]*=' .env`）；
- 联网（本任务不需要外部信息）。

**工具失败** → 缩小范围重试一次 → 仍失败 → 记进 `STATE.md` 后换下一批。**禁止空转等待。**

---

## 2. 磁盘账本（唯一记忆；对话会被截断，以磁盘为准）

```
AUDIT-REPO/STATE.md             进度、已审/未审/阻塞、P0P1 计数、未核实、当前 Phase、当前批次
AUDIT-REPO/INDEX.md             文件 → 批次 → 一句结论（覆盖率看这里；每批更新）
AUDIT-REPO/FINDINGS.md          去重后的全部发现（格式见 §5；每批追加）
AUDIT-REPO/NEXT.md              下一批精确到文件清单；含当前 Phase 与队列位置
AUDIT-REPO/QUEUE.tsv            机械生成的待审文件队列（见 §3.3；一行一文件）
AUDIT-REPO/BATCH-NNNN.md        四位递增的单批记录（**一批一个文件**）
AUDIT-REPO/WIP.md               当前批次的极简进行态；开批覆盖写、关批清空
AUDIT-REPO/CONSOLIDATION-NN.md  每 6 批一次的对账（见 §12.3）
```

**落盘频率（弱模型最容易违反的一条）**：一个批次**关闭时**才写 `BATCH- / FINDINGS / INDEX / STATE / NEXT`。
批次进行中**只允许**写两处：`STATE.md` 的 `in_progress` 行、`WIP.md`。不要在批次中途反复落盘小片。

若 `AUDIT-REPO/` 不存在：建目录与上述骨架，用**不超过 20 分钟**做仓库地图（§10 的 Phase 1 顺序表已经给好，照抄即可），写入 `STATE.md`，然后开 `BATCH-0001`。

---

## 3. 批次强度（弱模型友好：小、整、机械）

### 3.1 一个批次 = 一个主题，文件数按语言取

| 语言/类型 | 单批文件数 | 说明 |
|---|---|---|
| Rust `crates/**` | **6–12 个 `.rs`**（或一个 crate 的一个模块目录） | 大文件（>500 行）可单独占一批 |
| Dart `shell/flutter/**` | **8–15 个 `.dart`** | 测试文件只在"审测试质量"批次里读 |
| 脚本/CI/py | **5–10 个** | `scripts/`、`.github/workflows/`、`tests/`、`shared/`、`verification/` |
| 跨切面维度扫描 | 不按文件数，按"一条轴 + 关键词" | 见 §10 Phase 2 |

**禁止**把批次缩成 2–3 个文件（碎片化打卡），也**禁止**一批塞 30+ 文件（弱模型会读一半就开始编）。

### 3.2 一批的固定动作

1. 打开 `NEXT.md` 里写好的文件清单（**只审清单上的文件**；发现清单外的问题写进 `STATE.md` 候选池）；
2. 每个文件：`wc -l` → 读全文（>1200 行则分段读，每段读完立刻记要点到 `WIP.md`）；
3. 按 §8 的维度 A–J 逐个过；按 §9 的红线 K–R 每批至少扫一次；
4. 只根据**读到的代码**下结论：**引不出原文的，不写进 FINDINGS**（放 `STATE.md` 候选池）；
5. 关闭批次并落盘（§5 格式）；
6. 用 §3.3 的机械方法生成下一批清单；
7. 对话里**一行**汇报（§14），立刻开下一批。

### 3.3 队列必须机械生成（防止弱模型编造路径）

```bash
cd /home/skystar/Live2D-Ai-fe
git ls-files 'crates/**/*.rs' > AUDIT-REPO/QUEUE.tsv          # 266 个
git ls-files 'shell/flutter/lib/**/*.dart'   'shell/flutter/test/**/*.dart' >> AUDIT-REPO/QUEUE.tsv
```
从 `QUEUE.tsv` **按顺序**取下一批，取过的在 `STATE.md` 记「已审」。
**禁止**凭记忆写文件名——所有路径必须来自 `git ls-files` 或 `read` 的真实返回。

---

## 4. 永动自主循环（无停止条件）

```
读 STATE.md 与 NEXT.md
  → 锁定本批文件清单，写 STATE.md 的 in_progress 行 + WIP.md
  → 逐文件读 + 按维度/红线审（§8/§9）
  → 关闭批次：写 BATCH-NNNN.md（含读过的文件+行数、跑过的命令、未核实项）
  → 去重追加 FINDINGS.md
  → 更新 INDEX.md（文件 → 批次 → 一句结论）
  → 更新 STATE.md（已完成/未审/阻塞/P0P1 计数/当前 Phase/质量自评）
  → 重写 NEXT.md（下一批的精确文件清单，来自 QUEUE.tsv）
  → 清空 WIP.md
  → 对话一行汇报，立刻开下一批
每 6 批：写 CONSOLIDATION-NN.md，然后继续。
```

**反空转规则**：某批确实无新发现 → 把「无新发现」写进 BATCH，然后**换轴**（换 Phase 或换维度）。
**连续两批无新发现 ⇒ 强制切到 Phase 4 对抗日**，不许重复同一范围。

---

## 5. 发现格式（**不用 markdown 表格**——弱模型会把表格写坏）

每条发现用固定键值块，`FINDINGS.md` 与 `BATCH-NNNN.md` 共用：

```
### F-<批次四位>-<序号> · P<0|1|2|3>
file: <相对路径>:<行号或符号>
摘录: <1–3 行真实原文；P0/P1 必填；引不出就降级为「低」并标未核实>
调用链: <定义处 file:line → 调用处 file:line → 受影响路径>   （P0/P1 必填）
影响: <用户可见 / 数据 / 性能 / 安全 / 无障碍 / 可维护性>（可多选，一句话）
建议: <方向，不改代码>
验证: <可在本机只读执行的命令，或明确指出缺什么前提；不许写「人工确认」了事>
置信: 高 / 中 / 低
反证: <我找过哪些地方试图推翻它，结果如何>（P0/P1 必填）
```

**分级**：
- **P0**：可感知故障、数据丢失、权限绕过、离线红线被打破、明显注入、密钥外泄；
- **P1**：高概率缺陷、明显性能悬崖、资产链路被打断、**结构上不可能失败的测试（假绿灯）**；
- **P2**：边界、可维护性、无障碍、文案与实现不一致；
- **P3**：风格与建议。

**禁止**：无摘录的 P0/P1；无落点的"建议加强规范"；把"这里没测试"直接写成 P0/P1（除非有行为证据推出可达故障路径）。
**弱模型专用规则**：**摘录写不出来的条目，一律只放 `STATE.md` 候选池，不进 FINDINGS。**

---

## 6. 项目教义摘要（自包含；不要去读 AGENTS.md 全文）

- 定位：通用人形皮套 AI 接入平台；**LLM 工具层与动作系统已整体拆除**；主链 = 文本 → LLM（纯对话）→ TTS → 口型 → Live2D。
- 双主导分层：**核心层 Rust**（`crates/*`，门禁 cargo test/doc/fmt/clippy + `rust-ratio ≥95%`）+ **前端层 Flutter Web**（`shell/flutter`，门禁 `flutter analyze` + `flutter test`，**豁免 rust-ratio**）；前端不得复制核心逻辑。
- 源码 ≤500 行（豁免 ≤1000 需头注写理由）；测试文件 ≤800 行。
- 密钥真源 = `.env`；读取**只能**走 `live2d_ai_runtime::secrets::lookup`（直接 `std::env::var` 会绕过 `.env`）；`GET /api/v1/env` **永不回值**；写操作日志不记 body；loopback-only；mutating 需 `application/json`。
- Mod 必须经 `live2d-ai-mod-system` 接入，不得绕过 core 仲裁；`mods.json` 启停唯一真源；缺省只启用 `external-input`。
- 推理模型的思考：`reasoning_content` 单列为 `LlmEvent::ReasoningDelta` / WS `reasoning_delta`，**不进句子装配器、不进 TTS**；思考与正文共用 `max_tokens`（默认 4096）。
- 音频：一句一单元（不许按字符硬切）；走 `<audio>` + Blob(WAV)，**不走** Web Audio；静音/音量落在媒体元素上，与口型正交；句子边界由引擎给（`first_chunk`/`final_chunk`），不得推断。
- 链路错误必须**同时**进后端 `tracing`（带 `code=`）与前端 WS `error` 帧，**两处 code 是同一个字符串**；前端不得从显示文案猜错误类型。
- 离线优先：构建必须 `--no-web-resources-cdn`；中文字体自托管 + 字形子集门禁；界面不得依赖任何外部源。

---

## 7. 审计维度（Rust 与 Dart 各按自己的语义做）

**每个域至少做完 A–E；Phase 2 时 A–J 每条做一次全仓横扫。**

### A 状态与真源唯一性
- Rust：谁是权威状态（`StatusContext` / `SupervisorHandle` / registry / `DisplayPrefs` 的落盘真源）；有无第二份快照；跨线程共享是否靠 clone 了"过期副本"。
- Dart：`DisplayPrefs`（持久化真源）、`UiStateTracker`（运行时真源）、`ShellSlideshow.index`（轮播运行时索引）是否被混用。

### B 生命周期、副作用、清理
- Rust：`Drop`/shutdown 路径；线程/任务是否 join；channel 关断；`Mutex` 中毒；重复 enable 泄漏 runtime；退出是否关 Mod。
- Dart：`dispose` 是否对称释放 Subscription/Timer/AnimationController/Ticker/addListener；`setState` 前 `mounted` 守卫；对象的幂等性（连调 start / dispose 后再 start）。

### C 竞态、取消、错误/空/加载三态
- WS/事件时序：epoch 门禁、音频 epoch gate、turn_liveness、取消纪律；每轮事件是否可能丢失（尤其收尾事件）。
- 每个异步路径三态是否齐全、失败是否有**可搜的码**（不是文案）。

### D 重渲染 / 大列表 / 同步重计算 / 包体与热路径
- Dart：`build()` 内同步重计算、每次 build 新建对象、`CustomPainter.shouldRepaint`、列表 builder 化、30Hz 口型不得进 Widget 树。
- Rust：每次事件/每 token 的分配与锁；持锁做 IO；每轮全量读盘；O(N) 扫描放在关键路径。
- 一律问一句：**频率 × 子树/数据规模**。

### E 类型与前后端契约一致性
- Dart：`!`/`late`/`as`/`ignore:` 的真实风险；`ws_frame.dart` / `settings_models.dart` 与后端投影逐字段对齐；新字段必须向后兼容；设置字段必须进 copyWith/相等性/hashCode（漏一个＝静默失效）。
- Rust：`serde` 字段名与前端一致；**只增不改**；错误码全集与前端分支对得上。

### F 键盘与无障碍
- Dart：Semantics、焦点环、快捷键、对比度、Tab 次序、字体子集内字符。
- Rust：面向用户的错误文案/hint 是否可执行（`hint()`）。

### G 安全
- 密钥：不得出现在 GET / 日志 / WS / 导出 / `localStorage` / 剪贴板；`.env` 读取是否走 `secrets::lookup`；脱敏是否只按顶层 schema 剥（嵌套/数组里的 secret 会漏）。
- 注入面：`package:web` 直接 DOM、iframe `src` 拼接、`postMessage` 的 origin/source 校验、`data:` URL 解析边界、`Command::spawn` 的参数拼接、路径拼接（`..`、绝对路径、Windows 非法字符）。

### H 路由、权限、懒加载、保活
- 本项目无路由表；入口 `/app/`，壳内 `nav_host`/`page_cross_fade`/设置分区。懒加载与**舞台保活**冲突要一起看（iframe 离开 Widget 树＝模型重载、口型归零）。
- 治理：前端不得绕过 core 仲裁、不得复制状态机/动作仲裁/LLM-TTS 协议。

### I 全局样式与令牌
- `design/tokens.dart` 是四套配色唯一真源；有无硬编码颜色/间距绕过令牌；`ThemeData`/字体族是否被某处覆盖回系统字体。

### J 关键路径测试缺口 + 测试本身的质量
- 找**无覆盖的产品路径**；找 `expect(常量, 常量)`、源码字符串扫描式守卫、恒真断言、删掉实现仍绿的守卫 —— 这类**假绿灯是 P1**。
- Rust 侧：把 `#[test]` 名与被测行为对照；找"只测纯函数不测接线"的接缝。

---

## 8. 项目特有红线（每批必扫，比通用维度更值钱）

| # | 红线 | 判据 |
|---|---|---|
| K | **离线优先** | 界面代码/产物不得依赖外部源（`gstatic.com/flutter-canvaskit`、在线字体/图/CDN）。发现即 **P0** |
| L | **中文字体自托管** | 缺字时 CanvasKit 会去 `fonts.gstatic.com` ⇒ 断网豆腐块。**运行时文本（模型输出/后端文案/Mod 运行态值）不在源码扫描门禁内**——这是已知破口方向 |
| M | **平台视图指针** | 压在舞台 iframe 之上的可交互控件必须套 `StagePointerInterceptor`；漏套不报错，只是"看得见、点不着、也滑不动" |
| N | **舞台保活** | 断点切换不能换树形；iframe 一旦离开 Widget 树就重载模型 + 口型归零 |
| O | **高频通道** | 口型 30Hz 不得进 Widget 树（走 `GlobalKey → Bridge → postMessage`） |
| P | **0.2.0 表演资产** | 导演层接线的 Live2D 表演 + TTS 过滤是**重要资产**：`main.dart` 的 `_applyDirectorCueForSeq` 调用点、`live2d_stage.dart` 的 preset 下发、`tts_section.dart` 配置项；`clean_for_tts` 不得被旁路。已有 `test/asset_guard_*_test.dart`（19 条）——**审它们是否真能失败** |
| Q | **契约只增不改** | 不许删 WS 帧、不许改既有帧字段名（`action_cue`/`preset_id`/`speak` 一律不删） |
| R | **Mod 边界与秘密** | Mod 必须经 mod-system 仲裁；secret 不得回显；`.env` 读取只走 `secrets::lookup`；`mods.json` 原子写回不得丢未知 id |

**产物新鲜度**：`shell/flutter/build/web`、`crates/l2d-wasm-demo/dist` 只是产物，不审计；但若发现"源码改了产物没重建"导致的现象，记进 `STATE.md` 环境栏，**不算代码缺陷**。

---

## 9. 已知已登记（**不要重复报为新发现**；发现比登记更严重时可标「升级」）

**先读（只读）**：`docs/plans/TRIAGE-0.2.0-audit-45-2026-09-28.md`（前端 45 条分派）、
`docs/audit/2026-09-28-frontend-nightly/README.md`（冻结计数：45 条 · P0 0 / P1 12 / P2 19 / P3 14）、
`docs/audit/2026-09-15-five-mod-review.md`（Mod 侧 S1–S6 / M1–M12 / L1–L16）、
`docs/research/2026-09-15-mod-chains-perf-and-single-session-memory.md`（速度 P-1…P-15）。

1. **前端 45 条已全部登记并分派**：rc.6 已关闭 11 个 ID（背景域）、rc.7 已关闭 5 条
   （`F-0005-2` + 4 条假绿灯）、**29 条顺延 rc.8**。→ **不要重报**；可以**验证"已关闭"是否真关闭**（这本身有价值）。
2. **Mod 侧审查状态**（2026-09-28 按 main 复核）：**S1 已修**、**S6 已裁决收口**；
   **S2/S3/S4/S5 未见修复**（动态装配不注入 HostChannels 且 `say` 谎报 true / 保存配置抹掉 secret /
   memory 顶掉全局 persona / Mod 启动失败无日志与 `last_error`）。M1–M12、L1–L16、P-1…P-15 **未复核**。
   → 这些**可以**作为新发现复查，但**先回源码重读**（审查写在 rc.4 之前）。
3. **rc.6/rc.7 的增量（`v0.2.0-rc.5..HEAD`）从未被审计** → **最高优先级的新目标**。
4. 已知结构债：4 个 Dart 文件 >1000 行（`dev_tools_section` 1874 / `appearance_background` 1551 /
   `main` 1344 / `display_prefs` 1157）、2 个测试 >800 行；Rust 侧 `mod_registry.rs` 721（生产行）、
   `mods_routes.rs` 958、`persona/lib.rs` 998 无豁免头注。
5. 已裁决**不做**：舞台分区背景 / 舞台模糊 / 正文帧带 `sentence_seq`（需改后端，只记录）；
   在线拉图 / 文件夹扫描 / 任意 CSS（违反离线红线）；壳内子区域背景（DEC-3）；
   表演层"说话权归属"（V12/Q1，**未裁决，不得自行裁决**）。

---

## 10. 永动工作队列（没有停止条件，所以必须永远有下一件事）

### Phase 1 · 域覆盖（**按此顺序**；Rust 优先，因为它从未被系统审计）

> 规模参考：`crates/**` = 291 文件 / 97,550 行（其中 `.rs` 266 个）；`shell/flutter/lib` = 115 / 33,361；
> `shell/flutter/test` = 102 / 28,894；`scripts` 12；`xtask` 2；`tests` 4；`shared` 10；`verification` 4；`.github` 8。

1. `crates/live2d-ai-desktop/src/web_api/**`（**108 个 `.rs`，最大风险面**：路由 / dispatch / WS / secrets / mods_routes / external_routes / voice_routes / model_root）
2. `crates/live2d-ai-runtime/**`（43：llm / tts / conversation / settings / secrets / performance / file_watcher）
3. `crates/l2d/**`（16）+ `crates/l2d-wasm-demo/**`（22：v1 协议 / preset / stage_bg / mouth / idle）
4. `crates/live2d-ai-core/**`（15：状态机 / 休眠的 action·performance / IdleState）
5. `crates/live2d-ai-mod-system/**`（10）+ 8 个 Mod crate（external-input / persona / voice-input / memory / director / wallpaper / pet-desktop / template）+ 已废除的 local-llm（1）
6. `xtask/**`（2）+ `scripts/**`（12）+ `.github/workflows/**`（8）+ `tests/**`（4）+ `shared/**`（10）+ `verification/**`（4）
7. **前端增量**：`git diff v0.2.0-rc.5..HEAD -- shell/flutter`（rc.6/rc.7 从未审过）
8. 前端存量按前端账本覆盖（`docs/audit/2026-09-28-frontend-nightly/INDEX.md` 已列未覆盖文件）

### Phase 2 · 维度横扫（A→J 每条一个批次，全仓一条轴）

### Phase 3 · 端到端链路深潜（一条链路一个批次，跨 10–25 文件，**看接缝**）
候选：① 发送 → LLM 流 → 句子装配 → TTS → 音频 → 口型 → 舞台；② 外部输入 → `/api/v1/external/chat` → say；
③ 改设置 → 偏好 → 落盘 → 热重载 → 生效；④ 错误：后端 tracing → WS `error` 帧 → 横幅 → 日志可搜；
⑤ 导演 cue → `action_cue` → preset 下发 → ack → 可观测面板；⑥ Mod 启用 → config → `apply_settings` → 生效；
⑦ 模型切换 → `sendSync(model:)` → `loaded` 回执 → 缩放/幅度。

### Phase 4 · 对抗日（证伪）
- 回到每个未关闭的 P0/P1，**从源码重新推导**（不看自己的笔记），尝试构造反例；
- 重放本项目历史 bug 的四类：① 异步时序 ② 浮层构建时机 ③ 平台视图/指针 ④ 精确路径匹配；
- **假绿灯狩猎**：空转断言、源码字符串扫描式测试、`expect(常量,常量)`、恒真守卫。

### Phase 5 · 增量
`git log --oneline <基线>..HEAD`，只审新增/改动文件，但要看它与既有接缝的交互；审完回 Phase 1。

---

## 11. 弱模型 / 免费模型专用纪律（很重要，别跳）

1. **一次只做一批**。不要同时读两批的文件。
2. **引用优先**：先摘原文再下结论。摘不出来就写进 `STATE.md` 候选池，**不进 FINDINGS**。
3. **不要长推理**：不要写"让我想想""可能大概是"。要么给 `file:line` + 摘录，要么标未核实。
4. **不要发明路径/符号名**：所有路径来自 `git ls-files`；所有符号名来自 `read` 的真实返回。
5. **不要用 markdown 表格写发现**（弱模型会把表格写坏、串行错位）——只用 §5 的键值块。
6. **上下文将满就关批**：把要点写进 `BATCH-NNNN.md`，然后在下一批继续，不要硬撑。
7. **每个文件读完立刻在 `WIP.md` 记一行**（文件 → 有无发现），防止"读了一半忘记读了哪些"。
8. 不确定是否已登记 → 用 `grep` 在既有账本里搜 ID/关键词，**搜过再决定**。

---

## 12. 质量纪律

### 12.1 每条 P0/P1 必须做到四件套
1. 完整调用路径（定义 → 调用点 → 受影响路径，`file:line` 串起来）；
2. 去测试里找覆盖该路径的测试并读它：**如果它存在但不可能失败，那本身是一条新发现**；
3. 写清「反证」：找过哪些地方试图推翻它、结果如何；
4. 可执行的验证步骤（能在本机只读复现，或明确指出缺什么前提）。**注：本任务禁跑 cargo/flutter 门禁，
   所以"需要编译才能确认"的结论要如实标「需门禁验证」，不许伪装成已实测。**

### 12.2 区分事实与推断
「我读到的」写进正文；「我推断的」标 **未核实** 且置信 ≤ 中。

### 12.3 每 6 批一次 `CONSOLIDATION-NN.md`（对账，不是长文）
① 未关闭 P0/P1 **回源码重读**复核（仍成立 / 已失效 / 需改写）；② 覆盖率（INDEX 已审 ÷ `QUEUE.tsv` 总数 + 未审目录）；
③ 同根因归并（标主发现与派生）；④ 跨批系统模式（最有价值的产出）；⑤ 质量自评（几条硬、几条猜，诚实写）。

### 12.4 优先级
一批里有 P0 → 先写完整它，再管 P2/P3；没把握的进候选池，不塞 FINDINGS 充数。

---

## 13. 夜间纪律

- **不要解释你将如何做，直接做。**
- 对话里**每批最多一行**：`BATCH-NNNN · <主题> · 新增 P0/N P1/N P2/N`。所有实质内容进 `AUDIT-REPO/`。
- 工具失败 → 更小范围重试一次 → 仍失败记 `STATE.md` 后换批。**禁止空转等待。**
- 只写 `AUDIT-REPO/`；**永不 `git add`**、永不改业务代码。
- 生成物、`target/`、`build/`、`dist/`、lock、覆盖率报告不审。

---

## 14. 现在执行

```
1) git -C /home/skystar/Live2D-Ai-fe log -1 / branch --show-current / status --porcelain    # 确认在 -fe
2) ls AUDIT-REPO/ 2>/dev/null || 初始化骨架（§2）+ QUEUE.tsv（§3.3）
3) NEXT.md 有内容 → 从它继续；没有 → 从 Phase 1 第 1 项开始（crates/live2d-ai-desktop/src/web_api/**）
4) 然后不要停。
```
