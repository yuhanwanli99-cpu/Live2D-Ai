# ⛔ SPOQ — Plan-and-Execute 编排规则

> Phase 0: 自由探索（完全智能体模式）
> Phase 1: 结构化规划（智能体模式）
> Phase 2: 严格执行（状态机模式）
> **Phase 切换点：用户确认方案**

---

## 🔴 铁律

1. **Phase 0 绝对自由** — 可以读代码、搜索、推理、拆解、搭建框架原型
2. **Phase 1 结构规划** — 读代码、搜索、推理、拆解、向用户提问、输出结构化方案
3. **Phase 2 禁止思考** — 严格按硬转换表执行，不读源码、不写代码、不自己判断
4. **必须持久化** — 每个 Phase 结束时写入状态
5. **禁止代理越权** — 架构师只设计、开发者只实现、测试者只测试
6. **禁止盲目信任** — 测试者的 test-*.md 报告显示失败 → 必须退回开发者
7. **必读文件** — 子代理启动时必须先读取角色对应的必读文件清单（见下文）

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
| lessons-learned.md | 全局教训，防止重复踩坑 |
| plan-{task-id}.md | 当前任务的设计文档 |
| *→{myRole}.md | 邮箱收件：发给自己的所有消息 |

### 流程

```
1. UNDERSTAND  — 理解需求，必要时读代码库、搜索
2. DECOMPOSE   — 拆成子任务 DAG，标注依赖关系
3. WAVE        — 拓扑排序 → Wave 分组
4. PROPOSE     — 向用户展示方案，确认
5. COMMIT      — 用户确认后写入 .pi/spoq-state.json
6. HANDOFF     — 输出 "方案已确认，进入执行阶段"，Phase 1 结束
```

### 简单任务 vs 复杂任务

| 类型 | 定义 | 确认方式 |
| ------ | ------ | --------- |
| **简单** | 单文件修改、bug 修复（单函数范围）、配置变更（JSON/YAML/TOML）、删除操作 | 可直接进入 Phase 2，无需用户确认 |
| **复杂** | 多文件架构变更（3+ 文件）、新功能（需新增模块或接口）、涉及 3+ 模块的修改、数据库 schema 变更、影响现有 API 契约的修改 | 展示方案 → 等用户确认 |

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
| T1 | pending | 依赖全部 done 且 wave == currentWave | architecting | `Agent(software-architect, ..., run_in_background=true)` |
| T2 | architecting | agent 终止 + plan-{id}.md 存在 | plan_done | 验证 plan，记录 planPath |
| T3 | architecting | agent 终止 + plan 缺失 + retry < 3 | architecting | retry++，重派 |
| T4 | architecting | agent 终止 + plan 缺失 + retry >= 3 | blocked | 记录 error |
| T5 | plan_done | 无条件（自动） | developing | `Agent(developer, ..., run_in_background=true)` |
| T6 | developing | agent 终止 + src/{id}/ 有文件 + 无越权 | dev_done | 记录 srcPath |
| **T7** | **developing** | **agent 终止 + 失败 + retry < 3** | **developing** | **retry++，优先 resume 原 agentId（传入失败反馈）；resume 失败/不可行时新建 agent** |
| T8 | developing | agent 终止 + 失败 + retry >= 3 | blocked | 记录 error |
| T9 | dev_done | 无条件（自动） | testing | `Agent(tester, ..., run_in_background=true)` |
| T10 | testing | agent 终止 + test-{id}.md PASS | done | 任务完成 |
| T11 | testing | agent 终止 + test-{id}.md FAIL + retry < 3 | developing | retry++，退回开发者附详情 |
| T12 | testing | agent 终止 + 失败 + retry >= 3 | blocked | 记录 error |
| T13 | testing | agent 终止 + test 缺失 + retry < 3 | testing | retry++，重派 |

> **T7 Resume 机制说明**：
>
> 1. 检查原 `agentId` 是否支持 resume（通过 get_subagent_result 判断）
> 2. 若支持：以 resume 方式重新调度同一 agent，传入失败反馈作为上下文
> 3. 若不支持或 resume 失败：回退到新建 agent（等同于原逻辑）
> 4. resume 尝试不计入重试次数

### 构建子代理 Prompt

```
## 任务

{task.name}: {task.description}

## 你的角色

{architect/developer/tester}

## 必读文件

### 所有角色通用

- **.pi/lessons-learned.md** — 经验教训库，启动前必须阅读，避免重复犯同类错误
- plan-{task-id}.md（当前任务设计文档）
- .pi/spoq-mailbox/{task-id}/*→{myRole}.md（邮箱收件）

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
3. **同步** — 同时更新 `.pi/spoq-state.json` 的 `lessons[]` 数组
4. **注入** — 将最新 2-3 条教训注入下一 Wave 所有子代理的 prompt

### 教训提取阈值

| 场景 | 提取数量 | 说明 |
| ------ | ---------- | ------ |
| Wave 无失败（全 done） | 1 条 | 提取一条正向经验（"值得继续保持"） |
| Wave 全部 blocked | 2-3 条 | 提取失败教训，分析根因 |
| 重复教训 | 合并 | 标记"重复模式"，合并同类项 |

---

## 经验库规则

### 1. 职责分配

| 角色 | 职责 |
|------|------|
| **Orchestrator** | 每个 Wave 结束后提取 1-3 条教训 → 追加到 `.pi/lessons-learned.md` → 同步到 `spoq-state.json.lessons[]` → 注入下一 Wave prompt |
| **子代理** | 启动时**必须**读取 `.pi/lessons-learned.md`；如发现已过时的教训，通过邮箱 `any→orchestrator.md` 通知 Orchestrator 标记 |

### 2. 教训提取标准

每条教训必须包含以下三个字段才为有效：

| 字段 | 要求 |
| ------ | ------ |
| **教训** | 具体发生了什么问题 |
| **标准** | 可操作的具体判断标准 |
| **建议** | 具体如何避免 |

### 3. 粒度原则

- ✅ **好例子**："页面标题字号至少比正文字号大 8px，否则用户无法区分层级"
- ❌ **太细**："某行代码漏了 ? 运算符"
- ❌ **太宽**："注意代码质量"、"加强测试"

### 4. 注入时机

Orchestrator 在 Phase 2 主循环的 DISPATCH 步骤构建子代理 prompt 时注入

### 5. 生命周期

```
[Wave 结束] → Orchestrator 提取教训 → 追加到 lessons-learned.md → 同步到 state.json
                                                              ↓
[下一 Wave 开始] → DISPATCH → 注入 prompt → 子代理必读
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
        "wave": 0,
        "dependencies": [],
        "state": "pending",
        "retryCount": 0,
        "maxRetries": 3,
        "agentId": null,
        "agentType": null,
        "error": null,
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
Wave 1: task-c ❌ (blocked: 测试阶段连续失败)
━━━━━━━━━━━━━━━━━━━━
成功: 2 / 失败: 0 / blocked: 1
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
