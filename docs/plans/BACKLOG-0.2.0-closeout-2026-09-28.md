# 挂账清单（0.2.0 收尾会话产出）· 2026-09-28

> **性质**：本文件是**账本**，不是计划。每一条都给出**真源文件**与**本会话是否复核**。
> **纪律**（本项目纪律，写在这里防止下游误用）：
> 1. **账本里的行号/结论会腐烂**——动手前回源码重读，冲突以源码为准；
> 2. **本会话未复核的条目一律标「未复核」**，不冒充已核实；
> 3. 标「未核实/中」的**幅度类**结论不得被当成实测值。

## 1. Mod 侧 Rust 债务（**未复核**：出自 2026-09-15 五 Mod 审查，早于 rc.4）

真源：`docs/audit/2026-09-15-five-mod-review.md`（2026-09-28 已从旧 worktree 唯一副本入库）

| 组 | 条目 | 内容 |
|---|---|---|
| **P0（S1–S6）** | S1 | external/voice token 绕过 `.env` 真源 ⇒ 配了 token 却静默不鉴权 |
| | S2 | 动态装配路径不注入 `HostChannels` + 无 HostChannels 时 `say` 谎报成功 |
| | S3 | 保存任意 Mod 配置会**抹掉已存的 secret token** |
| | S4 | memory 会话注入把全局 `persona.system_prompt` 整段顶掉 |
| | S5 | Mod 启动失败：无日志、无 API 原因 ⇒ 界面只有 `failed` |
| | S6 | director 的 preset 实际形成一条**绕过 core 仲裁**的下行通道（治理问题） |
| **P1（M1–M12）** | M1–M4 | panic 拖垮全部 Mod · `TurnPrompt` flush 持 registry 锁 ≤1s · 事件内核「实现比文档弱」三处 · Mod 日志等级全变 `info` |
| | M5–M8 | 生命周期缺口（重复 enable 泄漏 runtime / 退出不关 Mod / 失败仍报成功）· 会话槽超限静默丢弃 · 命令面在 web_api 线程做文件 IO 与 spawn · voice sidecar 无互斥/无超时/stderr 全量入内存 |
| | M9–M12 | 脱敏与日志纪律两处缺口 · `mods.json` 存在即覆盖内建缺省且写回丢未知 id · memory 资源与内容边界 · `Number` 字段无 default ⇒ 界面回填 0 并写回 0 |
| **P2** | L1–L16 | 清理项（见真源 §4；**本会话未逐条核对**） |
| **速度（P-1…P-15）** | 真源 | `docs/research/2026-09-15-mod-chains-perf-and-single-session-memory.md` §2.2（按收益/成本排序的 15 条）。其中 **P-1**（4 个 Mod 对每个 TextDelta 逐条 `info` 且回显 payload）、**P-2**（每轮两次全量读同批记录）、**P-3**（订阅表只登记不路由）、**P-4**（`load(max)` 读完再 drain）、**P-5**（flush 持锁 ≤1s）是「大收益」五条 |
| **可读性** | §3 | 体积/注释/结构实测 + 「读起来最费劲的 5 处」+ 其他可读性问题 |

**注意（2026-09-28 提示）**：这份审查写在 **rc.4 之前**（`mod/l1-product` 时代）。
rc.4 之后 Mod 数、`mods.json` 写回、token 真源都动过 ⇒ **S1/S3/M6/M9/M10 很可能已部分修掉**，
**必须回源码重读再判定**（本会话只做背景域，未碰 Mod 侧）。

## 2. 动作 / 语音 / 记忆选型实施（**未开始**）

真源：`docs/plans/PLAN-actions-voice-memory-2026-09-15.md`（已跟踪）

| 章 | 范围 | 前置 |
|---|---|---|
| §1 | **动作系统选型**（标准皮套 / N.E.K.O 参考）：层级与仲裁、数据模型**外置**（不硬编码进 Rust）、用「语义通道」而非裸参数 ID | **实施前先改 `AGENTS.md` 的 director 台账与回归措辞**（本会话已在 rc.6/rc.7 把 AGENTS 版本线对齐，但**台账措辞未改**） |
| §2 | 按钮调试清单（放哪儿 / P0 按钮 / 最小改动 / 前端接线草图） | 同上 |
| §3 | **导演异步 LLM 链路**（TTS 输入 ↔ 动作同步）：异步旁路 + 规则兜底、输出契约是 **plan 而不是逐句 cue**、「导演是备注不是誊写员」 | 同上 |
| §4 | **主页语音 UI**（开关 + 按住说话 + 唤醒词三态按钮；ASR×唤醒×PTT 选型） | 现状 WIP 在 `mod/l1-product` 的**未提交**文件里；**该 worktree 已于 2026-09-28 按「已并入 main + 干净」移除**，分支 `mod/l1-product` 也已删除 —— 但**已提交内容在 main 历史里**，只有**未提交 WIP 从未入库**（属于「本地≠可以丢」的灰区，**本会话未取得那份 WIP**，如需请从 `/home/skystar/Live2D-Ai` 或维护者手里找） |
| §5 | 任务清单（建议顺序） | — |

## 3. 后端 R1–R8（**只记录，本会话不做**）

真源：`docs/plans/PLAN-frontend-redesign-2026-09-27.md`（2026-09-28 已入库）约 :250-257

| # | 需求 | 后端改动 | 本会话状态 |
|---|---|---|---|
| **R1** | 对话区**高亮正在朗读的那一句** | `text_delta`/`text_fallback` 帧加**可选**字段 `sentence_seq`（与 `audio` 帧同口径） | **唯一「不做就永远做不出来」的一条**（前端已有 `audio.sentence_seq`，正文没有对应编号 ⇒ 无法映射）。**rc.7 的 TRIAGE「不做」表已记账** |
| R2 | 舞台背景支持**铺法与位置** | `stage-bg` 扩字段 `{dataUrl, fit, align}`（扩字段不加帧） | **壳侧**铺法已在 **rc.6 做完**（`imageFit` 四档 + `tileSize` + 逐图覆盖）；**舞台（渲染面）侧仍需后端** ⇒ 本会话只完成壳侧 |
| R3 | 舞台图**解码成功与否**可观测 | 新增 `stage-bg-status {ok, reason}` 帧 | 未做（渲染面侧；rc.5 已有 HUD `bg: solid\|image` 被动信号） |
| R4 | 界面不给渲染面不支持的控件 | `capabilities` 加 `supports_stage_background_fit`/`supports_sentence_highlight` | 未做 |
| R5 | 「重新生成」带不同随机性 | `POST /api/v1/chat` 可选 `seed`/`temperature` | 未做（低优先） |
| R6 | 消息**编辑重发** | 按 `epoch` 回滚该轮的端点 | 未做（低优先） |
| R7 | 背景/外观**多端同步** | `GET/PUT /api/v1/display-prefs` | 未做（低优先；本产品是「这台设备长什么样」） |
| R8 | 背景图**以文件形式存进工作区** | 见真源 §5.1 | 未做（前端已用 IndexedDB，功能可用；只差字节不进仓库目录） |

**另（维护者 §8 点名，只记录）**：舞台**分区背景**、舞台**模糊** —— 需改后端（平台视图限制）；
**rc.7 的 TRIAGE「不做」表已逐条记账**。

## 4. 动作/表演波次 W0–W8 的未竟项（**未复核**）

真源：`docs/plans/HANDOFF-2026-09-21-actions-performance-round.md`（+ `…wave0-1.md`）

| 波次 | 交付状态（据真源） |
|---|---|
| W0 / W1 | **已交付 + 编排者独立核验**：W1 文档对齐 · W2 幅值重标定 · W3 调试面板 · W4 撤销语义（`none` 真归零）· W5 Rust 卫生 · W6 令牌走 `secrets::lookup` · W8/W8b 导演口径与文档漂移 · W2b 标定口径 |
| **W7** | **幅度下发通道整修（Wave 2，当时已放行）** + 自 Wave 1 转入的 **4 条欠账**：A 开关文案写实 / B 两个消费点优先标签表回落常量 / C `director_panel.dart` 三处文案 / D 三处陈旧注释（`live2d_stage.dart:156`、`shell_prefs.dart:31`、`live2d_bridge.dart:205`）。**本会话未复核它在 rc.4/rc.5 之后是否已清** ⇒ 需回源码重读 |

## 5. 本会话新增的挂账（rc.8 及以后）

真源：`docs/plans/TRIAGE-0.2.0-audit-45-2026-09-28.md` §5.1（**29 条审计发现逐条列出**）+ `docs/audit/2026-09-28-rc7/GROUNDING.md` §1/§3

| 项 | 内容 |
|---|---|
| **审计发现 29 条** | 组 C（F-0007-1/F-0005-4/F-0003-6）· 组 D（F-0006-2/F-0006-3/F-0005-5）· 组 E（F-0010-2/F-0001-4/F-0004-1）· 组 F（F-0012-1/F-0003-2/F-0003-4）· 其余 17 条（含 3 条 P1：`F-0001-1` / `F-0008-1` / `F-0012-1`） |
| **Stage C1** | `AGENTS.md` 单一化（以 main 版为底 + 补「前端显示层」节 + 变更历史补 rc.2/rc.3/rc.4）+ `v0.3.0.md` 收尾 + `asset_guard_director_cue_test.dart` 头注漂移行号（`:591`/`:481-483` → 实际 `:802`/`:693`） |
| **Stage C3** | 4 个超 1000 行文件拆分（`dev_tools_section` **1874** / `appearance_background` **1551** / `main` **1344** / `display_prefs` **1157**）+ 2 个超 800 行测试文件 + **W4 五项裁决**（可计算配色 / 自定义 accent / 三档密度 / 风格预设 / 设置区搜索框 —— 必须「做」或「正式减记」，不许继续悬空） |
| **rc.6/rc.7 的诚实项** | ① `F-0005-2` 只挡住「当前分区」子树，外壳/设置头部/`ChatPanel` 仍随 delta 重建，**幅度未真机 profile**；② 四档 fit 的 **`tile` 无像素级回归**（只有算术与结构级），四档像素级**只能靠 Win 肉眼**；③ **交互式肉眼 8 项**全部未做（CanvasKit 对 CDP 不暴露可交互无障碍树）；④ `state_pill._syncAnimation()` 仍不看 `reduced`；⑤ `BackgroundHydration.prefs` 派生视图待 C3 清理；⑥ `ui/field_row.dart` 的 `SliderField` 若补 `onChangeEnd`，应删掉 rc.6 在 appearance 区自建的防抖层 |
| **磁盘纪律（本轮新增教训）** | 24 个 worktree 的 `target/` 是磁盘黑洞（`-l1` 一个 35G）⇒ 本轮撞上 **ENOSPC（仅剩 48M）**，已按 `docs/audit/2026-09-28-rc7/REPO-HYGIENE.md` §4 处理。**建议：每次 `cargo build` 前先确认 worktree 数** |
| **未取得的一份 WIP** | `mod/l1-product` 里 §2 提到的**语音 UI 未提交改动**从未入库；该 worktree 已于 2026-09-28 移除（内容可重建，但那份 WIP 本身**不在 git 里**）。如需请向维护者确认是否还有副本 |
