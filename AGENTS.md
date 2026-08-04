# ⛔ SPOQ — Plan-and-Execute 编排规则

> Phase 0: 自由探索（完全智能体模式）
> Phase 1: 结构化规划（智能体模式）
> Phase 2: 严格执行（状态机模式）
> **Phase 切换点：用户确认方案**

## ⚠️ 适用范围说明（与 Orca 编排的关系，v7 已更新）

本文件描述的 SPOQ 状态机，是 **Pi agent 自身内部** 派发 sub-agent（architect / developer /
tester）时使用的编排规则：其状态存于仓库内 `.pi/spoq-state.json`，由 `spoq-enforcer.ts`
负责状态转换校验与硬性拦截。

它 **不** 管理、也不描述 Pi agent 本身是如何被外部系统（例如 Orca 编排系统）调用、监督或
派发任务的——那是另一层、由外部编排方（Orca 的 terminal/dispatch 机制）负责的机制，两者
使用完全独立的通道（SPOQ 状态在 `.pi/spoq-state.json`；Orca 的任务/心跳/上报走它自己的
task/dispatch 协议），互不冲突、也不会相互干扰。

**v7 修正（2026-08-04）**：v2-v6 曾规定"Pi agent 作为 Orca 派发的 worker 运行时，SPOQ 状态机
不介入"——这条排除规则已废止。原因：外部协作方（GitHub Copilot CLI）明确指出，本项目让
Pi agent 承担"团队"角色的核心价值是**成本**（把重活拆给更便宜的 sub-agent 模型并行跑）和
**可靠性**（Tester 子代理的独立证据门禁能防止"看起来做完了但其实没做对"的假完成），而这
两个价值只有在 SPOQ 真正被触发时才存在。如果"作为 Orca worker 运行"本身就永久禁用 SPOQ，
而 Pi agent 事实上**从未**以"完全独立、非 Orca 派发"的方式被使用过，那么 SPOQ 就永远是
摆设，与它被造出来的目的矛盾。

**新规则**：无论任务来源是用户直接对话、还是 Orca 派发的 worker 任务，一律按任务本身的
复杂度判断（见下方"简单任务 vs 复杂任务"表）决定是否走 Phase 1/2。命中 complex 时，即使
当前是作为 Orca worker 运行，也必须：

1. 正常写 `.pi/spoq-state.json`（`phase: "executing"`）、拆 DAG、按 Wave 派发
   architect/developer/tester 子代理（`Agent(..., run_in_background: true)`），完整走完
   硬转换表直到该任务 `done`；
2. 内部 SPOQ 循环运行期间，**不要**提前向 Orca 上报 `worker_done`——Orca 侧看到的仍然只是
   "这个 worker 任务还在跑"，直到 Pi agent 内部所有子任务 `done/blocked` 后，才产出一次
   汇总结果并按 Orca 协议正常上报 `worker_done`。对 Orca 而言，内部是否用了 SPOQ 完全透明，
   它不需要感知、也不应该被内部 Wave/Agent 调度打断心跳。
3. 简单任务（单文件/单步操作，见判断标准）仍然允许跳过 SPOQ、直接实现——这与之前一致，
   没有变化。

## v2 规则来源声明（轻量化）

为避免多文件漂移，状态与转换的唯一规则源为：`.pi/spoq-state.schema.md`。  
本文件与 `.pi/agent-loops/*.md`、`.pi/extensions/spoq-enforcer.ts` 仅做执行说明。

## v3 规则硬化说明（2026-07-28）

`.pi/agent-loops/*.md` 不再需要子代理主动 `read`：`spoq-enforcer.ts` 在 `before_agent_start` 时
根据派发 prompt 中的角色标记自动探测角色（architect/developer/tester），把对应
agent-loop 全文 + 最近 3 条 `lessons-learned.md` 教训直接注入子代理的 systemPrompt。
`agent-loops/*.md` 文件本身仍保留作为人类可读文档源，但不再是运行时必须 read 的对象。

同时新增：

- **角色错乱自动检测**：`message_end` 钩子扫描子代理输出，命中"作为 Orchestrator/拆任务派代理"等
  关键词时自动写 `any→orchestrator-*.md`，不再依赖子代理自觉上报。
- **上下文占用监控**：`agent_end` 钩子读取 `ctx.getContextUsage()`，占用 ≥70% 时输出告警，提示
  Orchestrator 考虑 `/compact` 或进一步拆分任务。

## v4 状态机硬转换 + 模型锁定说明（2026-07-29）

**转换表从文档挪进代码**：`spoq-enforcer.ts` 新增 `tool_call` 前置拦截（对 `write` 到
`.pi/spoq-state.json` 的调用），在写入落盘前对比新旧 `task.state` 字段，校验是否命中
`spoq-state.schema.md` 里 T1-T16 硬转换表允许的迁移路径；非法迁移直接 `block`，Orchestrator
的写入请求根本不会执行——比原先"写后校验+从备份回滚"更干净，避免非法状态曾短暂落盘。

**complexity / needsVisualEvidence 客观信号校验（仅警告）**：同一钩子里对 `description`
做关键词扫描——含 UI/视觉词（`.wxml`/`.wxss`/截图/像素等）但 `needsVisualEvidence=false`，
或含多文件/架构变更词但 `complexity=simple`，会打印警告日志（不阻断），供人工/Orchestrator
复核，防止弱模型为图省事把复杂任务错标为 simple 以跳过 Architect。

**模型锁定改为原生 frontmatter 机制**（而非扩展 hook）：`@tintinweb/pi-subagents` 的
agent frontmatter（`~/.pi/agent/agents/*.md`）里的 `model` 字段对 `Agent({model:...})`
调用参数有最高优先级，调用方无法覆盖。据此：

- `software-architect.md`：`model` 升级为 `deepseek/deepseek-v4-pro`（设计错误传播成本高，
  且低频调用，升级成本可接受）
- `developer.md`：保持 `deepseek/deepseek-v4-flash`（执行密集型，flash 足够）
- Tester 拆分为两个 agent 定义：
  - `tester.md`（默认/纯文本）：`deepseek/deepseek-v4-flash`
  - `tester-visual.md`（新增，视觉必需）：`zhipu/glm-4.1v`（免费视觉模型，注意其像素级精度
    可能弱于付费的 glm-4.6v，涉及精确测量优先走程序化/DOM 测量而非纯视觉判断）
  - Orchestrator 按 `task.needsVisualEvidence` 字段二选一派发，不再由模型自行判断该用谁

所有 agent 定义新增 `disallowed_tools: Agent, get_subagent_result, steer_subagent`，
作为工具允许清单之外的第二道防线，防止角色错乱的子代理反过来派发子代理。

`subagents.json`（项目级 + 全局级）新增 `scopeModelsEnabled: true`，对有效生效模型做
`enabledModels`（`deepseek-*`/`glm-*`）二次校验，调用方越权模型直接报错。

**教训提取机械化**：Architect/Developer/Tester 的报告/邮件模板新增固定格式的
`## 教训候选` 区块（`[类别] 教训：... 标准：... 建议：...`），Orchestrator 的 SAVE 步骤
改为**原文摘抄**这些区块进 `lessons[]`，不再要求 Orchestrator 自己总结/判断哪些内容
算"教训"——这类语义提炼超出 deepseek-v4-pro 作为纯状态机执行器的可靠能力带，机械化
摘抄后 Orchestrator 只需做格式匹配，不需要理解内容。

---

## v5 证据门禁 + 遥测 + 一致性检查（2026-07-29）+ 关键 bug 修复

**🔴 修复了一个从未生效的 bug**：v3/v4 版本里 spoq-state.json 的备份/回滚逻辑挂在
`pi.on("after_tool_call", ...)` 上——但 `"after_tool_call"` 根本不是本框架的合法事件名
（核对 `@earendil-works/pi-coding-agent` 官方 `types.d.ts` 的 `on(event: ...)` 重载列表，
只有 `tool_call`/`tool_execution_start`/`tool_execution_end`/`tool_result` 等），导致这段
备份/校验代码从写下的那一刻起就从未真正执行过。已改为合法的 `tool_execution_end` 事件。

**task-tester-evidence-gate（写前拦截，不是写后检查）**：Tester/tester-visual 写
`test-{id}.md` 报告时，若正文出现"结果: PASS"，扩展会在**写入真正落盘前**校验：
- `tester`（纯文本档）：本次子代理会话必须确实执行过至少 1 次 `bash`（通过
  `tool_execution_end` 计数），否则整个 write 直接 `block`，报告写不进去。
- `tester-visual`（视觉档）：报告正文必须引用至少 1 个磁盘上真实存在的截图/证据
  文件路径（markdown 图片语法或裸路径），否则同样 `block`。

**task-schema-conformance-check（仅警告）**：任务刚进入 `dev_done` 且
`complexity=complex` 时，扩展读取 `plan-{id}.schema.json` 里所有形如
`name/function/field/method/endpoint/path` 的标识符，和 `srcPath` 目录下所有源文件的
文本做一次朴素子串匹配；找不到的标识符只打印警告（纯文本匹配有误报可能，不适合做
硬阻断），供 Tester/人工复核"疑似漏实现"。

**task-telemetry-log**：新增 `.pi/spoq-telemetry.jsonl`，每行一个 JSON 事件，记录
`transition`（每次实际发生的状态迁移）、`transition_blocked`（非法迁移被拒绝）、
`heuristic_warning`、`schema_conformance_warning`、`evidence_gate_blocked` 五类事件，
带时间戳。用于事后诊断，不再需要翻整段 session 记录。

---

## v6 工作逻辑对齐说明（外部协作方对齐，2026-08-04）

外部编排方（GitHub Copilot CLI，作为 Orca worker 派发方 / 架构监督方）反馈：本仓库此前
两次 Orca 派发任务（tests/test_desktop_config.py 路径修复、README/docs 路径修复、骨架插件
测试隔离、AGENTS.md 分层说明）都是 Pi agent 以主代理身份直接改代码完成的，**完全没有进入
Phase 1/Phase 2、没有写 `.pi/spoq-state.json`**——这与"简单任务跳过 Architect"的既有设计
并不冲突（这类任务确实够简单，直接做是对的），但暴露出两条尚未成文的行为缺口，现补充为
铁律 8/9：

- 之前 `.pi/lessons-learned.md` 长期是空模板、`spoq-state.json` 长期停在
  `{"phase":"planning","currentWave":-1}` 的现象本身不是 bug——只是因为目前所有已完成工作
  都走了"简单任务直接做"路径，从未真正触发过 Phase 1 DECOMPOSE。SPOQ 状态机代码
  （`spoq-enforcer.ts`）是真实生效的，只是尚未被用在需要它的复杂任务上。

**铁律 8：含糊需求先问，不猜测实现**
无论走 SPOQ 完整流程还是"简单任务直接做"路径，只要需求存在多种合理实现方式、影响
范围不确定、或验收标准不清晰，必须先向用户 / 派发方提问澄清（Phase 1 的"向用户提问"
不再只是 Phase 1 专属——简单任务直通 Phase 2 时如遇到同样的含糊性，也必须先问，不能
因为走的是简化路径就默认可以自行猜测后直接实现。

**铁律 9：完成前必须自证，不能"改完就算完"**
任何任务（无论是否经过 Tester 子代理）在报告完成（`worker_done` / 向用户汇报）之前，
必须实际重新跑一遍相关的现有测试/构建命令，并复核自己的 diff 是否只做了该做的改动、
没有引入无关变更。仅仅"文件已保存/编译通过"不构成完成证据；报告里必须包含具体的
验证命令与输出结果（例如 `pytest` 的通过数、`gradlew` 的 BUILD SUCCESSFUL），而不能只
描述"已修复"。task-tester-evidence-gate 现有的"至少跑过 1 次 bash"仅是弱代理指标，
不能替代真正针对性的重跑验证。

---

## v7 SPOQ 全场景启用（外部协作方对齐，2026-08-04）

外部协作方进一步反馈：v6 虽然澄清了"Orca worker 模式下 SPOQ 代码本身是真实生效的"，
但顶部"适用范围说明"里"作为 Orca worker 运行时，SPOQ 状态机不介入"这句话，实质上是一条
**永久排除规则**——而本仓库迄今所有任务都是通过 Orca 派发的，从未有过"Pi agent 完全脱离
Orca、作为用户的独立主代理"这种场景。这意味着这条排除规则等价于"SPOQ 永远不介入"，
是此前两轮验证 SPOQ 始终未触发的**真正根因**（而不是任务粒度粗细、也不是子代理不主动）。

已废止该排除规则（见上方"适用范围说明"v7 更新内容）：现在，判断是否走 Phase 1/2 只看
任务本身复杂度，不再看任务来源是不是 Orca 派发。Orca 只看到一次 `worker_done`，内部
是否用了 SPOQ 对它透明。

**铁律 10：任务来源不影响 SPOQ 判断**
是否进入 Phase 1/2，只依据任务复杂度（见"简单任务 vs 复杂任务"表），与任务是用户直接
交代、还是 Orca 派发的 worker 任务无关。作为 Orca worker 运行 ≠ 自动跳过 SPOQ。

---

## 🔴 铁律

1. **Phase 0 绝对自由** — 可以读代码、搜索、推理、拆解、搭建框架原型
2. **Phase 1 结构规划** — 读代码、搜索、推理、拆解、向用户提问、输出结构化方案
3. **Phase 2 禁止思考** — 严格按硬转换表执行，不读源码、不写代码、不自己判断
4. **必须持久化** — 每个 Phase 结束时写入状态
5. **禁止代理越权** — 架构师只设计、开发者只实现、测试者只测试
6. **禁止盲目信任** — 测试者的 test-*.md 报告显示失败 → 必须退回开发者
7. **必读文件已自动化** — 角色定义 + 最近教训由 `spoq-enforcer.ts` 在启动时自动注入 systemPrompt（见 v3 说明），子代理无需再手动 `read`；`*→{myRole}.md` 邮箱消息仍需子代理自行读取（内容随时变化，无法预注入）
8. **含糊需求先问，不猜测实现** — 简单任务直通 Phase 2 不等于可以跳过澄清；需求含糊时无论走哪条路径都必须先问用户/派发方（见 v6）
9. **完成前必须自证** — 报告完成前必须重新运行相关测试/构建并复核 diff，验证证据必须写进完成报告，不能只凭"已修改"就声称完成（见 v6）

---

## 🚀 Phase 0: 自由探索（完全智能体模式）

### 触发条件

- 收到新需求，尚未进入正式规划
- 需要对代码库或领域知识做快速侦察

### 行为

- 完全自由：读任何代码、搜索网络、运行命令、搭建框架原型
- 无约束：不做结构化拆解，不需要输出方案
- 产出：Phase-0-notes.md（可选，仅供自己参考）

### 结束条件

- 理解充分，准备进入结构化规划 → 进入 Phase 1
- 或发现需求不明确 → 向用户提问澄清

### 注意

- Phase 0 的代码产出（原型）不强制持久化，不纳入正式任务状态
- Phase 0 的探索笔记也无需格式约束

---

## 🎯 Phase 1: 结构化规划（智能体模式）

### 输入

用户需求（可能是自然语言描述）

### 必读文件（所有角色启动前必须读取）

| 文件 | 说明 |
| ------ | ------ |
| plan-{task-id}.md | 当前任务的设计文档 |
| *→{myRole}.md | 邮箱收件：发给自己的所有消息 |

> `lessons-learned.md` 已由 `spoq-enforcer.ts` 自动注入 systemPrompt，不再需要手动 read。

### 流程

```
1. UNDERSTAND  — 理解需求，必要时读代码库、搜索
2. DECOMPOSE   — 拆成子任务 DAG，标注依赖关系 + **复杂度标签**（simple/complex）
3. WAVE        — 拓扑排序 → Wave 分组
4. PROPOSE     — 向用户展示方案，确认
5. COMMIT      — 用户确认后写入 .pi/spoq-state.json
6. HANDOFF     — 输出 "方案已确认，进入执行阶段"，Phase 1 结束
```

### 简单任务 vs 复杂任务

| 类型 | 定义 | 确认方式 |
| ------ | ------ | --------- |
| **简单** | 单文件修改、bug 修复（单函数范围）、配置变更（JSON/YAML/TOML）、删除操作、运维操作（npm install/uninstall） | 可直接进入 Phase 2，跳过 Architect，无需用户确认 |
| **复杂** | 多文件架构变更（3+ 文件）、新功能（需新增模块或接口）、涉及 3+ 模块的修改、数据库 schema 变更、影响现有 API 契约的修改 | 展示方案 → 等用户确认 → 走完整 Architect→Developer→Tester 链路 |

> **注（铁律 8）**：简单任务"无需用户确认"指的是不需要展示完整方案等待批准，**不代表可以跳过澄清**。若简单任务的具体做法有歧义（例如"修一下这几个失败的测试"没说清楚是改测试断言还是改被测代码），仍必须先问清楚再动手。

**复杂度标签**：Phase 1 DECOMPOSE 时必须为每个 task 标注 `complexity: "simple" | "complex"`，写入 spoq-state.json。Phase 2 硬转换表 T1/T1a 根据此字段决定是否跳过 Architect。

### 拆解输出格式（给用户看）

```
━━ SPOQ 规划方案 ━━

Wave 0（并行，无依赖）:
  task-auth-module   — 认证模块核心逻辑
  task-db-schema     — 数据库表结构设计

Wave 1（依赖 Wave 0）:
  task-login-api     — 登录 API 端点

Wave 2（依赖 Wave 1）:
  task-middleware     — 认证中间件 + 集成测试

━━━━━━━━━━━━━━━━━━━━
共 4 个子任务，分 3 个 Wave 执行
预计最大并行度：2 agents
```

---

## 🤖 Phase 2: 执行（状态机模式）

> 进入 Phase 2 后，你不再 "思考"。严格按以下规则执行。

### 主循环

```
1. LOAD    — 读 .pi/spoq-state.json
2. POLL    — 检查所有运行中的子代理（agentId != null）
3. APPLY   — 对已完成的代理，按转换表更新状态，清空 agentId
4. FIND    — 找到当前 Wave 中状态可推进的任务
5. DISPATCH— 按转换表派发对应的子代理类型（支持 resume）
6. SAVE    — 写回 .pi/spoq-state.json
7. CHECK   — 全部 done/blocked？→ 汇总报告，Phase 2 结束
```

### 硬转换表（逐条匹配，不可偏离）

对每个任务，取 `state` 字段，按顺序匹配以下规则：

| # | 当前状态 | 条件 | 下一状态 | 动作 |
| --- | --------- | ------ | --------- | ------ |
| T1 | pending | complexity="complex" 且依赖全部 done 且 wave == currentWave | architecting | `Agent(software-architect, ..., run_in_background=true)` |
| **T1a** | **pending** | **complexity="simple" 且依赖全部 done 且 wave == currentWave** | **developing** | **跳过 Architect，直接 `Agent(developer, ..., run_in_background=true, isolation="worktree")`** |
| T2 | architecting | agent 终止 + plan-{id}.md 存在 | plan_done | 验证 plan，记录 planPath |
| T3 | architecting | agent 终止 + plan 缺失 + retry < 3 | architecting | retry++，重派 |
| T4 | architecting | agent 终止 + plan 缺失 + retry >= 3 | done | 标记 low_quality_pass，记录 error |
| T5 | plan_done | 无条件（自动） | developing | `Agent(developer, ..., run_in_background=true, isolation="worktree")` |
| T6 | developing | agent 终止 + src/{id}/ 有文件 + 无越权 | dev_done | 记录 srcPath |
| **T7** | **developing** | **agent 终止 + 失败 + retry < 3** | **developing** | **retry++，优先 resume 原 agentId（传入失败反馈）；resume 失败/不可行时新建 agent** |
| T8 | developing | agent 终止 + 失败 + retry >= 3 | done | 标记 low_quality_pass，记录 error |
| T9 | dev_done | 无条件（自动） | testing | `Agent(tester, ..., run_in_background=true)` |
| T10 | testing | agent 终止 + test-{id}.md PASS | done | 任务完成 |
| **T11** | **testing** | **agent 终止 + test-{id}.md FAIL（实现缺陷）+ retry < 3** | **developing** | **retry++，退回开发者附详情** |
| **T11a** | **testing** | **agent 终止 + test-{id}.md FAIL（设计缺陷）+ retry < 3** | **architecting** | **retry++，退回架构师修正设计（混合协议：Agent 自选回退目标）** |
| T12 | testing | agent 终止 + test-{id}.md FAIL + retry >= 3 | done | 标记 low_quality_pass，记录 error |
| **T13** | **testing** | **agent 终止 + 超时/崩溃 + retry < 3** | **testing** | **retry++，优先 resume 原 agentId（保留审查上下文）；resume 失败/不可行时新建 agent** |
| T14 | testing | agent 终止 + 超时/崩溃 + retry >= 3 | done | 标记 low_quality_pass，记录 error |

> **T7 Resume 机制说明**：
>
> 1. 检查原 `agentId` 是否支持 resume（通过 get_subagent_result 判断）
> 2. 若支持：以 resume 方式重新调度同一 agent，传入失败反馈作为上下文
> 3. 若不支持或 resume 失败：回退到新建 agent（等同于原逻辑）
> 4. resume 尝试不计入重试次数

> **T13 Resume 机制说明**：
>
> T13 与 T7 使用相同的 resume 机制。Tester 超时/崩溃后优先 resume 原 agentId（保留上一轮的审查上下文和已发现的缺陷模式），resume 失败/不可行时回退到新建 agent。

> **low_quality_pass 机制说明**：
>
> 当任务在 architecting/developing/testing 任意阶段连续失败 3 轮（T4/T8/T12/T14），不再阻塞整个 Wave。任务标记为 `done` + `lowQualityPass: true`，Orchestrator 在最终汇总报告中标注低质量通过的任务。
>
> 设计依据：SPOQ 论文 §RQ4 HaaA——"human review reduces residual defects from 0.47 to 0.03 per task"。纯自动化系统中需要等价于人类 override 的逃生口，防止单个任务阻塞整个流水线。

> **混合协议机制说明（T11/T11a）**：
>
> T11 测试失败回退不再是固定退回到 Developer。Tester 在 test-{id}.md 中按失败根因分类：
>
> - **实现缺陷**：代码逻辑错误、边界未处理、类型不匹配 → T11 退回到 Developer
> - **设计缺陷**：API 签名矛盾、数据流设计错误、需求理解偏差 → T11a 退回到 Architect
>
> 设计依据：Endogeneity Paradox 2026——"mixed protocol — fixed rounds for coordination, self-chosen roles for task execution — outperforms fully designed SOPs by 44%"。保持固定轮次协调，但让 Agent 自选回退目标。

> **结构化 Handoff 机制说明（改动 7）**：
>
> Architect 除 plan-{id}.md 外还需产出 `plan-{id}.schema.json`，定义结构化接口契约：
>
> - 函数签名（名称、参数类型、返回类型）
> - 数据模型（字段名、类型、约束）
> - API 端点（路径、方法、请求/响应 schema）
>
> Developer 按 schema 实现，Tester 按 schema 验证。设计依据：ROMA 2026——"structured interfaces between recursive meta-agents reduce information loss by 37%"。

### 构建子代理 Prompt

```
## 任务

{task.name}: {task.description}

## 你的角色

{architect/developer/tester}

## 必读文件

### 所有角色通用

- plan-{task-id}.md（当前任务设计文档）
- .pi/spoq-mailbox/{task-id}/*→{myRole}.md（邮箱收件）

> `.pi/lessons-learned.md` 无需在 prompt 里列出——`spoq-enforcer.ts` 已在 systemPrompt
> 里自动注入最近 3 条教训，Orchestrator 派发时不用再手写这条指令。

### 开发者额外

- src/{task-id}/**/*（已完成代码结构）

### 测试者额外

- 需求文档 / PRD（验收标准）
- 验收标准 / Acceptance Criteria

## 输入

- 设计文档: {planPath 或 "无（由你产出）"}
- 源代码: {srcPath 或 "无（由你产出）"}
- 上一 Wave 教训: {lessons}（详细经验库见 .pi/lessons-learned.md）

## 邮箱

.pi/spoq-mailbox/{task-id}/
启动时读发给你的邮件（*→{yourRole}.md）
有疑问写邮件给对应角色

## 约束

{对应 agent-loop 约束摘要}
```

### 轮询子代理

```
对每个 agentId != null 的任务：
  result = get_subagent_result(agentId, wait=false)
  if completed → 按转换表处理
  if failed/timeout → 视为终止但无产出；若支持 resume 则标记为可 resume
  else → 跳过（仍在运行）
```

### Wave 间反馈

每个 Wave 完成后提取教训：

```
本轮教训：{最常见 1-2 个失败模式 + 修复建议}
```

注入下一 Wave 所有子代理的 prompt。

### 教训持久化操作

每个 Wave 结束后，Orchestrator 必须执行以下持久化操作：

1. **提取** — 从当前 Wave 的任务状态、错误信息、测试报告中提取 1-3 条教训
2. **追加** — 按 lessons-learned.md 格式追加到 `.pi/lessons-learned.md` 对应分类下
3. **同步** — 同时更新 `.pi/spoq-state.json` 的 `lessons[]` 数组（格式：`["[YYYY-MM-DD][Wave N][task-id] {类别} 教训：... 标准：... 建议：..."]`）
4. **注入** — 将最新 2-3 条教训注入下一 Wave 所有子代理的 prompt

### 教训提取阈值

| 场景 | 提取数量 | 说明 |
| ------ | ---------- | ------ |
| Wave 无失败（全 done） | 1 条 | 提取一条正向经验（"值得继续保持"） |
| Wave 全部 blocked | 2-3 条 | 提取失败教训，分析根因 |
| 重复教训 | 合并 | 标记"重复模式"，合并同类项 |

> **格式约定**：spoq-state.json 的 lessons[] 使用紧凑格式 `["[YYYY-MM-DD][Wave N][task-id] {类别} 教训：... 标准：... 建议：..."]`，完整内容以 `.pi/lessons-learned.md` 为准。

---

## 经验库规则

### 1. 职责分配

| 角色 | 职责 |
|------|------|
| **Orchestrator** | 每个 Wave 结束后提取 1-3 条教训 → 追加到 `.pi/lessons-learned.md` → 同步到 `spoq-state.json.lessons[]` → 注入下一 Wave prompt |
| **子代理** | 启动时**必须**读取 `.pi/lessons-learned.md`；如发现已过时的教训，通过邮箱 `any→orchestrator-{ISO时间戳}.md` 通知 Orchestrator 标记 |

### 2. 教训提取标准

每条教训必须包含以下三个字段才为有效：

| 字段 | 要求 |
| ------ | ------ |
| **教训** | 具体发生了什么问题（如 "Kotlin 协程在 Fragment 销毁后仍在执行，导致 IllegalStateException"） |
| **标准** | 可操作的具体判断标准（如 "Fragment/Activity 中启动的协程必须在 onDestroy/onDestroyView 中 cancel"） |
| **建议** | 具体如何避免（如 "使用 viewLifecycleOwner.lifecycleScope 替代 global scope"） |

### 3. 粒度原则

- ✅ **好例子**："页面标题字号至少比正文字号大 8px，否则用户无法区分层级"
- ❌ **太细**："某行 Kotlin 代码漏了 ? 运算符"
- ❌ **太宽**："注意代码质量"、"加强测试"

### 4. 注入时机

Orchestrator 在 Phase 2 主循环的 DISPATCH 步骤构建子代理 prompt 时注入：

1. 在 "## 必读文件" 中加入 `.pi/lessons-learned.md` 引用
2. 在 "## 输入" 的 "上一 Wave 教训" 中注入最新 2-3 条教训摘要

### 5. 生命周期

```
[Wave 结束] → Orchestrator 提取教训 → 追加到 lessons-learned.md → 同步到 state.json
                                                              ↓
[下一 Wave 开始] → DISPATCH → 注入 prompt → 子代理必读
                                                              ↓
[后续 Wave] → 子代理阅读 lessons-learned.md 全文 + 特别注意注入的教训
```

---

## 🔬 测试验证标准（6 维）

Tester 必须对代码进行以下 6 个维度的验证：

| # | 指标 | 检查内容 |
| --- | ------ | --------- |
| 1 | 功能正确性 | 需求文档中的每个功能点是否已实现并正确工作 |
| 2 | 代码质量 | 是否有冗余、反模式、硬编码、缺少注释 |
| 3 | 边界情况 | 空值、异常输入、超时、并发等边界是否处理 |
| 4 | 安全性 | 敏感数据是否硬编码、输入是否校验、权限是否检查 |
| 5 | 与现有代码的兼容性 | 是否破坏已有功能、接口契约是否保持 |
| 6 | **LLM-as-Judge** | **由模型自检：改动的代码是否真的完成了需求？有没有只改表面、没解决根因？** |

---

## 📊 状态文件（.pi/spoq-state.json）

```json
{
  "version": "1.0",
  "phase": "planning|executing|done",
  "dag": {
    "tasks": {
      "task-id": {
        "name": "...",
        "description": "...",
        "complexity": "simple|complex",
        "wave": 0,
        "dependencies": [],
        "state": "pending",
        "retryCount": 0,
        "maxRetries": 3,
        "agentId": null,
        "resumeAgentId": null,
        "agentType": null,
        "error": null,
        "lowQualityPass": false,
        "planPath": null,
        "srcPath": null,
        "testPath": null,
        "transitionLog": []
      }
    }
  },
  "currentWave": -1,
  "totalWaves": 0,
  "lessons": [],
  "createdAt": "",
  "updatedAt": ""
}
```

---

## 🏁 汇总格式

```
━━ SPOQ 流水线完成 ━━
Wave 0: task-a ✅ / task-b ✅
Wave 1: task-c ⚠️ (low_quality_pass: 测试阶段连续失败 3 轮)
━━━━━━━━━━━━━━━━━━━━
成功: 2 / low_quality_pass: 1 / blocked: 0
```

---

## 🧪 自检

- [ ] Phase 0 的探索是否过渡到了 Phase 1 的规划？（避免跳跃）
- [ ] Phase 1 结束时复杂任务是否经用户确认？
- [ ] Phase 2 是否严格按转换表执行，无 "我认为"？
- [ ] 状态文件每轮写回？
- [ ] T7 失败后是否优先尝试 resume 原 agent？
- [ ] 每个子代理启动时是否正确加载了必读文件？
- [ ] Tester 是否覆盖了全部 6 维验证（含 LLM-as-Judge）？
- [ ] 我自己没碰 src/ 下的源码？
